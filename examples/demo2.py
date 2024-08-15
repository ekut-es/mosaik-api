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
        "python": "controller:Controller",
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

# --------- Expected Output:
#
# Collected data:
# - ExampleCtrl-0.Agent_0:
#   - delta: {2: -1, 5: 1, 8: -1}
# - ExampleCtrl-0.Agent_1:
#   - delta: {1: -1, 4: 1, 7: -1}
# - ExampleCtrl-0.Agent_2:
#   - delta: {0: -1, 3: 1, 6: -1, 9: 1}
# - ExampleSim-0.Model_0:
#   - delta: {0: 1, 1: 1, 2: -1, 3: -1, 4: -1, 5: 1, 6: 1, 7: 1, 8: -1, 9: -1}
#   - val: {0: 0, 1: 2, 2: 2, 3: 0, 4: -2, 5: -2, 6: 0, 7: 2, 8: 2, 9: 0}
# - ExampleSim-0.Model_1:
#   - delta: {0: 1, 1: -1, 2: -1, 3: -1, 4: 1, 5: 1, 6: 1, 7: -1, 8: -1, 9: -1}
#   - val: {0: 2, 1: 2, 2: 0, 3: -2, 4: -2, 5: 0, 6: 2, 7: 2, 8: 0, 9: -2}
# - ExampleSim-0.Model_2:
#   - delta: {0: -1, 1: -1, 2: -1, 3: 1, 4: 1, 5: 1, 6: -1, 7: -1, 8: -1, 9: 1}
#   - val: {0: 2, 1: 0, 2: -2, 3: -2, 4: 0, 5: 2, 6: 2, 7: 0, 8: -2, 9: -2}
