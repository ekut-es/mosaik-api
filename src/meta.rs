use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaStruct {
    pub api_version: f32,
    #[serde(rename = "type")]
    pub advance_type: AdvanceType,
    pub models: Map<String, Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra_methods: Vec<String>,
}

impl Default for MetaStruct {
    fn default() -> Self {
        MetaStruct {
            api_version: 3.0,
            advance_type: AdvanceType::TimeBased,
            models: Map::new(),
            extra_methods: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub enum AdvanceType {
    #[serde(rename = "time-based")]
    TimeBased,
    #[serde(rename = "event-based")]
    EventBased,
    #[serde(rename = "hybrid")]
    #[default]
    Hybrid,
}   

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelMeta {
    pub public: bool,
    pub params: Vec<String>,
    pub attrs: Vec<String>,
    pub any_inputs: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<Vec<String>>,
    #[serde(rename = "non-persistent", skip_serializing_if = "Option::is_none")]
    pub non_persistent: Option<Vec<String>>,
}

impl Default for ModelMeta {
    fn default() -> Self {
        ModelMeta {
            public: true,
            params: Vec::new(),
            attrs: Vec::new(),
            any_inputs: false,
            trigger: None,
            non_persistent: None,
        }
    }
}
