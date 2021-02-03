use enerdag_marketplace::{bid::Bid, energybalance::EnergyBalance, market::Market, trade::Trade};
use log::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use mosaik_rust_api::{run_simulation, API_Helpers, AttributeId, MosaikAPI};

pub fn main() /*-> Result<()>*/
{
    env_logger::init();
    //The local addres mosaik connects to.
    let addr = "127.0.0.1:3456"; //wenn wir uns eigenständig verbinden wollen -> addr als option. accept_loop angepasst werden!!!
    let simulator = MarketplaceSim::init_sim();
    run_simulation(addr, simulator);
}

impl MosaikAPI for MarketplaceSim {
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
    entities: Map<String, Value>,
}

impl API_Helpers for MarketplaceSim {
    fn meta() -> Value {
        let meta = json!({
        "api_version": "2.2",
        "models":{
            "ExampleModel":{
                "public": true,
                "params": ["init_reading"],
                "attrs": ["p_mw_pv", "p_mw_load", "reading"]
                }
            }
        });
        return meta;
    }

    fn set_eid_prefix(&mut self, eid_prefix: &str) {
        self.eid_prefix = eid_prefix.to_string();
    }

    fn get_eid_prefix(&self) -> &str {
        &self.eid_prefix
    }

    fn get_mut_entities(&mut self) -> &mut Map<String, Value> {
        &mut self.entities
    }

    fn add_model(&mut self, model_params: Map<AttributeId, Value>) {
        if let Some(init_reading) = model_params.get("init_reading") {
            match init_reading.as_f64() {
                Some(init_reading) => {
                    let /*mut*/ model:Model = Model::initmodel(init_reading);
                    self.models.push(model);
                    self.data.push(vec![]); //Add list for simulation data
                }
                None => {}
            }
        }
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        self.models
            .get(model_idx as usize)
            .and_then(|x| x.get_value(attr))
    }

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

        // Stepe durch die Model und handle anschließend
        for (i, model) in self.models.iter_mut().enumerate() {
            model.step();
            model.trade_step();
            self.data[i].push(model.get_all_reading());
        }
    }
}

impl MarketplaceSim {
    pub fn init_sim() -> MarketplaceSim {
        println!("initiate marketplace simulation.");
        MarketplaceSim {
            eid_prefix: String::from("Model_"),
            entities: Map::new(),
            models: vec![],
            data: vec![],
        }
    }
}

pub struct Model {
    households: HashMap<String, ModelHousehold>,
    /// An Vector of the trades at each simulationstep
    trades: Vec<Vec<Trade>>,
    init_reading: f64,
}

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

impl Model {
    ///Function gets called from get_model() to give the model values.
    pub fn get_value(&self, attr: &str) -> Option<Value> {
        // if attr = "trades" {

        // } else {

        // }

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

    ///Function gets called from get_model() to give the model values.
    pub fn get_household_value(&self, household_id: &str, attr: &str) -> Option<Value> {
        let household = self.households.get(household_id)?;
        let result = match attr {
            "p_mw_pv" => Value::from(household.p_mw_pv),
            "p_mw_load" => Value::from(household.p_mw_load),
            "reading" => Value::from(household.reading),
            x => {
                error!("no known attr requested: {}", x);
                return None;
            }
        };
        Some(result)
    }

    /// Returns all current readings (step should be finished)
    pub fn get_all_reading(&self) -> Vec<f64> {
        self.households.values().map(|x| x.reading).collect()
    }

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
                error!("no known attr requested: {}", x);
            }
        };
        log::debug!("{:?}, {} {}", self.households, attr, delta);
    }

    pub fn reset_prosumption(&mut self) {
        for (_, household) in self.households.iter_mut() {
            household.reset_prosumption();
        }
    }

    fn initmodel(init_reading: f64) -> Model {
        Model {
            households: HashMap::new(),
            trades: Vec::new(),
            init_reading: init_reading,
        }
    }

    fn step(&mut self) {
        for (_, household) in self.households.iter_mut() {
            household.reading += household.p_mw_pv - household.p_mw_load;
        }
    }

    fn trade_step(&mut self) {
        let mut bids = Vec::new();

        debug!("{:?}", &self.households);

        for (name, household) in &self.households {
            let mut address_bytes = [0u8; enerdag_crypto::signature::ADDRESSBYTESLENGTH];
            let name_bytes = name.as_bytes();
            let length = if name_bytes.len() < enerdag_crypto::signature::ADDRESSBYTESLENGTH {
                name_bytes.len()
            } else {
                enerdag_crypto::signature::ADDRESSBYTESLENGTH
            };
            address_bytes.copy_from_slice(&name_bytes[..length]);

            let mut bid = Bid::default();
            bid.energy_balance = EnergyBalance::new(
                ((household.p_mw_pv - household.p_mw_load) * 1_000_000.0) as i64,
            );
            bid.price_buy_max = 30;
            bid.price_sell_min = 12;
            bids.push((address_bytes, bid));
        }

        let mut market = Market::new_from_bytes(&bids);
        market.trade();
        debug!("{:?}", market.get_trades());
        self.trades.push(market.get_trades().clone());
    }
}

//For a local run, without mosaik in the background
/*
pub fn run() {
    let mut sim: Simulator = Simulator::init_simulator(); //need an instance of Simulator, just like in init_model()
                                                          //sim = Simulator()

    for i in 0..3 {
        sim.add_model(Some(0.0));
    }

    sim.step(None); //values = 1.0 , 1.0
    sim.step(Some(vec![(0, 8.0), (1, 13.0), (2, 19.0)]));
    sim.step(Some(vec![(0, 23.0), (1, 42.0), (2, 68.0)])); //values = 24.0 , 43.0

    info!("Simulation finished with data:");

    for (i, inst) in sim.data.iter().enumerate() {
        info!("{}: {:?}", i, inst);
    }
}*/
/*
#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::Model;

    #[test]
    fn test_get_value() {
        let model = Model { val: 0.0, p_mw_load: 1.0 };

        let val_val = Some(Value::from(model.val));
        let val_p_mw_load = Some(Value::from(model.p_mw_load));

        assert_eq!(val_val, model.get_value("val"));
        assert_eq!(val_p_mw_load, model.get_value("p_mw_load"));
        assert_eq!(None, model.get_value("different"));
    }
}
*/
