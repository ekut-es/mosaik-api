use log::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

use enerdag_crypto::hashable::Hashable;
use enerdag_marketplace::{energybalance::EnergyBalance, market::Market};
use mosaik_rust_api::{run_simulation, ApiHelpers, AttributeId, ConnectionDirection, MosaikApi};

///Read, if we get an address or not
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
    let simulator = MarketplaceSim::init_sim();
    //start build_connection in the library.
    if let Err(e) = run_simulation(address, simulator) {
        error!("{:?}", e);
    }
}

impl MosaikApi for MarketplaceSim {
    fn setup_done(&self) {
        info!("Setup done!")
        //todo!()
    }

    fn stop(&self) {
        info!("Simulation has stopped!")
        //todo!()
    }
}
pub struct MarketplaceSim {
    pub models: Vec<Model>,
    data: Vec<Vec<Vec<f64>>>,
    eid_prefix: String,
    step_size: i64,
    entities: Map<String, Value>,
}

//Implementation of the helpers defined in the library
impl ApiHelpers for MarketplaceSim {
    fn meta() -> Value {
        json!({
        "api_version": "2.2",
        "models":{
            "MarktplatzModel":{
                "public": true,
                "params": ["init_reading"],
                "attrs": ["p_mw_pv", "p_mw_load", "reading", "trades", "total"]
                }
            }
        })
    }

    fn set_eid_prefix(&mut self, eid_prefix: &str) {
        self.eid_prefix = eid_prefix.to_string();
    }

    fn set_step_size(&mut self, step_size: i64) {
        self.step_size = step_size;
    }

    fn get_eid_prefix(&self) -> &str {
        &self.eid_prefix
    }

    fn get_step_size(&self) -> i64 {
        self.step_size
    }

    fn get_mut_entities(&mut self) -> &mut Map<String, Value> {
        &mut self.entities
    }

    fn add_neighborhood(&mut self, model_params: Map<AttributeId, Value>) -> Value {
        if let Some(init_reading) = model_params.get("init_reading").and_then(|x| x.as_f64()) {
            let /*mut*/ model:Model = Model::initmodel(init_reading);
            self.models.push(model);
            self.data.push(vec![]); //Add list for simulation data
        }
        serde_json::Value::Array(vec![])
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        self.models
            .get(model_idx as usize)
            .and_then(|x| x.get_value(attr))
    }

    //perform a simulation step and a auction of the marketplace
    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>) {
        // Resete die Prosumption (vom vorhergehenden step) bevor wir irgendetwas updaten
        for model in self.models.iter_mut() {
            model.reset_prosumption();
        }

        // Update die Models
        for (attr_id, idx, deltax) in deltas.into_iter() {
            for (household_id, value) in deltax {
                self.models[idx as usize].update_model(
                    &attr_id,
                    household_id,
                    value.as_f64().unwrap_or_default(),
                );
            }
        }

        // iterate through the models and step each model
        for (i, model) in self.models.iter_mut().enumerate() {
            model.step();
            model.trade_step();
            self.data[i].push(model.get_all_reading());
        }
    }
}

impl MarketplaceSim {
    ///initialize the simulator
    pub fn init_sim() -> MarketplaceSim {
        println!("initiate marketplace simulation.");
        MarketplaceSim {
            eid_prefix: String::from("Model_"),
            step_size: 15 * 60,
            entities: Map::new(),
            models: vec![],
            data: vec![],
        }
    }
}

//The household model with three attributes.
#[derive(Debug, Default)]
pub struct ModelHousehold {
    pub p_mw_pv: f64,
    pub p_mw_load: f64,
    pub reading: f64,
}

impl ModelHousehold {
    fn new(init_reading: f64) -> Self {
        ModelHousehold {
            reading: init_reading,
            ..Default::default()
        }
    }

    fn reset_prosumption(&mut self) {
        self.p_mw_load = 0.0;
        self.p_mw_pv = 0.0;
    }
}

//The simulator model containing the household models and the attributes for the collector.
pub struct Model {
    households: HashMap<String, ModelHousehold>,
    /// An Vector of the trades at each simulationstep
    trades: usize,
    init_reading: f64,
    total: i64,
}

impl Model {
    fn initmodel(init_reading: f64) -> Model {
        Model {
            households: HashMap::new(),
            trades: 0,
            init_reading,
            total: 0,
        }
    }

    ///Function gets called from get_model() to give the model values.
    pub fn get_value(&self, attr: &str) -> Option<Value> {
        if attr == "trades" {
            match serde_json::to_value(self.trades) {
                Ok(value_trades) => Some(value_trades),
                Err(e) => {
                    error!("failed to make a value of the number of trades: {}", e);
                    None
                }
            }
        } else if attr == "total" {
            match serde_json::to_value(self.total) {
                Ok(value_total) => Some(value_total),
                Err(e) => {
                    error!("failed to make a value of the total number: {}", e);
                    None
                }
            }
        } else {
            let mut map = Map::new();

            for (household_name, household) in &self.households {
                let result = match attr {
                    "p_mw_pv" => Value::from(household.p_mw_pv),
                    "p_mw_load" => Value::from(household.p_mw_load),
                    "reading" => Value::from(household.reading),
                    x => {
                        error!("no known attr requested: {}", x);
                        return None;
                    }
                };
                map.insert(household_name.clone(), result);
            }

            Some(Value::Object(map))
        }
    }

    /// Returns all current readings (step should be finished).
    pub fn get_all_reading(&self) -> Vec<f64> {
        self.households.values().map(|x| x.reading).collect()
    }

    /// Update the models that get new values from the household simulation.
    pub fn update_model(&mut self, attr: &str, household_id: String, delta: f64) {
        log::debug!("{}", &household_id);
        match attr {
            "p_mw_pv" => {
                let mut household = self
                    .households
                    .entry(household_id)
                    .or_insert(ModelHousehold::new(self.init_reading));
                household.p_mw_pv = delta
            }
            "p_mw_load" => {
                let mut household = self
                    .households
                    .entry(household_id)
                    .or_insert(ModelHousehold::new(self.init_reading));
                household.p_mw_load = delta
            }
            x => {
                error!("unknown attr requested: {}", x);
            }
        };
        log::debug!("{:?}, {} {}", self.households, attr, delta);
    }

    pub fn reset_prosumption(&mut self) {
        for (_, household) in self.households.iter_mut() {
            household.reset_prosumption();
        }
    }

    ///perform a normal simulation step.
    fn step(&mut self) {
        for (_, household) in self.households.iter_mut() {
            household.reading += household.p_mw_pv - household.p_mw_load;
        }
    }

    ///perform a market auction with the models in households.
    fn trade_step(&mut self) {
        let mut bids = Vec::new();

        debug!("{:?}", &self.households);

        #[cfg(feature = "random_prices")]
        use rand::{thread_rng, Rng};
        #[cfg(feature = "random_prices")]
        let mut rng = thread_rng();

        for (name, household) in &self.households {
            let mut bid = enerdag_marketplace::bid::Bid {
                energy_balance: EnergyBalance::new(
                    ((household.p_mw_pv - household.p_mw_load) * 1_000_000.0) as i64,
                ),
                ..Default::default()
            };

            #[cfg(feature = "random_prices")]
            {
                bid.price_buy_max = rng.gen_range(25..32);
                bid.price_sell_min = rng.gen_range(8..15);
            }
            #[cfg(not(feature = "random_prices"))]
            {
                bid.price_buy_max = 30;
                bid.price_sell_min = 12;
            }
            bids.push((name.hash(), bid));
        }

        let mut market = Market::new_from_bytes(&bids);
        market.trade();
        let total = market.get_total_leftover_energy();
        self.total = total.0 + total.1;
        println!("{:?}", market.get_trades());
        debug!("all the trades: {:?}", &self.trades);
        let trades = market.get_trades();
        self.trades = trades.len();
    }
}
