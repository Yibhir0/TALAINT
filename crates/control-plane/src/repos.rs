use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::{CheckpointRow, JobRow, PipelineRow, SourceRow, Tenant};

pub async fn create_tenant(pool: &PgPool, name: &str) -> Result<Tenant, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query_as::<_, Tenant>(
        "INSERT INTO tenants (id, name) VALUES ($1, $2) RETURNING id, name, created_at",
    )
    .bind(id)
    .bind(name)
    .fetch_one(pool)
    .await
}

pub async fn get_tenant_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT id, name, created_at FROM tenants WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn list_tenants(pool: &PgPool) -> Result<Vec<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT id, name, created_at FROM tenants ORDER BY created_at ASC")
        .fetch_all(pool)
        .await
}

pub async fn upsert_pipeline(
    pool: &PgPool,
    tenant_id: Uuid,
    intent: &str,
    config: Value,
) -> Result<PipelineRow, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query_as::<_, PipelineRow>(
        r#"
        INSERT INTO pipelines (id, tenant_id, intent, config)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (tenant_id, intent)
        DO UPDATE SET config = EXCLUDED.config, updated_at = now()
        RETURNING id, tenant_id, intent, config, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(tenant_id)
    .bind(intent)
    .bind(config)
    .fetch_one(pool)
    .await
}

pub async fn get_pipeline_by_intent(
    pool: &PgPool,
    tenant_id: Uuid,
    intent: &str,
) -> Result<Option<PipelineRow>, sqlx::Error> {
    sqlx::query_as::<_, PipelineRow>(
        "SELECT id, tenant_id, intent, config, created_at, updated_at FROM pipelines WHERE tenant_id = $1 AND intent = $2",
    )
    .bind(tenant_id)
    .bind(intent)
    .fetch_optional(pool)
    .await
}

pub async fn list_pipelines(pool: &PgPool, tenant_id: Uuid) -> Result<Vec<PipelineRow>, sqlx::Error> {
    sqlx::query_as::<_, PipelineRow>(
        "SELECT id, tenant_id, intent, config, created_at, updated_at FROM pipelines WHERE tenant_id = $1 ORDER BY intent",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_pipeline(pool: &PgPool, tenant_id: Uuid, intent: &str) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM pipelines WHERE tenant_id = $1 AND intent = $2")
        .bind(tenant_id)
        .bind(intent)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(Debug, Clone)]
pub struct SourceUpsert {
    pub name: String,
    pub source_type: String,
    pub config: Value,
    pub schedule_interval: String,
    pub initial_sync: String,
}

/// Fully reconcile pipeline sources to provided list by name: upsert present, delete missing.
/// Returns (upserted, removed_sources). Removed source rows are deleted (cascades checkpoints/jobs/logs).
pub async fn reconcile_sources(
    pool: &PgPool,
    pipeline_id: Uuid,
    desired: &[SourceUpsert],
) -> Result<(Vec<SourceRow>, Vec<SourceRow>), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let result = reconcile_sources_tx(&mut tx, pipeline_id, desired).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn reconcile_sources_tx(
    tx: &mut Transaction<'_, Postgres>,
    pipeline_id: Uuid,
    desired: &[SourceUpsert],
) -> Result<(Vec<SourceRow>, Vec<SourceRow>), sqlx::Error> {
    let existing = sqlx::query_as::<_, SourceRow>(
        r#"SELECT id, pipeline_id, name, type as "type", config, schedule_interval, initial_sync, paused, next_run_at, created_at, updated_at
           FROM sources WHERE pipeline_id = $1 FOR UPDATE"#,
    )
    .bind(pipeline_id)
    .fetch_all(&mut **tx)
    .await?;

    let desired_names: std::collections::HashSet<&str> = desired.iter().map(|s| s.name.as_str()).collect();
    let mut removed = Vec::new();
    for s in &existing {
        if !desired_names.contains(s.name.as_str()) {
            removed.push(s.clone());
        }
    }
    if !removed.is_empty() {
        let names: Vec<String> = removed.iter().map(|s| s.name.clone()).collect();
        sqlx::query("DELETE FROM sources WHERE pipeline_id = $1 AND name = ANY($2)")
            .bind(pipeline_id)
            .bind(&names)
            .execute(&mut **tx)
            .await?;
    }

    let mut upserted = Vec::new();
    for d in desired {
        let row = sqlx::query_as::<_, SourceRow>(
            r#"
            INSERT INTO sources (id, pipeline_id, name, type, config, schedule_interval, initial_sync)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT (pipeline_id, name)
            DO UPDATE SET type = EXCLUDED.type,
                          config = EXCLUDED.config,
                          schedule_interval = EXCLUDED.schedule_interval,
                          initial_sync = EXCLUDED.initial_sync,
                          updated_at = now()
            RETURNING id, pipeline_id, name, type as "type", config, schedule_interval, initial_sync, paused, next_run_at, created_at, updated_at
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(pipeline_id)
        .bind(&d.name)
        .bind(&d.source_type)
        .bind(&d.config)
        .bind(&d.schedule_interval)
        .bind(&d.initial_sync)
        .fetch_one(&mut **tx)
        .await?;
        upserted.push(row);
    }

    Ok((upserted, removed))
}

pub async fn list_sources(pool: &PgPool, pipeline_id: Uuid) -> Result<Vec<SourceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>(
        r#"SELECT id, pipeline_id, name, type as "type", config, schedule_interval, initial_sync, paused, next_run_at, created_at, updated_at
           FROM sources WHERE pipeline_id = $1 ORDER BY name"#,
    )
    .bind(pipeline_id)
    .fetch_all(pool)
    .await
}

pub async fn get_source_by_name(
    pool: &PgPool,
    pipeline_id: Uuid,
    name: &str,
) -> Result<Option<SourceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>(
        r#"SELECT id, pipeline_id, name, type as "type", config, schedule_interval, initial_sync, paused, next_run_at, created_at, updated_at
           FROM sources WHERE pipeline_id = $1 AND name = $2"#,
    )
    .bind(pipeline_id)
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn set_source_paused(pool: &PgPool, source_id: Uuid, paused: bool) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE sources SET paused = $2, updated_at = now() WHERE id = $1")
        .bind(source_id)
        .bind(paused)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_next_run_at(pool: &PgPool, source_id: Uuid, next_run_at: Option<DateTime<Utc>>) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE sources SET next_run_at = $2, updated_at = now() WHERE id = $1")
        .bind(source_id)
        .bind(next_run_at)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_source(pool: &PgPool, pipeline_id: Uuid, name: &str) -> Result<Option<SourceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>(
        r#"DELETE FROM sources WHERE pipeline_id = $1 AND name = $2
           RETURNING id, pipeline_id, name, type as "type", config, schedule_interval, initial_sync, paused, next_run_at, created_at, updated_at"#,
    )
    .bind(pipeline_id)
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn upsert_checkpoint(pool: &PgPool, source_id: Uuid, cursor_state: Value) -> Result<CheckpointRow, sqlx::Error> {
    sqlx::query_as::<_, CheckpointRow>(
        r#"
        INSERT INTO pipeline_checkpoints (source_id, cursor_state)
        VALUES ($1, $2)
        ON CONFLICT (source_id) DO UPDATE SET cursor_state = EXCLUDED.cursor_state, updated_at = now()
        RETURNING source_id, cursor_state, updated_at
        "#,
    )
    .bind(source_id)
    .bind(cursor_state)
    .fetch_one(pool)
    .await
}

pub async fn get_checkpoint(pool: &PgPool, source_id: Uuid) -> Result<Option<CheckpointRow>, sqlx::Error> {
    sqlx::query_as::<_, CheckpointRow>(
        "SELECT source_id, cursor_state, updated_at FROM pipeline_checkpoints WHERE source_id = $1",
    )
    .bind(source_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_jobs_for_source(pool: &PgPool, source_id: Uuid, limit: i64) -> Result<Vec<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"SELECT id, source_id, status, attempt, heartbeat_at, started_at, finished_at, summary, error, created_at
           FROM pipeline_jobs WHERE source_id = $1 ORDER BY created_at DESC LIMIT $2"#,
    )
    .bind(source_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
