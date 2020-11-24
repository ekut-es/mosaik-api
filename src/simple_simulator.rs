mod simulation_mosaik;
pub struct Model{
    val: f32,
    delta: f32,
}

pub trait RunModel{
    fn initmodel(init_value: f32) -> Model;
    fn step(&mut self);
}

pub impl RunModel for Model{
    fn initmodel(init_value: f32) -> Model{
        Model {
            val: init_value,
            delta: 1.0,
        }
    }

    fn step(&mut self){
        self.val += self.delta;
    }
}

pub struct Simulator{
    models: Vec<Model>,
    data: Vec<Vec<f32>> //brauch ich 2d Array?
}

pub trait RunSimulator{
    fn init_simulator() -> Simulator;
    fn add_model(&mut self, init_value: f32);
    fn step(&mut self, deltas: Option<Vec<(usize, f32)>>);
}


pub impl RunSimulator for Simulator{
    fn init_simulator() -> Simulator{
        Simulator{
            models: vec![],
            data: vec![]
        }
    }

    fn add_model(&mut self, init_value: f32){
        let /*mut*/ model:Model = Model::initmodel(init_value);
        self.models.push(model);
        self.data.push(vec![]); //Add list for simulation data
    }

    fn step(&mut self, deltas: Option<Vec<(usize, f32)>>){
        match deltas{
            Some(deltas) => {
                for (idx, deltax) in deltas.iter(){
                    self.models[*idx].delta = *deltax;
                }
            },
            None => {}
        }

        for (i, model) in self.models.iter_mut().enumerate(){
            model.step();
            self.data[i].push(model.val); 
        }
    }
}

pub fn run(){
    let mut sim:Simulator = Simulator::init_simulator(); //need an instance of Simulator, just like in init_model()
    //sim = Simulator()
    
    for i in 0..3{
        sim.add_model(0.0);
    }

    sim.step(None); //values = 1.0 , 1.0
    sim.step(Some(vec![(0,8.0), (1, 13.0), (2, 19.0)]));
    sim.step(Some(vec![(0,23.0), (1, 42.0), (2, 68.0)])); //values = 24.0 , 43.0

    println!("Simulation finished with data:");

    for (i, inst) in sim.data.iter().enumerate(){
        println!("{}: {:?}", i, inst);
    }
}
