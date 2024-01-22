# demo_2.py
import mosaik
import mosaik.util

# Sim config. and other parameters
SIM_CONFIG = {
    "ExampleSim": {
        "cmd": "cargo run --example example_sim -r -q -- -a=%(addr)s",
    },
    "ExampleCtrl": {
        "cmd": "cargo run --example controller -r -q -- -a=%(addr)s",
    },
    "Collector": {
        "cmd": "%(python)s examples/collector.py %(addr)s",
    },
}
END = 10  # 10 seconds

# Create World
world = mosaik.World(SIM_CONFIG, max_loop_iterations=3000)

# Start simulators
examplesim = world.start("ExampleSim", eid_prefix="RModel_")
examplectrl = world.start("ExampleCtrl")
collector = world.start("Collector")

# Instantiate models
models = [examplesim.RModel(init_val=i) for i in range(-2, 3, 2)]
agents = examplectrl.Agent.create(len(models))
monitor = collector.Monitor()

# Connect entities
for model, agent in zip(models, agents):
    world.connect(model, agent, ("val", "val_in"))
    world.connect(agent, model, "delta", weak=True)

mosaik.util.connect_many_to_one(world, models, monitor, "val", "delta")
mosaik.util.connect_many_to_one(world, agents, monitor, "delta")

# Run simulation
world.run(until=END)
