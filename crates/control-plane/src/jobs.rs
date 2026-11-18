use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::models::JobRow;

#[derive(Debug, Clone, Copy)]
pub enum ClaimedRunState {
    Claimed,
    AlreadyActive,
    SourceNotFound,
}

/// Claim the next queued job for a source, enforcing one-active-run per source.
/// Returns Claimed if a queued job was moved to running, AlreadyActive if a running job exists,
/// or SourceNotFound if the source id is invalid/missing.
pub async fn claim_next_job_for_source(
    pool: &PgPool,
    source_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(ClaimedRunState, Option<JobRow>), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let state = claim_next_job_for_source_tx(&mut tx, source_id, now).await?;
    tx.commit().await?;
    Ok(state)
}

pub async fn claim_next_job_for_source_tx(
    tx: &mut Transaction<'_, Postgres>,
    source_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(ClaimedRunState, Option<JobRow>), sqlx::Error> {
    // Lock the source row to serialize claim decisions per source.
    let src_exists =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM sources WHERE id = $1 FOR UPDATE")
            .bind(source_id)
            .fetch_optional(&mut **tx)
            .await?;
    if src_exists.is_none() {
        return Ok((ClaimedRunState::SourceNotFound, None));
    }

    let running = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM pipeline_jobs WHERE source_id = $1 AND status = 'running' LIMIT 1 FOR UPDATE",
    )
    .bind(source_id)
    .fetch_optional(&mut **tx)
    .await?;
    if running.is_some() {
        return Ok((ClaimedRunState::AlreadyActive, None));
    }

    let job = sqlx::query_as::<_, JobRow>(
        r#"
        WITH next_job AS (
            SELECT id
            FROM pipeline_jobs
            WHERE source_id = $1 AND status = 'queued'
            ORDER BY created_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        UPDATE pipeline_jobs j
        SET status = 'running',
            attempt = j.attempt + 1,
            started_at = COALESCE(j.started_at, $2),
            heartbeat_at = $2
        FROM next_job
        WHERE j.id = next_job.id
        RETURNING j.id, j.source_id, j.status, j.attempt, j.heartbeat_at, j.started_at, j.finished_at, j.summary, j.error, j.created_at
        "#,
    )
    .bind(source_id)
    .bind(now)
    .fetch_optional(&mut **tx)
    .await?;

    match job {
        Some(j) => Ok((ClaimedRunState::Claimed, Some(j))),
        None => Ok((ClaimedRunState::Claimed, None)), // no queued job available
    }
}

pub async fn heartbeat(
    pool: &PgPool,
    job_id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        "UPDATE pipeline_jobs SET heartbeat_at = $2 WHERE id = $1 AND status = 'running'",
    )
    .bind(job_id)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}

pub async fn complete_job(
    pool: &PgPool,
    job_id: Uuid,
    status: &str,
    summary: Option<Value>,
    error: Option<String>,
    finished_at: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let res = sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = $2,
            summary = $3,
            error = $4,
            finished_at = $5,
            heartbeat_at = COALESCE(heartbeat_at, $5)
        WHERE id = $1 AND status = 'running'
        "#,
    )
    .bind(job_id)
    .bind(status)
    .bind(summary)
    .bind(error)
    .bind(finished_at)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 1)
}

/// Requeue running jobs whose heartbeat is stale. Increments attempt is handled on (re)claim;
/// jobs exceeding max_requeues are marked failed.
pub async fn requeue_stuck_jobs(
    pool: &PgPool,
    stale_before: DateTime<Utc>,
    max_requeues: i32,
) -> Result<(u64, u64), sqlx::Error> {
    let mut tx = pool.begin().await?;

    let failed = sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = 'failed',
            error = COALESCE(error, 'job stuck without heartbeat'),
            finished_at = now()
        WHERE status = 'running'
          AND heartbeat_at IS NOT NULL
          AND heartbeat_at < $1
          AND attempt >= $2
        "#,
    )
    .bind(stale_before)
    .bind(max_requeues)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    let requeued = sqlx::query(
        r#"
        UPDATE pipeline_jobs
        SET status = 'queued',
            error = COALESCE(error, 'job heartbeat timed out, requeued'),
            heartbeat_at = NULL,
            started_at = NULL
        WHERE status = 'running'
          AND heartbeat_at IS NOT NULL
          AND heartbeat_at < $1
          AND attempt < $2
        "#,
    )
    .bind(stale_before)
    .bind(max_requeues)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    tx.commit().await?;
    Ok((requeued, failed))
}

pub async fn enqueue_job(pool: &PgPool, source_id: Uuid) -> Result<JobRow, sqlx::Error> {
    let id = Uuid::new_v4();
    sqlx::query_as::<_, JobRow>(
        r#"
        INSERT INTO pipeline_jobs (id, source_id, status, attempt)
        VALUES ($1, $2, 'queued', 0)
        RETURNING id, source_id, status, attempt, heartbeat_at, started_at, finished_at, summary, error, created_at
        "#,
    )
    .bind(id)
    .bind(source_id)
    .fetch_one(pool)
    .await
}

pub async fn has_active_or_queued_run(pool: &PgPool, source_id: Uuid) -> Result<bool, sqlx::Error> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM pipeline_jobs WHERE source_id = $1 AND status IN ('queued','running'))",
    )
    .bind(source_id)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}
