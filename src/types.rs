//! Mosaik types as defined in the [Mosaik API](https://gitlab.com/mosaik/api/mosaik-api-python/-/blob/3.0.9/mosaik_api_v3/types.py?ref_type=tags)

use std::collections::HashMap;

use serde_json::{Map, Value};

///Time is represented as the number of simulation steps since the
///simulation started. One step represents `time_resolution` seconds.
pub type Time = i64;

///An attribute name
pub type Attr = String;

///The name of a model.
pub type ModelName = String;

/*class ModelDescriptionOptionals(TypedDict, total=False):
    any_inputs: bool
    """Whether this model accepts inputs other than those specified in `attrs`."""
    trigger: Iterable[Attr]
    """The input attributes that trigger a step of the associated simulator.

    (Non-trigger attributes are collected and supplied to the simulator when it
    steps next.)"""
    persistent: Iterable[Attr]
    """The output attributes that are persistent."""

class ModelDescription(ModelDescriptionOptionals):
    """Description of a single model in `Meta`"""
    public: bool
    """Whether the model can be created directly."""
    params: List[str]
    """The parameters given during creating of this model."""
    attrs: List[Attr]
    """The input and output attributes of this model."""

class MetaOptionals(TypedDict, total=False):
    extra_methods: List[str]
    """The names of the extra methods this simulator supports."""

class Meta(MetaOptionals):
    """The meta-data for a simulator."""
    api_version: Literal["3.0"]
    """The API version that this simulator supports in the format "major.minor"."""
    type: Literal['time-based', 'event-based', 'hybrid']
    """The simulator's stepping type."""
    models: Dict[ModelName, ModelDescription]
    """The descriptions of this simulator's models."""
*/
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

/*class CreateResultOptionals(TypedDict, total=False):
    rel: List[EntityId]
    """The entity IDs of the entities of this simulator that are
    related to this entity."""
    children: List[CreateResult]
    """The child entities of this entity."""

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
