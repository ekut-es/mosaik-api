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

    let (_shutdown_sender, shutdown_receiver) = mpsc::unbounded::<Void>();
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
async fn broker_loop(events: Receiver<Event>) {
    let (disconnect_sender, mut disconnect_receiver) =
         mpsc::unbounded::<(String, Receiver<Vec<u8>>)>();
    let mut peers: HashMap<String /*std::net::SocketAddr*/, Sender<Vec<u8>>> = HashMap::new(); //brauchen wir nicht wirklich, wir haben nur mosaik (sender) der sich connected.
    let mut events = events.fuse();
    let mut simulator = init_sim();

    //loop for the different events.
    loop {
        let event = select! {
            event = events.next().fuse() => match event {
                None => break,
                Some(event) => event,
            },
            disconnect = disconnect_receiver.next().fuse() => {
                let (name, _pending_messages) = disconnect.unwrap();
                assert!(peers.remove(&name).is_some());
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
                        match handle_request(request, &mut simulator) {
                            Some(response) => {
                                if let Some(peer) = peers.get_mut(&name) {
                                    //-> send the message to mosaik channel reciever
                                    if let Err(e) = peer.send(response).await {
                                        error!("error sending response to peer: {}", e);
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
            //The event for a new connector.
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
                                    .await; //spawn a connection writer with the message recieved over the channel
                            disconnect_sender
                                .send((String::from("Mosaik"), client_receiver))
                                .await
                                .unwrap();
                            res
                        });
                    }
                }
            }
        }
    }
    println!("dropping peers");
    drop(peers);
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
