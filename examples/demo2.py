# demo_2.py
import mosaik
import mosaik.util


# Sim config
# this_folder = __file__.rsplit("/", 1)[0]
SIM_CONFIG = {
    "ExampleSim": {
        # "python": "simulator_mosaik:ExampleSim",
        "cmd": "cargo run --example example_sim -- -a=%(addr)s",
    },
    "ExampleCtrl": {
        # "cmd": f"%(python)s {this_folder}/controller.py %(addr)s",
        # "python": "controller:Controller",
        "cmd": "cargo run --example controller -- -a=%(addr)s",
    },
    "Collector": {
        # "cmd": f"%(python)s {this_folder}/collector.py %(addr)s",
        "cmd": "cargo run --example collector -- -a=%(addr)s",
    },
}
END = 10  # 10 seconds

# Create World
world = mosaik.World(SIM_CONFIG)
# End: Create World

# Start simulators
with world.group():
    examplesim = world.start("ExampleSim", eid_prefix="Model_")
    examplectrl = world.start("ExampleCtrl")
collector = world.start("Collector")
# End: Start simulators

# Instantiate models
models = [examplesim.ExampleModel(init_val=i) for i in range(-2, 3, 2)]
agents = examplectrl.Agent.create(len(models))
monitor = collector.Monitor()
# End: Instantiate models

# Connect entities
for model, agent in zip(models, agents):
    world.connect(model, agent, ("val", "val_in"))
    world.connect(agent, model, "delta", weak=True)
# End: Connect entities

# Connect to monitor
mosaik.util.connect_many_to_one(world, models, monitor, "val", "delta")
mosaik.util.connect_many_to_one(world, agents, monitor, "delta")

# Run simulation
world.run(until=END)
