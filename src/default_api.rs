//! This module contains some default implementations for the mosaik API functions.

use crate::types::*;
use log::{debug, error, info, trace};
use serde_json::{Map, Value};
use std::collections::HashMap;

pub trait ApiHelpers {
    /// Gets the meta from the simulator, needs to be implemented on the simulator side.
    fn meta(&self) -> Meta;
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

    for (key, value) in sim_params {
        match (key.as_str(), value) {
            ("eid_prefix", Value::String(eid_prefix)) => {
                simulator.set_eid_prefix(eid_prefix.as_str());
            }
            ("step_size", Value::Number(step_size)) => {
                simulator.set_step_size(step_size.as_i64().unwrap());
            }
            _ => {
                info!("Unknown parameter: {}", key);
            }
        }
    }

    simulator.meta()
}

pub fn default_create<T: ApiHelpers>(
    simulator: &mut T,
    num: usize,
    model_name: String,
    model_params: Map<Attr, Value>,
) -> Vec<CreateResult> {
    let mut out_vector = Vec::new();
    let next_eid = simulator.get_mut_entities().len();
    for i in next_eid..(next_eid + num) {
        let eid = format!("{}{}", simulator.get_eid_prefix(), i);
        simulator
            .get_mut_entities()
            .insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
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

    debug!("the created model: {:?}", out_vector);
    out_vector
}

pub fn default_step<T: ApiHelpers>(
    simulator: &mut T,
    time: usize,
    inputs: InputData,
    _max_advance: usize,
) -> Option<usize> {
    trace!("the inputs in step: {:?}", inputs);
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

    Some(time + (simulator.get_step_size() as usize))
}

pub fn default_get_data<T: ApiHelpers>(simulator: &mut T, outputs: OutputRequest) -> OutputData {
    let mut data: HashMap<String, HashMap<Attr, Value>> = HashMap::new();
    for (eid, attrs) in outputs.into_iter() {
        let model_idx = match simulator.get_mut_entities().get(&eid) {
            Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
            _ => panic!("No correct model eid available."),
        };
        let mut attribute_values: HashMap<Attr, Value> = HashMap::new();
        for attr in attrs.into_iter() {
            //Get the values of the model
            if let Some(value) = simulator.get_model_value(model_idx, &attr) {
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
    OutputData {
        requests: data,
        time: None,
    }
    // TODO https://mosaik.readthedocs.io/en/latest/mosaik-api/low-level.html#get-data
    // api-v3 needs optional 'time' entry in output map for event-based and hybrid Simulators
}
