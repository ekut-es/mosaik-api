use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::{
    simple_simulator::{self, RunSimulator, Simulator},
    Model, MosaikAPI,
};

pub fn meta() -> serde_json::Value {
    let mut meta = json!({
    "api_version": "2.2",
    "models":{
        "ExampleModel":{
            "public": true,
            "params": ["init_val"],
            "attrs": ["val", "delta"]
            }
        }
    });
    return meta;
}

type Entity = String;
pub struct ExampleSim {
    simulator: simple_simulator::Simulator,
    eid_prefix: String,
    entities: Map<String, Value>,
    meta: serde_json::Value,
}

fn init_sim() -> ExampleSim {
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

    fn create<Entity>(
        &self,
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
                        self.entities.insert(eid, Value::from(i)); //create a mapping from the entity ID to our model
                        out_entities.insert(String::from("eid"), Value::from(eid));
                        out_entities.insert("type".to_string(), Value::from(model));
                    }
                }
            }
            None => {}
        }
        return out_entities;
    }

    fn step(&self, time: usize, inputs: HashMap<String, Map<String, Value>>) -> usize {
        let mut deltas: Vec<(Option<u64>, Option<f64>)> = Vec::new();
        let mut new_delta: Value;
        for (eid, attrs) in inputs.iter() {
            for (attr, attr_values) in attrs.iter() {
                let mut model_idx = self.entities.get(eid);
                new_delta = attr_values.values().sum(); //values() ist ein iterator der die werte von ihren keys nimmt
                deltas.push((model_idx.as_u64(), new_delta.as_f64())); //data conversion from value to usize and f64
            }
        }
        Simulator::step(&mut self.simulator, Some(deltas)); //wie bekomm ich das option weg
        time = time + 60;
        return time;
    }

    fn get_data(&self, output: HashMap<String, Vec<String>>) -> Map<String, Value> {
        let mut meta = meta();
        let mut models = self.simulator.models;
        let mut data: Map<String, Value> = Map::new();
        for (eid, attrs) in output.iter() {
            let mut model_idx = self.entities.get(eid);
            let mut values = Map::new();
            data.insert(*eid, Value::from(values));
            for attr in attrs.iter() {
                assert_eq!(meta["models"]["ExampleModel"]["attrs"], json!(attr));

                /*if attr not in self.meta['models']['ExampleModel']['attrs']:
                raise ValueError('Unknown output attribute: %s' % attr)*/

                //Get model.val or model.delta:
                data.insert(*eid, models.get(model_idx)); //data[eid][attr] = getattr(models[model_idx], attr)
            }
        }
        return data;
    }

    /*
    ) -> HashMap<String, Map<String, Value>> {
        let mut models = self.simulator.models;
        let mut data: HashMap<String, Map<String, Value>> = HashMap::new();
        for (eid, attrs) in output.iter() {
            let mut model_idx = self.entities.get(eid);
            data.insert(*eid, Value::from(vec![]));
            for attr in attrs {
                /*if attr not in self.meta['models']['ExampleModel']['attrs']:
                    raise ValueError('Unknown output attribute: %s' % attr)

                # Get model.val or model.delta:
                data[eid][attr] = getattr(models[model_idx], attr)*/ //data.insert(String::from(eid), attr)
            }
        }
        return data;
    }*/

    fn stop() {}

    fn setup_done(&self) {
        println!("Setup is done.");
    }
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
