pub mod mosaik_protocol;
pub mod tcp;
pub mod types;

use crate::{
    tcp::{build_connection, ConnectionDirection},
    types::{Attr, CreateResult, InputData, Meta, OutputData, OutputRequest, SimId, Time},
};

use async_std::task;
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;
use serde_json::{Map, Value};

type AResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

///Main calls this function with the simulator that should run and the direction with an address for the connection to mosaik.
pub fn run_simulation<T: MosaikApi>(addr: ConnectionDirection, simulator: T) -> AResult<()> {
    task::block_on(build_connection(addr, simulator))
}

/// The `MosaikApi` trait defines the interface for a Mosaik simulator API.
/// Errors will result in a Failure Response being sent to Mosaik containing the Error's message and/or Stack Trace.
#[cfg_attr(test, automock)]
pub trait MosaikApi: Send + 'static {
    /// Initialize the simulator with the specified ID (`sid`), time resolution (`time_resolution`), and additional parameters (`sim_params`).
    /// Returns the meta data (`Meta`) of the simulator.
    fn init(
        &mut self,
        sid: SimId,
        time_resolution: f64,
        sim_params: Map<String, Value>,
    ) -> Result<&'static Meta, String>;

    /// Create `num` instances of the specified `model_name` using the provided `model_params`.
    /// The returned list must have the same length as `num`.
    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<Attr, Value>,
    ) -> Result<Vec<CreateResult>, String>;

    /// This function is called by Mosaik when the `init()` and `create()` calls are done.
    /// Returns `Null`.
    fn setup_done(&self) -> Result<(), String>;

    /// Perform the next simulation step at `time` and return the new simulation time (the time at which `step` should be called again),
    /// or `None` if the simulator doesn't need to step itself. The Return value must be set for time-based simulators.
    fn step(
        &mut self,
        time: Time,
        inputs: InputData,
        max_advance: Time,
    ) -> Result<Option<Time>, String>;

    /// Collect data from the simulation and return a nested vector (`OutputData`) containing the information.
    fn get_data(&self, outputs: OutputRequest) -> Result<OutputData, String>;

    /// This function is called by Mosaik when the simulation is finished.
    /// Returns `Null`. The simulation API stops as soon as the function returns.
    fn stop(&self);

    /// A wrapper for extra methods that can be implemented by the simulator.
    /// This method is not required by the Mosaik API, but can be used for additional functionality.
    /// Returns a `Result` containing the result of the method call or an error message if the method is not found.
    fn extra_method(
        &mut self,
        method: &str,
        args: &Vec<Value>,
        kwargs: &Map<String, Value>,
    ) -> Result<Value, String> {
        Err(format!(
            "Method '{method}' not found with args: {args:?} and kwargs: {kwargs:?}"
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockMosaikApi;
    use mockall::predicate::*;
    use serde_json::json;

    #[test]
    fn test_extra_method_with_logic() {
        let mut mock = MockMosaikApi::new();
        let args = vec![json!(1), json!("hello")];
        let kwargs = Map::new();

        // Implement the logic for extra_method
        let actual_method = |method: &str| match method {
            "example_method" => Ok(Value::String("example result".to_string())),
            _ => Err(format!("Method not found: {method}")),
        };

        // Set up expectation
        mock.expect_extra_method()
            .with(
                mockall::predicate::eq("example_method"),
                mockall::predicate::eq(args.clone()),
                mockall::predicate::eq(kwargs.clone()),
            )
            .returning(move |method, _, _| actual_method(method));

        mock.expect_extra_method()
            .with(
                mockall::predicate::eq("unknown_method"),
                mockall::predicate::eq(args.clone()),
                mockall::predicate::eq(kwargs.clone()),
            )
            .returning(move |method, _, _| actual_method(method));

        // Call the method and assert for known method
        let result = mock.extra_method("example_method", &args, &kwargs);
        assert_eq!(result, Ok(Value::String("example result".to_string())));

        // Call the method and assert for unknown method
        let result = mock.extra_method("unknown_method", &args, &kwargs);
        assert_eq!(result, Err("Method not found: unknown_method".to_string()));
    }
}
