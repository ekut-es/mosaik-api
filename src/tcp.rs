//! The async TCP-Manager for the communication between Mosaik and the simulators.

use crate::{json, MosaikApi};
use json::Response;

use async_std::{
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
};
use futures::{channel::mpsc, select, sink::SinkExt, FutureExt};
use log::{debug, error, info, trace};
use std::{future::Future, net::SocketAddr, sync::Arc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
//channels needed for the communication in the async tcp
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum Void {}

/// The direction of the connection with the address of the socket.
/// Either we listen on an address or we connect to an address.
/// This is used in the `run_simulation` function.
pub enum ConnectionDirection {
    ConnectToAddress(SocketAddr),
    ListenOnAddress(SocketAddr),
}

///Build the connection between Mosaik and us. 2 cases, we connect to them or they connect to us.
pub(crate) async fn build_connection<T: MosaikApi>(
    addr: ConnectionDirection,
    simulator: T,
) -> Result<()> {
    debug!("accept loop debug");
    match addr {
        //Case: we need to listen for a possible connector
        ConnectionDirection::ListenOnAddress(addr) => {
            let listener = TcpListener::bind(addr).await?;
            let (broker_sender, broker_receiver) = mpsc::unbounded();
            let (shutdown_connection_loop_sender, shutdown_connection_loop_receiver) =
                mpsc::unbounded::<bool>();
            let broker_handle = task::spawn(broker_loop(
                broker_receiver,
                shutdown_connection_loop_sender,
                simulator,
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
        //case: We need to connect to a stream
        ConnectionDirection::ConnectToAddress(addr) => {
            let stream = TcpStream::connect(addr).await?;
            let (broker_sender, broker_receiver) = mpsc::unbounded();
            let (shutdown_connection_loop_sender, shutdown_connection_loop_receiver) =
                mpsc::unbounded::<bool>();
            let broker_handle = task::spawn(broker_loop(
                broker_receiver,
                shutdown_connection_loop_sender,
                simulator,
            ));
            spawn_and_log_error(connection_loop(
                broker_sender,
                shutdown_connection_loop_receiver,
                stream,
            ));
            //connection_handle.await;
            broker_handle.await;
            Ok(())
        }
    }
}

///Receive the Requests, send them to the broker_loop.
async fn connection_loop(
    mut broker: Sender<Event>,
    mut connection_shutdown_receiver: Receiver<bool>,
    stream: TcpStream,
) -> Result<()> {
    info!("Started connection loop");

    let mut stream = stream;
    let name = String::from("Mosaik");

    let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
    broker
        .send(Event::NewPeer {
            name: name.clone(),
            stream: Arc::new(stream.clone()),
            shutdown: shutdown_receiver,
        })
        .await
        .unwrap();

    let mut size_data = [0u8; 4]; // use 4 byte buffer for the big_endian number in front of the request.

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
                                    full_data: String::from_utf8(full_package[0..size].to_vec())
                                        .expect("string from utf 8 connection loops"),
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
                Err(e) => {
                    match e.kind() {
                        async_std::io::ErrorKind::UnexpectedEof => {
                            error!("Unexpected EOF. Error: {}", e);
                            info!("Read unexpected EOF. Simulation and therefore TCP Connection should be finished. Waiting for Shutdown Request from Shutdown Sender.");
                        },
                        _ => {
                            error!("Error reading Stream Data: {:?}. Stopping connection loop.", e);
                            break;
                        }
                    }
                }
            },
            void = connection_shutdown_receiver.next().fuse() => match void {
                Some(_) => {
                    info!("receive connection_shutdown command");
                    break;
                },
                None => {
                    error!("shutdown sender channel is closed and therefore we assume the connection can and should be stopped.");
                    break;
                }
            }
        }
    }
    Ok(())
}

//Receive the Response from the broker_loop and write it in the stream.
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
        name: String, //here Mosaik
        stream: Arc<TcpStream>,
        shutdown: Receiver<Void>,
    },
    Request {
        full_data: String,
        name: String,
    },
}

///Receive requests from the connection_loop, parse them, get the values from the API and send the finished response to the connection_writer_loop
async fn broker_loop<T: MosaikApi>(
    events: Receiver<Event>,
    mut connection_shutdown_sender: Sender<bool>,
    mut simulator: T,
) {
    let (disconnect_sender, mut disconnect_receiver) =
        mpsc::unbounded::<(String, Receiver<Vec<u8>>)>();
    let mut peer: (std::net::SocketAddr, Sender<Vec<u8>>);
    let mut events = events.fuse();

    info!("New peer -> creating channels");
    if let Some(Event::NewPeer {
        name: _,
        stream,
        shutdown,
    }) = events.next().await
    {
        let (client_sender, mut client_receiver) = mpsc::unbounded();
        peer = (
            stream
                .peer_addr()
                .expect("unable to read remote peer address from {name}"),
            client_sender,
        );
        let mut disconnect_sender = disconnect_sender.clone();
        spawn_and_log_error(async move {
            let res = connection_writer_loop(&mut client_receiver, stream, shutdown).await; //spawn a connection writer with the message received over the channel
            disconnect_sender
                .send((String::from("Mosaik"), client_receiver))
                .await
                .unwrap();
            res
        });
    } else {
        panic!("Didn't receive new peer as first event.");
    }

    //loop for the different events.
    'event_loop: loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break,
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                let (_name, _pending_messages) = disconnect.unwrap();
                //assert!(peer.remove(&name).is_some());
                continue;
            },
        };
        debug!("Received event: {:?}", event);
        match event {
            //The event that will happen the rest of the time, because the only connector is mosaik.
            Event::Request { full_data, name } => {
                //parse the request
                match json::parse_json_request(&full_data) {
                    Ok(request) => {
                        //Handle the request -> simulations calls etc.
                        trace!("The request: {:?} from {name}", request);
                        match json::handle_request(&mut simulator, &request) {
                            Response::Reply(mosaik_msg) => {
                                let response = mosaik_msg.serialize_to_vec();

                                //get the second argument in the tuple of peer
                                //-> send the message to mosaik channel receiver
                                if let Err(e) = peer.1.send(response).await {
                                    error!("error sending response to peer: {}", e);
                                    // FIXME what to send in this case? Failure?
                                }
                            }
                            Response::Stop => {
                                if let Err(e) = connection_shutdown_sender.send(true).await {
                                    error!("error sending to the shutdown channel: {}", e);
                                }
                                break 'event_loop;
                            }
                            Response::NoReply => {
                                info!("Nothing to respond");
                            }
                        }
                    }
                    Err(e) => {
                        //if let Err(e) = peer.1.send()
                        error!("Error while parsing the request from {name}: {:?}", e);
                        todo!("send a failure message")
                    }
                }
            }
            //The event for a new connector. //TODO: Check if new peer is even needed
            Event::NewPeer {
                name,
                stream: _,
                shutdown: _,
            } => {
                error!("There is a peer already. No new peer from {name} needed.");
            }
        }
    }
    info!("dropping peer");
    drop(peer);
    info!("closing channels");
    drop(disconnect_sender);
    while let Some((_name, _pending_messages)) = disconnect_receiver.next().await {}
}

///spawns the tasks and handles errors.
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
