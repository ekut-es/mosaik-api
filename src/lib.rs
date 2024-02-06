pub mod json;
pub mod tcp;

use std::collections::HashMap;

use crate::tcp::{build_connection, ConnectionDirection};
use async_std::task;
use async_trait::async_trait;
use log::{debug, error, info, trace};
use serde_json::{json, Map, Value};
type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

///Main calls this function with the simulator that should run. For the option that we connect our selfs addr as option!...
pub fn run_simulation<T: MosaikApi>(addr: ConnectionDirection, simulator: T) -> AResult<()> {
    task::block_on(build_connection(addr, simulator))
}

///information about the model(s) of the simulation
pub type Meta = serde_json::Value;

///Id of the simulation
pub type Sid = String;

///Id of an entity
pub type Eid = String;

///Id of an attribute of a Model
pub type AttributeId = String;

pub type Children = Value;

pub struct Simulator {
    // public static final String API_VERSION
    pub api_version: &'static str,
    // private final String simName
    // - sim_name: &'static str,
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
    fn get_mut_entities(&mut self) -> &mut Map<Eid, Value>;
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

pub trait DefaultMosaikApi: ApiHelpers {
    fn init(&mut self, sid: Sid, time_resolution: f64, sim_params: Map<String, Value>) -> Meta {
        if time_resolution != 1.0 {
            info!("time_resolution must be 1.0"); // TODO this seems not true
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

    fn create(
        &mut self,
        num: usize,
        model_name: String,
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
            out_entities.insert(String::from("type"), Value::String(model_name.clone()));
            if let Some(children) = children {
                out_entities.insert(String::from("children"), children);
            }
            debug!("{:?}", out_entities);
            out_vector.push(out_entities);
        }

        debug!("the created model: {:?}", out_vector);
        out_vector
    }

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
}

///the class for the "empty" API calls
pub trait MosaikApi: Send + 'static {
    /// Initialize the simulator with the ID sid and apply additional parameters (sim_params) sent by mosaik.
    /// Return the meta data meta.
    fn init(&mut self, sid: Sid, time_resolution: f64, sim_params: Map<String, Value>) -> Meta;

    ///Create *num* instances of *model* using the provided *model_params*.
    /// Returned list must have the same length as *num*
    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<AttributeId, Value>,
    ) -> Vec<Map<String, Value>>;

    ///The function mosaik calls, if the init() and create() calls are done. Return Null
    fn setup_done(&self);

    /// Perform the next simulation step at time and return the new simulation time (the time at which step should be called again)
    ///  or null if the simulator doesn’t need to step itself.
    fn step(
        &mut self,
        time: usize,
        inputs: HashMap<Eid, Map<AttributeId, Value>>,
        max_advance: usize,
    ) -> Option<usize>;

    //collect data from the simulation and return a nested Vector containing the information
    fn get_data(&mut self, outputs: HashMap<Eid, Vec<AttributeId>>) -> Map<Eid, Value>;

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
