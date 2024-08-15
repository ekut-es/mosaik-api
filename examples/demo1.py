# demo_1.py
import mosaik
import mosaik.util

# Sim config. and other parameters
SIM_CONFIG = {
    "ExampleSim": {
        "cmd": "cargo run --example example_sim -- -a=%(addr)s",
    },
    "Collector": {
        "cmd": "cargo run --example collector -- -a=%(addr)s",
    },
}
END = 10  # 10 seconds

# Create World
world = mosaik.World(SIM_CONFIG)

# Start simulators
# NOTE sim_name must match String in SIM_CONFIG
examplesim = world.start(
    "ExampleSim", eid_prefix="Model_"
)  # FIXME eid_prefix param does not connect with Rust yet
collector = world.start("Collector")

# Instantiate models
# NOTE model class name must match String in META of Simulator
model = examplesim.ExampleModel(
    init_val=2
)  # FIXME init_val param does not connect with Rust yet
monitor = collector.Monitor()

# Connect entities
world.connect(model, monitor, "val", "delta")

# Create more entities
more_models = examplesim.ExampleModel.create(
    2, init_val=3
)  # FIXME init_val param does not connect with Rust yet
mosaik.util.connect_many_to_one(world, more_models, monitor, "val", "delta")

# Run simulation
world.run(until=END)

# --------- Expected Output:
#
# Collected data:
# - ExampleSim-0.Model_0:
#   - delta: {0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1, 8: 1, 9: 1}
#   - val: {0: 3, 1: 4, 2: 5, 3: 6, 4: 7, 5: 8, 6: 9, 7: 10, 8: 11, 9: 12}
# - ExampleSim-0.Model_1:
#   - delta: {0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1, 8: 1, 9: 1}
#   - val: {0: 4, 1: 5, 2: 6, 3: 7, 4: 8, 5: 9, 6: 10, 7: 11, 8: 12, 9: 13}
# - ExampleSim-0.Model_2:
#   - delta: {0: 1, 1: 1, 2: 1, 3: 1, 4: 1, 5: 1, 6: 1, 7: 1, 8: 1, 9: 1}
#   - val: {0: 4, 1: 5, 2: 6, 3: 7, 4: 8, 5: 9, 6: 10, 7: 11, 8: 12, 9: 13}
