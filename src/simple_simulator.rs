use log::{error, info};
use serde_json::{Value};
pub struct Model {
    val: f64,
    delta: f64,
}
impl Model {

    pub fn get_value(&self, attr: &str) -> Option<Value> {
        
        let result = match attr {
            "val" => Value::from(self.val),
            "delta" => Value::from(self.delta),
            x => {
                error!("no known attr requested: {}", x);
                return None;
            }
        };
        Some(result)
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
            None => {error!("Got no deltas for the step.");}
        }

        for (i, model) in self.models.iter_mut().enumerate() {
            model.step();
            self.data[i].push(model.val);
        }
    }
}
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

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::Model;

    #[test]
    fn test_get_value() {
        let model = Model {
            val: 0.0,
            delta: 1.0,
        };

        let val_val = Some(Value::from(model.val));
        let val_delta = Some(Value::from(model.delta));

        assert_eq!(val_val, model.get_value("val"));
        assert_eq!(val_delta, model.get_value("delta"));
        assert_eq!(None, model.get_value("different"));
    }
}
