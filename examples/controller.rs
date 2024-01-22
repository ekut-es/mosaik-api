use std::collections::HashMap;

use log::error;
use mosaik_rust_api::{run_simulation, ApiHelpers, ConnectionDirection, MosaikApi, API_VERSION};
use serde_json::{json, Map, Value};
use structopt::StructOpt;

// A simple demo controller. Inspired by the python tutorial
pub struct Controller {
    agents: Vec<String>,
    data: Map<String, Value>,
    time: i64,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            agents: vec![],
            data: Map::new(),
            time: 0,
        }
    }
}

impl ApiHelpers for Controller {
    fn meta() -> serde_json::Value {
        todo!()
    }

    fn set_eid_prefix(&mut self, eid_prefix: &str) {
        todo!()
    }

    fn set_step_size(&mut self, step_size: i64) {
        todo!()
    }

    fn get_eid_prefix(&self) -> &str {
        todo!()
    }

    fn get_step_size(&self) -> i64 {
        todo!()
    }

    fn get_mut_entities(&mut self) -> &mut Map<String, Value> {
        todo!()
    }

    fn add_model(
        &mut self,
        model_params: Map<mosaik_rust_api::AttributeId, Value>,
    ) -> Option<Value> {
        todo!()
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        todo!()
    }

    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>) {
        todo!()
    }
}

impl MosaikApi for Controller {
    fn init(&mut self, _: String, _: Option<Map<String, Value>>) -> Value {
        json!({
            "api_version": API_VERSION,
            "type": "event-based",
            "models": {
                "Agent": {
                    "public": true,
                    "params": [],
                    "attrs": ["val_in", "delta"],
                },
            },
        })
    }

    fn create(
        &mut self,
        num: usize,
        model: Value,
        model_params: Option<Map<String, Value>>,
    ) -> Vec<Map<String, Value>> {
        let n_agents = self.agents.len();
        let mut entities: Vec<Map<String, Value>> = vec![];
        for i in n_agents..(n_agents + num) {
            let eid = format!("Agent_{}", i);
            self.agents.push(eid.clone());
            let mut map = Map::new();
            map.insert("eid".to_owned(), json!(eid.clone()));
            map.insert("type".to_owned(), model.clone());
            entities.push(map);
        }
        entities
    }

    fn setup_done(&self) {}

    fn stop(&self) {}

    fn step(
        &mut self,
        time: usize,
        inputs: std::collections::HashMap<
            mosaik_rust_api::Eid,
            Map<mosaik_rust_api::AttributeId, Value>,
        >,
        // max_advance: usize,
    ) -> usize {
        /* self.time = time
        data = {}
        for agent_eid, attrs in inputs.items():
            delta_dict = attrs.get('delta', {})
            if len(delta_dict) > 0:
                data[agent_eid] = {'delta': list(delta_dict.values())[0]}
                continue

            values_dict = attrs.get('val_in', {})
            if len(values_dict) != 1:
                raise RuntimeError('Only one ingoing connection allowed per '
                                   'agent, but "%s" has %i.'
                                   % (agent_eid, len(values_dict)))
            value = list(values_dict.values())[0]

            if value >= 3:
                delta = -1
            elif value <= -3:
                delta = 1
            else:
                continue

            data[agent_eid] = {'delta': delta}

        self.data = data

        return None*/
        self.time = time as i64;
        let mut data = Map::new();
        for (agent_eid, attrs) in inputs {
            let delta_dict = attrs.get("delta").and_then(|x| x.as_object());
            if let Some(delta_dict) = delta_dict {
                if delta_dict.len() > 0 {
                    data.insert(
                        agent_eid,
                        json!({"delta": delta_dict.values().next().unwrap()}),
                    );
                    continue;
                }
            }
            let mut values_dict = attrs.get("val_in").and_then(|x| x.as_object());
            if let Some(values_dict) = values_dict {
                if values_dict.len() != 1 {
                    panic!(
                        "Only one ingoing connection allowed per agent, but \"{}\" has {}.",
                        agent_eid,
                        values_dict.len()
                    );
                }
                let value: f64 = values_dict.values().next().unwrap().as_f64().unwrap();
                if value >= 3.0 {
                    data.insert(agent_eid, json!({"delta": -1.0}));
                } else if value <= -3.0 {
                    data.insert(agent_eid, json!({"delta": 1.0}));
                } else {
                    continue;
                }
            }
        }
        self.data = data;
        0
    }

    fn get_data(
        &mut self,
        outputs: HashMap<String, Vec<mosaik_rust_api::AttributeId>>,
    ) -> Map<mosaik_rust_api::Eid, Value> {
        self.data.clone()
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
    let mut controller = Controller::default();
    if let Err(e) = run_simulation(address, controller) {
        error!("{:?}", e);
    }
}
