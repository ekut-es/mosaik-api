use log::{error, info};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::{
    simple_simulator::{self, RunSimulator, Simulator},
    AttributeId, Eid, MosaikAPI,
};

pub fn meta() -> serde_json::Value {
    let meta = json!({
    "api_version": "2.2",
    "models":{
        "ExampleModel":{
            "public": true,
            "params": ["init_val"],
            "attrs": ["val", "delta"]
            }
        }
    }); /*
        let meta = r#"{
            "api_version": "2.2",
            "models":{
                "ExampleModel":{
                    "public": true,
                    "params": ["init_val"],
                    "attrs": ["val", "delta"]
                    }
                }
            }"#;*/
    return meta;
}

pub struct ExampleSim {
    simulator: simple_simulator::Simulator,
    eid_prefix: String,
    entities: Map<String, Value>,
    meta: serde_json::Value,
}

pub fn init_sim() -> ExampleSim {
    ExampleSim {
        simulator: Simulator::init_simulator(),
        eid_prefix: String::from("model_"),
        entities: Map::new(),
        meta: meta(), //sollte eigentlich die richtige meta sein und keine funktion
    }
}

///implementation of the trait in mosaik_api.rs
impl MosaikAPI for ExampleSim {
    fn init(&mut self, sid: String, sim_params: Option<Map<String, Value>>) -> serde_json::Value {
        match sim_params {
            Some(sim_params) => {
                if let Some(eid_prefix) = sim_params.get("eid_prefix") {
                    self.eid_prefix = eid_prefix.to_string();
                }
            }
            None => {}
        }
        meta()
    }

    fn create(
        &mut self,
        num: usize,
        model: Value,
        model_params: Option<Map<String, Value>>,
    ) -> Vec<Value> {
        let mut out_entities: Map<String, Value> = Map::new();
        let mut out_vector = Vec::new();
        let next_eid = self.entities.len();
        match model_params {
            Some(model_params) => {
                if let Some(init_val) = model_params.get("init_val") {
                    for i in next_eid..(next_eid + num) {
                        let mut eid = format!("{}{}", self.eid_prefix, i);
                        Simulator::add_model(&mut self.simulator, init_val.as_f64());
                        self.entities.insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
                        out_entities.insert(String::from("eid"), Value::from(eid));
                        out_entities.insert("type".to_string(), Value::from(model.clone()));
                    }
                }
            }
            None => {}
        }
        out_vector.push(Value::from(out_entities));
        return out_vector;
    }

    fn step(&mut self, mut time: usize, inputs: HashMap<Eid, Map<AttributeId, Value>>) -> usize {
        let mut deltas: Vec<(u64, f64)> = Vec::new();
        let mut new_delta: f64;
        for (eid, attrs) in inputs.iter() {
            for (attr, attr_values) in attrs.iter() {
                let mut model_idx = match self.entities.get(eid) {
                    Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                    _ => panic!(
                        "No correct model eid available. Input: {:?}, Entities: {:?}",
                        inputs, self.entities
                    ),
                };
                if let Value::Object(values) = attr_values {
                    new_delta = values
                        .values()
                        .map(|x| x.as_f64().unwrap_or_default())
                        .sum(); //unwrap -> default = 0 falls kein f64
                    deltas.push((model_idx, new_delta));
                };
            }
        }
        Simulator::step(&mut self.simulator, Some(deltas));
        time = time + 60;
        return time;
    }

    fn get_data(&mut self, output: HashMap<Eid, Vec<AttributeId>>) -> Map<String, Value> {
        let meta = meta();
        let models = &self.simulator.models;
        let mut data: Map<String, Value> = Map::new();
        for (eid, attrs) in output.into_iter() {
            let mut model_idx = match self.entities.get(&eid) {
                Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                _ => panic!("No correct model eid available.",),
            };
            let mut attribute_values = Map::new();
            match models.get(model_idx as usize) {
                Some(model) => {
                    for attr in attrs.into_iter() {
                        assert!(
                            meta["models"]["ExampleModel"]
                                .as_array()
                                .map_or(false, |x| x.contains(&json!(attr))),
                            "Unknown output attribute: {}",
                            attr
                        );
                        //Get model.val or model.delta:
                        if let Some(value) = model.get_value(&attr) {
                            attribute_values.insert(attr, value);
                        }
                    }
                    data.insert(eid, Value::from(attribute_values));
                }
                None => error!("No model_idx in models: {}", model_idx),
            }
        }
        return data;
    }
    fn stop() {}

    fn setup_done(&self) {
        info!("Setup is done.");
    }
}
#[test]
fn test() {
    let meta = json!({
    "api_version": "2.2",
    "models":{
        "ExampleModel":{
            "public": true,
            "params": ["init_val"],
            "attrs": ["val", "delta"]
            }
        }
    });
    let attr = "val";
    assert_eq!(meta["models"]["ExampleModel"]["attrs"][0], json!(attr));
}
