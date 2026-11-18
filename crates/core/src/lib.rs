pub mod config;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    Rag,
    FineTuning,
    MlTraining,
    Multimodal,
}

impl Intent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Intent::Rag => "rag",
            Intent::FineTuning => "fine_tuning",
            Intent::MlTraining => "ml_training",
            Intent::Multimodal => "multimodal",
        }
    }

    pub fn dataset_name(&self) -> &'static str {
        match self {
            Intent::Rag => "rag_documents",
            Intent::FineTuning => "finetune_datasets",
            Intent::MlTraining => "tabular_features",
            Intent::Multimodal => "multimodal_assets",
        }
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Intent {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rag" => Ok(Intent::Rag),
            "fine_tuning" => Ok(Intent::FineTuning),
            "ml_training" => Ok(Intent::MlTraining),
            "multimodal" => Ok(Intent::Multimodal),
            _ => Err(format!("unknown intent: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Running => "running",
            JobStatus::Succeeded => "succeeded",
            JobStatus::Failed => "failed",
            JobStatus::Skipped => "skipped",
        }
    }
}

impl fmt::Display for JobStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for JobStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(JobStatus::Queued),
            "running" => Ok(JobStatus::Running),
            "succeeded" => Ok(JobStatus::Succeeded),
            "failed" => Ok(JobStatus::Failed),
            "skipped" => Ok(JobStatus::Skipped),
            _ => Err(format!("unknown job status: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialSync {
    Full,
    Latest,
}

impl Default for InitialSync {
    fn default() -> Self {
        InitialSync::Full
    }
}

impl InitialSync {
    pub fn as_str(&self) -> &'static str {
        match self {
            InitialSync::Full => "full",
            InitialSync::Latest => "latest",
        }
    }
}

impl FromStr for InitialSync {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full" => Ok(InitialSync::Full),
            "latest" => Ok(InitialSync::Latest),
            _ => Err(format!("unknown initial_sync: {s}")),
        }
    }
}
