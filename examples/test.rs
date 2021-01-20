use mosaik_rust_api::{run_simulation, simulation_mosaik::init_sim, MosaikAPI};

pub fn main() /*-> Result<()>*/
{
    let addr = "127.0.0.1:3456"; //The local addres mosaik connects to.
    let simulator = init_sim();
    run_simulation(addr, simulator);
}
