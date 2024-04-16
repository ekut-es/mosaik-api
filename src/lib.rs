pub mod json;
pub mod tcp;
pub mod types;

use crate::{
    tcp::{build_connection, ConnectionDirection},
    types::CreateResultOptionals,
};
use async_std::task;
use async_trait::async_trait;
use log::{debug, error, info, trace};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use types::{
    Attr, CreateResult, CreateResultChild, EntityId, InputData, Meta, OutputData, OutputRequest,
    SimId,
};
type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

///Main calls this function with the simulator that should run. For the option that we connect our selfs addr as option!...
pub fn run_simulation<T: MosaikApi>(addr: ConnectionDirection, simulator: T) -> AResult<()> {
    task::block_on(build_connection(addr, simulator))
}

// pub struct Simulator {
//     // public static final String API_VERSION
//     pub api_version: &'static str,
//     // private final String simName
//     // - sim_name: &'static str,
//     // missing -> mosaik: MosaikProxy
// }

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
    fn get_mut_entities(&mut self) -> &mut Map<EntityId, Value>;
    /// Create a model instance (= entity) with an initial value. Returns the
    /// [types](CreateResultChild) representation of the children, if the entity has children.
    fn add_model(&mut self, model_params: Map<Attr, Value>) -> Option<Vec<CreateResultChild>>;
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
    fn init(&mut self, sid: SimId, time_resolution: f64, sim_params: Map<String, Value>) -> Meta {
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
        model_params: Map<Attr, Value>,
    ) -> Vec<CreateResult> {
        let mut out_vector = Vec::new();
        let next_eid = self.get_mut_entities().len();
        for i in next_eid..(next_eid + num) {
            let eid = format!("{}{}", self.get_eid_prefix(), i);
            self.get_mut_entities().insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
            let out_entity = CreateResult {
                eid,
                r#type: model_name.clone(),
                optionals: Some(CreateResultOptionals {
                    rel: None,
                    children: self.add_model(model_params.clone()),
                    extra_info: None,
                }),
            };
            debug!("{:?}", out_entity);
            out_vector.push(out_entity);
        }

        debug!("the created model: {:?}", out_vector);
        out_vector
    }

    fn step(&mut self, time: usize, inputs: InputData, max_advance: usize) -> Option<usize> {
        trace!("the inputs in step: {:?}", inputs);
        let mut deltas: Vec<(EntityId, u64, Map<Attr, Value>)> = Vec::new();
        for (eid, attrs) in inputs.into_iter() {
            for (attr, attr_values) in attrs.into_iter() {
                let model_idx = match self.get_mut_entities().get(&eid.clone()) {
                    Some(entity_val) if entity_val.is_u64() => entity_val.as_u64().unwrap(), //unwrap safe, because we check for u64
                    _ => panic!(
                        "No correct model eid available. Input: {:?}, Entities: {:?}",
                        eid,
                        self.get_mut_entities()
                    ),
                };
                deltas.push((attr, model_idx, attr_values));
            }
        }
        self.sim_step(deltas);

        Some(time + (self.get_step_size() as usize))
    }

    fn get_data(&mut self, outputs: OutputRequest) -> OutputData {
        let mut data: HashMap<String, HashMap<Attr, Value>> = HashMap::new();
        for (eid, attrs) in outputs.into_iter() {
            let model_idx = match self.get_mut_entities().get(&eid) {
                Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                _ => panic!("No correct model eid available."),
            };
            let mut attribute_values: HashMap<Attr, Value> = HashMap::new();
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
            data.insert(eid, attribute_values);
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
    fn init(&mut self, sid: SimId, time_resolution: f64, sim_params: Map<String, Value>) -> Meta;

    ///Create *num* instances of *model* using the provided *model_params*.
    /// Returned list must have the same length as *num*
    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<Attr, Value>,
    ) -> Vec<CreateResult>;

    ///The function mosaik calls, if the init() and create() calls are done. Return Null
    fn setup_done(&self);

    /// Perform the next simulation step at time and return the new simulation time (the time at which step should be called again)
    ///  or null if the simulator doesn’t need to step itself.
    fn step(&mut self, time: usize, inputs: InputData, max_advance: usize) -> Option<usize>;

    //collect data from the simulation and return a nested Vector containing the information
    fn get_data(&mut self, outputs: OutputRequest) -> OutputData;

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
