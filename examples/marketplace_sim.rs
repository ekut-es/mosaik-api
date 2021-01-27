use log::error;
use serde_json::{json, Map, Value};

use mosaik_rust_api::{API_Helpers, AttributeId, MosaikAPI, run_simulation};

pub fn main() /*-> Result<()>*/
{
    //The local addres mosaik connects to.
    let addr = "127.0.0.1:3456"; //wenn wir uns eigenständig verbinden wollen -> addr als option. accept_loop angepasst werden!!!
    let simulator = MarketplaceSim::init_sim();
    run_simulation(addr, simulator);
}


impl MosaikAPI for MarketplaceSim{
    fn setup_done(&self) {
        todo!()
    }

    fn stop(&self) {
        todo!()
    }
}
pub struct MarketplaceSim {
    pub models: Vec<Model>,
    data: Vec<Vec<f64>>,
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
        self.models.get(model_idx as usize).and_then(|x| x.get_value(attr))
    }

    fn sim_step(&mut self, deltas: Vec<(String, u64, Map<String, Value>)>) {
        for (attr_id, idx, deltax) in deltas.iter() {
            let delta = deltax
                .values()
                .map(|x| x.as_f64().unwrap_or_default())
                .sum(); //unwrap -> default = 0 falls kein f64
            self.models[*idx as usize].update_model(attr_id, delta); //wird einfach mit 0 überschrieben -> anpassen zu ...
        }

        for (i, model) in self.models.iter_mut().enumerate() {
            model.step();
            self.data[i].push(model.reading);
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
    p_mw_pv: f64,
    p_mw_load: f64,
    reading: f64,
}
impl Model {
    ///Function gets called from get_model() to give the model values.
    pub fn get_value(&self, attr: &str) -> Option<Value> {
        let result = match attr {
            "p_mw_pv" => Value::from(self.p_mw_pv),
            "p_mw_load" => Value::from(self.p_mw_load),
            "reading" => Value::from(self.reading),
            x => {
                error!("no known attr requested: {}", x);
                return None;
            }
        };
        Some(result)
    }

    pub fn update_model(&mut self, attr: &str, delta: f64) {
        match attr {
            "p_mw_pv" => self.p_mw_pv = delta,
            "p_mw_load" => self.p_mw_load = delta,
            x => {
                error!("no known attr requested: {}", x);
            }
        };
    }
}

pub trait RunModel {
    fn initmodel(init_reading: f64) -> Model;
    fn step(&mut self);
}

impl RunModel for Model {
    fn initmodel(init_reading: f64) -> Model {
        Model {
            p_mw_pv: 0.0,
            p_mw_load: 0.0, //ersetze mit Funktion die Werte von smart-meter alle 15 minuten aus gibt
            reading: init_reading,
        }
    }

    fn step(&mut self) {
        self.reading += self.p_mw_pv - self.p_mw_load;
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
