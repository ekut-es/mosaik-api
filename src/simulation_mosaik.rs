
///Current Python code needs to get converted to rust for testing purposes.



/* # simulator_mosaik.py
"""Mosaik interface for the example simulator"""
import mosaik_api # contains the method start_simulation() which creates a socket, connects to mosaik and listens for requests from it
import simulator

# simulator meta data
# tells mosaik which models our simulator implements and which parameters and attributes it has
META = {
    'models':{
        'ExampleModel':{
            'public' : True,
            'params': ['init_val'], # added model
            'attrs': ['delta', 'val'], # with attributes delta and val
        },
    },
}

class ExampleSim(mosaik_api.Simulator):
    def __init__(self):
        super().__init__(META)
        self.simulator = simulator.Simulator()
        self.eid_prefix = 'Model_'
        self.entities = {} #Maps entitiy IDs to model indicies in self.simulator
    # Four API calls: init, create, step and get_data

# called once, after the simulator has been started
# used for additional initializiation tasks (eg. parameters)
# must return the meta
    def init(self, sid, eid_prefix=None):
        if eid_prefix is not None:
            self.eid_prefix = eid_prefix
        return self.meta

# called to initialize a number of simulation entities
# must return a list with information about each enitity created
    def create(self, num, model, init_val):
        next_eid = len(self.entities)
        entities = []

        for i in range(next_eid, next_eid + num): # each entity gets a new ID and a model instance
            eid = '%s%d' % (self.eid_prefix, i) 
            self.simulator.add_model(init_val)
            self.entities[eid] = i # mapping from EID to our model (i think)
            entities.append({'eid': eid, 'type': model})
        return entities


# perform simulation step
# returns time at which it wants to its next step
# recieves current simulation time and a dictionary with input values
    def step(self, time, inputs):
        # Get inputs
        deltas = {}
        for eid, attrs in inputs.items():
            for attr, values in attrs.items():
                model_idx = self.entities[eid]
                new_delta = sum(values.values())
                deltas[model_idx] = new_delta
        
        # Perform simulation step
        self.simulator.step(deltas)

        return time + 60 # Step size is 1 minute


# allows to get the values of the delta and val attributes of our models
    def get_data(self, outputs):
        models = self.simulator.models
        data = {}
        for eid, attrs in outputs.items():
            model_idx = self.entities[eid]
            data[eid] = {}
            for attr in attrs:
                if attr not in self.meta['models']['ExampleModel']['attrs']:
                    raise ValueError('Unknown output attribute: %s' % attr)

                # Get model.val or model.delta:
                data[eid][attr] = getattr(models[model_idx], attr)

        return data

def main():
    return mosaik_api.start_simulation(ExampleSim())# call start_simulation with simulator class


if __name__ == '__main__':
    main()*/