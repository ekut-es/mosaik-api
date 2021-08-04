use log::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

use enerdag_core::battery::forecast::DisposableEnergyCalcToml;
use enerdag_core::config::BatteryConfigToml;
use enerdag_core::HouseholdBatteries;
use enerdag_crypto::hashable::Hashable;
use enerdag_marketplace::{energybalance::EnergyBalance, market::Market};
use enerdag_time::TimePeriod;
use mosaik_rust_api::{run_simulation, ApiHelpers, AttributeId, ConnectionDirection, MosaikApi};
use sled::Db;
use std::convert::TryInto;

///Read, if we get an address or not
#[derive(StructOpt, Debug)]
struct Opt {
    //The local addres mosaik connects to or none, if we connect to them
    #[structopt(short = "a", long)]
    addr: Option<String>,
}

type MW = f64;
type kWh = f64;
type Wh = f64;

fn mw_to_k_wh(power_m_w: MW, time_in_s: f64) -> kWh {
    let p_in_k_w = power_m_w * 1000.;
    let e_in_k_wh = p_in_k_w * (time_in_s / 3600.);
    return e_in_k_wh;
}

fn mw_to_wh(power_mw: MW, time_in_s: f64) -> Wh {
    mw_to_k_wh(power_mw, time_in_s) * 1000.
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_mw_to_kwh() {
        assert_eq!(mw_to_k_wh(0.1, 900.), 25.0);
    }

    #[test]
    fn test_mw_to_kwh_realistic() {
        assert_eq!(mw_to_k_wh(-0.000359375, 900.), -0.08984375);
    }

    #[test]
    fn test_mw_to_wh() {
        assert_eq!(mw_to_wh(0.1, 900.), 25000.0);
    }

    #[test]
    fn test_mw_to_wh_realistic() {
        assert_eq!(mw_to_wh(-0.000359375, 900.), -89.84375);
    }
}
fn k_wh_to_mw(energy: MW, time_in_s: i64) -> kWh {
    let hours = time_in_s as f64 / (3600.);
    let p_in_k_w = energy / hours;
    let p_in_w = p_in_k_w * 1000.;
    let p_in_mw = p_in_w / (1000. * 1000.);
    return p_in_mw;
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

/// A Simulation can have multiple Neighborhoods with different eids.
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
            self.params_array_into_vec(&model_params, &"battery_capacities".to_string(), |x| {
                x.as_u64().unwrap()
            });
        let initial_charges =
            self.params_array_into_vec(&model_params, &"initial_charges".to_string(), |x| {
                x.as_u64().unwrap()
            });
        let battery_configs =
            self.params_array_into_vec(&model_params, &"battery_configs".to_string(), |x| {
                serde_json::from_value(x.clone()).unwrap()
            });

        let battery_types =
            self.params_array_into_vec(&model_params, &"battery_types".to_string(), |x| {
                serde_json::from_value(x.clone()).unwrap()
            });

        let /*mut*/ model: Neighborhood = Neighborhood::initmodel(init_reading,
                                                                  battery_capacities,
                                                                  initial_charges, battery_types, battery_configs,
                                                                  self.step_size.clone());
        self.neighborhoods.push(model);

        self.data.push(vec![]); //Add list for simulation data

        serde_json::Value::Array(vec![])
    }

    fn get_model_value(&self, model_idx: u64, attr: &str) -> Option<Value> {
        self.neighborhoods
            .get(model_idx as usize)
            .and_then(|x| x.get_value(attr))
    }

    /// perform a simulation step and a auction of the marketplace for every neighborhood in
    /// the simulation
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
        param: &AttributeId,
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
    pub battery_type: HouseholdBatteries,
    pub battery_config: BatteryConfigToml,
    db: Db,
}

impl ModelHousehold {
    fn new(
        init_reading: f64,
        battery_config: BatteryConfigToml,
        battery_type: HouseholdBatteries,
        battery_capacity: u64,
        initial_charge: u64,
        time: enerdag_time::TimePeriod,
    ) -> Self {
        let db = test_utilities::setup_db();
        match battery_type {
            HouseholdBatteries::SmartBattery => {
                Self::setup_smart_battery(
                    &db,
                    &battery_config,
                    battery_capacity,
                    initial_charge as i64,
                    &time,
                );
            }
            _ => {
                warn!("Not implemented yet");
            }
        }

        ModelHousehold {
            energy_balance: init_reading,
            db,
            p_mw_load: 0.0,
            p_mw_pv: 0.0,
            battery_capacity,
            battery_type,
            battery_config,
        }
    }

    pub(crate) fn get_disposable_energy(&self, time: &enerdag_time::TimePeriod) -> i64 {
        use enerdag_core::contracts::HouseholdBatteries::SmartBattery;
        HouseholdBatteries::get_disposable_energy(&SmartBattery, &self.db, time)
    }

    pub fn get_power_load(&self, time: &enerdag_time::TimePeriod) -> i64 {
        todo!("Calculate powerload");
    }

    pub fn dispatch_methods_battery_type<F, T>(
        &self,
        time: &enerdag_time::TimePeriod,
        f_smart: F,
        f_simple: F,
        f_no: F,
    ) -> T
    where
        F: FnOnce(&Self, &enerdag_time::TimePeriod) -> T,
    {
        use enerdag_core::db::battery::get_battery_config;
        use enerdag_core::db::config::_get_battery_type;
        let bt = _get_battery_type(&self.db).unwrap();
        match bt {
            HouseholdBatteries::SmartBattery => f_smart(self, time),
            HouseholdBatteries::SimpleBattery => f_simple(self, time),
            HouseholdBatteries::NoBattery => f_no(self, time),
        }
    }

    pub fn perform_trade_round(
        &self,
        energy_balance: EnergyBalance,
        grid_power_load: i64,
        total_disposable_energy: i64,
    ) -> enerdag_utils::Result<EnergyBalance> {
        use test_utilities::perform_trading_round;
        perform_trading_round(
            &self.db,
            &energy_balance.period,
            energy_balance.energy_balance,
            grid_power_load,
            total_disposable_energy,
        )
    }

    fn setup_smart_battery(
        db: &Db,
        config: &BatteryConfigToml,
        capacity: u64,
        charge: i64,
        initial_period: &TimePeriod,
    ) {
        todo!("")
    }

    pub(crate) fn get_battery_charge(&self, time: &TimePeriod) -> EnergyBalance {
        use enerdag_core::db::battery::get_battery_charge;
        if let Ok(balance) = get_battery_charge(&self.db, time) {
            balance
        } else {
            panic!("No Battery Charge found!");
        }
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
    battery_configs: Vec<BatteryConfigToml>,
    battery_types: Vec<HouseholdBatteries>,
    step_size: i64,
}

/// A Neighborhood with Prosumers and Consumers.
impl Neighborhood {
    fn initmodel(
        init_reading: f64,
        battery_capacity: Vec<u64>,
        initial_charge: Vec<u64>,
        battery_types: Vec<HouseholdBatteries>,
        battery_configs: Vec<BatteryConfigToml>,
        step_size: i64,
    ) -> Neighborhood {
        Neighborhood {
            households: HashMap::new(),
            trades: 0,
            init_reading,
            total: 0,
            time: TimePeriod::last(),
            battery_capacity,
            battery_types,
            initial_charge,
            battery_configs,
            step_size,
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
                    "energy_balance" => Value::from(household.energy_balance),
                    "battery_charges" => Value::from(
                        household
                            .get_battery_charge(&self.time.previous())
                            .energy_balance,
                    ),
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
            print!("Create Next Household:{}", &household_id);
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
        let idx = self.households.len();
        ModelHousehold::new(
            self.init_reading.clone(),
            self.battery_configs[idx].clone(),
            self.battery_types[idx].clone(),
            self.battery_capacity[idx].clone(),
            self.initial_charge[idx].clone(),
            self.time.clone(),
        )
    }

    ///perform a normal simulation step.
    fn step(&mut self) {
        for (_, household) in self.households.iter_mut() {
            let energy_balance = household.p_mw_pv - household.p_mw_load;
            household.energy_balance = mw_to_wh(energy_balance, self.step_size as f64);
            debug!(
                "Converting {} MW to  {} Wh, stepsize: {}",
                energy_balance, household.energy_balance, self.step_size as f64
            );
        }
        let total_disposable_energy = self.calculate_total_disposable_energy();
        let grid_power_load = self.calculate_grid_power_load();
        let bids = self.perform_trading_rounds(total_disposable_energy, grid_power_load);

        self.trade_step(&bids);
        self.time = self.time.following();
    }

    fn calculate_grid_power_load(&self) -> i64 {
        self.aggregate_household(
            |_, x: &ModelHousehold| x.get_power_load(&self.time),
            |x, y| x + y,
        )
    }

    fn calculate_total_disposable_energy(&mut self) -> i64 {
        let disposable_energy: Vec<i64> = self
            .households
            .iter()
            .map(|(_, h)| h.get_disposable_energy(&self.time))
            .collect();
        let total_disposable_energy = disposable_energy.iter().sum();
        info!(
            "\n\tTotal Disposable Energy {}\t\n",
            total_disposable_energy
        );
        total_disposable_energy
    }

    fn aggregate_household<T, F1, F2>(&self, mapping: F1, agg: F2) -> T
    where
        F1: Fn(&String, &ModelHousehold) -> T,
        F2: Fn(T, T) -> T,
    {
        let m = self.households.iter().map(|x| mapping(x.0, x.1));
        let r: T = m.reduce(|x, y| agg(x, y)).unwrap();
        r
    }

    fn perform_trading_rounds(
        &mut self,
        total_disposable_energy: i64,
        grid_power_load: i64,
    ) -> Vec<([u8; 32], enerdag_marketplace::bid::Bid)> {
        let mut bids = Vec::new();

        #[cfg(feature = "random_prices")]
        use rand::{thread_rng, Rng};
        #[cfg(feature = "random_prices")]
        let mut rng = thread_rng();

        for (name, household) in &self.households {
            let energy_balance = EnergyBalance::new_with_period(
                (household.energy_balance) as i64,
                self.time.clone(),
            );

            let published_energy_balance = household
                .perform_trade_round(energy_balance, grid_power_load, total_disposable_energy)
                .unwrap();

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
        bids
    }

    ///perform a market auction with the models in households.
    fn trade_step(&mut self, bids: &Vec<([u8; 32], enerdag_marketplace::bid::Bid)>) {
        let mut market = Market::new_from_bytes(&bids, enerdag_currency::Currency::from_cents(7));
        market.trade();
        let total = market.get_total_leftover_energy();
        self.total = total.0 + total.1;
        println!("{:?}", market.get_trades());
        debug!("all the trades: {:?}", &self.trades);
        let trades = market.get_trades();
        self.trades = trades.len();
    }
}
