use log::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

use enerdag_crypto::hashable::Hashable;
use enerdag_marketplace::{energybalance::EnergyBalance, market::Market};
use mosaik_rust_api::{
    run_simulation, tcp::ConnectionDirection, ApiHelpers, AttributeId, MosaikApi,
};
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
    /*fn get_mut_params<T: ApiHelpers>(&mut self) -> &mut T {
        &mut self
    }*/

    // copied from default implementation
    fn init(
        &mut self,
        sid: mosaik_rust_api::Sid,
        time_resolution: f64,
        sim_params: Map<String, Value>,
    ) -> mosaik_rust_api::Meta {
        if time_resolution != 1.0 {
            info!("time_resolution must be 1.0"); // TODO this seems not true
            self.set_time_resolution(1.0f64);
        } else {
            self.set_time_resolution(time_resolution);
        }

        for (key, value) in sim_params {
            match (key.as_str(), value) {
                /*("time_resolution", Value::Number(time_resolution)) => {
                    self.set_time_resolution(time_resolution.as_f64().unwrap_or(1.0f64));
                }*/
                ("eid_prefix", Value::String(eid_prefix)) => {
                    self.set_eid_prefix(&eid_prefix);
                }
                ("step_size", Value::Number(step_size)) => {
                    self.set_step_size(step_size.as_i64().unwrap());
                }
                _ => {
                    info!("Unknown parameter: {}", key);
                }
            }
        }

        Self::meta()
    }

    // copied from default implementation
    fn create(
        &mut self,
        num: usize,
        model_name: String,
        model_params: Map<AttributeId, Value>,
    ) -> Vec<Map<String, Value>> {
        let mut out_vector = Vec::new();
        let next_eid = self.get_mut_entities().len();
        for i in next_eid..(next_eid + num) {
            let mut out_entities: Map<String, Value> = Map::new();
            let eid = format!("{}{}", self.get_eid_prefix(), i);
            let children = self.add_model(model_params.clone());
            self.get_mut_entities().insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
            out_entities.insert(String::from("eid"), json!(eid));
            out_entities.insert(String::from("type"), Value::String(model_name.clone()));
            if let Some(children) = children {
                out_entities.insert(String::from("children"), children);
            }
            debug!("{:?}", out_entities);
            out_vector.push(out_entities);
        }

        debug!("the created model: {:?}", out_vector);
        out_vector
    }

    fn setup_done(&self) {
        info!("Setup done!")
        //todo!()
    }

    // copied from default implementation
    fn step(
        &mut self,
        time: usize,
        inputs: HashMap<mosaik_rust_api::Eid, Map<AttributeId, Value>>,
        max_advance: usize,
    ) -> Option<usize> {
        trace!("the inputs in step: {:?}", inputs);
        let mut deltas: Vec<(String, u64, Map<String, Value>)> = Vec::new();
        for (eid, attrs) in inputs.into_iter() {
            for (attr, attr_values) in attrs.into_iter() {
                let model_idx = match self.get_mut_entities().get(&eid) {
                    Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                    _ => panic!(
                        "No correct model eid available. Input: {:?}, Entities: {:?}",
                        eid,
                        self.get_mut_entities()
                    ),
                };
                if let Value::Object(values) = attr_values {
                    deltas.push((attr, model_idx, values));
                    debug!("the deltas for sim step: {:?}", deltas);
                };
            }
        }
        self.sim_step(deltas);

        Some(time + (self.get_step_size() as usize))
    }

    // copied from default implementation
    fn get_data(
        &mut self,
        outputs: HashMap<mosaik_rust_api::Eid, Vec<AttributeId>>,
    ) -> Map<mosaik_rust_api::Eid, Value> {
        let mut data: Map<String, Value> = Map::new();
        for (eid, attrs) in outputs.into_iter() {
            let model_idx = match self.get_mut_entities().get(&eid) {
                Some(eid) if eid.is_u64() => eid.as_u64().unwrap(), //unwrap safe, because we check for u64
                _ => panic!("No correct model eid available."),
            };
            let mut attribute_values = Map::new();
            for attr in attrs.into_iter() {
                //Get the values of the model
                if let Some(value) = self.get_model_value(model_idx, &attr) {
                    attribute_values.insert(attr, value);
                } else {
                    error!(
                        "No attribute called {} available in model {}",
                        &attr, model_idx
                    );
                }
            }
            data.insert(eid, Value::from(attribute_values));
        }
        data
        // TODO https://mosaik.readthedocs.io/en/latest/mosaik-api/low-level.html#get-data
        // api-v3 needs optional 'time' entry in output map for event-based and hybrid Simulators
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
    time_resolution: f64,
}

//Implementation of the helpers defined in the library
impl ApiHelpers for MarketplaceSim {
    fn meta() -> Value {
        json!({
        "api_version": "3.0",
        "type": "time-based",
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

    fn add_model(&mut self, model_params: Map<AttributeId, Value>) -> Option<Value> {
        if let Some(init_reading) = model_params.get("init_reading").and_then(|x| x.as_f64()) {
            let /*mut*/ model:Model = Model::initmodel(init_reading);
            self.models.push(model);
            self.data.push(vec![]); //Add list for simulation data
        }
        None
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

    fn get_time_resolution(&self) -> f64 {
        self.time_resolution
    }

    fn set_time_resolution(&mut self, time_resolution: f64) {
        self.time_resolution = time_resolution;
    }
}

impl MarketplaceSim {
    ///initialize the simulator
    pub fn init_sim() -> MarketplaceSim {
        info!("initiate marketplace simulation.");
        MarketplaceSim {
            eid_prefix: String::from("Model_"),
            step_size: 15 * 60,
            entities: Map::new(),
            models: vec![],
            data: vec![],
            time_resolution: 1.0f64,
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
                let household = self
                    .households
                    .entry(household_id)
                    .or_insert(ModelHousehold::new(self.init_reading));
                household.p_mw_pv = delta
            }
            "p_mw_load" => {
                let household = self
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

        let mut market = Market::new_from_bytes(&bids, enerdag_currency::Currency::from_cents(7));
        market.trade();
        let total = market.get_total_leftover_energy();
        self.total = total.0 + total.1;
        info!("{:?}", market.get_trades());
        debug!("all the trades: {:?}", &self.trades);
        let trades = market.get_trades();
        self.trades = trades.len();
    }
}
