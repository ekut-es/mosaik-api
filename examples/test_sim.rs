// ignore this file when building with cargo
#![allow(dead_code)]
#![cfg(not(test))]

use std::{collections::HashMap, todo};

use log::error;
use mosaik_rust_api::{
    types::{
        Attr, CreateResult, EntityId, InputData, Meta, ModelDescription, ModelDescriptionOptionals,
        OutputData, OutputRequest, SimulatorType,
    },
    ApiHelpers, DefaultMosaikApi, MosaikApi,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use structopt::StructOpt;

use mosaik_rust_api::{run_simulation, tcp::ConnectionDirection};
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
        error!("{:?}", e);
    }
}

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
    time: u64,
    time_resolution: f64,
    entities: HashMap<EntityId, RModel>,
    simulator: RSimulator,
}

impl ApiHelpers for RExampleSim {
    fn meta() -> Meta {
        let example_model = ModelDescription {
            public: true,
            params: vec!["init_val".to_string()],
            attrs: vec!["delta".to_string(), "val".to_string()],
            optionals: Some(ModelDescriptionOptionals {
                trigger: Some(vec!["delta".to_string()]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let meta = Meta {
            api_version: "3.0",
            type_: SimulatorType::TimeBased,
            models: {
                let mut m = HashMap::new();
                m.insert("ExampleModel".to_string(), example_model);
                m
            },
            ..Default::default()
        };
        meta
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

    fn get_mut_entities(&mut self) -> &mut Map<EntityId, Value> {
        todo!()
    }

    fn add_model(&mut self, model_params: Map<Attr, Value>) -> Option<Vec<CreateResult>> {
        let init_val = model_params.get("init_val").map(|v| v.as_f64().unwrap());
        self.simulator.models.push(RModel::new(init_val));
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

impl DefaultMosaikApi for RExampleSim {}

impl MosaikApi for RExampleSim {
    fn init(&mut self, sid: String, time_resolution: f64, sim_params: Map<String, Value>) -> Meta {
        DefaultMosaikApi::init(self, sid, time_resolution, sim_params)
    }
    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<Attr, Value>,
    ) -> Vec<CreateResult> {
        let next_eid = self.entities.len();
        let mut result: Vec<CreateResult> = Vec::new();

        for i in next_eid..(next_eid + num) {
            let model_instance = RModel::new(None);
            let eid = format!("{}_{}", self.eid_prefix, i);

            self.entities.insert(eid.clone(), model_instance);

            let dict = CreateResult {
                eid,
                r#type: model_name.clone(),
                optionals: None,
            };
            result.push(dict);
        }

        // entities must have length of num
        assert_eq!(result.len(), num); // TODO improve error handling
        result
    }

    fn step(&mut self, time: usize, inputs: InputData, max_advance: usize) -> Option<usize> {
        /* FIXME: this is not yet compatible with new json typing
        self.time = time as u64;

        for (eid, model_instance) in &mut self.entities {
            if let Some(attrs) = inputs.get(eid) {
                for (_, values) in attrs {
                    if let Some(new_delta) = values
                        .as_object()
                        .map(|v| v.values().map(|val| val.as_f64().unwrap_or(0.0)).sum())
                    {
                        model_instance.delta = new_delta
                    }
                }
            }

            model_instance.step();
        }*/

        return Some(time + 1); // Step size is 1 second
    }

    fn get_data(&mut self, outputs: OutputRequest) -> OutputData {
        DefaultMosaikApi::get_data(self, outputs)
    }

    fn setup_done(&self) {
        // Nothing to do
    }

    fn stop(&self) {
        // Nothing to do
    }
}

impl RExampleSim {
    pub fn new() -> Self {
        Self {
            eid_prefix: "Model_".to_string(),
            step_size: 1,
            time: 0,
            time_resolution: 1.0,
            entities: HashMap::new(),
            simulator: RSimulator::new(),
        }
    }
}
