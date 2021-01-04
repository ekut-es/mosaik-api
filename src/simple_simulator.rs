use log::{error, info};
use serde_json::{json, Map, Value};
pub struct Model {
    val: f64,
    delta: f64,
}
impl Model {
    pub fn to_serde_values(&self) -> Map<String, Value> {
        let mut map = Map::new();
        map.insert("val".to_string(), Value::from(self.val));
        map.insert("delta".to_string(), Value::from(self.delta));

        map
    }

    pub fn get_value(&self, attr: String) -> Option<Value> {
        let attribute_1 = json!("val");
        let attribute_1_1 = attribute_1.to_string();
        let attribute_2 = json!("delta");
        let attribute_2_1 = attribute_2.to_string();
        if attr == attribute_1_1 {
            let result = Value::from(self.val);
            return Some(result);
        } else if attr == attribute_2_1 {
            let result = Value::from(self.delta);
            return Some(result);
        } else {
            println!("no known attr requested:");
            return None;
        };

        /*let result = match attr {
            "val" => Value::from(self.val),
            "delta" => Value::from(self.delta),
            x => {
                error!("no known attr requested: {}", x);
                return None;
            }
        };
        Some(result)*/
    }
}

pub trait RunModel {
    fn initmodel(init_value: f64) -> Model;
    fn step(&mut self);
}

impl RunModel for Model {
    fn initmodel(init_value: f64) -> Model {
        Model {
            val: init_value,
            delta: 1.0,
        }
    }

    fn step(&mut self) {
        self.val += self.delta;
    }
}

pub struct Simulator {
    pub models: Vec<Model>,
    data: Vec<Vec<f64>>, //brauch ich 2d Array?
}

pub trait RunSimulator {
    fn init_simulator() -> Simulator;
    fn add_model(&mut self, init_value: Option<f64>);
    fn step(&mut self, deltas: Option<Vec<(u64, f64)>>);
}

impl RunSimulator for Simulator {
    fn init_simulator() -> Simulator {
        println!("initiation of simulator.");
        Simulator {
            models: vec![],
            data: vec![],
        }
    }

    fn add_model(&mut self, init_value: Option<f64>) {
        match init_value {
            Some(init_value) => {
                let /*mut*/ model:Model = Model::initmodel(init_value);
                self.models.push(model);
                self.data.push(vec![]); //Add list for simulation data
            }
            None => {}
        }
    }

    fn step(&mut self, deltas: Option<Vec<(u64, f64)>>) {
        match deltas {
            Some(deltas) => {
                for (idx, deltax) in deltas.iter() {
                    self.models[*idx as usize].delta = *deltax;
                }
            }
            None => {println!("Got no deltas for the step.");}
        }

        for (i, model) in self.models.iter_mut().enumerate() {
            model.step();
            self.data[i].push(model.val);
        }
    }
}

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
}
