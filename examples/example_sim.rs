// ignore this file when building with cargo
#![allow(dead_code)]
#![cfg(not(test))]

use log::error;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::HashMap, todo, unimplemented};
use structopt::StructOpt;

use mosaik_rust_api::{
    default_api::{self, ApiHelpers},
    run_simulation,
    tcp::ConnectionDirection,
    types::{
        Attr, CreateResult, EntityId, InputData, Meta, ModelDescription, OutputData, OutputRequest,
        SimulatorType, Time,
    },
    MosaikApi,
};
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
#[derive(StructOpt, Debug)]
struct Opt {
    //The local addres mosaik connects to or none, if we connect to them
    #[structopt(short = "a", long)]
    addr: Option<String>,
}
pub fn main() /*-> Result<()>*/
{
    //get the address if there is one
    let opt = Opt::from_args();
    env_logger::init();

    let address = match opt.addr {
        //case if we connect us to mosaik
        Some(mosaik_addr) => ConnectionDirection::ConnectToAddress(
            mosaik_addr.parse().expect("Address is not parseable."),
        ),
        //case if mosaik connects to us
        None => {
            let addr = "127.0.0.1:3456";
            ConnectionDirection::ListenOnAddress(addr.parse().expect("Address is not parseable."))
        }
    };

    //initialize the simulator.
    let simulator = RExampleSim::new();
    //start build_connection in the library.
    if let Err(e) = run_simulation(address, simulator) {
        error!("Error running RExampleSim: {:?}", e);
    }
}

/// Rust implementation of the Python example model
#[derive(Debug, Serialize, Deserialize)]
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
    pub fn new() -> Self {
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
        self.models.get(index).unwrap().get_val() //self.models[index].get_val()
    }

    pub fn get_delta(&self, index: usize) -> f64 {
        self.models[index].get_delta()
    }

    pub fn set_delta(&mut self, index: usize, delta: f64) {
        self.models[index].set_delta(delta);
    }
}

pub struct RExampleSim {
    // The meta information in the original example is a constant
    meta: Meta,
    // init values of the original example
    eid_prefix: String,
    entities: Map<EntityId, Value>,
    time: Time,
    // some more values
    step_size: u64,
    time_resolution: f64,
    simulator: RSimulator,
}

impl RExampleSim {
    pub fn new() -> Self {
        Self {
            meta: Meta::new(
                SimulatorType::Hybrid,
                HashMap::from([("ExampleModel".to_string(), EXAMPLE_MODEL)]),
                None,
            ),
            eid_prefix: "Model_".to_string(),
            step_size: 1,
            time: 0,
            time_resolution: 1.0,
            entities: Map::new(), // Maps EIDs to model instances/entities
            simulator: RSimulator::new(),
        }
    }
}

impl ApiHelpers for RExampleSim {
    fn meta(&self) -> Meta {
        self.meta.clone()
    }

    fn set_eid_prefix(&mut self, eid_prefix: &str) {
        self.eid_prefix = eid_prefix.to_string();
    }

    fn get_eid_prefix(&self) -> &str {
        &self.eid_prefix
    }

    fn get_step_size(&self) -> u64 {
        self.step_size
    }

    fn set_step_size(&mut self, step_size: u64) {
        self.step_size = step_size;
    }

    fn get_mut_entities(&mut self) -> &mut Map<EntityId, Value> {
        todo!()
    }

    fn add_model(&mut self, model_params: Map<Attr, Value>) -> Option<Vec<CreateResult>> {
        let init_val = model_params.get("init_val").map(|v| v.as_f64().unwrap());
        self.simulator.models.push(RModel::new(init_val));
        None
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Result<Value, String> {
        unimplemented!("not implemented")
    }

    fn sim_step(&mut self, deltas: Vec<(String, u64, serde_json::Map<String, Value>)>) {
        unimplemented!("not implemented")
    }

    fn get_time_resolution(&self) -> f64 {
        self.time_resolution
    }

    fn set_time_resolution(&mut self, time_resolution: f64) {
        self.time_resolution = time_resolution;
    }

    fn set_time(&mut self, time: Time) {
        self.time = time;
    }

    fn get_time(&self) -> Time {
        self.time
    }
}

impl MosaikApi for RExampleSim {
    fn init(
        &mut self,
        _sid: String,
        time_resolution: f64,
        sim_params: Map<String, Value>,
    ) -> Result<Meta, String> {
        if time_resolution != 1.0f64 {
            return Err(format!(
                "ExampleSim only supports time_resolution=1., but {} was set.",
                time_resolution
            ));
        }
        default_api::default_init(self, time_resolution, sim_params)
    }

    fn create(
        &mut self,
        num: usize,
        model_name: String,
        _model_params: Map<Attr, Value>,
    ) -> Result<Vec<CreateResult>, String> {
        let next_eid = self.entities.len();
        let mut result: Vec<CreateResult> = Vec::new();

        for i in next_eid..(next_eid + num) {
            let model_instance = RModel::new(None);
            let eid = format!("{}_{}", self.eid_prefix, i);

            self.entities.insert(eid.clone(), model_instance);

            let dict = CreateResult::new(eid, model_name.clone());
            result.push(dict);
        }

        // entities must have length of num
        assert_eq!(result.len(), num); // TODO improve error handling
        Ok(result)
    }

    fn step(
        &mut self,
        time: Time,
        inputs: InputData,
        _max_advance: Time,
    ) -> Result<Option<Time>, String> {
        self.time = time;
        // FIXME this code is implemented as on https://mosaik.readthedocs.io/en/latest/tutorials/examplesim.html#step
        // but it seems to contain a bug. The delta is overridden by each loop before it's written to the model_instance.
        for (eid, model_instance) in &mut self.entities {
            if let Some(attrs) = inputs.get(eid) {
                let mut new_delta = 0.0;
                for value in attrs.values() {
                    new_delta = value.values().map(|v| v.as_f64().unwrap()).sum();
                }
                model_instance.delta = new_delta;
            }
            model_instance.step();
        }

        Ok(Some(time + 1)) // Step size is 1 second
    }

    fn get_data(&mut self, outputs: OutputRequest) -> Result<OutputData, String> {
        default_api::default_get_data(self, outputs)
    }

    fn setup_done(&self) -> Result<(), String> {
        // Nothing to do
        Ok(())
    }

    fn stop(&self) {
        // Nothing to do
    }
}
