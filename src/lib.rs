pub mod default_api;
pub mod json;
pub mod tcp;
pub mod types;

#[cfg(test)]
use mockall::automock;

use crate::{
    tcp::{build_connection, ConnectionDirection},
    types::*,
};

use async_std::task;
use async_trait::async_trait;
use serde_json::{json, Map, Value};

type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

///Main calls this function with the simulator that should run. For the option that we connect our selfs addr as option!...
pub fn run_simulation<T: MosaikApi>(addr: ConnectionDirection, simulator: T) -> AResult<()> {
    task::block_on(build_connection(addr, simulator))
}

///the class for the "empty" API calls
#[cfg_attr(test, automock)]
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

    fn extra_method(
        &mut self,
        method: &str,
        args: &Vec<Value>,
        kwargs: &Map<String, Value>,
    ) -> Result<Value, crate::json::MosaikError> {
        Err(crate::json::MosaikError::MethodNotFound(method.to_string()))
    }
}

///Async API calls, not implemented!
#[allow(dead_code)]
#[async_trait]
trait AsyncApi {
    async fn get_progress();
    async fn get_related_entities();
    async fn get_data();
    async fn set_data();
}
