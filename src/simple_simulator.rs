use log::error;
use serde_json::Value;
pub struct Model {
    p_mw_pv: f64,
    p_mw_load: f64,
}
impl Model {
    ///Function gets called from get_model() to give the model values.
    pub fn get_value(&self, attr: &str) -> Option<Value> {
        let result = match attr {
            "p_mw_pv" => Value::from(self.p_mw_pv),
            "p_mw_load" => Value::from(self.p_mw_load),
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
            p_mw_pv: init_value,
            p_mw_load: 1.0, //ersetze mit Funktion die Werte von smart-meter alle 15 minuten aus gibt
        }
    }

    fn step(&mut self) {
        self.p_mw_pv += self.p_mw_load;
    }
}

pub struct Simulator {
    pub models: Vec<Model>,
    data: Vec<Vec<f64>>,
}

pub trait RunSimulator {
    fn init_simulator() -> Simulator;
    fn add_model(&mut self, init_value: Option<f64>);
    fn step(&mut self, deltas: Option<Vec<(u64, f64)>>);
}

impl RunSimulator for Simulator {
    /// Initiate the the vectors for the models and the data
    fn init_simulator() -> Simulator {
        println!("Initiation of simulator.");
        Simulator {
            models: vec![],
            data: vec![],
        }
    }

    ///Add a model instance to the list.
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

    ///Call the step function to perform a simulation step and include the deltas from mosaik, if there are any.
    fn step(&mut self, deltas: Option<Vec<(u64, f64)>>) {
        match deltas {
            Some(deltas) => {
                for (idx, deltax) in deltas.iter() {
                    self.models[*idx as usize].p_mw_load = *deltax;
                }
            }
            None => {
                error!("Got no deltas for the step.");
            }
        }

        for (i, model) in self.models.iter_mut().enumerate() {
            model.step();
            self.data[i].push(model.p_mw_pv);
        }
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
