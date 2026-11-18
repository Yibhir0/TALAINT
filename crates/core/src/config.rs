use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::{InitialSync, Intent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub intent: Intent,
    pub sources: Vec<SourceConfig>,
    #[serde(default)]
    pub processing: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSchedule {
    #[serde(default = "default_interval")]
    pub interval: String,
}

fn default_interval() -> String {
    "1m".to_string()
}

impl Default for SourceSchedule {
    fn default() -> Self {
        Self { interval: default_interval() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    pub r#ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StringOrSecretRef {
    String(String),
    Secret(SecretRef),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(default)]
    pub schedule: SourceSchedule,
    #[serde(default)]
    pub initial_sync: InitialSync,
    #[serde(default)]
    pub credentials: Option<SecretRef>,
    #[serde(default)]
    pub headers: BTreeMap<String, StringOrSecretRef>,
    #[serde(default)]
    pub query_params: BTreeMap<String, String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub record_path: Option<String>,
    #[serde(default)]
    pub flatten: bool,
    #[serde(default = "default_error_policy")]
    pub error_policy: String,
    /// Connector-specific fields (e.g. url, method, body, id_field, cursor_field, pagination)
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_error_policy() -> String {
    "fail".to_string()
}
