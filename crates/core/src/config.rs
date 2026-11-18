use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::{InitialSync, Intent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub intent: Intent,
    pub sources: Vec<SourceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processing: Option<Value>,
}

impl PipelineConfig {
    pub fn from_yaml_str(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    pub fn to_yaml_string(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if self.sources.is_empty() {
            errors.push("sources: must contain at least one source".to_string());
        }
        let mut seen = BTreeSet::new();
        for (i, s) in self.sources.iter().enumerate() {
            let prefix = format!(
                "sources[{i}] ({})",
                if s.name.is_empty() {
                    "<unnamed>"
                } else {
                    &s.name
                }
            );
            if s.name.trim().is_empty() {
                errors.push(format!("{prefix}: name is required"));
            } else if !seen.insert(s.name.clone()) {
                errors.push(format!("{prefix}: duplicate source name '{}'", s.name));
            }
            if s.source_type.trim().is_empty() {
                errors.push(format!("{prefix}: type is required"));
            }
            if !is_valid_interval(&s.schedule.interval) {
                errors.push(format!(
                    "{prefix}: schedule.interval '{}' is invalid (use durations like 30s, 1m, 5m, 1h)",
                    s.schedule.interval
                ));
            }
            if s.error_policy != "fail" {
                errors.push(format!(
                    "{prefix}: error_policy '{}' is not supported in v1 (only 'fail')",
                    s.error_policy
                ));
            }
            if s.source_type == "http_json" {
                let url = s
                    .extra
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if url.is_empty() {
                    errors.push(format!("{prefix}: url is required for type 'http_json'"));
                }
            }
            // collect secret refs validity (non-empty)
            if let Some(c) = &s.credentials {
                if c.r#ref.trim().is_empty() {
                    errors.push(format!("{prefix}: credentials.ref must not be empty"));
                }
            }
            for (k, v) in &s.headers {
                if let StringOrSecretRef::Secret(r) = v {
                    if r.r#ref.trim().is_empty() {
                        errors.push(format!("{prefix}: headers.{k}.ref must not be empty"));
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn collect_secret_refs(&self) -> BTreeSet<String> {
        let mut refs = BTreeSet::new();
        for s in &self.sources {
            if let Some(c) = &s.credentials {
                if !c.r#ref.trim().is_empty() {
                    refs.insert(c.r#ref.clone());
                }
            }
            for v in s.headers.values() {
                if let StringOrSecretRef::Secret(r) = v {
                    if !r.r#ref.trim().is_empty() {
                        refs.insert(r.r#ref.clone());
                    }
                }
            }
        }
        refs
    }
}

pub fn is_valid_interval(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 2 {
        return false;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    matches!(unit, "s" | "m" | "h")
        && !num.is_empty()
        && num.chars().all(|c| c.is_ascii_digit())
        && num != "0"
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
        Self {
            interval: default_interval(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    pub r#ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credentials: Option<SecretRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub headers: BTreeMap<String, StringOrSecretRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub query_params: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_path: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flatten: bool,
    #[serde(
        default = "default_error_policy",
        skip_serializing_if = "is_default_error_policy"
    )]
    pub error_policy: String,
    /// Connector-specific fields (e.g. url, method, body, id_field, cursor_field, pagination)
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_error_policy() -> String {
    "fail".to_string()
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn is_default_error_policy(value: &str) -> bool {
    value == "fail"
}

impl SourceConfig {
    pub fn http_url(&self) -> Option<&str> {
        self.extra.get("url").and_then(|v| v.as_str())
    }

    /// JSON config persisted in DB (excludes name/type/schedule/initial_sync columns stored separately)
    pub fn to_db_config_value(&self) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some(c) = &self.credentials {
            obj.insert(
                "credentials".into(),
                serde_json::to_value(c).unwrap_or(Value::Null),
            );
        }
        if !self.headers.is_empty() {
            obj.insert(
                "headers".into(),
                serde_json::to_value(&self.headers).unwrap_or(Value::Null),
            );
        }
        if !self.query_params.is_empty() {
            obj.insert(
                "query_params".into(),
                serde_json::to_value(&self.query_params).unwrap_or(Value::Null),
            );
        }
        if let Some(f) = &self.format {
            obj.insert("format".into(), Value::String(f.clone()));
        }
        if let Some(rp) = &self.record_path {
            obj.insert("record_path".into(), Value::String(rp.clone()));
        }
        if self.flatten {
            obj.insert("flatten".into(), Value::Bool(true));
        }
        if self.error_policy != "fail" {
            obj.insert(
                "error_policy".into(),
                Value::String(self.error_policy.clone()),
            );
        }
        for (k, v) in &self.extra {
            obj.insert(k.clone(), v.clone());
        }
        Value::Object(obj)
    }
}
