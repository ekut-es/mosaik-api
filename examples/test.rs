use mosaik_rust_api::{run_simulation, simulation_mosaik::init_sim, MosaikAPI};

pub fn main() /*-> Result<()>*/
{
    //The local addres mosaik connects to.
    let addr = "127.0.0.1:3456"; //wenn wir uns eigenständig verbinden wollen -> addr als option. accept_loop angepasst werden!!!
    let simulator = init_sim();
    run_simulation(addr, simulator);
}
