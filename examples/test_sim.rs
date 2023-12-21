/* API calls:

    init

    create

    setup_done

    step

    get_data

    stop

Async. requests:

    get_progress

    get_related_entities

    get_data

    set_data

    set_event
 */
use std::collections::HashMap;

use mosaik_rust_api::{ApiHelpers, Eid, MosaikApi, Sid};
use serde_json::{json, Value};

pub struct RModel {
    val: f64,
    delta: f64,
}

impl RModel {
    pub fn new(init_val: Option<f64>) -> Self {
        Self {
            val: init_val.unwrap_or_default(),
            delta: 1.0,
        }
    }

    pub fn get_val(&self) -> f64 {
        self.val
    }

    pub fn get_delta(&self) -> f64 {
        self.delta
    }

    pub fn set_delta(&mut self, delta: f64) {
        self.delta = delta;
    }

    pub fn step(&mut self) {
        self.val += self.delta;
    }
}

pub struct RSimulator {
    models: Vec<RModel>,
}

impl RSimulator {
    pub fn __init__() -> Self {
        Self { models: Vec::new() }
    }

    pub fn add_model(&mut self, init_val: Option<f64>) {
        self.models.push(RModel::new(init_val));
    }

    pub fn step(&mut self) {
        for model in self.models.iter_mut() {
            model.step();
        }
    }

    pub fn get_val(&self, index: usize) -> f64 {
        self.models[index].get_val()
    }

    pub fn get_delta(&self, index: usize) -> f64 {
        self.models[index].get_delta()
    }

    pub fn set_delta(&mut self, index: usize, delta: f64) {
        self.models[index].set_delta(delta);
    }
}

pub struct RExampleSim {
    eid_prefix: String,
    step_size: i64,
    time_resolution: f64,
    entities: serde_json::Map<String, Value>,
    models: Vec<RModel>,
}

impl ApiHelpers for RExampleSim {
    fn meta() -> Value {
        META
    }

    fn set_eid_prefix(&mut self, eid_prefix: &str) {
        self.eid_prefix = eid_prefix.to_string();
    }

    fn get_eid_prefix(&self) -> &str {
        &self.eid_prefix
    }

    fn get_step_size(&self) -> i64 {
        self.step_size
    }

    fn set_step_size(&mut self, step_size: i64) {
        self.step_size = step_size;
    }

    fn get_mut_entities(&mut self) -> &mut serde_json::Map<String, Value> {
        &mut self.entities
    }

    fn add_model(
        &mut self,
        model_params: serde_json::Map<mosaik_rust_api::AttributeId, Value>,
    ) -> Option<Value> {
        let init_val = model_params.get("init_val").map(|v| v.as_f64().unwrap());
        self.models.push(RModel::new(init_val));
        None
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        todo!("not implemented")
    }

    fn sim_step(&mut self, deltas: Vec<(String, u64, serde_json::Map<String, Value>)>) {
        todo!("not implemented")
    }

    fn get_time_resolution(&self) -> f64 {
        self.time_resolution
    }

    fn set_time_resolution(&mut self, time_resolution: f64) {
        self.time_resolution = time_resolution;
    }
}

const META: Value = json!({
    "type": "hybrid",
    "models": {
        "ExampleModel": {
            "public": true,
            "params": ["init_val"],
            "attrs": ["delta", "val"],
            "trigger": ["delta"],
        },
    },
});

impl MosaikApi for RExampleSim {
    fn create(
        &mut self,
        num: usize,
        model: mosaik_rust_api::Model,
        model_params: serde_json::Map<mosaik_rust_api::AttributeId, Value>,
    ) -> Vec<serde_json::Map<String, Value>> {
        let next_eid = self.entities.len();
        let mut result: Vec<serde_json::Map<String, Value>> = Vec::new();

        for i in next_eid..(next_eid + num) {
            let model_instance = model.clone(); // FIXME this is a String, but shouldn't this rather be a RModel?
            let eid = format!("{}_{}", self.eid_prefix, i);
            self.entities.insert(eid.clone(), model); // FIXME shouldn't this be a model instance?

            let mut dict = serde_json::Map::new();
            dict.insert("eid".to_string(), json!(eid));
            dict.insert("type".to_string(), json!(model));

            result.push(dict);
        }

        // entities must have length of num
        assert_eq!(result.len(), num); // TODO improve error handling
        result
    }

    fn setup_done(&self) {
        // Nothing to do
    }

    fn stop(&self) {
        // Nothing to do
    }
}

//let mut r: Value = ExampleSim.init(0, 1.0, None);

// !ExampleSim.create(1, "ExampleModel", None);
