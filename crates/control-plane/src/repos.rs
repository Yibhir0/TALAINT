use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::{
    AuthTokenRow, CheckpointRow, JobRow, PipelineRow, RunLogRow, SecretMetaRow, SecretRow,
    SourceRow, Tenant, TenantKeyRow,
};

// --- Tenants ---

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

pub async fn get_tenant_by_name(pool: &PgPool, name: &str) -> Result<Option<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT id, name, created_at FROM tenants WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
}

pub async fn list_tenants(pool: &PgPool) -> Result<Vec<Tenant>, sqlx::Error> {
    sqlx::query_as::<_, Tenant>("SELECT id, name, created_at FROM tenants ORDER BY created_at ASC")
        .fetch_all(pool)
        .await
}

// --- Auth tokens ---

pub async fn insert_auth_token(
    pool: &PgPool,
    kind: &str,
    tenant_id: Option<Uuid>,
    token_hash: &str,
) -> Result<AuthTokenRow, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query_as::<_, AuthTokenRow>(
        r#"INSERT INTO auth_tokens (id, kind, tenant_id, token_hash)
           VALUES ($1, $2, $3, $4)
           RETURNING id, kind, tenant_id, token_hash, created_at, revoked_at, last_used_at"#,
    )
    .bind(id)
    .bind(kind)
    .bind(tenant_id)
    .bind(token_hash)
    .fetch_one(pool)
    .await
}

pub async fn find_active_token_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<AuthTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, AuthTokenRow>(
        r#"SELECT id, kind, tenant_id, token_hash, created_at, revoked_at, last_used_at
           FROM auth_tokens WHERE token_hash = $1 AND revoked_at IS NULL"#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
}

pub async fn count_admin_tokens(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM auth_tokens WHERE kind = 'admin'")
        .fetch_one(pool)
        .await
}

pub async fn touch_token(pool: &PgPool, token_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE auth_tokens SET last_used_at = now() WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn revoke_active_tenant_tokens(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE auth_tokens SET revoked_at = now() WHERE tenant_id = $1 AND kind = 'tenant' AND revoked_at IS NULL",
    )
    .bind(tenant_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}

pub async fn rotate_tenant_token(
    pool: &PgPool,
    tenant_id: Uuid,
    new_token_hash: &str,
) -> Result<AuthTokenRow, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE auth_tokens SET revoked_at = now() WHERE tenant_id = $1 AND kind = 'tenant' AND revoked_at IS NULL",
    )
    .bind(tenant_id)
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query_as::<_, AuthTokenRow>(
        r#"INSERT INTO auth_tokens (id, kind, tenant_id, token_hash)
           VALUES ($1, 'tenant', $2, $3)
           RETURNING id, kind, tenant_id, token_hash, created_at, revoked_at, last_used_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(tenant_id)
    .bind(new_token_hash)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row)
}

// --- Tenant keys / secrets ---

pub async fn get_tenant_key(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Option<TenantKeyRow>, sqlx::Error> {
    sqlx::query_as::<_, TenantKeyRow>(
        "SELECT tenant_id, wrapped_dek, key_version, created_at FROM tenant_keys WHERE tenant_id = $1",
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn insert_tenant_key(
    pool: &PgPool,
    tenant_id: Uuid,
    wrapped_dek: &[u8],
    key_version: i32,
) -> Result<TenantKeyRow, sqlx::Error> {
    sqlx::query_as::<_, TenantKeyRow>(
        r#"INSERT INTO tenant_keys (tenant_id, wrapped_dek, key_version)
           VALUES ($1, $2, $3)
           RETURNING tenant_id, wrapped_dek, key_version, created_at"#,
    )
    .bind(tenant_id)
    .bind(wrapped_dek)
    .bind(key_version)
    .fetch_one(pool)
    .await
}

pub async fn upsert_secret(
    pool: &PgPool,
    tenant_id: Uuid,
    name: &str,
    nonce: &[u8],
    ciphertext: &[u8],
    key_version: i32,
) -> Result<SecretRow, sqlx::Error> {
    sqlx::query_as::<_, SecretRow>(
        r#"INSERT INTO secrets (tenant_id, name, nonce, ciphertext, key_version)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (tenant_id, name)
           DO UPDATE SET nonce = EXCLUDED.nonce, ciphertext = EXCLUDED.ciphertext,
                         key_version = EXCLUDED.key_version, updated_at = now()
           RETURNING tenant_id, name, nonce, ciphertext, key_version, created_at, updated_at"#,
    )
    .bind(tenant_id)
    .bind(name)
    .bind(nonce)
    .bind(ciphertext)
    .bind(key_version)
    .fetch_one(pool)
    .await
}

pub async fn get_secret(
    pool: &PgPool,
    tenant_id: Uuid,
    name: &str,
) -> Result<Option<SecretRow>, sqlx::Error> {
    sqlx::query_as::<_, SecretRow>(
        "SELECT tenant_id, name, nonce, ciphertext, key_version, created_at, updated_at FROM secrets WHERE tenant_id = $1 AND name = $2",
    )
    .bind(tenant_id)
    .bind(name)
    .fetch_optional(pool)
    .await
}

pub async fn list_secret_metas(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<SecretMetaRow>, sqlx::Error> {
    sqlx::query_as::<_, SecretMetaRow>(
        "SELECT name, key_version, created_at, updated_at FROM secrets WHERE tenant_id = $1 ORDER BY name",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_secret(
    pool: &PgPool,
    tenant_id: Uuid,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM secrets WHERE tenant_id = $1 AND name = $2")
        .bind(tenant_id)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn secret_exists(
    pool: &PgPool,
    tenant_id: Uuid,
    name: &str,
) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM secrets WHERE tenant_id = $1 AND name = $2)",
    )
    .bind(tenant_id)
    .bind(name)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

// --- Pipelines ---

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

pub async fn list_pipelines(
    pool: &PgPool,
    tenant_id: Uuid,
) -> Result<Vec<PipelineRow>, sqlx::Error> {
    sqlx::query_as::<_, PipelineRow>(
        "SELECT id, tenant_id, intent, config, created_at, updated_at FROM pipelines WHERE tenant_id = $1 ORDER BY intent",
    )
    .bind(tenant_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_pipeline(
    pool: &PgPool,
    tenant_id: Uuid,
    intent: &str,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("DELETE FROM pipelines WHERE tenant_id = $1 AND intent = $2")
        .bind(tenant_id)
        .bind(intent)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

// --- Sources ---

#[derive(Debug, Clone)]
pub struct SourceUpsert {
    pub name: String,
    pub source_type: String,
    pub config: Value,
    pub schedule_interval: String,
    pub initial_sync: String,
}

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

    let desired_names: std::collections::HashSet<&str> =
        desired.iter().map(|s| s.name.as_str()).collect();
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

pub async fn set_source_paused(
    pool: &PgPool,
    source_id: Uuid,
    paused: bool,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE sources SET paused = $2, updated_at = now() WHERE id = $1")
        .bind(source_id)
        .bind(paused)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn update_next_run_at(
    pool: &PgPool,
    source_id: Uuid,
    next_run_at: Option<DateTime<Utc>>,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query("UPDATE sources SET next_run_at = $2, updated_at = now() WHERE id = $1")
        .bind(source_id)
        .bind(next_run_at)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete_source(
    pool: &PgPool,
    pipeline_id: Uuid,
    name: &str,
) -> Result<Option<SourceRow>, sqlx::Error> {
    sqlx::query_as::<_, SourceRow>(
        r#"DELETE FROM sources WHERE pipeline_id = $1 AND name = $2
           RETURNING id, pipeline_id, name, type as "type", config, schedule_interval, initial_sync, paused, next_run_at, created_at, updated_at"#,
    )
    .bind(pipeline_id)
    .bind(name)
    .fetch_optional(pool)
    .await
}

// --- Checkpoints / runs / logs ---

pub async fn upsert_checkpoint(
    pool: &PgPool,
    source_id: Uuid,
    cursor_state: Value,
) -> Result<CheckpointRow, sqlx::Error> {
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

pub async fn get_checkpoint(
    pool: &PgPool,
    source_id: Uuid,
) -> Result<Option<CheckpointRow>, sqlx::Error> {
    sqlx::query_as::<_, CheckpointRow>(
        "SELECT source_id, cursor_state, updated_at FROM pipeline_checkpoints WHERE source_id = $1",
    )
    .bind(source_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_jobs_for_source(
    pool: &PgPool,
    source_id: Uuid,
    limit: i64,
) -> Result<Vec<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"SELECT id, source_id, status, attempt, heartbeat_at, started_at, finished_at, summary, error, created_at
           FROM pipeline_jobs WHERE source_id = $1 ORDER BY created_at DESC LIMIT $2"#,
    )
    .bind(source_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn list_jobs_for_tenant(
    pool: &PgPool,
    tenant_id: Uuid,
    intent: Option<&str>,
    source_name: Option<&str>,
    limit: i64,
) -> Result<Vec<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT j.id, j.source_id, j.status, j.attempt, j.heartbeat_at, j.started_at, j.finished_at, j.summary, j.error, j.created_at
        FROM pipeline_jobs j
        JOIN sources s ON s.id = j.source_id
        JOIN pipelines p ON p.id = s.pipeline_id
        WHERE p.tenant_id = $1
          AND ($2::text IS NULL OR p.intent = $2)
          AND ($3::text IS NULL OR s.name = $3)
        ORDER BY j.created_at DESC
        LIMIT $4
        "#,
    )
    .bind(tenant_id)
    .bind(intent)
    .bind(source_name)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn get_job(pool: &PgPool, job_id: Uuid) -> Result<Option<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"SELECT id, source_id, status, attempt, heartbeat_at, started_at, finished_at, summary, error, created_at
           FROM pipeline_jobs WHERE id = $1"#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await
}

pub async fn get_job_for_tenant(
    pool: &PgPool,
    tenant_id: Uuid,
    job_id: Uuid,
) -> Result<Option<JobRow>, sqlx::Error> {
    sqlx::query_as::<_, JobRow>(
        r#"
        SELECT j.id, j.source_id, j.status, j.attempt, j.heartbeat_at, j.started_at, j.finished_at, j.summary, j.error, j.created_at
        FROM pipeline_jobs j
        JOIN sources s ON s.id = j.source_id
        JOIN pipelines p ON p.id = s.pipeline_id
        WHERE j.id = $1 AND p.tenant_id = $2
        "#,
    )
    .bind(job_id)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_run_logs(
    pool: &PgPool,
    job_id: Uuid,
    limit: i64,
) -> Result<Vec<RunLogRow>, sqlx::Error> {
    sqlx::query_as::<_, RunLogRow>(
        "SELECT id, job_id, ts, level, message, data FROM run_logs WHERE job_id = $1 ORDER BY ts ASC, id ASC LIMIT $2",
    )
    .bind(job_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
