import random
import mosaik
from mosaik.util import connect_many_to_one
from sys import platform


def connect_prosumer_to_grid(world, sim_data_entities, grid):
    data_prosumers = [e for e in sim_data_entities if e.type in (
        'Householdsim_Prosumer')]
    world.connect(data_prosumers[0], grid, ('power_generation_mW', 'p_mw_pv'),
                  ('power_consumption_mW', 'p_mw_load'))
    #prosumers = [e for e in grid if e.type in ('Prosumer')]
    #index = 0
    # for prosumer in prosumers:
    #    world.connect(data_prosumers[index], prosumer, ('power_generation_mW', 'p_mw_pv'),
    #                  ('power_consumption_mW', 'p_mw_load'))
    #    index += 1


sim_config = {
    'rust_sim': {
        'connect': '127.0.0.1:3456',
    },
    # 'Collector': {
    #    'cmd': 'python collector.py %(addr)s',
    # },
    'HouseholdSim': {
        # 'python': 'householdsim.mosaik:HouseholdSim',
        'cmd': 'python ../bsc_thesis_dang/householdsim/mosaik.py %(addr)s',
    },
    # 'PandaPower': {
    # 'python': 'pandapowermosaik:PandapowerMosaik',
    #    'cmd': '../bsc_thesis_dang/cosimulation_city_energy/pandapowermosaik.py %(addr)s',
    # },
}


START = '2016-11-21 00:00:00'  # TODO edit simulation time frame
END = 10 * 60  # 10 Min.
step_size = 15*60
DATABASE_PATH = r"../bsc_thesis_dang/cosimulation_city_energy/simulation_data/household_data_prepared.sqlite"
print("call Sim_Manager")
world = mosaik.World(sim_config)

# create_scenario(world)
rustAPI = world.start('rust_sim', eid_prefix='Model_')
#pandapower = world.start('PandaPower', step_size=step_size)
#collector = world.start('Collector', step_size=60)
hhsim = world.start('HouseholdSim', step_size=step_size)

# Instantiate models
model = rustAPI.ExampleModel(init_val=2)
sim_data_entities = hhsim.householdsim(
    num_of_consumer=0, num_of_PV=0, num_of_prosumer=1, data_base_path=DATABASE_PATH, start_time=START).children

connect_prosumer_to_grid(world, sim_data_entities, model)
#grid = pandapower.VorStadtNetz(num_of_PV=0, num_of_prosumer=14).children

#monitor = collector.Monitor()
# Create one instance of of our example model and one database instance

# Connect entities
world.connect(sim_data_entities, model,
              'power_consumption_mW', 'power_generation_mW')
# through the connection we tell mosaik to send the outputs of the example to the monitor

# Create more entities (you usually work with larger sets of entities)
# instead of instantiating the example model directly, we called its static method
# create() and passed the number of instances to it
#more_models = rustAPI.ExampleModel.create(2, init_val=3)

# Connects all entities to the database
#mosaik.util.connect_many_to_one(world, more_models, monitor, 'val', 'kw')


print('world run starting')

# Run simulation
world.run(until=END)  # to start the simulation .... rt_factor=0.1,
