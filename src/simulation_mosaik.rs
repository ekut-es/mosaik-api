use log::{error, info};
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::{
    simple_simulator::{self, RunSimulator, Simulator},
    Attribute_Id, Eid, MosaikAPI,
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
        model: String,
        model_params: Option<Map<String, Value>>,
    ) -> Map<String, Value> {
        let mut out_entities: Map<String, Value> = Map::new();
        let next_eid = self.entities.len();
        match model_params {
            Some(model_params) => {
                if let Some(init_val) = model_params.get("init_val") {
                    for i in next_eid..(next_eid + num) {
                        let mut eid = format!("{}_{}", self.eid_prefix, i);
                        Simulator::add_model(&mut self.simulator, init_val.as_f64());
                        self.entities.insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
                        out_entities.insert(String::from("eid"), Value::from(eid));
                        out_entities.insert("type".to_string(), Value::from(model.clone()));
                    }
                }
            }
            None => {}
        }
        return out_entities;
    }

    fn step(&mut self, mut time: usize, inputs: HashMap<Eid, Map<Attribute_Id, Value>>) -> usize {
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

    fn get_data(&mut self, output: HashMap<Eid, Vec<Attribute_Id>>) -> Map<String, Value> {
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

pub fn run() {
    init_sim();
}

/* # simulator_mosaik.py
"""Mosaik interface for the example simulator"""
import mosaik_api # contains the method start_simulation() which creates a socket, connects to mosaik and listens for requests from it
import simulator

# simulator meta data
# tells mosaik which models our simulator implements and which parameters and attributes it has
META = {
    'models':{
        'ExampleModel':{
            'public' : True,
            'params': ['init_val'], # added model
            'attrs': ['delta', 'val'], # with attributes delta and val
        },
    },
}

class ExampleSim(mosaik_api.Simulator):
    def __init__(self):
        super().__init__(META) #gibt die META an die mosaik_api.Simulator weiter
        self.simulator = simulator.Simulator()
        self.eid_prefix = 'Model_'
        self.entities = {} #Maps entitiy IDs to model indicies in self.simulator
    # Four API calls: init, create, step and get_data

# called once, after the simulator has been started
# used for additional initializiation tasks (eg. parameters)
# must return the meta
    def init(self, sid, eid_prefix=None):
        if eid_prefix is not None:
            self.eid_prefix = eid_prefix
        return self.meta

# called to initialize a number of simulation entities
# must return a list with information about each enitity created
    def create(self, num, model, init_val):
        next_eid = len(self.entities)
        entities = []

        for i in range(next_eid, next_eid + num): # each entity gets a new ID and a model instance
            eid = '%s%d' % (self.eid_prefix, i)
            self.simulator.add_model(init_val)
            self.entities[eid] = i # mapping from EID to our model (i think)
            entities.append({'eid': eid, 'type': model})
        return entities


# perform simulation step
# returns time at which it wants to its next step
# recieves current simulation time and a dictionary with input values
    def step(self, time, inputs):
        # Get inputs
        deltas = {}
        for eid, attrs in inputs.items():
            for attr, values in attrs.items():
                model_idx = self.entities[eid]
                new_delta = sum(values.values())
                deltas[model_idx] = new_delta

        # Perform simulation step
        self.simulator.step(deltas)

        return time + 60 # Step size is 1 minute


# allows to get the values of the delta and val attributes of our models
    def get_data(self, outputs):
        models = self.simulator.models
        data = {}
        for eid, attrs in outputs.items():
            model_idx = self.entities[eid]
            data[eid] = {}
            for attr in attrs:
                if attr not in self.meta['models']['ExampleModel']['attrs']:
                    raise ValueError('Unknown output attribute: %s' % attr)

                # Get model.val or model.delta:
                data[eid][attr] = getattr(models[model_idx], attr)

        return data

def main():
    return mosaik_api.start_simulation(ExampleSim())# call start_simulation with simulator class


if __name__ == '__main__':
    main()*/
