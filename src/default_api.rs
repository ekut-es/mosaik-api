//! This module contains some default implementations for the mosaik API functions.

use crate::types::*;
use core::panic;
use log::{debug, error, info, trace};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Some helper functions for the default API.
pub trait ApiHelpers {
    /// Retrieves the metadata associated with the current instance.
    ///
    /// # Used in:
    /// - [default_init]
    fn meta(&self) -> Meta;

    /// Set the `eid_prefix` on the simulator, which we got from the interface.
    ///
    /// # Used in:
    /// - [default_init]
    fn set_eid_prefix(&mut self, eid_prefix: &str);

    /// Set the step_size on the simulator, which we got from the interface.
    ///
    /// # Used in:
    /// - [default_init]
    fn set_step_size(&mut self, step_size: i64);

    ///Get the eid_prefix.
    ///
    /// # Used in:
    /// - [default_create]
    fn get_eid_prefix(&self) -> &str;

    /// Get the step_size in the api call step().
    ///
    /// # Used in:
    /// - [default_step]
    fn get_step_size(&self) -> i64;

    /// Get the list containing the created entities.
    ///
    /// # Used in:
    /// - [default_create]
    /// - [default_get_data]
    /// - [default_step]
    fn get_mut_entities(&mut self) -> &mut Map<EntityId, Value>;

    /// Create a model instance (= entity) with an initial value. Returns the
    /// [types](CreateResultChild) representation of the children, if the entity has children.
    ///
    /// # Used in:
    /// - [default_create]
    fn add_model(&mut self, model_params: Map<Attr, Value>) -> Option<Vec<CreateResultChild>>;

    /// Get the value from an entity.
    ///
    /// # Used in:
    /// - [default_get_data]
    fn get_model_value(&self, model_idx: u64, attr: &str) -> Result<Value, String>;

    /// Call the step function to perform a simulation step and include the deltas from mosaik, if there are any.
    ///
    /// # Used in:
    /// - [default_step]
    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>);

    /// Get the time resolution of the Simulator.
    ///
    /// # Used in:
    ///
    fn get_time_resolution(&self) -> f64;

    /// Set the time resolution of the Simulator.
    ///
    /// # Used in:
    /// - [default_init]
    fn set_time_resolution(&mut self, time_resolution: f64);

    /// Set the time of the Simulator.
    /// # Used in:
    /// - [default_step]
    fn set_time(&mut self, time: Time);

    /// Get the time of the Simulator.
    ///
    /// # Used in:
    /// - [default_get_data]
    fn get_time(&self) -> Time;
}

/// The default implementation for the init function.
/// This function sets the time resolution and sets eid prefix and step size if they are given in the `sim_params`.
pub fn default_init<T: ApiHelpers>(
    simulator: &mut T,
    sid: SimId,
    time_resolution: f64,
    sim_params: Map<String, Value>,
) -> Meta {
    if time_resolution <= 0.0 {
        debug!("Non positive time resolution was given!");
    }
    simulator.set_time_resolution(time_resolution.abs());

    for (key, value) in sim_params.into_iter() {
        match (key.as_str(), value) {
            ("eid_prefix", Value::String(eid_prefix)) => {
                simulator.set_eid_prefix(eid_prefix.as_str());
            }
            ("step_size", Value::Number(step_size)) => {
                if let Some(step_size) = step_size.as_i64() {
                    simulator.set_step_size(step_size);
                } else {
                    error!("Step size is not a valid number: {:?}", step_size);
                    // TODO return error
                }
            }
            _ => {
                info!("Init: Unknown parameter: {}", key);
            }
        }
    }

    simulator.meta()
}

/// The default implementation for the create function.
/// Adds the entities to the simulator and returns the [types](CreateResult) representation of the created entities.
pub fn default_create<T: ApiHelpers>(
    simulator: &mut T,
    num: usize,
    model_name: ModelName,
    model_params: Map<Attr, Value>,
) -> Vec<CreateResult> {
    let next_eid = simulator.get_mut_entities().len();
    let mut out_vector = Vec::new();
    for i in next_eid..(next_eid + num) {
        let model_instance = Value::from(i); // TODO check if this is correct
        let eid = format!("{}{}", simulator.get_eid_prefix(), i);
        simulator
            .get_mut_entities()
            .insert(eid.clone(), model_instance); // create a mapping from the entity ID to our model
        let out_entity = CreateResult {
            eid,
            model_type: model_name.clone(),
            rel: None,
            children: simulator.add_model(model_params.clone()), // FIXME check if this is correct
            extra_info: None,
        };
        debug!("{:?}", out_entity);
        out_vector.push(out_entity);
    }

    debug!("the created entities: {:?}", out_vector);
    out_vector
}

/// The default implementation for the step function.
/// Gets the model deltas and calls the simulation step function on the simulator.
pub fn default_step<T: ApiHelpers>(
    simulator: &mut T,
    time: Time,
    inputs: InputData,
    _max_advance: usize,
) -> Option<Time> {
    trace!("the inputs in step: {:?}", inputs);
    simulator.set_time(time);
    // Check for new delta and do step for each model instance:
    let mut deltas: Vec<(EntityId, u64, Map<Attr, Value>)> = Vec::new();
    for (eid, attrs) in inputs.into_iter() {
        for (attr, attr_values) in attrs.into_iter() {
            let model_idx = match simulator.get_mut_entities().get(&eid.clone()) {
                Some(entity_val) if entity_val.is_u64() => entity_val.as_u64().unwrap(), //unwrap safe, because we check for u64
                _ => panic!(
                    "No correct model eid available. Input: {:?}, Entities: {:?}",
                    eid,
                    simulator.get_mut_entities()
                ),
            };
            deltas.push((attr, model_idx, attr_values));
        }
    }
    simulator.sim_step(deltas);

    Some(time + simulator.get_step_size())
}

/// The default implementation for the get_data function.
/// Allows to get `attr` values of the model instances.
pub fn default_get_data<T: ApiHelpers>(simulator: &mut T, outputs: OutputRequest) -> OutputData {
    let mut data: HashMap<String, HashMap<Attr, Value>> = HashMap::new();
    for (eid, attrs) in outputs.into_iter() {
        let model_idx = simulator
            .get_mut_entities()
            .get(&eid)
            .and_then(|eid| eid.as_u64())
            .expect("No correct model eid available.");

        let mut attribute_values: HashMap<Attr, Value> = HashMap::new();
        for attr in attrs.into_iter() {
            //Get the values of the model
            match simulator.get_model_value(model_idx, &attr) {
                Ok(value) => {
                    attribute_values.insert(attr, value);
                }
                Err(e) => {
                    error!(
                        "Unknown output attribute `{}` for model {} results in error: {}",
                        &attr, model_idx, e
                    );
                }
            }
        }
        data.insert(eid, attribute_values);
    }
    OutputData {
        requests: data,
        time: Some(simulator.get_time()),
    }
}
