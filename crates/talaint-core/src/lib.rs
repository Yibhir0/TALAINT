//! TALAINT core domain.
//!
//! Current slice: HTTP sources only, and only the *configuration* skeleton.
//! There is no networking, no database, and no detection here yet.
//! Each piece will be added in its own phase.

use std::fmt;
use std::str::FromStr;

/// Errors produced while building or merging configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    UnknownDataFormat(String),
    UnknownChangeFrequency(String),
    UnknownPurpose(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDataFormat(value) => write!(f, "unknown format: {value}"),
            Self::UnknownChangeFrequency(value) => {
                write!(f, "unknown change frequency: {value}")
            }
            Self::UnknownPurpose(value) => write!(f, "unknown purpose: {value}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// The format of the data we expect to receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataFormat {
    Csv,
    Json,
    JsonLines,
    Parquet,
    Xml,
}

impl DataFormat {
    /// The canonical name, used in output and (later) in the database.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Json => "json",
            Self::JsonLines => "jsonl",
            Self::Parquet => "parquet",
            Self::Xml => "xml",
        }
    }
}

impl FromStr for DataFormat {
    type Err = ConfigError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "csv" => Ok(Self::Csv),
            "json" => Ok(Self::Json),
            "jsonl" => Ok(Self::JsonLines),
            "parquet" => Ok(Self::Parquet),
            "xml" => Ok(Self::Xml),
            _ => Err(ConfigError::UnknownDataFormat(input.to_string())),
        }
    }
}

/// How often the source data changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeFrequency {
    Once,
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl ChangeFrequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Once => "once",
            Self::Hourly => "hourly",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }
}

impl FromStr for ChangeFrequency {
    type Err = ConfigError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "once" => Ok(Self::Once),
            "hourly" => Ok(Self::Hourly),
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "monthly" => Ok(Self::Monthly),
            _ => Err(ConfigError::UnknownChangeFrequency(input.to_string())),
        }
    }
}

/// What the data will be used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Analytics,
    Rag,
    Agent,
    Search,
    Other,
}

impl Purpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Analytics => "analytics",
            Self::Rag => "rag",
            Self::Agent => "agent",
            Self::Search => "search",
            Self::Other => "other",
        }
    }
}

impl FromStr for Purpose {
    type Err = ConfigError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "analytics" => Ok(Self::Analytics),
            "rag" => Ok(Self::Rag),
            "agent" => Ok(Self::Agent),
            "search" => Ok(Self::Search),
            "other" => Ok(Self::Other),
            _ => Err(ConfigError::UnknownPurpose(input.to_string())),
        }
    }
}

/// What a tenant declares for one HTTP source.
///
/// This is the tenant's intent. It is never silently overwritten by what we
/// observe; disagreements are surfaced through [`Conflict`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSourceConfig {
    pub url: String,
    pub format: DataFormat,
    pub change_frequency: ChangeFrequency,
    pub purpose: Purpose,
}

/// What we actually observed when we fetched the source.
///
/// Every field is optional because detection can fail or be partial.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObservedConfig {
    pub url: Option<String>,
    pub detected_format: Option<DataFormat>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub size_bytes: Option<u64>,
    pub checksum: Option<String>,
}

/// A place where the declared and observed values disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub field: &'static str,
    pub declared: String,
    pub observed: String,
}

/// The configuration we actually use: declared values win, observation is kept
/// alongside them for later decisions (detection policy, audit, UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub url: String,
    pub format: DataFormat,
    pub change_frequency: ChangeFrequency,
    pub purpose: Purpose,
    pub observed: ObservedConfig,
}

impl HttpSourceConfig {
    pub fn new(
        url: String,
        format: DataFormat,
        change_frequency: ChangeFrequency,
        purpose: Purpose,
    ) -> Self {
        Self {
            url,
            format,
            change_frequency,
            purpose,
        }
    }

    /// Merge what the tenant declared with what we observed.
    ///
    /// Declared values always win. Any disagreement is recorded as a
    /// [`Conflict`] so the caller can surface it (log, UI, audit) without
    /// silently changing the tenant's intent.
    pub fn merge(&self, observed: &ObservedConfig) -> (EffectiveConfig, Vec<Conflict>) {
        let mut conflicts = Vec::new();

        if let Some(observed_url) = &observed.url {
            if observed_url != &self.url {
                conflicts.push(Conflict {
                    field: "url",
                    declared: self.url.clone(),
                    observed: observed_url.clone(),
                });
            }
        }

        if let Some(detected) = observed.detected_format {
            if detected != self.format {
                conflicts.push(Conflict {
                    field: "format",
                    declared: self.format.as_str().to_string(),
                    observed: detected.as_str().to_string(),
                });
            }
        }

        let effective = EffectiveConfig {
            url: self.url.clone(),
            format: self.format,
            change_frequency: self.change_frequency,
            purpose: self.purpose,
            observed: observed.clone(),
        };

        (effective, conflicts)
    }
}

/// The crate version, surfaced by the CLI.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> HttpSourceConfig {
        HttpSourceConfig::new(
            "https://example.com/data.csv".into(),
            DataFormat::Csv,
            ChangeFrequency::Daily,
            Purpose::Rag,
        )
    }

    #[test]
    fn parses_known_values() {
        assert_eq!("csv".parse::<DataFormat>(), Ok(DataFormat::Csv));
        assert_eq!("JSONL".parse::<DataFormat>(), Ok(DataFormat::JsonLines));
        assert_eq!(
            "daily".parse::<ChangeFrequency>(),
            Ok(ChangeFrequency::Daily)
        );
        assert_eq!("rag".parse::<Purpose>(), Ok(Purpose::Rag));
    }

    #[test]
    fn rejects_unknown_format() {
        let error = "yaml".parse::<DataFormat>().unwrap_err();

        assert_eq!(error, ConfigError::UnknownDataFormat("yaml".into()));
        assert_eq!(error.to_string(), "unknown format: yaml");
    }

    #[test]
    fn merge_without_observation_keeps_declared_values() {
        let (effective, conflicts) = declared().merge(&ObservedConfig::default());

        assert_eq!(effective.url, "https://example.com/data.csv");
        assert_eq!(effective.format, DataFormat::Csv);
        assert_eq!(effective.change_frequency, ChangeFrequency::Daily);
        assert_eq!(effective.purpose, Purpose::Rag);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn merge_records_format_conflict_and_keeps_declared() {
        let observed = ObservedConfig {
            detected_format: Some(DataFormat::Json),
            ..ObservedConfig::default()
        };

        let (effective, conflicts) = declared().merge(&observed);

        assert_eq!(effective.format, DataFormat::Csv, "declared value wins");
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "format");
        assert_eq!(conflicts[0].declared, "csv");
        assert_eq!(conflicts[0].observed, "json");
    }

    #[test]
    fn merge_records_url_conflict_after_redirect() {
        let observed = ObservedConfig {
            url: Some("https://cdn.example.com/data.csv".into()),
            ..ObservedConfig::default()
        };

        let (_, conflicts) = declared().merge(&observed);

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "url");
    }

    #[test]
    fn merge_keeps_observed_metadata() {
        let observed = ObservedConfig {
            etag: Some("abc".into()),
            size_bytes: Some(1024),
            ..ObservedConfig::default()
        };

        let (effective, _) = declared().merge(&observed);

        assert_eq!(effective.observed.etag.as_deref(), Some("abc"));
        assert_eq!(effective.observed.size_bytes, Some(1024));
    }
}
