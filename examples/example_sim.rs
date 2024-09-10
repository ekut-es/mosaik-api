#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Taken from official [tutorial](https://mosaik.readthedocs.io/en/3.3.3/tutorials/examplesim.html)
//! This includes the `example_model.py` and the `simulator_mosaik.py` of the Python tutorial.

use clap::Parser;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::HashMap, sync::LazyLock};

use mosaik_api::{
    run_simulation,
    tcp::ConnectionDirection,
    types::{
        Attr, CreateResult, EntityId, InputData, Meta, ModelDescription, OutputData, OutputRequest,
        SimulatorType, Time,
    },
    MosaikApi,
};

static META: LazyLock<Meta> = LazyLock::new(|| {
    Meta::new(
        SimulatorType::Hybrid,
        HashMap::from([(
            "ExampleModel".to_string(),
            ModelDescription {
                public: true,
                params: &["init_val"],
                attrs: &["delta", "val"],
                trigger: Some(&["delta"]),
                any_inputs: None,
                persistent: None,
            },
        )]),
        Some(vec!["print_something".to_string()]),
    )
});

/// Rust implementation of the Python [example_model.py](https://mosaik.readthedocs.io/en/3.3.3/tutorials/examplesim.html#the-model)
#[derive(Debug, Serialize, Deserialize)]
pub struct Model {
    val: f64,
    delta: f64,
}

impl Model {
    pub fn new(init_val: f64) -> Self {
        Self {
            val: init_val,
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
    ) -> Result<&'static Meta, String> {
        if (time_resolution - 1.0f64).abs() >= f64::EPSILON {
            return Err(format!(
                "ExampleSim only supports time_resolution=1., but {time_resolution} was set."
            ));
        }
        self.time_resolution = time_resolution;

        for (key, value) in sim_params {
            match (key.as_str(), value) {
                ("eid_prefix", Value::String(eid_prefix)) => {
                    info!("EID prefix is set to: {}", eid_prefix);
                    self.eid_prefix = eid_prefix;
                }
                ("step_size", Value::Number(step_size)) => {
                    if let Some(step_size) = step_size.as_u64() {
                        info!("Step size is set to: {}", step_size);
                        self.step_size = step_size;
                    } else {
                        let e = format!("Step size is not a valid number: {step_size:?}");
                        error!("Error in default_init: {}", e);
                        return Err(e);
                    }
                }
                _ => {
                    warn!("Init: Unknown parameter: {}", key);
                }
            }
        }

        Ok(&META)
    }

    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<Attr, Value>,
    ) -> Result<Vec<CreateResult>, String> {
        let next_eid = self.entities.len();
        let mut result: Vec<CreateResult> = Vec::new();

        let init_val = model_params
            .get("init_val")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);

        for i in next_eid..(next_eid + num) {
            let model_instance =
                serde_json::to_value(Model::new(init_val)).map_err(|e| e.to_string())?;
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

        // NOTE this code is implemented as on https://mosaik.readthedocs.io/en/latest/tutorials/examplesim.html#step
        // but it seems to contain a bug. The delta is overridden by each loop before it's written to the model_instance.

        // Check for new delta and do step for each model instance:
        for (eid, model_value) in &mut self.entities {
            let mut model_instance: Model =
                Model::deserialize(model_value.clone()).map_err(|e| e.to_string())?;
            if let Some(attrs) = inputs.get(eid) {
                let mut new_delta = 0.0;
                for value in attrs.values() {
                    new_delta = value.values().map(|v| v.as_f64().unwrap()).sum();
                }
                model_instance.delta = new_delta;
            }
            model_instance.step();
            *model_value = serde_json::to_value(&model_instance).map_err(|e| e.to_string())?;
        }

        Ok(Some(time + 1)) // Step size is 1 second
    }

    fn get_data(&self, outputs: OutputRequest) -> Result<OutputData, String> {
        let mut data: HashMap<EntityId, HashMap<Attr, Value>> = HashMap::new();

        for (eid, attrs) in &outputs {
            let instance = self.entities.get(eid);
            let instance: Option<Model> =
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

    fn extra_method(
        &mut self,
        method: &str,
        args: &Vec<Value>,
        kwargs: &Map<String, Value>,
    ) -> Result<Value, String> {
        match method {
            "print_something" => {
                self.print_something(args, kwargs);
                Ok(Value::String("Printed something!".to_string()))
            }
            _ => Err(format!(
                "Method '{method}' not found with args: {args:?} and kwargs: {kwargs:?}"
            )),
        }
    }
}

impl ExampleSim {
    pub fn print_something(&self, args: &Vec<Value>, kwargs: &Map<String, Value>) {
        println!(
            "This is a print statement from an extra method! With args: {args:?} and kwargs: {kwargs:?}",
        );
    }
}

#[derive(Parser, Debug)]
struct Args {
    /// The local address mosaik connects to, or none if we connect to them
    #[clap(short, long)]
    addr: Option<String>,
}

pub fn main() {
    //get the address if there is one
    let args = Args::parse();
    env_logger::init();

    // TODO: Should this be part of `run_simulation`?
    let address = match args.addr {
        //case if we connect us to mosaik
        Some(addr) => {
            ConnectionDirection::ConnectToAddress(addr.parse().expect("Address is not parseable."))
        }
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
        panic!("Error running ExampleSim: {e:?}");
    }
}
