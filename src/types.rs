//! Mosaik types as defined in the [Mosaik API](https://gitlab.com/mosaik/api/mosaik-api-python/-/blob/3.0.9/mosaik_api_v3/types.py?ref_type=tags)

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
///Time is represented as the number of simulation steps since the
///simulation started. One step represents `time_resolution` seconds.
pub type Time = i64;

///An attribute name
pub type Attr = String;

///The name of a model.
pub type ModelName = String;

///A simulator ID
pub type SimId = String;

///An entity ID
pub type EntityId = String;

///A full ID of the form "sim_id.entity_id"
pub type FullId = String;

///The format of input data for simulator's step methods.
pub type InputData = HashMap<EntityId, HashMap<Attr, Map<FullId, Value>>>;

///The requested outputs for get_data. For each entity where data is
///needed, the required attributes are listed.
pub type OutputRequest = HashMap<EntityId, Vec<Attr>>;

///The format of output data as return by ``get_data``
pub type OutputData = HashMap<EntityId, HashMap<Attr, Value>>;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModelDescriptionOptionals {
    // Whether this model accepts inputs other than those specified in `attrs`.
    pub any_inputs: Option<bool>,
    // The input attributes that trigger a step of the associated simulator.
    // (Non-trigger attributes are collected and supplied to the simulator when it
    // steps next.)
    pub trigger: Option<Vec<Attr>>,
    // The output attributes that are persistent.
    pub persistent: Option<Vec<Attr>>,
}

// Description of a single model in `Meta`
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ModelDescription {
    // Whether the model can be created directly.
    pub public: bool,
    // The parameters given during creating of this model.
    pub params: Vec<String>,
    // The input and output attributes of this model.
    pub attrs: Vec<Attr>,
    #[serde(flatten)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optionals: Option<ModelDescriptionOptionals>,
}

// The meta-data for a simulator.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Meta {
    // The API version that this simulator supports in the format "major.minor".
    pub api_version: &'static str,
    // The simulator's stepping type.
    #[serde(rename = "type")]
    pub type_: SimulatorType,
    // The descriptions of this simulator's models.
    pub models: HashMap<ModelName, ModelDescription>,
    // The names of the extra methods this simulator supports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_methods: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SimulatorType {
    TimeBased,
    EventBased,
    #[default]
    Hybrid,
}

// The below types are copied from the python implementation.
// Not yet implemented in rust, mostly due to complex JSON handling.

/*class CreateResultOptionals(TypedDict, total=False):
    rel: List[EntityId]
    """The entity IDs of the entities of this simulator that are
    related to this entity."""
    children: List[CreateResult]
    """The child entities of this entity."""
    extra_info: Any
    """Any additional information about the entity that the simulator
    wants to pass back to the scenario.
    """


class CreateResult(CreateResultOptionals):
    """The type for elements of the list returned by `create` calls in
    the mosaik API."""
    eid: EntityId
    """The entity ID of this entity."""
    type: ModelName
    """The model name (as given in the simulator's meta) of this entity.
    """

pub type CreateResultChild = CreateResult;

class EntitySpec(TypedDict):
    type: ModelName

class EntityGraph(TypedDict):
    nodes: Dict[FullId, EntitySpec]
    edges: List[Tuple[FullId, FullId, Dict]]*/
