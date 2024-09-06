//! The async TCP-Manager for the communication between Mosaik and the simulators.

use crate::{mosaik_protocol, MosaikApi};
use mosaik_protocol::Response;

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
    let stream: TcpStream = match addr {
        //Case: we need to listen for a possible connector
        ConnectionDirection::ListenOnAddress(addr) => {
            let listener = TcpListener::bind(addr).await?;
            let (stream, _addr) = listener.accept().await?;
            info!("Accepting from: {}", stream.peer_addr()?);
            stream
        }
        //case: We need to connect to a stream
        ConnectionDirection::ConnectToAddress(addr) => TcpStream::connect(addr).await?,
    };

    let (broker_sender, broker_receiver) = mpsc::unbounded();
    let (shutdown_connection_loop_sender, shutdown_connection_loop_receiver) =
        mpsc::unbounded::<bool>();
    let broker_handle = task::spawn(broker_loop(
        broker_receiver,
        shutdown_connection_loop_sender,
        simulator,
        Arc::new(stream.clone()),
    ));

    let connection_handle = spawn_and_log_error(connection_loop(
        broker_sender,
        shutdown_connection_loop_receiver,
        stream,
    ));

    if let ConnectionDirection::ListenOnAddress(_) = addr {
        // FIXME should this be run in both cases
        connection_handle.await;
    }

    broker_handle.await;
    Ok(())
}

///Receive the Requests, send them to the `broker_loop`.
async fn connection_loop(
    mut broker: Sender<String>,
    mut connection_shutdown_receiver: Receiver<bool>,
    mut stream: TcpStream,
) -> Result<()> {
    info!("Started connection loop");

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
                            let json_string = String::from_utf8(full_package[0..size].to_vec()).expect("Should convert to string from utf 8 in connection loops");
                            if let Err(e) = broker.send(json_string).await {
                                error!("Error sending package to broker: {:?}", e);
                            }
                        }
                        Err(e) => error!("Error reading Full Package: {:?}", e),
                    }

                },
                Err(e) => {
                    if e.kind() == async_std::io::ErrorKind::UnexpectedEof {
                            info!("Read unexpected EOF. Simulation and therefore TCP Connection should be finished. Waiting for Shutdown Request from Shutdown Sender.");
                            panic!("Unexpected EOF. Error: {e}");
                        } else {
                            error!("Error reading Stream Data: {:?}. Stopping connection loop.", e);
                            break;
                        }
                }
            },
            void = connection_shutdown_receiver.next().fuse() => {
                if void.is_some() {
                    info!("receive connection_shutdown command");
                } else{
                    error!("shutdown sender channel is closed and therefore we assume the connection can and should be stopped.");
                }
                break;
            }
        }
    }
    Ok(())
}

//Receive the Response from the broker_loop and write it in the stream.
async fn connection_writer_loop(
    messages: &mut Receiver<Vec<u8>>,
    stream: Arc<TcpStream>,
) -> Result<()> {
    let mut stream = &*stream;
    while let Some(msg) = messages.next().await {
        stream.write_all(&msg).await?; //write the message
    }
    Ok(())
}

///Receive requests from the `connection_loop`, parse them, get the values from the API and send the finished response to the `connection_writer_loop`
async fn broker_loop<T: MosaikApi>(
    mut events: Receiver<String>,
    mut connection_shutdown_sender: Sender<bool>,
    mut simulator: T,
    stream: Arc<TcpStream>,
) {
    // Channel to the writer loop
    let (mut client_sender, mut client_receiver) = mpsc::unbounded();

    spawn_and_log_error(async move {
        //spawn a connection writer with the message received over the channel
        connection_writer_loop(&mut client_receiver, stream).await
    });

    //loop for the different events.
    'event_loop: while let Some(json_string) = events.next().await {
        debug!("Received event: {:?}", json_string);
        //The event that will happen the rest of the time, because the only connector is mosaik.
        //parse the request
        match mosaik_protocol::parse_json_request(&json_string) {
            Ok(request) => {
                //Handle the request -> simulations calls etc.
                trace!("The request: {:?}", request);
                match mosaik_protocol::handle_request(&mut simulator, &request) {
                    Response::Reply(mosaik_msg) => {
                        let response = mosaik_msg.serialize_to_vec();

                        //get the second argument in the tuple of peer
                        //-> send the message to mosaik channel receiver
                        if let Err(e) = client_sender.send(response).await {
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
                }
            }
            Err(e) => {
                //if let Err(e) = peer.1.send()
                error!("Error while parsing the request: {:?}", e);
                todo!("send a failure message")
            }
        }
    }
    info!("closing channels");
    drop(client_sender);
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
            error!("{}", e); // FIXME does this function simply log errors but continue running?
                             // ... if so, should we introduce an Error enum with Unrecoverable and Recoverable errors,
                             //  to be able to "panic" gracefully?
        }
    })
}
