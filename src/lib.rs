use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::{Map, Value};
pub mod householdsim;
pub mod json;
mod simple_simulator;
pub mod simulation_mosaik;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

///information about the model(s) of the simulation
pub type META = serde_json::Value;

///Id of the simulation
pub type Sid = String;

pub type Model = Value;

///Id of an entity
pub type Eid = String;

///Id of an attribute of a Model
pub type AttributeId = String;

pub enum Object {}

pub trait API_Helpers {
    fn meta() -> serde_json::Value;
    fn set_eid_prefix(&mut self, eid_prefix: &str);
}
///the class for the "empty" API calls
pub trait MosaikAPI: API_Helpers + Send + 'static {
    /// Initialize the simulator with the ID sid and apply additional parameters (sim_params) sent by mosaik. Return the meta data meta.
    fn init(&mut self, sid: Sid, sim_params: Option<Map<String, Value>>) -> META {
        match sim_params {
            Some(sim_params) => {
                if let Some(eid_prefix) = sim_params.get("eid_prefix") {
                    if let Some(prefix) = eid_prefix.as_str() {
                        self.set_eid_prefix(prefix);
                    }
                }
            }
            None => {}
        }
        Self::meta()
    }

    ///Create *num* instances of *model* using the provided *model_params*.
    fn create(
        &mut self,
        num: usize,
        model: Model,
        model_params: Option<Map<String, Value>>,
    ) -> Vec<Map<String, Value>>;

    ///The function mosaik calls, if the init() and create() calls are done. Return Null
    fn setup_done(&self);

    ///perform a simulatino step and return the new time
    fn step(&mut self, time: usize, inputs: HashMap<Eid, Map<AttributeId, Value>>) -> usize;

    //collect data from the simulation and return a nested Vector containing the information
    fn get_data(&mut self, outputs: HashMap<Eid, Vec<AttributeId>>) -> Map<Eid, Value>; //Map<Eid, Map<Attribute_Id, Value>>;

    ///The function mosaik calls, if the simulation finished. Return Null
    fn stop(&self);
}

#[async_trait]
trait async_api {
    async fn get_progress();
    async fn get_related_entities();
    async fn get_data();
    async fn set_data();
}
