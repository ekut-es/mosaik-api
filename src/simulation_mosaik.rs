use std::collections::HashMap;
use jsonrpc_core::to_string;

use crate::{Model, Object, mosaik_api};//, RunSimulator};

fn meta()-> json::JsonValue {
    let meta = json::parse(r#"
    {
    "api_version": "2.2",
    "models":{
        "ExampleModel":{
            "public": True,
            "params": ["init_val"],
            "attrs": ["val", "delta"]
            }
        }
    }"#).unwrap();
    return meta;
}

type Value = usize;
type Entity = String;
pub struct ExampleSim{
    simulator: simple_simulator::RunSimulator,  //simple_Simulator.simulator()
    eid_prefix: String,                         //HashMap<String, Object>,
    entities: Vec<Entity>,
    meta: json::JsonValue,
}

fn init_sim() -> ExampleSim{
    ExampleSim{
        simulator: RunSimulator::init_simulator(), //"simple_simulator_aufruf", //simple_Simulator.simulator()
        eid_prefix: String::from("model_"),
        entities: vec![], //HashMap
        meta: meta(),
    }
}

///implementation of the trait in mosaik_api.rs
impl mosaik_api for ExampleSim{
    
    fn init(&mut self, sid: String, sim_params: Option<HashMap<String, Object>>) -> json::JsonValue{
        match sim_params {
            Some(sim_params) => {
                self.eid_prefix = todo!();//ExampleSim.eid_prefix;
            }
            None => {}
        }
        meta()
        //return self.META; 
    }
    
    fn create<Entity>(&self, num: usize, model: String, init_val: usize) -> Vec<Entity>{ //, model_params: HashMap<String, Vec<Value>>
        let mut out_entities: HashMap<String, String> = HashMap::new();
        let next_eid = self.entities.len();

        for i in next_eid..(next_eid + num){
            let mut eid = format!(self.eid_prefix, i); //eid = '%s%d' % (self.eid_prefix, i)
            //self.simulator::add_model(init_val);
            self.entities[eid] = i; //create a mapping from the entity ID to our model
            //out_entities.push( ("eid": eid, "type": model) );
        }
        return out_entities;
    }

    fn step<Value: std::ops::AddAssign>(&self, time: usize, inputs: HashMap<String, HashMap<String, Vec<Value>>>) -> usize{
        let mut deltas = vec![];
        let mut new_delta: Value;
        for (eid, attrs) in inputs.iter(){
            let mut i = 0;
            for (attr, values) in attrs.iter(){ //values ist eine weitere HashMap<String, Value>.
                let mut model_idx = todo!();                    //self.entities[*eid];
                new_delta += values[i];                           //new_delta = sum(values.values())
                deltas[model_idx] = new_delta;
            }
        }
        //self.simulator::step(deltas);
        time = time + 60;
        return time;
    }

    fn get_data<Value>(&self, output: HashMap<String, Vec<String>>) -> HashMap<String, HashMap<String, Vec<Value>>>{
        //models = self.simulator.models
        let mut data: HashMap<String, HashMap<String, Vec<Value>>> = HashMap::new();//vec![];
        for (eid, attrs) in output.iter(){
            let mut model_idx = self.entities;//[eid];
            //data[eid] = vec![];
            for attr in attrs{  
                /*if attr not in self.meta['models']['ExampleModel']['attrs']:
                    raise ValueError('Unknown output attribute: %s' % attr)

                # Get model.val or model.delta:
                data[eid][attr] = getattr(models[model_idx], attr)*/ //data.insert(String::from(eid), attr)
            }
        }
        return data;

    }

    fn stop(){
    }

    fn setup_done(&self) {
        println!("Setup is done.");
    }
}

pub fn run(){
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