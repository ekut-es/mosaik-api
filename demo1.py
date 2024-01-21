# demo_1.py
import mosaik
import mosaik.util

# Sim config. and other parameters
SIM_CONFIG = {
    "RExampleSim": {
        "connect": "127.0.0.1:3456",
    },
    "Collector": {
        "cmd": "%(python)s collector.py %(addr)s",
    },
}
END = 10 * 60  # 10 minutes

# Create World
world = mosaik.World(SIM_CONFIG)

# Start simulators
examplesim = world.start("RExampleSim", eid_prefix="RModel_")
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
