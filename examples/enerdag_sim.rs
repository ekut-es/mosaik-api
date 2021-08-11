use log::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use structopt::StructOpt;

use enerdag_core::battery::forecast::DisposableEnergyCalcToml;
use enerdag_core::battery::smart_battery::wh_to_w;
use enerdag_core::config::BatteryConfigToml;
use enerdag_core::HouseholdBatteries;
use enerdag_crypto::hashable::Hashable;
use enerdag_marketplace::{energybalance::EnergyBalance, market::Market};
use enerdag_time::TimePeriod;
use mosaik_rust_api::{
    run_simulation, ApiHelpers, AttributeId, ConnectionDirection, Eid, Model, MosaikApi,
};
use sled::Db;

///Read, if we get an address or not
#[derive(StructOpt, Debug)]
struct Opt {
    //The local addres mosaik connects to or none, if we connect to them
    #[structopt(short = "a", long)]
    addr: Option<String>,
}

type MW = f64;
#[allow(non_camel_case_types)]
type kWh = f64;
type Wh = f64;

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

const MOSAIK_PARAM_HOUSEHOLD_DESCRIPTION: &str = "household_descriptions";
const MOSAIK_PARAM_START_TIME: &str = "start_time";

/// A Simulation can have multiple [Neighborhoods](Neighborhood) that communicate with MOSAIK.
/// The Neighborhoods then contain an enerdag system.
pub struct HouseholdBatterySim {
    pub neighborhood: Option<Neighborhood>,

    eid_prefix: String,
    step_size: i64,
    entities: Map<String, Value>,
}

impl MosaikApi for HouseholdBatterySim {
    ///Create *num* instances of *model* using the provided *model_params*.
    /// *panics!* if more than one neighborhood is created.
    fn create(
        &mut self,
        num: usize,
        model: Model,
        model_params: Option<Map<AttributeId, Value>>,
    ) -> Vec<Map<String, Value>> {
        if num > 1 || self.neighborhood.is_some() {
            todo!("Create Support for more  than one Neighborhood");
        }
        let mut out_entities: Map<String, Value>;
        let mut out_vector = Vec::with_capacity(1);
        let next_eid = self.get_mut_entities().len();
        if let Some(model_params) = model_params {
            for i in next_eid..(next_eid + num) {
                out_entities = Map::new();

                let children = self.add_neighborhood(model_params.clone());
                let nhdb = self.neighborhood.as_ref().unwrap();
                let eid = nhdb.eid.clone();
                self.get_mut_entities().insert(eid.clone(), Value::from(i)); //create a mapping from the entity ID to our model
                out_entities.insert(String::from("eid"), json!(eid));
                out_entities.insert(String::from("type"), model.clone());
                out_entities.insert(String::from("children"), children);

                out_vector.push(out_entities);
            }
        }
        debug!("the created model: {:?}", out_vector);
        out_vector
    }

    fn get_data(&mut self, outputs: HashMap<Eid, Vec<AttributeId>>) -> Map<Eid, Value> {
        let mut data: Map<String, Value> = Map::new();

        if let Some(nbhd) = &mut self.neighborhood {
            nbhd.add_output_values(&outputs, &mut data);
        }

        data
    }

    fn setup_done(&self) {
        info!("Setup done!")
        //todo!()
    }

    fn step(&mut self, time: usize, inputs: HashMap<Eid, Map<AttributeId, Value>>) -> usize {
        // println!("Inputs; {:?}", inputs);

        if let Some(nbhd) = &mut self.neighborhood {
            nbhd.step(time, inputs);
        }

        time + self.step_size as usize
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
                "params": [MOSAIK_PARAM_HOUSEHOLD_DESCRIPTION, MOSAIK_PARAM_START_TIME],
                "attrs": ["trades", "total","total_disposable_energy", "grid_power_load"
                        ]
                },
                "Consumer": {
                    "public": false,
                    "params": [],
                    "attrs": ["p_mw_load", "energy_balance", "published_energy_balance", "trades", "battery_charge", "total_disposable_energy"]
                },
                "Prosumer": {
                    "public": false,
                    "params": [],
                    "attrs": ["p_mw_load",  "energy_balance", "published_energy_balance", "p_mw_pv", "battery_charge", "trades",
                    "disposable_energy", ]
                },
                "PV": {
                    "public": false,
                    "params": [],
                    "attrs": ["p_mw_pv", "trades"]
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
        let household_configs = self.params_to_household_config(&model_params);

        let start_time = if !model_params.contains_key(MOSAIK_PARAM_START_TIME) {
            warn!("No Start Time in Parameters, using Fallback to actual time.");
            TimePeriod::last()
        } else {
            serde_json::from_value(model_params.get(MOSAIK_PARAM_START_TIME).unwrap().clone())
                .unwrap()
        };

        let /*mut*/ model: Neighborhood = Neighborhood::initmodel(self.get_next_neighborhood_eid(), start_time,0., household_configs, self.step_size);

        let mosaik_children = model.households_as_mosaik_children();

        self.neighborhood = Some(model);

        mosaik_children
    }

    fn get_model_value(&self, _model_idx: u64, _attr: &str) -> Option<Value> {
        panic!("Not implemented for this instance")
    }

    /// perform a simulation step and a auction of the marketplace for every neighborhood in
    /// the simulation
    fn sim_step(&mut self, _deltas: Vec<(String, u64, Map<String, Value>)>) {
        panic!("Not implemented in this instance");
    }
}

impl HouseholdBatterySim {
    ///initialize the simulator
    pub fn init_sim() -> HouseholdBatterySim {
        println!("initiate marketplace simulation.");
        HouseholdBatterySim {
            eid_prefix: String::from("Model_"),
            step_size: 5 * 60,
            entities: Map::new(),
            neighborhood: None,
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

    fn params_to_household_config(
        &self,
        model_params: &Map<AttributeId, Value>,
    ) -> Vec<HouseholdDescription> {
        self.params_array_into_vec(
            model_params,
            &MOSAIK_PARAM_HOUSEHOLD_DESCRIPTION.to_string(),
            |x| {
                serde_json::from_value(x.clone())
                    .expect("Could not parse household description from value")
            },
        )
    }
    /// Returns the EID for the next neighborhood
    fn get_next_neighborhood_eid(&self) -> String {
        return "Neighborhood0".to_string();
    }
}

//The simulator model containing the household models and the attributes for the collector.
#[derive(Debug)]
pub struct Neighborhood {
    /// Entity ID of this Neighborhood
    eid: String,
    households: HashMap<String, ModelHousehold>,
    /// An Vector of the trades at each simulationstep
    trades: usize,
    init_reading: f64,
    total: i64,
    time: TimePeriod,
    total_disposable_energy: i64,
    step_size: i64,
    grid_power_load: i64,
}

/// A Neighborhood with Prosumers and Consumers.
impl Neighborhood {
    fn initmodel(
        eid: String,
        time: TimePeriod,
        init_reading: f64,
        descriptions: Vec<HouseholdDescription>,
        step_size: i64,
    ) -> Neighborhood {
        //assert_eq!(step_size, 300, "One Simulation step per TradePeriod");

        let households = Self::create_households(descriptions, &time);
        Neighborhood {
            eid,
            households,
            trades: 0,
            init_reading,
            total: 0,
            time: TimePeriod::last(),
            total_disposable_energy: 0,
            grid_power_load: 0,
            step_size,
        }
    }

    /// For each output requested by MOSAIK, this method adds an entry to the `output_data` map.
    pub(crate) fn add_output_values(
        &self,
        requested_outputs: &HashMap<Eid, Vec<AttributeId>>,
        output_data: &mut Map<String, Value>,
    ) {
        // The step method goes to the next period, so there is only data for the previous.
        let last_period = self.time.previous();

        for (eid, attrs) in requested_outputs.iter() {
            let mut requested_values: HashMap<String, Value> = HashMap::with_capacity(attrs.len());

            for attr in attrs {
                let value = if eid.eq(&self.eid) {
                    self.get_neighborhood_attr(attr)
                } else {
                    if let Some(household) = self.households.get(eid) {
                        household.get_value(attr, &last_period)
                    } else {
                        panic!("Requested Unknown EID {}", eid);
                    }
                };
                requested_values.insert(attr.clone(), value);
            }

            output_data.insert(eid.clone(), serde_json::to_value(requested_values).unwrap());
        }
    }
    /// Returns a MOSAIK compatible JSON representation of the [households](ModelHousehold).
    fn households_as_mosaik_children(&self) -> Value {
        let mut child_descriptions: Vec<HashMap<String, String>> =
            Vec::with_capacity(self.households.len());
        for (eid, household) in self.households.iter() {
            let mut hash_map = HashMap::with_capacity(2);
            hash_map.insert("eid".to_string(), eid.clone());
            hash_map.insert("type".to_string(), household.household_type.clone());
            child_descriptions.push(hash_map);
        }

        serde_json::to_value(child_descriptions).unwrap()
    }

    /// Returns  values for *Mosaik attrs* of the Mosaik *Neighborhood* object.
    fn get_neighborhood_attr(&self, attr: &str) -> Value {
        match attr {
            "trades" => serde_json::to_value(self.trades).unwrap(),
            "total" => serde_json::to_value(self.total).unwrap(),
            "total_disposable_energy" => {
                serde_json::to_value(self.total_disposable_energy).unwrap()
            }
            "grid_power_load" => serde_json::to_value(self.grid_power_load).unwrap(),
            _ => {
                panic!("Unknown Attribute requested for Neighborhood: {}", attr)
            }
        }
    }

    /// Resets power generation / usage data for every household.
    pub(crate) fn reset_prosumption(&mut self) {
        for (_, household) in self.households.iter_mut() {
            household.reset_prosumption();
        }
    }

    fn create_households(
        descriptions: Vec<HouseholdDescription>,
        time: &TimePeriod,
    ) -> HashMap<String, ModelHousehold> {
        let mut households: HashMap<String, ModelHousehold> =
            HashMap::with_capacity(descriptions.len());
        let mut types: HashMap<String, u32> = HashMap::new();

        let descriptions = descriptions;
        for household in descriptions.into_iter() {
            let hh_type = &household.household_type;
            let num_type = *types.get(hh_type).unwrap_or(&0);

            let eid = format!("{}_{}", hh_type, num_type);
            types.insert(hh_type.clone(), num_type + 1);

            households.insert(
                eid.to_ascii_lowercase(),
                ModelHousehold::new_from_description(time.clone(), household),
            );
        }
        households
    }

    ///perform a normal simulation step.
    fn step(&mut self, _time: usize, inputs: HashMap<Eid, Map<AttributeId, Value>>) {
        self.reset_prosumption();
        self.update_households(inputs);

        self.calculate_household_energy_balances();

        let total_disposable_energy = self.calculate_total_disposable_energy();
        let grid_power_load = self.calculate_grid_power_load();
        let bids = self.perform_trading_rounds(total_disposable_energy, grid_power_load);

        self.trade_step(&bids);
        self.time = self.time.following();
    }

    fn update_households(&mut self, inputs: HashMap<Eid, Map<AttributeId, Value>>) {
        for (eid, map) in inputs.into_iter() {
            if let Some(household) = self.households.get_mut(&*eid) {
                household.update_inputs(map);
            } else {
                panic!("Could not find household {}!", eid);
            }
        }
    }

    fn calculate_household_energy_balances(&mut self) {
        for (_, household) in self.households.iter_mut() {
            let energy_balance = household.p_mw_pv - household.p_mw_load;
            let energy_balance_wh = mw_to_wh(energy_balance, self.step_size as f64);
            debug!(
                "Converting {} MW to  {} Wh, stepsize: {}",
                energy_balance, energy_balance_wh, self.step_size as f64
            );
            household.set_energy_balance(EnergyBalance::new_with_period(
                energy_balance_wh as i64,
                self.time.clone(),
            ))
        }
    }

    fn calculate_grid_power_load(&mut self) -> i64 {
        self.grid_power_load = self.aggregate_household(
            |_, x: &ModelHousehold| x.get_power_load(&self.time),
            |x, y| x + y,
        );
        self.grid_power_load
    }

    fn calculate_total_disposable_energy(&mut self) -> i64 {
        let disposable_energies: Arc<Mutex<Vec<i64>>> =
            Arc::new(Mutex::new(Vec::with_capacity(self.households.len())));

        use crossbeam::thread;
        /// Calculate in parallel with scoped threads.
        /// scoped threads avoid the need for static lifetimes.
        thread::scope(|scope| {
            for (_, household) in &mut self.households {
                let time = self.time;
                let disposable_energies = Arc::clone(&disposable_energies);
                scope.spawn(move |_| {
                    let disposable_energy = household.get_disposable_energy(&time);
                    let mut energies = disposable_energies.lock().unwrap();
                    energies.push(disposable_energy);
                });
            }
        })
        .expect("Error when calculating disposable energy.");

        let disposable_energy = Arc::try_unwrap(disposable_energies)
            .unwrap()
            .into_inner()
            .unwrap();
        let total_disposable_energy = disposable_energy.iter().sum();
        info!(
            "\n\tTotal Disposable Energy {}\t\n",
            total_disposable_energy
        );
        self.total_disposable_energy = total_disposable_energy;
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
        let bids: Arc<Mutex<Vec<([u8; 32], enerdag_marketplace::bid::Bid)>>> =
            Arc::new(Mutex::new(Vec::new()));

        #[cfg(feature = "random_prices")]
        use rand::{thread_rng, Rng};
        #[cfg(feature = "random_prices")]
        let mut rng = thread_rng();

        /// Use crossbeam scoped threads instead of "vanilla" rust ones, because otherwise the compiler
        /// would require a static lifetime of households, which is not necessary for the logic of
        ///the program.  
        use crossbeam::thread;
        thread::scope(|scope| {
            for (name, household) in &mut self.households {
                let bids = Arc::clone(&bids);
                let time = self.time.clone();
                scope.spawn(move |_| {
                    let energy_balance =
                        EnergyBalance::new_with_period((household.energy_balance) as i64, time);

                    let published_energy_balance = household.perform_trade_round(
                        energy_balance,
                        grid_power_load,
                        total_disposable_energy,
                    );

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
                    let mut bids = bids.lock().unwrap();
                    bids.push((name.hash(), bid));
                });
            }
        })
        .unwrap();

        let mutex = Arc::try_unwrap(bids).expect("Could not move bids out of Arc.");
        let b: Vec<([u8; 32], enerdag_marketplace::bid::Bid)> =
            mutex.into_inner().expect("Could not get bids from mutex");

        b
    }

    ///perform a market auction with the models in households.
    fn trade_step(&mut self, bids: &Vec<([u8; 32], enerdag_marketplace::bid::Bid)>) {
        let mut market = Market::new_from_bytes(&bids, enerdag_currency::Currency::from_cents(7));
        market.trade();
        let total = market.get_total_leftover_energy();
        self.total = total.0 + total.1;
        debug!("all the trades: {:?}", &self.trades);
        let trades = market.get_trades();
        self.trades = trades.len();
    }
}

/// Used to Dispatch Functions
type DispatchFunc<T> = fn(&ModelHousehold, &enerdag_time::TimePeriod) -> T;
use serde_derive::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Descriptor of a household to simulate. Used to be sent via JSON
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HouseholdDescription {
    /// Should be one of "Prosumer", "Consumer" or "PV", but does not have effect on internal calculations
    pub household_type: String,
    /// The Initial energy balance
    pub initial_energy_balance: f64,
    /// How a possible Smart Battery is set uo
    pub battery_config: BatteryConfigToml,
    /// Type of the Battery
    pub battery_type: HouseholdBatteries,
    /// Battery Capacity in WH (10 000 WH = 10 kWH)
    pub battery_capacity: u64,
    /// Initial Charge of the battery in WH
    pub initial_charge: u64,
}

#[derive(Debug)]
pub struct ModelHousehold {
    /// One of "Consumer", "Prosumer" or "PV".
    pub household_type: String,
    /// Produced Energy
    pub p_mw_pv: f64,
    /// Energy used in a given time
    pub p_mw_load: f64,
    /// Difference of `p_mw_pv` and `p_mw_load`
    pub energy_balance: f64,
    /// The energy balance published to the neighborhood
    pub published_balance: i64,
    /// If there is a battery, its capacity in WH
    pub battery_capacity: u64,
    /// Which type of battery
    pub battery_type: HouseholdBatteries,
    /// DB that holds all the information about this household
    db: Db,
}

impl ModelHousehold {
    /// Creates an ModelHousehold instance from the given [HouseholdDescription] and a starting
    /// time.
    fn new_from_description(time: TimePeriod, description: HouseholdDescription) -> Self {
        Self::new(
            description.household_type,
            description.initial_energy_balance,
            &description.battery_config,
            description.battery_type,
            description.battery_capacity,
            description.initial_charge,
            time,
        )
    }
    fn new(
        household_type: String,
        init_reading: f64,
        battery_config: &BatteryConfigToml,
        battery_type: HouseholdBatteries,
        battery_capacity: u64,
        initial_charge: u64,
        time: enerdag_time::TimePeriod,
    ) -> Self {
        let db = test_utilities::setup_db();

        Self::setup_battery(
            &battery_type,
            &db,
            battery_config,
            battery_capacity as i64,
            initial_charge as i64,
            &time,
        );
        match battery_type {
            HouseholdBatteries::SmartBattery => {
                Self::setup_smart_battery(
                    &db,
                    battery_config,
                    battery_capacity as i64,
                    initial_charge as i64,
                    &time,
                );
            }
            _ => {
                warn!("Not implemented yet");
            }
        }

        ModelHousehold {
            household_type,
            energy_balance: init_reading,
            db,
            p_mw_load: 0.0,
            p_mw_pv: 0.0,
            battery_capacity,
            battery_type,
            published_balance: 0,
        }
    }
    /// Set input variables to zero.
    fn reset_prosumption(&mut self) {
        self.p_mw_load = 0.0;
        self.p_mw_pv = 0.0;
    }

    /// Performs all the calculations of the PublishEnergyBalance Smart Contract and returns the
    /// the EnergyBalance a "real" household would publish
    pub fn perform_trade_round(
        &mut self,
        energy_balance: EnergyBalance,
        grid_power_load: i64,
        total_disposable_energy: i64,
    ) -> EnergyBalance {
        use test_utilities::perform_trading_round;
        let eb = perform_trading_round(
            &self.db,
            &energy_balance.period,
            energy_balance.energy_balance,
            grid_power_load,
            total_disposable_energy,
        )
        .unwrap();
        debug!(
            "{}: ({}, {})Internally/Published {}/{}",
            self.battery_type.to_string(),
            grid_power_load,
            total_disposable_energy,
            energy_balance.energy_balance,
            eb.energy_balance
        );
        self.published_balance = eb.energy_balance.clone();
        eb
    }

    pub(crate) fn update_inputs(&mut self, inputs: Map<AttributeId, Value>) {
        for (attribute, value) in inputs.into_iter() {
            let arr: HashMap<String, f64> = serde_json::from_value(value).unwrap();
            let agg: f64 = arr.into_iter().map(|(_, x)| x).sum();
            if attribute.eq("p_mw_load") {
                self.p_mw_load = agg;
            } else if attribute.eq("p_mw_pv") {
                self.p_mw_pv = agg;
            } else {
                panic!(
                    "Unknown Attribute: {} for {}",
                    attribute, &self.household_type
                );
            }
        }
    }

    /// Inserts energy balance into db and sets `self.energy_balance` field.
    pub(crate) fn set_energy_balance(&mut self, energy_balance: EnergyBalance) {
        enerdag_core::db::insert_energy_balance(&self.db, &energy_balance)
            .expect("Could not insert Energy Balance.");
        enerdag_core::db::insert_household_energy_balance(&self.db, &energy_balance)
            .expect("Could not insert Household Energy balance");
        self.energy_balance = energy_balance.energy_balance as f64;
    }

    /// Performs Calculations for the disposable energy in this period.
    pub(crate) fn get_disposable_energy(&self, time: &enerdag_time::TimePeriod) -> i64 {
        use enerdag_core::contracts::HouseholdBatteries::SmartBattery;

        HouseholdBatteries::get_disposable_energy(&SmartBattery, &self.db, time)
    }

    /// Calculate the load the household draws from / gives to the grid
    pub fn get_power_load(&self, time: &enerdag_time::TimePeriod) -> i64 {
        use enerdag_core::contracts::HousholdBatterySmartContract;
        use enerdag_core::db::config::_get_battery_type;
        let energy_balance =
            EnergyBalance::new_with_period(self.energy_balance as i64, time.clone());
        if let Ok(HouseholdBatteries::NoBattery) = _get_battery_type(&self.db) {
            let l = chrono::Duration::seconds(enerdag_time::TIME_PERIOD_LENGTH_SECONDS as i64);
            wh_to_w(energy_balance.energy_balance, l)
        } else {
            HousholdBatterySmartContract::powerload_in_period(&self.db, energy_balance).unwrap()
        }
    }

    /// Sets up the Battery for the given Household.
    pub fn setup_battery(
        battery_type: &HouseholdBatteries,
        db: &Db,
        config: &BatteryConfigToml,
        capacity: i64,
        charge: i64,
        initial_period: &TimePeriod,
    ) {
        match battery_type {
            HouseholdBatteries::SmartBattery => {
                Self::setup_smart_battery(db, config, capacity, charge, initial_period)
            }
            HouseholdBatteries::SimpleBattery => todo!(),
            HouseholdBatteries::NoBattery => Self::setup_no_battery(db),
        }
    }
    fn setup_smart_battery(
        db: &Db,
        config: &BatteryConfigToml,
        capacity: i64,
        charge: i64,
        initial_period: &TimePeriod,
    ) {
        use test_utilities::config::insert_uema_battery_config;
        use test_utilities::data::insert_initial_state;
        use test_utilities::test_helper_re::setup_smart_battery;
        setup_smart_battery(db, capacity);
        insert_initial_state(db, initial_period, charge);

        match config.disposable_energy_calc {
            DisposableEnergyCalcToml::SARIMA => {
                #[cfg(feature = "sarima")]
                test_utilities::config::insert_sarima_battery_config(db, capacity);
                #[cfg(not(feature = "sarima"))]
                panic!("SARIMA is not activated as a feature but was selected in the config.")
            }
            DisposableEnergyCalcToml::UEMA => insert_uema_battery_config(db, capacity),
            DisposableEnergyCalcToml::TwentyPercent => {}
        }
    }

    fn setup_no_battery(db: &Db) {
        use enerdag_core::db::config::set_battery_type;
        set_battery_type(db, &HouseholdBatteries::NoBattery).expect("Couldn't set battery type");
    }

    /// Apply different methods based on the type of battery.
    pub fn dispatch_methods_battery_type<T>(
        &self,
        time: &enerdag_time::TimePeriod,
        f_smart: DispatchFunc<T>,
        f_simple: DispatchFunc<T>,
        f_no: DispatchFunc<T>,
    ) -> T {
        use enerdag_core::db::config::_get_battery_type;
        let bt = _get_battery_type(&self.db).unwrap();
        match bt {
            HouseholdBatteries::SmartBattery => f_smart(self, time),
            HouseholdBatteries::SimpleBattery => f_simple(self, time),
            HouseholdBatteries::NoBattery => f_no(self, time),
        }
    }

    pub(crate) fn get_battery_charge(&self, time: &TimePeriod) -> EnergyBalance {
        use enerdag_core::db::battery::get_battery_charge;
        if let Ok(balance) = get_battery_charge(&self.db, time) {
            balance
        } else {
            error!("Could not find Battery Charge for period : {:?}", time);
            EnergyBalance::new_with_period(0, time.clone())
        }
    }

    pub(crate) fn get_value(&self, attr: &AttributeId, time: &TimePeriod) -> Value {
        match attr as &str {
            "p_mw_load" => serde_json::to_value(self.p_mw_load).unwrap(),
            "published_energy_balance" => serde_json::to_value(self.published_balance).unwrap(),
            "energy_balance" => serde_json::to_value(self.energy_balance).unwrap(),
            "p_mw_pv" => serde_json::to_value(self.p_mw_pv).unwrap(),
            "disposable_energy" => serde_json::to_value(self.get_disposable_energy(time)).unwrap(),
            "battery_charge" => {
                serde_json::to_value(self.get_battery_charge(time).energy_balance).unwrap()
            }
            _ => {
                panic!("Unknown Attribute {} for ModelHousehold", attr)
            }
        }
    }
}

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
#[allow(dead_code)]
fn k_wh_to_mw(energy: MW, time_in_s: i64) -> kWh {
    let hours = time_in_s as f64 / (3600.);
    let p_in_k_w = energy / hours;
    let p_in_w = p_in_k_w * 1000.;
    let p_in_mw = p_in_w / (1000. * 1000.);
    return p_in_mw;
}
