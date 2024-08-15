use log::error;
use mosaik_rust_api::tcp::ConnectionDirection;
use mosaik_rust_api::types::{
    Attr, CreateResult, EntityId, InputData, Meta, ModelDescription, OutputData, OutputRequest,
    SimId, SimulatorType, Time,
};
use mosaik_rust_api::{run_simulation, MosaikApi};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

const AGENT_MODEL: ModelDescription = ModelDescription {
    public: true,
    params: &[],
    attrs: &["val_in", "delta"],
    trigger: None,
    any_inputs: None,
    persistent: None,
};

// A simple demo controller. Inspired by the python tutorial
pub struct Controller {
    agents: Vec<String>,
    data: HashMap<EntityId, HashMap<Attr, Value>>,
    time: Time,
    meta: Meta,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            agents: vec![],
            data: HashMap::new(),
            time: 0,
            meta: Meta::new(
                SimulatorType::EventBased,
                HashMap::from([("Agent".to_string(), AGENT_MODEL)]),
                None,
            ),
        }
    }
}

impl MosaikApi for Controller {
    fn init(
        &mut self,
        _sid: SimId,
        _time_resolution: f64,
        _sim_params: Map<String, Value>,
    ) -> Result<Meta, String> {
        Ok(self.meta.clone())
    }

    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<String, Value>,
    ) -> Result<Vec<CreateResult>, String> {
        let n_agents = self.agents.len();
        let mut entities: Vec<CreateResult> = vec![];
        for i in n_agents..(n_agents + num) {
            let eid = format!("Agent_{}", i);
            self.agents.push(eid.clone());
            entities.push(CreateResult {
                eid,
                model_type: model_name.clone(),
                rel: None,
                children: None,
                extra_info: None,
            });
        }
        Ok(entities)
    }

    fn setup_done(&self) -> Result<(), String> {
        Ok(())
    }

    fn stop(&self) {}

    fn step(
        &mut self,
        time: Time,
        inputs: InputData,
        max_advance: Time,
    ) -> Result<Option<Time>, String> {
        self.time = time;
        let mut data = HashMap::new();

        for (agent_eid, attrs) in inputs {
            if let Some(delta_dict) = attrs.get("delta") {
                if !delta_dict.is_empty() {
                    data.insert(
                        agent_eid.clone(),
                        HashMap::from([(
                            "delta".to_string(),
                            delta_dict.values().next().unwrap().clone(),
                        )]),
                    );
                    continue;
                }
            }

            if let Some(values_dict) = attrs.get("val_in") {
                if values_dict.len() != 1 {
                    panic!(
                        "Only one ingoing connection allowed per agent, but \"{}\" has {}.",
                        agent_eid,
                        values_dict.len()
                    );
                }

                let value = values_dict.values().next().unwrap();

                let delta = if value.as_f64().unwrap() >= 3.0 {
                    -1
                } else if value.as_f64().unwrap() <= -3.0 {
                    1
                } else {
                    continue;
                };

                data.insert(
                    agent_eid.clone(),
                    HashMap::from([("delta".to_string(), json!(delta))]),
                );
            }
        }

        self.data = data;

        Ok(None)
    }

    fn get_data(&self, outputs: OutputRequest) -> Result<OutputData, String> {
        let mut data: HashMap<String, HashMap<String, Value>> = HashMap::new();

        for (agent_eid, attrs) in outputs {
            for attr in attrs {
                if attr != "delta" {
                    return Err(format!("Unknown output attribute \"{}\"", attr));
                }

                if let Some(agent_data) = self.data.get(&agent_eid) {
                    data.entry("time".to_string())
                        .or_insert_with(HashMap::new)
                        .insert("time".to_string(), json!(self.time));

                    data.entry(agent_eid.clone())
                        .or_insert_with(HashMap::new)
                        .insert(
                            attr.clone(),
                            agent_data.get(&attr).unwrap_or(&json!(0.0)).clone(),
                        );
                }
            }
        }

        Ok(OutputData {
            requests: data,
            time: Some(self.time),
        })
    }
}

#[derive(StructOpt, Debug)]
struct Opt {
    //The local addres mosaik connects to or none, if we connect to them
    #[structopt(short = "a", long)]
    addr: Option<String>,
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
    let controller = Controller::default();
    if let Err(e) = run_simulation(address, controller) {
        error!("Error running controller: {:?}", e);
    }
}
