use std::collections::HashMap;

use log::error;
use mosaik_rust_api::{
    run_simulation,
    tcp::ConnectionDirection,
    types::{
        Attr, CreateResult, InputData, Meta, ModelDescription, OutputData, OutputRequest, SimId,
        SimulatorType, Time,
    },
    MosaikApi,
};
use serde_json::{Map, Value};
use structopt::StructOpt;

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
    println!("opt: {:?}", opt);

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
    let simulator = Collector::new();
    //start build_connection in the library.
    if let Err(e) = run_simulation(address, simulator) {
        error!("Error running RExampleSim: {:?}", e);
    }
}

struct Collector {
    eid: Option<String>,
    data: HashMap<String, HashMap<String, HashMap<u64, f64>>>,
    meta: Meta,
}

const MONITOR_MODEL: ModelDescription = ModelDescription {
    public: true,
    params: &[],
    attrs: &[],
    trigger: None,
    any_inputs: Some(true),
    persistent: None,
};

impl Collector {
    fn new() -> Self {
        Collector {
            eid: None,
            data: HashMap::new(),
            meta: Meta::new(
                SimulatorType::EventBased,
                HashMap::from([("Monitor".to_string(), MONITOR_MODEL)]),
                None,
            ),
        }
    }
}

impl MosaikApi for Collector {
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
        _model_params: Map<Attr, Value>,
    ) -> Result<Vec<CreateResult>, String> {
        if num > 1 || self.eid.is_some() {
            return Err("Can only create one instance of Monitor.".into());
        }

        self.eid = Some("Monitor".to_string());
        Ok(vec![CreateResult::new(
            self.eid.clone().unwrap(),
            model_name,
        )])
    }

    fn setup_done(&self) -> Result<(), String> {
        Ok(())
    }

    fn step(
        &mut self,
        time: Time,
        inputs: InputData,
        _max_advance: Time,
    ) -> Result<Option<Time>, String> {
        if let Some(data) = inputs.get(self.eid.as_ref().unwrap()) {
            for (attr, values) in data {
                for (src, value) in values {
                    self.data
                        .entry(src.clone())
                        .or_default()
                        .entry(attr.clone())
                        .or_default()
                        .insert(time, value.as_f64().unwrap_or_default());
                }
            }
        }
        Ok(None)
    }

    fn stop(&self) {
        println!("Collected data:");

        let mut sims: Vec<_> = self.data.iter().collect();
        sims.sort_by_key(|&(sim, _)| sim);

        for (sim, sim_data) in sims {
            println!("- {}:", sim);

            let mut attrs: Vec<_> = sim_data.iter().collect();
            attrs.sort_by_key(|&(attr, _)| attr);

            for (attr, values) in attrs {
                println!("  - {}: {:?}", attr, values);
            }
        }
    }

    fn get_data(&self, _outputs: OutputRequest) -> Result<OutputData, String> {
        unimplemented!()
    }
}
