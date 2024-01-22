# demo_1.py
import mosaik
import mosaik.util

# Sim config. and other parameters
this_folder = __file__.rsplit("/", 1)[0]
SIM_CONFIG = {
    "ExampleSim": {
        "connect": "127.0.0.1:3456",
    },
    "Collector": {
        "cmd": f"%(python)s {this_folder}/collector.py %(addr)s",
    },
}
END = 10 * 60

# Create World
world = mosaik.World(SIM_CONFIG)

# Start simulators
examplesim = world.start("ExampleSim", eid_prefix="RModel_")
collector = world.start("Collector")

# Instantiate models
model = examplesim.RModel(init_val=2)
monitor = collector.Monitor()

# Connect entities
world.connect(model, monitor, "val", "delta")

# Create more entities
more_models = examplesim.RModel.create(2, init_val=3)
mosaik.util.connect_many_to_one(world, more_models, monitor, "val", "delta")

# Run simulation
world.run(until=END)
