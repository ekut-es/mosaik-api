//! The async TCP-Manager for the communication between Mosaik and the simulators.
//!
//! It consists of 3 loops:
//! 1. The `tcp_receiver` reads the requests from the TCP-Stream and sends them to the `broker_loop`.
//! 2. The `broker_loop` receives the requests, parses them, calls the API and sends the response to the `tcp_sender`.
//! 3. The `tcp_sender` receives the responses from the `broker_loop` and writes them to the TCP-Stream.
//!
//! The `build_connection` function is the entry point for the TCP-Manager. It creates a TCP-Stream and spawns the 3 loops.

use crate::{mosaik_protocol, MosaikApi};
use mosaik_protocol::Response;

use async_std::{
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
};
use futures::{
    channel::{mpsc, oneshot},
    select,
    sink::SinkExt,
    FutureExt,
};
use log::{debug, error, info, trace};
use std::{future::Future, net::SocketAddr, sync::Arc};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
    // Create a TCP Stream
    let stream: TcpStream = match addr {
        // Case: We need to listen for a possible connector
        ConnectionDirection::ListenOnAddress(addr) => {
            let listener = TcpListener::bind(addr).await?;
            let (stream, _addr) = listener.accept().await?;
            info!("Accepting from: {}", stream.peer_addr()?);
            stream
        }
        // Case: We need to connect to a stream
        ConnectionDirection::ConnectToAddress(addr) => TcpStream::connect(addr).await?,
    };

    // Wrap the stream in an Arc to share it between the tasks
    let stream = Arc::new(stream);

    // Create the channels for the communication between the tasks
    let (receiver2broker_tx, receiver2broker_rx) = mpsc::unbounded();
    // The broker_loop needs to be able to shutdown the receiver_loop
    // the tcp_sender will be shutdown by dropping the channel to it in the broker_loop
    let (shutdown_signal_tx, shutdown_signal_rx) = oneshot::channel::<bool>();
    // Channel to the writer loop, gets shutdown by dropping the channel in the broker_loop
    let (broker2sender_tx, broker2sender_rx) = mpsc::unbounded();

    // Spawn the tasks
    // 1. Read the requests from the TCP-Stream and send them to the broker_loop
    let receiver_handle = spawn_and_log_error(tcp_receiver(
        receiver2broker_tx,
        shutdown_signal_rx,
        Arc::clone(&stream),
    ));
    // 2. Connect broker_loop with the receiver_loop, simulator and sender_loop, add a connection to
    let broker_handle = task::spawn(broker_loop(
        receiver2broker_rx,
        broker2sender_tx,
        simulator,
        shutdown_signal_tx,
    ));
    // 3. Connect broker loop to the TCP sender loop.
    spawn_and_log_error(async move {
        //spawn a connection writer with the message received over the channel
        tcp_sender(broker2sender_rx, Arc::clone(&stream)).await
    });

    receiver_handle.await;
    broker_handle.await;
    info!("Finished TCP");
    Ok(())
}

///Receive the Requests, send them to the `broker_loop`.
async fn tcp_receiver(
    mut broker: mpsc::UnboundedSender<String>,
    shutdown_signal_rx: oneshot::Receiver<bool>,
    stream: Arc<TcpStream>,
) -> Result<()> {
    info!("Started connection loop");
    let mut stream = &*stream;
    let mut size_data = [0u8; 4]; // use 4 byte buffer for the big_endian number in front of the request.

    let mut rx = shutdown_signal_rx.fuse();
    //Read the rest of the data and send it to the broker_loop
    loop {
        // handle all breakouts from the loop
        select! {
            msg = stream.read_exact(&mut size_data).fuse() => {
                if msg.is_err() {
                    error!("Error reading size data: {:?}", msg.err());
                    break;
                }
                // escape select! to read the full message without breaking the loop
            },
            shutdown_signal = rx => {
                if shutdown_signal.is_ok() {
                    info!("Received shutdown signal.");
                } else {
                    info!("Shutdown signal channel closed.");
                }
                break;
            },
        }
        // continue with handling the message
        let size = u32::from_be_bytes(size_data) as usize;
        info!("Received {} Bytes Message", size);
        let mut full_package = vec![0; size];
        if let Err(e) = stream.read_exact(&mut full_package).await {
            error!("Error reading Full Package: {:?}", e);
            continue;
        }
        let json_string = String::from_utf8(full_package[0..size].to_vec())
            .expect("Should convert to string from utf 8 in connection loops");
        if let Err(e) = broker.send(json_string).await {
            error!("Error sending package to broker: {:?}", e);
            //TODO I would add a break; here or just use await? instead
        }
    }
    info!("Receiver finished.");
    Ok(())
}

//Receive the Response from the broker_loop and write it in the stream.
async fn tcp_sender(
    mut messages: mpsc::UnboundedReceiver<Vec<u8>>,
    stream: Arc<TcpStream>,
) -> Result<()> {
    let mut stream = &*stream;

    // loop for the messages
    // messages will be None when the broker_loop is closed -> which ends the loop
    while let Some(msg) = messages.next().await {
        stream.write_all(&msg).await?; //write the message
    }

    info!("Sender finished.");
    Ok(())
}

///Receive requests from the `connection_loop`, parse them, get the values from the API and send the finished response to the `connection_writer_loop`
async fn broker_loop<T: MosaikApi>(
    mut received_requests: mpsc::UnboundedReceiver<String>,
    mut response_sender: mpsc::UnboundedSender<Vec<u8>>,
    mut simulator: T,
    shutdown_signal_tx: oneshot::Sender<bool>,
) {
    //loop for the different events.
    'event_loop: while let Some(json_string) = received_requests.next().await {
        debug!("Received event: {:?}", json_string);
        //The event that will happen the rest of the time, because the only connector is mosaik.
        //parse the request
        match mosaik_protocol::parse_json_request(&json_string) {
            Ok(request) => {
                //Handle the request -> simulations calls etc.
                trace!("The request: {:?}", request);
                match mosaik_protocol::handle_request(&mut simulator, request) {
                    Response::Reply(mosaik_msg) => {
                        let response = mosaik_msg.serialize_to_vec();

                        //get the second argument in the tuple of peer
                        //-> send the message to mosaik channel receiver
                        if let Err(e) = response_sender.send(response).await {
                            error!("error sending response to peer: {}", e);
                            // FIXME what to send in this case? Failure?
                        }
                    }
                    Response::Stop => {
                        info!("Received stop signal. Closing all connections ...");
                        // shutdown sender loop
                        drop(response_sender);
                        drop(received_requests);
                        // shutdown receiver loop
                        if let Err(e) = shutdown_signal_tx.send(true) {
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
    info!("Broker finished.");
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
            error!("{}", e);
        }
    })
}
