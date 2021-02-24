//use log::error;
use serde_json::{json, Map, Value};
//use std::collections::HashMap;

use crate::{
    householdsim::{self, Householdsim},
    API_Helpers, AttributeId, Eid, MosaikAPI,
};

pub struct ExampleSim {
    simulator: householdsim::Householdsim,
    eid_prefix: String,
    step_size: i64,
    entities: Map<String, Value>,
}

pub fn init_sim() -> ExampleSim {
    ExampleSim {
        simulator: Householdsim::init_simulator(),
        eid_prefix: String::from("Model_"),
        step_size: 15 * 60,
        entities: Map::new(),
    }
}

impl API_Helpers for ExampleSim {
    fn meta() -> Value {
        let meta = json!({
        "api_version": "2.2",
        "models":{
            "ExampleModel":{
                "public": true,
                "params": ["init_reading"],
                "attrs": ["p_mw_pv", "p_mw_load", "reading"]
                }
            }
        });
        return meta;
    }

    fn set_eid_prefix(&mut self, eid_prefix: &str) {
        self.eid_prefix = eid_prefix.to_string();
    }

    fn get_eid_prefix(&self) -> &str {
        &self.eid_prefix
    }

    fn get_mut_entities(&mut self) -> &mut Map<String, Value> {
        &mut self.entities
    }

    fn add_model(&mut self, model_params: Map<AttributeId, Value>) {
        self.simulator.add_model(model_params);
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        self.simulator
            .models
            .get(model_idx as usize)
            .and_then(|x| x.get_value(attr))
    }

    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>) {
        self.simulator.step(deltas)
    }

    fn set_step_size(&mut self, step_size: i64) {
        self.step_size = step_size;
    }

    fn get_step_size(&self) -> i64 {
        self.step_size
    }
}

///implementation of the trait in mosaik_api.rs
impl MosaikAPI for ExampleSim {
    /*
    fn create(
        &mut self,
        num: usize,
        model: Value,
        model_params: Option<Map<String, Value>>,
    ) -> Vec<Map<String, Value>> {
        let mut out_entities: Map<String, Value>;
        let mut out_vector = Vec::new();
        let next_eid = self.entities.len();
        match model_params {
            Some(model_params) => {
                if let Some(init_reading) = model_params.get("init_reading") {
                    for i in next_eid..(next_eid + num) {
                        out_entities = Map::new();
                        let eid = format!("{}{}", self.eid_prefix, i);
                        self.simulator.add_model(init_reading.as_f64());
                        self.entities.insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
                        out_entities.insert(String::from("eid"), json!(eid));
                        out_entities.insert(String::from("type"), model.clone());
                        out_vector.push(out_entities);
                    }
                }
            }
            None => {}
        }
        println!("the created model: {:?}", out_vector);
        return out_vector;
    }*/
    /*
    fn step(&mut self, time: usize, inputs: HashMap<Eid, Map<AttributeId, Value>>) -> usize {
        println!("the inputs in step: {:?}", inputs);
        let mut deltas: Vec<(String, u64, Map<String, Value>)> = Vec::new();
        for (eid, attrs) in inputs.into_iter() {
            for (attr, attr_values) in attrs.into_iter() {
                let model_idx = match self.entities.get(&eid) {
                    Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                    _ => panic!(
                        "No correct model eid available. Input: {:?}, Entities: {:?}",
                        eid, self.entities
                    ),
                };
                if let Value::Object(values) = attr_values {
                    deltas.push((attr, model_idx, values));
                    println!("the deltas for sim step: {:?}", deltas);
                };
            }
        }
        self.simulator.step(deltas);
        return time + 60;
    }*/
    /*
    fn get_data(&mut self, output: HashMap<Eid, Vec<AttributeId>>) -> Map<String, Value> {
        let meta = Self::meta();
        let models = &self.simulator.models;
        let mut data: Map<String, Value> = Map::new();
        for (eid, attrs) in output.into_iter() {
            let model_idx = match self.entities.get(&eid) {
                Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                _ => panic!("No correct model eid available."),
            };
            let mut attribute_values = Map::new();
            match models.get(model_idx as usize) {
                Some(model) => {
                    for attr in attrs.into_iter() {
                        //Wir müssen überprüfen, ob das Attribut sich überhaupt in unserer META data befindet.
                        assert!(
                            meta["models"]["ExampleModel"]["attrs"]
                                .as_array()
                                .map_or(false, |x| x.contains(&json!(attr))),
                            "Unknown output attribute: {}",
                            json!(attr)
                        );
                        //Get model.val or model.p_mw_load:
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
    }*/
    fn stop(&self) {
        println!("Stop the simulation.");
    }

    fn setup_done(&self) {
        println!("Setup is done.");
    }
}
/*
#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn test_meta() {
        let meta = json!({
        "api_version": "2.2",
        "models":{
            "ExampleModel":{
                "public": true,
                "params": ["init_p_mw_pv"],
                "attrs": ["val", "p_mw_load"]
                }
            }
        });
        let attr = "val";
        assert_eq!(meta["models"]["ExampleModel"]["attrs"][0], json!(attr));
    }
}
*/
