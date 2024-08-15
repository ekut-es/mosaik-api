//! Taken from official [tutorial](https://mosaik.readthedocs.io/en/3.3.3/tutorials/examplesim.html)
//! This includes the example_model.py and the simulator_mosaik.py of the Python tutorial.
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

use mosaik_rust_api::{
    run_simulation,
    tcp::ConnectionDirection,
    types::{
        Attr, CreateResult, EntityId, InputData, Meta, ModelDescription, OutputData, OutputRequest,
        SimulatorType, Time,
    },
    MosaikApi,
};

/// The model description used in the simulators META.
const EXAMPLE_MODEL: ModelDescription = ModelDescription {
    public: true,
    params: &["init_val"],
    attrs: &["delta", "val"],
    trigger: Some(&["delta"]),
    any_inputs: None,
    persistent: None,
};

/// Rust implementation of the Python [example_model.py](https://mosaik.readthedocs.io/en/3.3.3/tutorials/examplesim.html#the-model)
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

/// Rust implementation of the Python [simulator_mosaik.py](https://mosaik.readthedocs.io/en/3.3.3/tutorials/examplesim.html#the-simulator-class)
pub struct ExampleSim {
    // The meta information in the original example is a constant
    meta: Meta,
    // init values of the original example
    eid_prefix: String,
    entities: Map<EntityId, Value>,
    time: Time,
    // some more values
    step_size: u64,
    time_resolution: f64,
}

impl Default for ExampleSim {
    // This is like the __init__ in the original example
    fn default() -> Self {
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
        }
    }
}

impl MosaikApi for ExampleSim {
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
        self.time_resolution = time_resolution;

        for (key, value) in sim_params.into_iter() {
            match (key.as_str(), value) {
                ("eid_prefix", Value::String(eid_prefix)) => {
                    self.eid_prefix = eid_prefix;
                }
                ("step_size", Value::Number(step_size)) => {
                    if let Some(step_size) = step_size.as_u64() {
                        self.step_size = step_size;
                    } else {
                        let e = format!("Step size is not a valid number: {:?}", step_size);
                        error!("Error in default_init: {}", e);
                        return Err(e);
                    }
                }
                _ => {
                    info!("Init: Unknown parameter: {}", key);
                }
            }
        }

        Ok(self.meta.clone())
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
            let model_instance =
                serde_json::to_value(RModel::new(None)).map_err(|e| e.to_string())?;
            let eid = format!("{}{}", self.eid_prefix, i);

            self.entities.insert(eid.clone(), model_instance);

            let dict = CreateResult::new(eid, model_name.clone());
            result.push(dict);
        }

        // entities must have length of num
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

        // Check for new delta and do step for each model instance:
        for (eid, model_instance) in self.entities.iter_mut() {
            let mut r_model: RModel =
                RModel::deserialize(model_instance.clone()).map_err(|e| e.to_string())?;
            if let Some(attrs) = inputs.get(eid) {
                let mut new_delta = 0.0;
                for value in attrs.values() {
                    new_delta = value.values().map(|v| v.as_f64().unwrap()).sum();
                }
                r_model.delta = new_delta;
            }
            r_model.step();
            *model_instance = serde_json::to_value(&r_model).map_err(|e| e.to_string())?;
        }

        Ok(Some(time + 1)) // Step size is 1 second
    }

    fn get_data(&self, outputs: OutputRequest) -> Result<OutputData, String> {
        let mut data: HashMap<EntityId, HashMap<Attr, Value>> = HashMap::new();

        for (eid, attrs) in outputs.iter() {
            let instance = self.entities.get(eid);
            let instance: Option<RModel> =
                instance.and_then(|i| serde_json::from_value(i.clone()).ok());

            if let Some(instance) = instance {
                let mut values: HashMap<String, Value> = HashMap::new();

                for attr in attrs {
                    if attr == "val" {
                        values.insert(attr.clone(), instance.get_val().into());
                    } else if attr == "delta" {
                        values.insert(attr.clone(), instance.get_delta().into());
                    }
                }

                data.insert(eid.clone(), values);
            }
        }
        Ok(OutputData {
            requests: data,
            time: None,
        })
    }

    fn setup_done(&self) -> Result<(), String> {
        // Nothing to do
        Ok(())
    }

    fn stop(&self) {
        // Nothing to do
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    //The local addres mosaik connects to or none, if we connect to them
    #[structopt(short = "a", long)]
    addr: Option<String>,
    eid_prefix: Option<String>,
}

pub fn main() {
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
    let simulator = ExampleSim::default();
    //start build_connection in the library.
    if let Err(e) = run_simulation(address, simulator) {
        error!("Error running RExampleSim: {:?}", e);
    }
}
