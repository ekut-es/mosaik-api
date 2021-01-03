use async_std::{
    net::{TcpListener, TcpStream, ToSocketAddrs},
    prelude::*,
    task,
};
use futures::sink::SinkExt;
use futures::FutureExt;
use futures::{channel::mpsc, select};
use log::{debug, error, info, trace};
use std::{
    collections::{hash_map::Entry, HashMap},
    future::Future,
    sync::Arc,
};

use mosaik_rust_api::json::{handle_request, parse_request};
use mosaik_rust_api::simulation_mosaik::init_sim;

type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn main() -> AResult<()> {
    env_logger::init();
    let addr = "127.0.0.1:3456";
    // tcp ersetzen mit async tcp:
    // https://book.async.rs/tutorial/all_together.html
    debug!("main debug");
    task::block_on(accept_loop(addr))
    //run();
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
//channels?
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum Void {}

async fn accept_loop(addr: impl ToSocketAddrs) -> Result<()> {
    debug!("accept loop debug");
    let listener = TcpListener::bind(addr).await?;
    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let broker_handle = task::spawn(broker_loop(broker_receiver));
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        info!("Accepting from: {}", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream));
    }
    drop(broker_sender);
    broker_handle.await;
    Ok(())
}

async fn connection_loop(mut broker: Sender<Event>, stream: TcpStream) -> Result<()> {
    info!("Started connection loop");

    let mut stream = stream;
    let name = String::from("Mosaik");
    //let addr = stream.peer_addr().unwrap();

    let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    broker
        .send(Event::NewPeer {
            name: name.clone(),
            stream: Arc::new(stream.clone()),
            shutdown: shutdown_receiver,
        })
        .await
        .unwrap();

    let mut size_data = [0u8; 4]; // using 4 byte buffer
                                  //let mut full_package = [0u8; 10000];
    while let Ok(()) = stream.read_exact(&mut size_data).await {
        let size = u32::from_be_bytes(size_data) as usize;
        info!("Received {} Bytes Message", size);
        let mut full_package = vec![0; size];
        match stream.read_exact(&mut full_package).await {
            Ok(()) => {
                if let Err(e) = broker
                    .send(Event::Request {
                        full_data: String::from_utf8(full_package[0..(size as usize)].to_vec())
                            .expect("string from utf 8 connction loops"),
                        name: name.clone(),
                    })
                    .await
                {
                    error!("Error sending package to broker: {:?}", e);
                }
            }
            Err(e) => error!("Error reading Full Package: {:?}", e),
        }
    }

    Ok(())
}

async fn connection_writer_loop(
    messages: &mut Receiver<String>,
    stream: Arc<TcpStream>,
    shutdown: Receiver<Void>,
) -> Result<()> {
    let mut stream = &*stream;
    let mut messages = messages.fuse();
    let mut shutdown = shutdown.fuse();
    loop {
        select! {
            msg = messages.next().fuse() => match msg {
                Some(msg) => {
                    let mut big_endian = (msg.len() as u32).to_be_bytes().to_vec();
                    big_endian.append(&mut msg.as_bytes().to_vec());
                    stream.write_all(&big_endian).await?
                }, //write the message
                None => break,
            },
            void = shutdown.next().fuse() => match void {
                Some(void) => match void {},
                None => break,
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
enum Event {
    NewPeer {
        name: String, //std::net::SocketAddr,
        stream: Arc<TcpStream>,
        shutdown: Receiver<Void>,
    },
    Request {
        full_data: String,
        name: String,
    },
}

async fn broker_loop(events: Receiver<Event>) {
    let (disconnect_sender, mut disconnect_receiver) = // 1
         mpsc::unbounded::<(String, Receiver<String>)>();
    let mut peers: HashMap<String /*std::net::SocketAddr*/, Sender<String>> = HashMap::new(); //brauchen wir nicht wirklich, wir haben nur mosaik (sender) der sich connected.
    let mut events = events.fuse();
    let mut simulator = init_sim();

    loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break, // 2
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                let (name, _pending_messages) = disconnect.unwrap(); // 3
                assert!(peers.remove(&name).is_some());
                continue;
            },
        };
        debug!("Received event: {:?}", event);
        match event {
            Event::Request { full_data, name } => {
                //parse the request
                match parse_request(full_data) {
                    Ok(request) => {
                        println!("Received Request: {:?}", request);
                        match handle_request(request, &mut simulator) {
                            Some(response) => {
                                //parse the response with the request
                                match String::from(response) {
                                    Ok(response_string) => {
                                        println!("Responding with: {}", response_string);
                                        if let Some(peer) = peers.get_mut(&name) {
                                            if let Err(e) = peer.send(response_string).await {
                                                //-> send the message to mosaik channel reciever
                                                error!("error sending response to peer: {}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!("failed to make string from utf8: {}", e);
                                    }
                                }
                            }
                            None => {
                                info!("Nothing to respond");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error while parsing the request: {}", e);
                    }
                }
            }
            Event::NewPeer {
                name,
                stream,
                shutdown,
            } => {
                println!("New peer -> creating channels");
                match peers.entry(name.clone()) {
                    Entry::Occupied(..) => (),
                    Entry::Vacant(entry) => {
                        let (client_sender, mut client_receiver) = mpsc::unbounded();
                        entry.insert(client_sender);
                        let mut disconnect_sender = disconnect_sender.clone();
                        spawn_and_log_error(async move {
                            let res =
                                connection_writer_loop(&mut client_receiver, stream, shutdown)
                                    .await; //spawn a connection writer with the message recieved over the channel?
                            disconnect_sender
                                .send((String::from("Mosaik"), client_receiver))
                                .await // 4
                                .unwrap();
                            res
                        });
                    }
                }
            }
        }
    }
    println!("dropping peers");
    drop(peers); // 5
    println!("closing channels");
    drop(disconnect_sender); // 6
    while let Some((_name, _pending_messages)) = disconnect_receiver.next().await {}
}

fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    trace!("Spawn task");
    task::spawn(async move {
        trace!("Task Spawned");
        if let Err(e) = fut.await {
            error!("{}", e)
        }
    })
}
/*
fn tcp<T: std::net::ToSocketAddrs>(addr: T) {
    match std::net::TcpListener::bind(addr) {
        Ok(mut listener) => {
            for stream in listener.incoming() {
                match stream {
                    Ok(mut stream) => {
                        println!("New client from {:?}!", stream.peer_addr());
                        //von hier übernehmen bis inkl. full_package
                        let mut size_data = [0u8; 4]; // using 4 byte buffer
                        let mut full_package = [0u8; 1000000];

                        match stream.read_exact(&mut size_data) {
                            Ok(()) => {
                                let size = u32::from_be_bytes(size_data);
                                println!("Received {} Bytes Message", size);
                                match stream.read(&mut full_package) {
                                    Ok(size) => {
                                        match String::from_utf8(
                                            full_package[0..(size as usize)].to_vec(), //dieses full_package dann über channel an broker senden.
                                        ) {
                                            Ok(json_data) => {
                                                /* parse */
                                                println!("JSON: {}", &json_data);
                                                println!(
                                                    "JSON: {:?}",
                                                    parse_request(json_data.clone())
                                                );

                                                match parse_request(json_data) {
                                                    Ok(request) => {
                                                        /* call specific function */

                                                        /* and reply */

                                                        if let Some(value) =
                                                            parse_response(request, init_sim())
                                                        {
                                                            if let Err(e) = stream.write(&value) {
                                                                error!("{:?} ", e);
                                                            }
                                                            println!("Antwort wurde geschrieben!");
                                                        }
                                                    }
                                                    Err(e) => {
                                                        error!(
                                                            "Error when parsing Request: {:?}",
                                                            e
                                                        );
                                                        /* Reply to mosaik with an Error */

                                                        // .to/as_bytes()
                                                        // größe vorne anhängen
                                                        // stream.write()
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                /* use log::err! instead of println */
                                                println!("Parsing failed: {:?}", e);
                                            }
                                        }
                                    }
                                    Err(e) => {}
                                }
                            }
                            Err(e) => {
                                println!("Failed to receive data: {}", e);
                            }
                        }
                    }
                    Err(e) => { /* connection failed */ }
                }
            }
        }
        Err(_) => {}
    }

    println!("Terminated.");
}*/
