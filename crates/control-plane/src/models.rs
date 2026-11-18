use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Tenant {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PipelineRow {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub intent: String,
    pub config: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SourceRow {
    pub id: Uuid,
    pub pipeline_id: Uuid,
    pub name: String,
    pub r#type: String,
    pub config: Value,
    pub schedule_interval: String,
    pub initial_sync: String,
    pub paused: bool,
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct JobRow {
    pub id: Uuid,
    pub source_id: Uuid,
    pub status: String,
    pub attempt: i32,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub summary: Option<Value>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CheckpointRow {
    pub source_id: Uuid,
    pub cursor_state: Value,
    pub updated_at: DateTime<Utc>,
}
