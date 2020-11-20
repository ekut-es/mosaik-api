import random
import mosaik
from mosaik.util import connect_many_to_one
from sys import platform


sim_config = {
    'rust_sim':{
        #'cmd': 'cargo run %(addr)s', #cd ../../mosaik-rust-api && 
        #'env': '../../mosaik-rust-api',
        'connect': '127.0.0.1:3030',
    },
}

END = 100   # 100 s

print("call Sim_Manager")
world = mosaik.World(sim_config)
#create_scenario(world)
rustAPI = world.start('rust_sim')
world.run(until=END)  # As fast as possible


#pandapower = world.start('PandaPower', step_size=step_size)
    

