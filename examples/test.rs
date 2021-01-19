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
    let addr = "127.0.0.1:3456"; //The local addres mosaik connects to.
    debug!("main debug");
    task::block_on(accept_loop(addr))
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
//channels needed for the communication in the async tcp
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum Void {}

async fn accept_loop(addr: impl ToSocketAddrs) -> Result<()> {
    debug!("accept loop debug");
    let listener = TcpListener::bind(addr).await?;
    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let (shutdown_connection_loop_sender, shutdown_connection_loop_receiver) =
        mpsc::unbounded::<bool>();
    let broker_handle = task::spawn(broker_loop(
        broker_receiver,
        shutdown_connection_loop_sender,
    ));
    let mut incoming = listener.incoming();
    let connection_handle = if let Some(stream) = incoming.next().await {
        let stream = stream?;
        info!("Accepting from: {}", stream.peer_addr()?);
        spawn_and_log_error(connection_loop(
            broker_sender,
            shutdown_connection_loop_receiver,
            stream,
        ))
    } else {
        panic!("No stream available.")
    };
    connection_handle.await;
    broker_handle.await;

    Ok(())
}

async fn connection_loop(
    mut broker: Sender<Event>,
    mut connection_shutdown_reciever: Receiver<bool>,
    stream: TcpStream,
) -> Result<()> {
    info!("Started connection loop");

    let mut stream = stream;
    let name = String::from("Mosaik");

    let (shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    broker
        .send(Event::NewPeer {
            name: name.clone(),
            stream: Arc::new(stream.clone()),
            shutdown: shutdown_receiver,
        })
        .await
        .unwrap();

    let mut size_data = [0u8; 4]; // use 4 byte buffer for the big_endian number infront the request.

    //Read the rest of the data and send it to the broker_loop
    loop {
        select! {
            msg = stream.read_exact(&mut size_data).fuse() => match msg {
                Ok(()) => {
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

                },
                Err(_) => break,
            },
            void = connection_shutdown_reciever.next().fuse() => match void {
                Some(_) => {
                    println!("recieve connection_shutdown command");
                    break;
                },
                None => break,
            }
        }
    }
    Ok(())
}

//The loop to actually write the message to mosaik as [u8].
async fn connection_writer_loop(
    messages: &mut Receiver<Vec<u8>>,
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
                    stream.write_all(&msg).await?//write the message
                },
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

//The loop that does the actual work.
async fn broker_loop(events: Receiver<Event>, mut connection_shutdown_sender: Sender<bool>) {
    let (disconnect_sender, mut disconnect_receiver) =
        mpsc::unbounded::<(String, Receiver<Vec<u8>>)>();
    let mut peer: (std::net::SocketAddr, Sender<Vec<u8>>);
    //: HashMap<String /*std::net::SocketAddr*/, Sender<Vec<u8>>> = HashMap::new(); //brauchen wir nicht wirklich, wir haben nur mosaik (sender) der sich connected.
    let mut events = events.fuse();
    let mut simulator = init_sim();

    println!("New peer -> creating channels");
    if let Some(Event::NewPeer {
        name,
        stream,
        shutdown,
    }) = events.next().await
    {
        let (client_sender, mut client_receiver) = mpsc::unbounded();
        peer = (
            stream
                .peer_addr()
                .expect("unaible to read remote peer address"),
            client_sender,
        ); //expect ist ok, da es zum verbinden benötigt wird, und falls nicht erfolgreich das Programm beendet werden soll.
        let mut disconnect_sender = disconnect_sender.clone();
        spawn_and_log_error(async move {
            let res = connection_writer_loop(&mut client_receiver, stream, shutdown).await; //spawn a connection writer with the message recieved over the channel
            disconnect_sender
                .send((String::from("Mosaik"), client_receiver))
                .await
                .unwrap();
            res
        });
    } else {
        panic!("Didn't recieve new peer as first event.");
    }

    //loop for the different events.
    'event_loop: loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break,
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                let (name, _pending_messages) = disconnect.unwrap();
                //assert!(peer.remove(&name).is_some());
                continue;
            },
        };
        debug!("Received event: {:?}", event);
        match event {
            //The event that will happen the rest of the time, because the only connector is mosaik.
            Event::Request { full_data, name } => {
                //parse the request
                match parse_request(full_data) {
                    Ok(request) => {
                        //Handle the request -> simulations calls etc.
                        println!("The request: {:?}", request);
                        use mosaik_rust_api::json::Response::*;
                        match handle_request(request, &mut simulator) {
                            Successfull(response) => {
                                //get the second argument in the tuple of peer
                                //-> send the message to mosaik channel reciever
                                if let Err(e) = peer.1.send(response).await {
                                    error!("error sending response to peer: {}", e);
                                }
                            }
                            Stop(response) => {
                                if let Err(e) = peer.1.send(response).await {
                                    error!("error sending response to peer: {}", e);
                                }
                                connection_shutdown_sender.send(true);
                                break 'event_loop;
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
            //The event for a new connector.
            Event::NewPeer {
                name,
                stream,
                shutdown,
            } => {
                error!("There is a peer already. No new peer needed.");
            }
        }
    }
    println!("dropping peer");
    drop(peer);
    println!("closing channels");
    drop(disconnect_sender);
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
