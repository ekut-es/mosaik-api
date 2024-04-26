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

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelDescriptionOptionals {
    // Whether this model accepts inputs other than those specified in `attrs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub any_inputs: Option<bool>,
    // The input attributes that trigger a step of the associated simulator.
    // (Non-trigger attributes are collected and supplied to the simulator when it
    // steps next.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Vec<Attr>>,
    // The output attributes that are persistent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistent: Option<Vec<Attr>>,
}

/// Description of a single model in `Meta`
#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
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

/// The meta-data for a simulator.
#[derive(Debug, Serialize, Deserialize, PartialEq, Default)]
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

#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SimulatorType {
    TimeBased,
    EventBased,
    #[default]
    Hybrid,
}

// class CreateResultOptionals(TypedDict, total=False):
#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct CreateResultOptionals {
    /// The entity IDs of the entities of this simulator that are related to this entity.
    pub rel: Option<Vec<EntityId>>,
    /// The child entities of this entity.
    pub children: Option<Vec<CreateResult>>,
    /// Any additional information about the entity that the simulator wants to pass back to the scenario.
    pub extra_info: Option<HashMap<String, String>>,
}

/// The type for elements of the list returned by `create` calls in the mosaik API."""
#[derive(Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct CreateResult {
    /// The entity ID of this entity.
    pub eid: EntityId,
    /// The model name (as given in the simulator's meta) of this entity.
    pub r#type: ModelName,

    pub optionals: Option<CreateResultOptionals>,
}

pub type CreateResultChild = CreateResult;

/*
// The below types are copied from the python implementation.

class EntitySpec(TypedDict):
    type: ModelName

class EntityGraph(TypedDict):
    nodes: Dict[FullId, EntitySpec]
    edges: List[Tuple[FullId, FullId, Dict]]
*/

// tests for Meta
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_description_without_optionals() {
        let mut model = ModelDescription::default();

        assert_eq!(model.public, false);
        assert_eq!(model.params.len(), 0);
        assert_eq!(model.attrs.len(), 0);
        assert_eq!(model.optionals, None);

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(r#"{"public":false,"params":[],"attrs":[]}"#, model_json);

        model.public = true;
        model.params.push("init_reading".to_string());
        model.attrs.push("trades".to_string());
        model.attrs.push("total".to_string());

        assert_eq!(model.public, true);
        assert_eq!(model.params.len(), 1);
        assert_eq!(model.attrs.len(), 2);
        assert_eq!(model.optionals, None);

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            r#"{"public":true,"params":["init_reading"],"attrs":["trades","total"]}"#,
            model_json
        )
    }

    #[test]
    fn test_model_description_with_optionals() {
        let mut model = ModelDescription::default();
        model.public = true;
        model.params.push("init_reading".to_string());
        model.attrs.push("p_mw_pv".to_string());
        model.attrs.push("p_mw_load".to_string());
        model.optionals = Some(ModelDescriptionOptionals {
            any_inputs: Some(true),
            trigger: Some(vec!["trigger1".to_string()]),
            persistent: Some(vec!["trades".to_string()]),
        });

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            r#"{"public":true,"params":["init_reading"],"attrs":["p_mw_pv","p_mw_load"],"any_inputs":true,"trigger":["trigger1"],"persistent":["trades"]}"#,
            model_json
        );

        model.optionals = Some(ModelDescriptionOptionals {
            any_inputs: None,
            trigger: Some(vec!["trigger1".to_string()]),
            persistent: None,
        });

        let model_json = serde_json::to_string(&model).unwrap();
        assert_eq!(
            r#"{"public":true,"params":["init_reading"],"attrs":["p_mw_pv","p_mw_load"],"trigger":["trigger1"]}"#,
            model_json
        )
    }

    #[test]
    fn test_meta() {
        let mut meta = Meta::default();
        meta.api_version = "3.0";
        let model1 = ModelDescription {
            public: true,
            params: vec!["init_reading".to_string()],
            attrs: vec![
                "p_mw_pv".to_string(),
                "p_mw_load".to_string(),
                "reading".to_string(),
                "trades".to_string(),
                "total".to_string(),
            ],

            ..Default::default()
        };
        meta.models.insert("MarktplatzModel".to_string(), model1);

        assert_eq!(meta.api_version, "3.0");
        assert_eq!(meta.type_, SimulatorType::Hybrid);
        assert_eq!(meta.models.len(), 1);

        let meta_json = serde_json::to_string(&meta).unwrap();
        assert_eq!("{\"api_version\":\"3.0\",\"type\":\"hybrid\",\"models\":{\"MarktplatzModel\":{\"public\":true,\"params\":[\"init_reading\"],\"attrs\":[\"p_mw_pv\",\"p_mw_load\",\"reading\",\"trades\",\"total\"]}}}", meta_json)
    }

    #[test]
    fn test_meta_optionals() {
        let mut meta = Meta::default();
        meta.api_version = "3.0";
        let model1 = ModelDescription {
            public: true,
            params: vec!["init_reading".to_string()],
            attrs: vec!["p_mw_pv".to_string(), "p_mw_load".to_string()],
            optionals: Some(ModelDescriptionOptionals {
                any_inputs: Some(true),
                trigger: Some(vec!["trigger1".to_string()]),
                persistent: Some(vec!["trades".to_string()]),
            }),
            ..Default::default()
        };
        meta.models.insert("MarktplatzModel".to_string(), model1);
        meta.extra_methods = Some(vec!["foo".to_string()]);

        let meta_json = serde_json::to_string(&meta).unwrap();
        assert_eq!(
            r#"{"api_version":"3.0","type":"hybrid","models":{"MarktplatzModel":{"public":true,"params":["init_reading"],"attrs":["p_mw_pv","p_mw_load"],"any_inputs":true,"trigger":["trigger1"],"persistent":["trades"]}},"extra_methods":["foo"]}"#,
            meta_json
        )
    }
}
