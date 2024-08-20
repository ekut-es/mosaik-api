//! Mosaik types as defined in the [Mosaik API](https://gitlab.com/mosaik/api/mosaik-api-python/-/blob/3.0.9/mosaik_api_v3/types.py?ref_type=tags)

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

///Time is represented as the number of simulation steps since the
///simulation started. One step represents `time_resolution` seconds.
/// All time-based or hybrid simulators start at time=0.
pub type Time = u64;

///An attribute name
pub type Attr = String;

///The name of a model.
pub type ModelName = String;

///A simulator ID
pub type SimId = String;

///An entity ID
pub type EntityId = String;

///A full ID of the form "`sim_id.entity_id`"
pub type FullId = String;

///The format of input data for simulator's step methods.
pub type InputData = HashMap<EntityId, HashMap<Attr, Map<FullId, Value>>>;

///The requested outputs for `get_data`. For each entity where data is
///needed, the required attributes are listed.
pub type OutputRequest = HashMap<EntityId, Vec<Attr>>;

///The compatible Mosaik version with this edition of the API
pub const API_VERSION: &str = "3.0";

///The format of output data as return by ``get_data``
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputData {
    #[serde(flatten)]
    pub requests: HashMap<EntityId, HashMap<Attr, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Time>,
}

/// Description of a single model in `Meta`
///
/// ## Example implementation
/// ```rust
/// use mosaik_rust_api::types::ModelDescription;
///
/// const foo: ModelDescription = ModelDescription {
///     public: true,
///     params: &["init_val"],
///     attrs: &["delta", "val"],
///     trigger: Some(&["delta"]),
///     any_inputs: None,
///     persistent: None,
/// };
/// ```
#[derive(Debug, Serialize, PartialEq, Clone, Default)]
pub struct ModelDescription {
    /// Whether the model can be created directly.
    pub public: bool,
    /// The parameters given during creating of this model.
    pub params: &'static [&'static str],
    /// The input and output attributes of this model.
    pub attrs: &'static [&'static str],
    /// Whether this model accepts inputs other than those specified in `attrs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any_inputs: Option<bool>,
    /// The input attributes that trigger a step of the associated simulator.
    /// (Non-trigger attributes are collected and supplied to the simulator when it
    /// steps next.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<&'static [&'static str]>,
    /// The output attributes that are persistent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<&'static [&'static str]>,
}

impl ModelDescription {
    /// Creates a new `ModelDescription` with fields `any_inputs`, `trigger` and `persistent` set to `None`.
    pub fn new(
        public: bool,
        params: &'static [&'static str],
        attrs: &'static [&'static str],
    ) -> Self {
        Self {
            public,
            params,
            attrs,
            any_inputs: None,
            trigger: None,
            persistent: None,
        }
    }
}

/// The meta-data for a simulator.
#[derive(Debug, Serialize, PartialEq, Clone)]
pub struct Meta {
    /// The API version that this simulator supports in the format "major.minor".
    api_version: &'static str,
    /// The simulator's stepping type.
    #[serde(rename = "type")]
    pub simulator_type: SimulatorType,
    /// The descriptions of this simulator's models.
    pub models: HashMap<ModelName, ModelDescription>,
    /// The names of the extra methods this simulator supports.
    ///
    /// # Note
    /// > These methods can be called while the scenario is being created and can be used
    /// > for operations that don’t really belong into init() or create().
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_methods: Option<Vec<String>>,
}

impl Meta {
    pub fn new(
        simulator_type: SimulatorType,
        models: HashMap<ModelName, ModelDescription>,
        extra_methods: Option<Vec<String>>,
    ) -> Self {
        Self {
            api_version: API_VERSION,
            simulator_type,
            models,
            extra_methods,
        }
    }

    pub fn get_version(&self) -> &str {
        self.api_version
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self {
            api_version: API_VERSION,
            simulator_type: SimulatorType::default(),
            models: HashMap::new(),
            extra_methods: None,
        }
    }
}

/// The three types of simulators. With `Hybrid` being the default.
/// - `TimeBased`: start at time 0, return the next step time after each step, and produce data valid for \([t_{now}, t_{next})\).
/// - `EventBased`: start whenever their first event is scheduled, step at event times, can schedule their own events, and produce output valid at specific times.
/// - `Hybrid`: a mix of the two. Also starts at time 0.
#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum SimulatorType {
    TimeBased,
    EventBased,
    #[default]
    Hybrid,
}

/// The type for elements of the list returned by `create` calls in the mosaik API.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CreateResult {
    /// The entity ID of this entity.
    pub eid: EntityId,
    /// The model name (as given in the simulator's meta) of this entity.
    #[serde(rename = "type")]
    pub model_type: ModelName,
    /// The entity IDs of the entities of this simulator that are related to this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel: Option<Vec<EntityId>>,
    /// The child entities of this entity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<CreateResult>>,
    /// Any additional information about the entity that the simulator wants to pass back to the scenario.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_info: Option<HashMap<String, String>>,
}

impl CreateResult {
    pub fn new(eid: EntityId, model_type: ModelName) -> Self {
        Self {
            eid,
            model_type,
            rel: None,
            children: None,
            extra_info: None,
        }
    }
}

/*
// The below types are copied from the python implementation.
// pub type CreateResultChild = CreateResult;

class EntitySpec(TypedDict):
    type: ModelName

class EntityGraph(TypedDict):
    nodes: Dict[FullId, EntitySpec]
    edges: List[Tuple[FullId, FullId, Dict]]
*/

// tests for Meta
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_output_data() {
        // Example JSON data
        let json_data = r#"{
        "eid_1": {"attr_1": "val_1"},
        "time": 64
    }
    "#
        .replace(['\n', ' '], "");

        // Deserialize JSON to OutputData struct
        let data: OutputData = serde_json::from_str(&json_data).unwrap();
        assert_ne!(data.requests, HashMap::new());
        assert_eq!(data.time, Some(64));

        // Serialize EventData struct to JSON
        let serialized_json = serde_json::to_string(&data).unwrap();
        assert!(!serialized_json.contains("requests"));
        assert!(serialized_json.contains("time"));

        assert_eq!(serialized_json, json_data);
    }

    #[test]
    fn test_model_description_without_optionals() {
        let mut model = ModelDescription::default();

        assert!(!model.public);
        assert_eq!(model.params.len(), 0);
        assert_eq!(model.attrs.len(), 0);
        assert_eq!(model.any_inputs, None);
        assert_eq!(model.trigger, None);
        assert_eq!(model.persistent, None);

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(r#"{"public":false,"params":[],"attrs":[]}"#, model_json);

        model.public = true;
        model.params = &["init_reading"];
        model.attrs = &["trades", "total"];

        assert!(model.public);
        assert_eq!(model.params.len(), 1);
        assert_eq!(model.attrs.len(), 2);

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            r#"{"public":true,"params":["init_reading"],"attrs":["trades","total"]}"#,
            model_json
        );
    }

    #[test]
    fn test_model_description_with_optionals() {
        let mut model = ModelDescription::new(true, &["init_reading"], &["p_mw_pv", "p_mw_load"]);
        model.any_inputs = Some(true);
        model.trigger = Some(&["trigger1"]);
        model.persistent = Some(&["trades"]);

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            r#"{"public":true,"params":["init_reading"],"attrs":["p_mw_pv","p_mw_load"],"any_inputs":true,"trigger":["trigger1"],"persistent":["trades"]}"#,
            model_json
        );

        model.trigger = Some(&["trigger1"]);
        model.any_inputs = None;
        model.persistent = None;

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            r#"{"public":true,"params":["init_reading"],"attrs":["p_mw_pv","p_mw_load"],"trigger":["trigger1"]}"#,
            model_json
        );
    }

    #[test]
    fn test_meta_empty() {
        let meta = Meta::new(SimulatorType::default(), HashMap::new(), None);
        assert_eq!(
            meta.api_version, API_VERSION,
            "API version should match the global variable."
        );
        assert_eq!(
            meta.get_version(),
            API_VERSION,
            "get_version should return the API version."
        );
        assert_eq!(
            meta.simulator_type,
            SimulatorType::Hybrid,
            "Default type should be Hybrid"
        );

        let empty_meta_json = serde_json::to_string(&meta).unwrap();
        assert_eq!(
            r#"{"api_version":"3.0","type":"hybrid","models":{}}"#, empty_meta_json,
            "Empty meta should not have extra_methods and empty models."
        );
        assert!(meta.models.is_empty());
    }

    #[test]
    fn test_meta_with_models() {
        let model1 = ModelDescription::new(true, &["init_reading"], &["trades", "total"]);
        let meta = Meta::new(
            SimulatorType::default(),
            HashMap::from([("MarktplatzModel".to_string(), model1)]),
            None,
        );
        assert_eq!(meta.models.len(), 1, "Should have one model");

        assert!(meta.extra_methods.is_none());
        let meta_json = serde_json::to_string(&meta).unwrap();
        assert_eq!(
            r#"{"api_version":"3.0","type":"hybrid","models":{"MarktplatzModel":{"public":true,"params":["init_reading"],"attrs":["trades","total"]}}}"#,
            meta_json,
            "Meta should have one model and no extra methods."
        );
    }

    #[test]
    fn test_meta_optionals() {
        let meta = Meta::new(
            SimulatorType::default(),
            HashMap::new(),
            Some(vec!["foo".to_string(), "bar".to_string()]),
        );

        assert_eq!(
            meta.extra_methods.as_ref().unwrap().len(),
            2,
            "Should have 2 extra methods."
        );

        let meta_json = serde_json::to_string(&meta).unwrap();
        assert_eq!(
            r#"{"api_version":"3.0","type":"hybrid","models":{},"extra_methods":["foo","bar"]}"#,
            meta_json,
            "JSON String should contain 'foo' and 'bar' as extra methods."
        );
    }

    #[test]
    fn test_create_result_new() {
        let create_result = CreateResult::new(String::from("eid_1"), String::from("model_name"));
        assert_eq!(create_result.eid, "eid_1");
        assert_eq!(create_result.model_type, "model_name");
        assert!(create_result.rel.is_none());
        assert!(create_result.children.is_none());
        assert!(create_result.extra_info.is_none());

        let create_result_json = serde_json::to_string(&create_result).unwrap();
        assert_eq!(
            r#"{"eid":"eid_1","type":"model_name"}"#, create_result_json,
            "New CreateResult should not contain any optional fields"
        );
    }

    #[test]
    fn test_create_results_filled() {
        let mut create_result = CreateResult::new("eid_1".to_string(), "model_name".to_string());

        create_result.rel = Some(vec!["eid_2".to_string()]);
        create_result.children = Some(vec![CreateResult::new(
            "child_1".to_string(),
            "child".to_string(),
        )]);

        assert_eq!(create_result.eid, "eid_1");
        assert_eq!(create_result.model_type, "model_name");
        assert_eq!(create_result.rel, Some(vec!["eid_2".to_string()]));
        assert!(create_result.children.is_some());
        if let Some(children) = &create_result.children {
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].eid, "child_1");
        }

        let create_result_json = serde_json::to_string(&create_result).unwrap();
        assert_eq!(
            r#"{"eid":"eid_1","type":"model_name","rel":["eid_2"],"children":[{"eid":"child_1","type":"child"}]}"#,
            create_result_json,
            "Filled create result should contain optional fields without extra_info"
        );
    }
}
