pub mod json; //the tcp manager

use async_std::{
    net::{TcpListener, TcpStream},
    prelude::*,
    task,
};
use async_trait::async_trait;
use futures::sink::SinkExt;
use futures::FutureExt;
use futures::{channel::mpsc, select};
use log::{debug, error, info, trace};
use serde_json::{json, Map, Value};
use std::{collections::HashMap, future::Future, net::SocketAddr, sync::Arc};
type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

///Main calls this function with the simulator that should run. For the option that we connect our selfs addr as option!...
pub fn run_simulation<T: MosaikApi>(addr: ConnectionDirection, simulator: T) -> AResult<()> {
    task::block_on(build_connection(addr, simulator))
}

///information about the model(s) of the simulation
pub type Meta = serde_json::Value;

///Id of the simulation
pub type Sid = String;

pub type Model = Value;

///Id of an entity
pub type Eid = String;

///Id of an attribute of a Model
pub type AttributeId = String;

pub type Children = Value;

pub struct Simulator {
    // public static final String API_VERSION
    pub api_version: &'static str,
    // private final String simName
    sim_name: &'static str,
    // missing -> mosaik: MosaikProxy
}

pub trait ApiHelpers {
    /// Gets the meta from the simulator, needs to be implemented on the simulator side.
    fn meta() -> Meta;
    /// Set the eid\_prefix on the simulator, which we got from the interface.
    fn set_eid_prefix(&mut self, eid_prefix: &str);
    /// Set the step_size on the simulator, which we got from the interface.
    fn set_step_size(&mut self, step_size: i64);
    /// Get the eid_prefix.
    fn get_eid_prefix(&self) -> &str;
    /// Get the step_size for the api call step().
    fn get_step_size(&self) -> i64;
    /// Get the list containing the created entities.
    fn get_mut_entities(&mut self) -> &mut Map<String, Value>;
    /// Create a model instance (= entity) with an initial value. Returns the [JSON-Value](Value)
    /// representation of the children, if the entity has children.
    fn add_model(&mut self, model_params: Map<AttributeId, Value>) -> Option<Children>;
    /// Get the value from a entity.
    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value>;
    /// Call the step function to perform a simulation step and include the deltas from mosaik, if there are any.
    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>);
    // Get the time resolution of the Simulator.
    fn get_time_resolution(&self) -> f64;
    // Set the time resolution of the Simulator.
    fn set_time_resolution(&mut self, time_resolution: f64);
}
///the class for the "empty" API calls
pub trait MosaikApi: ApiHelpers + Send + 'static {
    //fn params<T: ApiHelpers>(&mut self) -> &mut T;

    /// Initialize the simulator with the ID sid and apply additional parameters (sim_params) sent by mosaik.
    /// Return the meta data meta.
    fn init(&mut self, sid: Sid, time_resolution: f64, sim_params: Map<String, Value>) -> Meta {
        if time_resolution != 1.0 {
            info!("time_resolution must be 1.0");
            self.set_time_resolution(1.0f64);
        } else {
            self.set_time_resolution(time_resolution);
        }

        for (key, value) in sim_params {
            match (key.as_str(), value) {
                /*("time_resolution", Value::Number(time_resolution)) => {
                    self.set_time_resolution(time_resolution.as_f64().unwrap_or(1.0f64));
                }*/
                ("eid_prefix", Value::String(eid_prefix)) => {
                    self.set_eid_prefix(&eid_prefix);
                }
                ("step_size", Value::Number(step_size)) => {
                    self.set_step_size(step_size.as_i64().unwrap());
                }
                _ => {
                    info!("Unknown parameter: {}", key);
                }
            }
        }

        Self::meta()
    }

    ///Create *num* instances of *model* using the provided *model_params*.
    fn create(
        &mut self,
        num: usize,
        model: Model,
        model_params: Map<AttributeId, Value>,
    ) -> Vec<Map<String, Value>> {
        let mut out_vector = Vec::new();
        let next_eid = self.get_mut_entities().len();
        for i in next_eid..(next_eid + num) {
            let mut out_entities: Map<String, Value> = Map::new();
            let eid = format!("{}{}", self.get_eid_prefix(), i);
            let children = self.add_model(model_params.clone());
            self.get_mut_entities().insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
            out_entities.insert(String::from("eid"), json!(eid));
            out_entities.insert(String::from("type"), model.clone()); // FIXME shouldn't this be a Model instance, not a String?
            if let Some(children) = children {
                out_entities.insert(String::from("children"), children);
            }
            debug!("{:?}", out_entities);
            out_vector.push(out_entities);
        }

        debug!("the created model: {:?}", out_vector);
        out_vector
    }

    ///The function mosaik calls, if the init() and create() calls are done. Return Null
    fn setup_done(&self);

    /// Perform the next simulation step at time and return the new simulation time (the time at which step should be called again)
    ///  or null if the simulator doesn’t need to step itself.
    fn step(
        &mut self,
        time: usize,
        inputs: HashMap<Eid, Map<AttributeId, Value>>,
        max_advance: usize,
    ) -> Option<usize> {
        trace!("the inputs in step: {:?}", inputs);
        let mut deltas: Vec<(String, u64, Map<String, Value>)> = Vec::new();
        for (eid, attrs) in inputs.into_iter() {
            for (attr, attr_values) in attrs.into_iter() {
                let model_idx = match self.get_mut_entities().get(&eid) {
                    Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                    _ => panic!(
                        "No correct model eid available. Input: {:?}, Entities: {:?}",
                        eid,
                        self.get_mut_entities()
                    ),
                };
                if let Value::Object(values) = attr_values {
                    deltas.push((attr, model_idx, values));
                    debug!("the deltas for sim step: {:?}", deltas);
                };
            }
        }
        self.sim_step(deltas);

        Some(time + (self.get_step_size() as usize))
    }

    //collect data from the simulation and return a nested Vector containing the information
    fn get_data(&mut self, outputs: HashMap<Eid, Vec<AttributeId>>) -> Map<Eid, Value> {
        let mut data: Map<String, Value> = Map::new();
        for (eid, attrs) in outputs.into_iter() {
            let model_idx = match self.get_mut_entities().get(&eid) {
                Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                _ => panic!("No correct model eid available."),
            };
            let mut attribute_values = Map::new();
            for attr in attrs.into_iter() {
                //Get the values of the model
                if let Some(value) = self.get_model_value(model_idx, &attr) {
                    attribute_values.insert(attr, value);
                } else {
                    error!(
                        "No attribute called {} available in model {}",
                        &attr, model_idx
                    );
                }
            }
            data.insert(eid, Value::from(attribute_values));
        }
        data
        // TODO https://mosaik.readthedocs.io/en/latest/mosaik-api/low-level.html#get-data
        // api-v3 needs optional 'time' entry in output map for event-based and hybrid Simulators
    }

    ///The function mosaik calls, if the simulation finished. Return Null. The simulation API stops as soon as the function returns.
    fn stop(&self);
}

///Async API calls, not implemented!
#[async_trait]
trait AsyncApi {
    async fn get_progress();
    async fn get_related_entities();
    async fn get_data();
    async fn set_data();
}

//------------------------------------------------------------------
// Here begins the async TCP-Manager
//------------------------------------------------------------------

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
//channels needed for the communication in the async tcp
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum Void {}
pub enum ConnectionDirection {
    ConnectToAddress(SocketAddr),
    ListenOnAddress(SocketAddr),
}

///Build the connection between Mosaik and us. 2 cases, we connect to them or they connect to us.
async fn build_connection<T: MosaikApi>(addr: ConnectionDirection, simulator: T) -> Result<()> {
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

///Recieve the Requests, send them to the broker_loop.
async fn connection_loop(
    mut broker: Sender<Event>,
    mut connection_shutdown_reciever: Receiver<bool>,
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
                                    full_data: String::from_utf8(full_package[0..size].to_vec())
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
                    info!("recieve connection_shutdown command");
                    break;
                },
                None => break,
            }
        }
    }
    Ok(())
}

///Recieve the Response from the broker_loop and write it in the stream.
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

///Recieve requests from the connection_loop, parse them, get the values from the API and send the finished response to the connection_writer_loop
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
                .expect("unaible to read remote peer address from {name}"),
            client_sender,
        );
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
                match json::parse_request(full_data) {
                    Ok(request) => {
                        //Handle the request -> simulations calls etc.
                        trace!("The request: {:?} from {name}", request);
                        use json::Response::*;
                        match json::handle_request(request, &mut simulator) {
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
                                if let Err(e) = connection_shutdown_sender.send(true).await {
                                    error!("error sending to the shutdown channel: {}", e);
                                }
                                break 'event_loop;
                            }
                            None => {
                                info!("Nothing to respond");
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error while parsing the request from {name}: {:?}", e);
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
