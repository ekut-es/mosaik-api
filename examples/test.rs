use async_std::{io::BufReader, net::{TcpListener, TcpStream, ToSocketAddrs}, prelude::*, task};
use futures::sink::SinkExt;
use futures::FutureExt;
use futures::{channel::mpsc, select};
use log::error;
use std::{collections::{hash_map::Entry, HashMap}, future::Future, io::Read, io::Write, sync::Arc};

use mosaik_rust_api::simulation_mosaik::{init_sim, run, ExampleSim};
use mosaik_rust_api::{
    json::{parse_request, parse_response},
    MosaikAPI,
};

pub fn main() {
    let addr = "127.0.0.1:3456";
    // tcp ersetzen mit async tcp:
    // https://book.async.rs/tutorial/all_together.html
    //task::block_on(accept_loop(addr))
    accept_loop(addr);
    run();
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
//channels?
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum Void {}

async fn accept_loop(addr: impl ToSocketAddrs) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let broker_handle = task::spawn(broker_loop(broker_receiver));
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        println!("Accepting from: {}", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(broker_sender.clone(), stream));
    }
    drop(broker_sender);
    broker_handle.await;
    Ok(())
}

async fn connection_loop(mut broker: Sender<Event>, stream: TcpStream) -> Result<()> {
    
    let stream = Arc::new(stream);
    let name = String::from("Mosaik");
    let addr = stream.peer_addr().unwrap();

    let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    broker
        .send(Event::NewPeer {
            name: name.clone(),
            stream: Arc::clone(&stream),
            shutdown: shutdown_receiver,
        })
        .await
        .unwrap();

    let mut size_data = [0u8; 4]; // using 4 byte buffer
    let mut full_package = [0u8; 1000000];

   (&*stream).read_exact(&mut size_data); //no check if Ok or not
    let size = u32::from_be_bytes(size_data);
    println!("Received {} Bytes Message", size);
    (&*stream).read(&mut full_package);
    let read_size = full_package.len() as u32;
    if size == read_size {
        broker
            .send(Event::Request {
                full_data: String::from_utf8(full_package[0..(size as usize)].to_vec())
                    .unwrap(),
                name: name,
            })
            .await
            .unwrap();
    } else {
        error!("Problem with reading the full request");
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
                Some(msg) => stream.write_all(msg.as_bytes()).await?, //write the message
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
        name: String,//std::net::SocketAddr,
        stream: Arc<TcpStream>,
        shutdown: Receiver<Void>,
    },
    Request {
        full_data: String,
        name: String
    },
}

async fn broker_loop(events: Receiver<Event>) {
    let (disconnect_sender, mut disconnect_receiver) = // 1
         mpsc::unbounded::<(String, Receiver<String>)>();
    let mut peers: HashMap< String/*std::net::SocketAddr*/, Sender<String>> = HashMap::new(); //brauchen wir nicht wirklich, wir haben nur mosaik (sender) der sich connected.
    let mut events = events.fuse();
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
        
        match event {
            Event::Request { full_data, name } => {
                let request = parse_request(full_data).unwrap();
                let response = parse_response(request, init_sim()).unwrap();
                if let Some(peer) = peers.get_mut(&name) {
                    peer.send(String::from_utf8(response).unwrap()); //-> send the message to mosaik channel reciever
                }
                
            }
            Event::NewPeer {
                name,
                stream,
                shutdown,
            } => {
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
    drop(peers); // 5
    drop(disconnect_sender); // 6
    while let Some((_name, _pending_messages)) = disconnect_receiver.next().await {}
}

fn spawn_and_log_error<F>(fut: F) -> task::JoinHandle<()>
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    task::spawn(async move {
        if let Err(e) = fut.await {
            eprintln!("{}", e)
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