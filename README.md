# Rust API for Mosaik

This project allows Rust code to operate with Mosaik simulations in Python.
It provides a high-level API for simulators written in Rust to communicate with the Mosaik Smart-Grid co-simulation framework.

The API is based on the [Mosaik API](https://mosaik.readthedocs.io/en/3.3.3/api.html) and is compatible with Mosaik version 3 <mark>but does not support asynchronous communication yet.</mark>

## Content of this repository
The [src](./src/) folder contains the components for communicating between Rust simulators and Mosaik:
  - The trait for the **high-level API** in [lib.rs](./src/lib.rs).
  - The **data types** used in the API and simulators in [types.rs](./src/types.rs).
  - The **TCP manager** in [tcp.rs](./src/tcp.rs).
  - The **parsing** and **handling of MosaikMessages** and low-level connection to simulators in [mosaik_protocol.rs](./src/mosaik_protocol.rs).

The [examples](./examples/) folder contains example simulators based on the [official Python tutorial](https://mosaik.readthedocs.io/en/3.3.3/tutorials/index.html) to demonstrate the API. See section [Running the example scenarios](#running-the-example-scenarios) for more information.

## Requirements
- See [Cargo.toml](./Cargo.toml) for the Rust dependencies.
- To run the examples you need the dev-dependencies and you will also need a Python environment, the setup of which is described [below](#running-the-example-scenarios).

## Implementing a simulation in Rust
For a simulator written in Rust to successfully communicate with Mosaik you must:
- add this repo to your `Cargo.toml` as a dependency with `mosaik-api-rust = { git = "link/to/this/repo" }`,
- implement the `MosaikApi`  trait for your simulator struct and,
- use the `run_simulation()` function to connect your simulator with Mosaik in your `main`. This connects your simulator to Mosaik and handles the communication over a TCP channel. The `ConnectionDirection` depends on how you connect your simulator to Mosaik in Python. (Described below.)
- Then connect the simulators in the `SIM_CONFIG` of your Mosaik python script as described in the following paragraph.

We support two ways to connect Rust simulators for communication with Mosaik. It is sufficient to implement one but possible to implement both simultaneously as shown in the [examples](#running-the-example-scenarios).
1. Invoke the Rust simulator and parse Mosaik's `addr` to it, via the `"cmd"` keyword in Mosaik's `SIM_CONFIG` in Python. For this you need the `ConnectionDirecton::ConnectToAddress` in Rust with the given address of Mosaik.
2. Run the Rust simulator manually with a predefined `addr` and connect Mosaik to it with the `"connect"` keyword in Mosaik's `SIM_CONFIG` in Python. In Rust you need to use `ConnectionDirecton::ListenOnAddress` for the `run_simulation()`.

Example setup to illustrate these two options with `ADDR` as the address of the communication channel between Mosaik and the Rust simulator, e.g. `127.0.0.1:5678`:
| Mosaik `SIM_CONFIG` key                       | Rust `ConnectionDirection` for `run_simulation()` | Notes                                                                                  |
| --------------------------------------------- | ------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `"cmd": "cargo run SIMULATOR -- -a=%(addr)s"` | `ConnectionDirection::ConnectToAddress(ADDR)`     | ADDR needs to be read in from the CLI in Rust.                                         |
| `"connect": "ADDR"`                           | `ConnectionDirection::ListenOnAddress(ADDR)`      | Simulator needs to be started before running the Python script on the predefined ADDR. |

## Running the example scenarios

The example simulations are located in the [examples folder](./examples/).
It includes two scenarios inspired by the [Python tutorials](https://mosaik.readthedocs.io/en/3.3.3/tutorials/index.html): demo1 and demo2.
In contrast to the Python tutorials, the simulations are written in Rust and only invoked by a modified Python script to connect the Rust simulators with Mosaik (or connecting Mosaik with the simulators as shown by the Controller in demo2).
- The scenario in **Demo 1** consists of an example model `Model` used in the *hybrid* `ExampleSim` both implemented in [example_sim.rs](./examples/example_sim.rs) and an *event-based* Monitor called `Collector` in [collector.rs](./examples/collector.rs).
- The scenario in **Demo 2** consists additionaly of the Rust implementation of an event-based [`Controller`](./examples/controller.rs). This controller is connected via a TCP connection instead of being run by the Python script for the sake of demonstration.

To run them, build a virtual environment for python:

```bash
virtualenv .venv_examples && \
source .venv_examples/bin/activate && \
pip install "mosaik>=3.3"
```

Then run the example scenarios with:

```bash
cargo build --examples
python examples/demo1.py
```
or
```bash
cargo build --examples
cargo run --example connector & python examples/demo2.py
```
