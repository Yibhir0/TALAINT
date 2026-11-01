//! TALAINT core domain.
//!
//! Current slice: HTTP sources only, and only the *configuration* skeleton.
//! There is no networking, no database, and no detection here yet.
//! Each piece will be added in its own phase.

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
    /// Parse a user-supplied format name. Returns `None` when unknown.
    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "csv" => Some(Self::Csv),
            "json" => Some(Self::Json),
            "jsonl" => Some(Self::JsonLines),
            "parquet" => Some(Self::Parquet),
            "xml" => Some(Self::Xml),
            _ => None,
        }
    }

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
    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "once" => Some(Self::Once),
            "hourly" => Some(Self::Hourly),
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            _ => None,
        }
    }

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
    pub fn parse(input: &str) -> Option<Self> {
        match input {
            "analytics" => Some(Self::Analytics),
            "rag" => Some(Self::Rag),
            "agent" => Some(Self::Agent),
            "search" => Some(Self::Search),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

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

/// What a tenant configures for one HTTP source.
///
/// This is the whole configuration for the current slice. Later we will add
/// tenant id, authentication, schedule, and the detected (observed) format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpSourceConfig {
    pub url: String,
    pub format: DataFormat,
    pub change_frequency: ChangeFrequency,
    pub purpose: Purpose,
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
}

/// The crate version, surfaced by the CLI.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_formats() {
        assert_eq!(DataFormat::parse("csv"), Some(DataFormat::Csv));
        assert_eq!(DataFormat::parse("jsonl"), Some(DataFormat::JsonLines));
        assert_eq!(DataFormat::parse("yaml"), None);
    }

    #[test]
    fn config_keeps_its_parts() {
        let config = HttpSourceConfig::new(
            "https://example.com/data.csv".into(),
            DataFormat::Csv,
            ChangeFrequency::Daily,
            Purpose::Rag,
        );

        assert_eq!(config.format.as_str(), "csv");
        assert_eq!(config.change_frequency.as_str(), "daily");
        assert_eq!(config.purpose.as_str(), "rag");
    }
}
