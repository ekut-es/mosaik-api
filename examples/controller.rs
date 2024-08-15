use log::error;
use mosaik_rust_api::tcp::ConnectionDirection;
use mosaik_rust_api::types::{
    CreateResult, InputData, Meta, ModelDescription, OutputData, OutputRequest, SimId,
    SimulatorType, Time,
};
use mosaik_rust_api::{run_simulation, MosaikApi};
use serde_json::{Map, Value};
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
    data: Map<String, Value>,
    time: i64,
    meta: Meta,
}

impl Default for Controller {
    fn default() -> Self {
        Controller {
            agents: vec![],
            data: Map::new(),
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
        /* =====================================================================================
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
        Ok(Some(0))*/
        unimplemented!()
    }

    fn get_data(&self, outputs: OutputRequest) -> Result<OutputData, String> {
        // self.data.clone()
        unimplemented!()
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
