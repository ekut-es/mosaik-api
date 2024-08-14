# demo_1.py
import mosaik
import mosaik.util

# Sim config. and other parameters
this_folder = __file__.rsplit("/", 1)[0]
SIM_CONFIG = {
    "ExampleSim": {
        "cmd": "cargo run --example example_sim -- -a=%(addr)s",
    },
    "Collector": {
        # "cmd": f"%(python)s {this_folder}/collector.py %(addr)s",
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
    init_val=10
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
