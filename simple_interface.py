import random
import mosaik
from mosaik.util import connect_many_to_one
from sys import platform


sim_config = {
    'rust_sim': {
        'connect': '127.0.0.1:3456',
    },
    'Collector': {
        'cmd': 'python collector.py %(addr)s',
    },
}

END = 10 * 60  # 10 Min.

print("call Sim_Manager")
world = mosaik.World(sim_config)

# create_scenario(world)
rustAPI = world.start('rust_sim', eid_prefix='Model_')
collector = world.start('Collector', step_size=60)

# Instantiate models
model = rustAPI.ExampleModel(init_val=2)
monitor = collector.Monitor()
# Create one instance of of our example model and one database instance

# Connect entities
world.connect(model, monitor, 'val', 'delta')
# through the connection we tell mosaik to send the outputs of the example to the monitor

# Create more entities (you usually work with larger sets of entities)
# instead of instantiating the example model directly, we called its static method
# create() and passed the number of instances to it
more_models = rustAPI.ExampleModel.create(2, init_val=3)

# Connects all entities to the database
mosaik.util.connect_many_to_one(world, more_models, monitor, 'val', 'delta')

print('world run starting')

# Run simulation
world.run(until=END)  # to start the simulation .... rt_factor=0.1,
