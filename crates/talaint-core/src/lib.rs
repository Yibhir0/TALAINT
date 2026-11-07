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

/// The crate version, surfaced by the CLI.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
