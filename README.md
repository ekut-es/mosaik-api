# mosaik-rust-api

Currently for mosaik < v3

Repository for the marketplace simulation.

The [src](./src/) folder contains the API and the TCP manager in `lib.rs` with a parser in `json.rs`. These are the main components for communicating with mosaik.

The [examples](./examples/) folder contains `marketplace_sim.rs`, which is the `main` file in Rust terms for which we present the simulator. It contains the simulator for the enerDAG marketplace.

## Requirements

- Rust & mosaik-rust-api
- (Mosaik) simulation repository and its requirements

### Starting the simulation

1. The first thing to do is to build `marketplace_sim.rs`:

    - `cargo build --example marketplace_sim`

2. The second thing to do is to decide how to start the simulation.

    There are currently two ways to run the simulation:
    - The first is to run the current version of the `city_energy_simulation.py` interface in the simulation repo under cosimulation_city_energy.
This version will run *marketplace_sim.rs itself.

    - The second way is to modify `city_energy_simulation.py` first:
        - `'cmd': '../../mosaik-rust-api/target/debug/examples/marketplace_sim.exe -a %(addr)s',` this line needs to be changed to
        - ` 'connect': '127.0.0.1:3456', `.

        Once the change has been made, `marketplace_sim.rs` needs to be started by running
        - `cargo run --example marketplace_sim`.
            For debugging: `$env:RUST_LOG="debug"; cargo run --example marketplace_sim`

    Once started, it will wait for a client to connect to the TCP manager, now the `city_energy_simulation.py`-Cosimulation can be started.

#### Changing the scenario

To change the parameters of the simulation, define the scenarios in "rust_interface.py" located in the simulation repo under cosimulation_city_energy.

You can change the number of consumers, photovoltaic units and prosumers in the line with:

```Python
sim_data_entities = hhsim.householdsim(num_of_consumer=5, num_of_pv=5, num_of_prosumer=5, data_base_path=DATABASE_PATH, start_time=START).children
```

The step size of the simulation is currently 15 minutes and can be changed in `city_energy_simulation.py`, as well as the start and end time of the simulation.

### Running the example scenarios

The example simulations are located in the [examples folder](./examples/).
It includes two scenarios inspired by the [Python tutorials](https://mosaik.readthedocs.io/en/3.3.3/tutorials/index.html): demo1 and demo2.
In contrast to the Python tutorials, the simulations are written in Rust and only invoked by a modified Python script to connect the Rust simulators with Mosaik.
- The scenario in **Demo 1** consists of an example model `Model` used in the *hybrid* `ExampleSim` both implemented in [example_sim.rs](./examples/example_sim.rs) and an *event-based* Monitor called `Collector` in [collector.rs](./examples/collector.rs).
- The scenario in **Demo 2** consists additionaly of the Rust implementation of an event-based [`Controller`](./examples/controller.rs).

To run them, build a virtual environment for python:

```bash
virtualenv .venv_examples && \
source .venv_examples/bin/activate && \
pip install -r examples/requirements.txt
```

Then run the example scenarios with:

```bash
python examples/demo1.py
```
or
```bash
python examples/demo2.py
```
