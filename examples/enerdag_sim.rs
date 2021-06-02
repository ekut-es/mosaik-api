use log::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

use enerdag_core::db::add_disposable_energy;
use enerdag_core::HouseholdBatteries;
use enerdag_crypto::hashable::Hashable;
use enerdag_marketplace::{energybalance::EnergyBalance, market::Market};
use enerdag_time::TimePeriod;
use mosaik_rust_api::{run_simulation, ApiHelpers, AttributeId, ConnectionDirection, MosaikApi};
use sled::Db;

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
    let simulator = HouseholdBatterySim::init_sim();
    //start build_connection in the library.
    if let Err(e) = run_simulation(address, simulator) {
        error!("{:?}", e);
    }
}
pub struct HouseholdBatterySim {
    pub neighborhoods: Vec<Neighborhood>,
    data: Vec<Vec<Vec<f64>>>,
    eid_prefix: String,
    step_size: i64,
    entities: Map<String, Value>,
}

impl MosaikApi for HouseholdBatterySim {
    fn setup_done(&self) {
        info!("Setup done!")
        //todo!()
    }

    fn stop(&self) {
        info!("Simulation has stopped!")
        //todo!()
    }
}

impl ApiHelpers for HouseholdBatterySim {
    fn meta() -> Value {
        json!({
        "api_version": "2.2",
        "models":{
            "Neighborhood":{
                "public": true,
                "params": ["init_balance", "battery_capacities", "initial_charges"],
                "attrs": ["p_mw_pv", "p_mw_load", "energy_balance", "trades", "total",
                        "battery_charges"]
                },
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
        let init_reading = model_params
            .get("init_balance")
            .and_then(|x| x.as_f64())
            .unwrap();
        let battery_capacities =
            self.params_array_into_vec(&model_params, "battery_capacities", |x| {
                x.as_u64().unwrap()
            });
        let initial_charges =
            self.params_array_into_vec(&model_params, "initial_charges", |x| x.as_u64().unwrap());

        let /*mut*/ model: Neighborhood = Neighborhood::initmodel(init_reading, battery_capacities, initial_charges);
        self.neighborhoods.push(model);

        self.data.push(vec![]); //Add list for simulation data

        serde_json::Value::Array(vec![])
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        self.neighborhoods
            .get(model_idx as usize)
            .and_then(|x| x.get_value(attr))
    }

    //perform a simulation step and a auction of the marketplace
    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>) {
        // Resete die Prosumption (vom vorhergehenden step) bevor wir irgendetwas updaten
        for neighborhood in self.neighborhoods.iter_mut() {
            neighborhood.reset_prosumption();
        }

        // Update die Models
        for (attr_id, idx, deltax) in deltas.into_iter() {
            for (household_id, value) in deltax {
                self.neighborhoods[idx as usize].update_model(
                    &attr_id,
                    household_id,
                    value.as_f64().unwrap_or_default(),
                );
            }
        }

        // iterate through the models and step each model
        for (i, neighborhood) in self.neighborhoods.iter_mut().enumerate() {
            neighborhood.step();
            self.data[i].push(neighborhood.get_all_reading());
        }
    }
}

impl HouseholdBatterySim {
    ///initialize the simulator
    pub fn init_sim() -> HouseholdBatterySim {
        println!("initiate marketplace simulation.");
        HouseholdBatterySim {
            eid_prefix: String::from("Model_"),
            step_size: 15 * 60,
            entities: Map::new(),
            neighborhoods: vec![],
            data: vec![],
        }
    }
    fn params_array_into_vec<'a, B, F>(
        &self,
        model_params: &'a Map<AttributeId, Value>,
        param: &str,
        f: F,
    ) -> Vec<B>
    where
        F: FnMut(&'a Value) -> B,
    {
        model_params
            .get(param)
            .and_then(|x| x.as_array())
            .unwrap()
            .iter()
            .map(f)
            .collect()
    }
}

//The household model with three attributes.
#[derive(Debug)]
pub struct ModelHousehold {
    pub p_mw_pv: f64,
    pub p_mw_load: f64,
    pub energy_balance: f64,
    pub battery_capacity: u64,
    db: Db,
}

impl ModelHousehold {
    fn new(
        init_reading: f64,
        battery_capacity: u64,
        initial_charge: u64,
        time: enerdag_time::TimePeriod,
    ) -> Self {
        let db = Self::setup_db();
        Self::setup_smart_battery(&db, battery_capacity, initial_charge as i64, &time);

        ModelHousehold {
            energy_balance: init_reading,
            db,
            p_mw_load: 0.0,
            p_mw_pv: 0.0,
            battery_capacity,
        }
    }

    pub(crate) fn calculate_published_energy_balance(
        &self,
        energy_balance: &EnergyBalance,
        time: &enerdag_time::TimePeriod,
    ) -> EnergyBalance {
        enerdag_core::db::insert_energy_balance(&self.db, &energy_balance)
            .expect("Insert Energy Balance failed");
        let energy_balance_after_battery =
            enerdag_core::contracts::publishenergybalance::calculate_energy_balance(&self.db, time)
                .expect("Calculating Energy Balance failed");
        energy_balance_after_battery
    }
    pub(crate) fn get_disposable_energy(&self, time: &enerdag_time::TimePeriod) -> i64 {
        use enerdag_core::contracts::HouseholdBatteries::SmartBattery;
        HouseholdBatteries::get_disposable_energy(&SmartBattery, &self.db, time)
    }
    pub(crate) fn set_disposable_energy(
        &self,
        disposable_energy: i64,
        time: &enerdag_time::TimePeriod,
    ) {
        use enerdag_core::db::add_disposable_energy;
        add_disposable_energy(&self.db, time, disposable_energy)
            .expect("Setting Disposable Energy Failed");
    }

    fn setup_db() -> Db {
        sled::Config::default()
            .temporary(true)
            .open()
            .expect("unable to setup tmp database for db-txs tests")
    }

    fn setup_smart_battery(db: &Db, capacity: u64, charge: i64, initial_period: &TimePeriod) {
        use enerdag_core::db::config::{_set_battery_capacity, _set_battery_type};
        use enerdag_core::db::insert_battery_charge;

        _set_battery_type(&db, &HouseholdBatteries::SmartBattery)
            .expect("Could not Set Battery Type");

        _set_battery_capacity(&db, &capacity).expect("Could not set Battery Capacity");

        insert_battery_charge(
            &db,
            &EnergyBalance::new_with_period(charge, initial_period.previous()),
        )
        .expect("Could not set Charge");
    }

    pub(crate) fn get_battery_charge(&self, time: &TimePeriod) -> EnergyBalance {
        use enerdag_core::db::get_battery_charge;
        get_battery_charge(&self.db, time).unwrap()
    }

    fn reset_prosumption(&mut self) {
        self.p_mw_load = 0.0;
        self.p_mw_pv = 0.0;
    }
}

//The simulator model containing the household models and the attributes for the collector.
pub struct Neighborhood {
    households: HashMap<String, ModelHousehold>,
    /// An Vector of the trades at each simulationstep
    trades: usize,
    init_reading: f64,
    total: i64,
    time: TimePeriod,
    battery_capacity: Vec<u64>,
    initial_charge: Vec<u64>,
}

impl Neighborhood {
    fn initmodel(
        init_reading: f64,
        battery_capacity: Vec<u64>,
        initial_charge: Vec<u64>,
    ) -> Neighborhood {
        Neighborhood {
            households: HashMap::new(),
            trades: 0,
            init_reading,
            total: 0,
            time: TimePeriod::last(),
            battery_capacity,
            initial_charge,
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
                    "reading" => Value::from(household.energy_balance),
                    "battery_charges" => {
                        Value::from(household.get_battery_charge(&self.time).energy_balance)
                    }
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
        self.households.values().map(|x| x.energy_balance).collect()
    }

    /// Update the models that get new values from the household simulation.
    pub fn update_model(&mut self, attr: &str, household_id: String, delta: f64) {
        log::debug!("{}", &household_id);
        if !self.households.contains_key(&household_id) {
            let next_household = self.create_next_household();
            self.households.insert(household_id.clone(), next_household);
        }
        match attr {
            "p_mw_pv" => {
                let mut household = self.households.get_mut(&*household_id).unwrap();
                household.p_mw_pv = delta
            }
            "p_mw_load" => {
                let mut household = self.households.get_mut(&*household_id).unwrap();
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

    fn create_next_household(&self) -> ModelHousehold {
        ModelHousehold::new(
            self.init_reading.clone(),
            self.battery_capacity
                .get(self.households.len())
                .unwrap()
                .clone(),
            self.initial_charge
                .get(self.households.len())
                .unwrap()
                .clone(),
            self.time.clone(),
        )
    }

    ///perform a normal simulation step.
    fn step(&mut self) {
        for (_, household) in self.households.iter_mut() {
            household.energy_balance += household.p_mw_pv - household.p_mw_load;
        }
        self.calculate_disposable_energy();
        self.trade_step();
    }

    fn calculate_disposable_energy(&mut self) {
        let time = &self.time;
        let disposable_energy: Vec<i64> = self
            .households
            .iter()
            .map(|(_, h)| h.get_disposable_energy(time))
            .collect();
        let total_disposable_energy = disposable_energy.iter().sum();
        for (_, household) in self.households {
            household.set_disposable_energy(total_disposable_energy, time);
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
            let energy_balance = EnergyBalance::new_with_period(
                ((household.p_mw_pv - household.p_mw_load) * 1_000_000.0) as i64,
                self.time.clone(),
            );

            let published_energy_balance =
                household.calculate_published_energy_balance(&energy_balance, &self.time);

            let mut bid = enerdag_marketplace::bid::Bid {
                energy_balance: published_energy_balance,
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
