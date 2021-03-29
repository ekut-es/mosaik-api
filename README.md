# mosaik-rust-api
Repo for the marketplace simulation.
The `src` folder contains the API and the TCP manager in `lib.rs` with a parser in `json.rs`. These two are the main components for the communication with mosaik.
The `examples` folder contains `marketplace_sim.rs` which is the `main` file in rust terms, it holds the simulator for the marketplace of enerDAG. 

## Requirements
Rust and the requirements of the simulation repo.

### Start the simulation
The first thing that needs to be done is to build `marketplace_sim.rs`:
`cargo build --main marketplace_sim`

The second thing is to decide how to start the simulation.

There are currently two ways to start the simulation.
The first one is run the current version of `city_energy_simulation.py` interface in the simulation repo under cosimulation_city_energy.
This version starts `marketplace_sim.rs` itself.

For the second way `city_energy_simulation.py` needs to be changed first:
` 'cmd': '../../mosaik-rust-api/target/debug/examples/marketplace_sim.exe -a %(addr)s', ` this line need to be changed to
` 'connect': '127.0.0.1:3456', `.
If the change is done, `marketplace_sim.rs` needs to be started beforehand via:
`cargo run --main marketplace_sim`.
For debug: `$env:RUST_LOG="debug"; cargo run --main marketplace_sim`
After it got started it is waiting on a client to connect to the TCP manager, now `city_energy_simulation.py` can be started.

#### Change the scenario
To change the parameters of the simulation, define the scenarios in "rust_interface.py" which is located in the simulation repo under cosimulation_city_energy.
In the line with:
`sim_data_entities = hhsim.householdsim(num_of_consumer=5, num_of_PV=5, num_of_prosumer=5, data_base_path=DATABASE_PATH, start_time=START).children`
one can change the number of consumer, photovoltaic units and prosumer.
The step size of the simulation is currently 15 minutes and can be changed in `city_energy_simulation.py`, as well as the start and end time of the simulation.
