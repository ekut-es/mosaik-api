//mod lib;
//use lib::mosaik_api;
//mod simple_simulator;
use std::collections::HashMap;

fn meta(){
    let META = json::parse(r#"
    {"models":{
        "ExampleModel":{
            "public": True,
            "params": ["init_val"],
            "attrs": ["val", "delta"]
            }
        }
    }"#).unwrap();
    return META;
}

pub struct ExampleSim{
    simulator: String, //simple_Simulator.simple_simulator()
    eid_prefix: String,
    entities: Vec!(),
}

pub trait mosaik_api{

}

///implementation of the trait in mosaik_api.rs
impl mosaik_api for ExampleSim{
    fn _init_();

    fn init(&mut self, sid: String, eid_prefix: Option<String>){
        match eid_prefix {
            Some(eid_prefix) => {
                self.eid_prefix = eid_prefix;
            }
            None() => {}
        }
        return self.META; 
    }
    
    fn create(&self, num: usize, model: Model, init_val: usize){
        let mut entities = vec::new();
        let next_eid = self.entities.len();

        for i in next_eid..(next_eid + num){
            let mut eid = to_string(self.eid_prefix, i); //eid = '%s%d' % (self.eid_prefix, i)
            //self.simulator::add_model(init_val);
            self.entities[eid] = i;
            entities.push(/* {'eid': eid, 'type': model} */)
        }
        return entities;
    }

    /*fn setup_done(&self){

    }*/

    fn step(&self, time: usize, inputs: HashMap<>){
        let mut deltas = vec::new();
        let mut new_delta = 0;
        for (eid, attrs) in inputs.iter(){
            for (attr, value) in attrs.iter(){
                let mut model_idx = self.entities[eid];
                new_delta += value;
                deltas[model_idx] = new_delta;
            }
        }
        //self.simulator::step(deltas);
        
        return time + 60;
    }

    fn get_data(&self, Output: HashMap<>){
        let mut data = vec::new();
        for (eid, attrs) in Output.iter(){
            let mut model_idx = self.enitites[eid];
            data[eid] = vec::new();
            for attr in attrs{
                /*if attr not in self.meta['models']['ExampleModel']['attrs']:
                    raise ValueError('Unknown output attribute: %s' % attr)

                # Get model.val or model.delta:
                data[eid][attr] = getattr(models[model_idx], attr)*/
            }
        }
        return data;

    }

    fn stop(){
        break;
    }
}






///Current Python code needs to get converted to rust for testing purposes.



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