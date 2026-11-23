use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use talaint_control_plane::{auth as cp_auth, jobs, repos, secrets as cp_secrets, DbPool};
use talaint_core::config::{
    is_valid_interval, PipelineConfig, SecretRef, SourceConfig, SourceSchedule, StringOrSecretRef,
};
use talaint_core::{InitialSync, Intent};
use uuid::Uuid;

use crate::creds::{self, StoredCredentials};
use crate::output::{self, OutputMode};

pub struct AppCtx {
    pub pool: DbPool,
}

impl AppCtx {
    pub async fn connect() -> Result<Self> {
        let _ = dotenvy::dotenv();
        let database_url = std::env::var("DATABASE_URL").map_err(|_| {
            anyhow!(
                "DATABASE_URL is not set (e.g. postgres://talaint:talaint@localhost:5432/talaint)"
            )
        })?;
        let pool = talaint_control_plane::create_pool(&database_url).await?;
        Ok(Self { pool })
    }
}

#[derive(Debug, Serialize)]
struct ErrorOut {
    ok: bool,
    error: String,
}

pub fn handle_result(mode: OutputMode, res: Result<()>) {
    if let Err(e) = res {
        if mode.is_json() {
            let _ = output::print_json(&ErrorOut {
                ok: false,
                error: format!("{e:#}"),
            });
        } else {
            eprintln!("Error: {e:#}");
        }
        std::process::exit(1);
    }
}

async fn resolve_auth(
    pool: &DbPool,
) -> Result<(
    talaint_control_plane::models::AuthTokenRow,
    Option<talaint_control_plane::models::Tenant>,
)> {
    let stored = creds::require_credentials()?;
    let token_row = cp_auth::resolve_active_token(pool, &stored.token)
        .await?
        .ok_or_else(|| {
            anyhow!("stored token is invalid or revoked; run `talaint auth login` again")
        })?;
    let tenant = if let Some(tid) = token_row.tenant_id {
        repos::get_tenant_by_id(pool, tid).await?
    } else {
        None
    };
    Ok((token_row, tenant))
}

async fn require_tenant(
    pool: &DbPool,
) -> Result<(
    talaint_control_plane::models::Tenant,
    talaint_control_plane::models::AuthTokenRow,
)> {
    let (tok, tenant_opt) = resolve_auth(pool).await?;
    if tok.kind != "tenant" {
        return Err(anyhow!("this command requires a tenant token (not admin)"));
    }
    let tenant = tenant_opt.ok_or_else(|| anyhow!("tenant not found for token"))?;
    Ok((tenant, tok))
}

async fn require_admin(pool: &DbPool) -> Result<talaint_control_plane::models::AuthTokenRow> {
    let (tok, _) = resolve_auth(pool).await?;
    if tok.kind != "admin" {
        return Err(anyhow!("this command requires an admin token"));
    }
    Ok(tok)
}

pub fn init_template_source(source_type: &str) -> String {
    match source_type {
        "http_json" => r#"intent: rag
sources:
  - name: my-http-source
    type: http_json
    url: http://localhost:8080/data
    method: GET
    schedule:
      interval: 1m
    initial_sync: full
    # headers:
    #   Authorization: { ref: my_secret }
    # record_path: items
"#
        .to_string(),
        other => format!(
            r#"intent: rag
sources:
  - name: my-source
    type: {other}
    # TODO: add connector-specific fields (e.g. location/url)
    schedule:
      interval: 1m
    initial_sync: full
"#
        ),
    }
}

pub fn init_template_intent(intent: &str) -> String {
    match intent {
        "rag" => r#"intent: rag
sources:
  - name: my-http-source
    type: http_json
    url: http://localhost:8080/data
    schedule:
      interval: 1m
processing: {}
"#
        .to_string(),
        "fine_tuning" => r#"intent: fine_tuning
sources:
  - name: my-http-source
    type: http_json
    url: http://localhost:8080/data
    schedule:
      interval: 1m
processing:
  template: ./templates/finetune.jinja
"#
        .to_string(),
        "ml_training" => r#"intent: ml_training
sources:
  - name: my-http-source
    type: http_json
    url: http://localhost:8080/data
    schedule:
      interval: 1m
processing:
  schema:
    columns:
      - { name: id, type: string }
      - { name: value, type: float }
    # label: target
"#
        .to_string(),
        "multimodal" => r#"intent: multimodal
sources:
  - name: my-http-source
    type: http_json
    url: http://localhost:8080/images
    schedule:
      interval: 1m
processing: {}
"#
        .to_string(),
        other => format!("intent: {other}\nsources: []\n"),
    }
}

pub fn list_sources_catalog() -> Vec<&'static str> {
    vec!["http_json"]
}
pub fn list_intents_catalog() -> Vec<&'static str> {
    vec!["rag", "fine_tuning", "ml_training", "multimodal"]
}
pub fn list_formats_catalog() -> Vec<&'static str> {
    vec!["json", "csv", "parquet", "text", "image"]
}

pub fn load_pipeline_file(path: &str) -> Result<(PipelineConfig, String)> {
    let raw = fs::read_to_string(path).with_context(|| format!("failed to read {}", path))?;
    let content = raw.trim_start_matches('\u{feff}').to_string();
    let cfg: PipelineConfig = PipelineConfig::from_yaml_str(&content)
        .map_err(|e| anyhow!("failed to parse YAML: {e}"))?;
    Ok((cfg, content))
}

pub fn validate_pipeline_cfg(cfg: &PipelineConfig) -> Result<()> {
    if let Err(errs) = cfg.validate() {
        return Err(anyhow!("validation failed:\n- {}", errs.join("\n- ")));
    }
    Ok(())
}

fn to_source_upserts(cfg: &PipelineConfig) -> Vec<repos::SourceUpsert> {
    cfg.sources
        .iter()
        .map(|s| repos::SourceUpsert {
            name: s.name.clone(),
            source_type: s.source_type.clone(),
            config: s.to_db_config_value(),
            schedule_interval: s.schedule.interval.clone(),
            initial_sync: s.initial_sync.as_str().to_string(),
        })
        .collect()
}

async fn ensure_secret_refs(pool: &DbPool, tenant_id: Uuid, refs: &BTreeSet<String>) -> Result<()> {
    let missing = cp_secrets::check_secret_refs_exist(pool, tenant_id, refs).await?;
    if !missing.is_empty() {
        return Err(anyhow!(
            "missing secret(s) in database: {} (create with `talaint secrets set <name>`)",
            missing.join(", ")
        ));
    }
    Ok(())
}

pub async fn health_check_source(source: &talaint_core::config::SourceConfig) -> Result<()> {
    // Minimal: for http_json perform a lightweight request (HEAD, fallback GET) if url present.
    if source.source_type == "http_json" {
        let url = source
            .http_url()
            .ok_or_else(|| anyhow!("http_json source missing url"))?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let mut req = client.request(reqwest::Method::HEAD, url);
        // literal headers only for now (secret refs not resolved in v1 check without decrypt plumbing)
        for (k, v) in &source.headers {
            if let talaint_core::config::StringOrSecretRef::String(val) = v {
                req = req.header(k, val);
            }
        }
        let resp = req.send().await;
        match resp {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r)
                if r.status().as_u16() == 405
                    || r.status().is_client_error()
                    || r.status().is_server_error() =>
            {
                let mut req2 = client.get(url);
                for (k, v) in &source.headers {
                    if let talaint_core::config::StringOrSecretRef::String(val) = v {
                        req2 = req2.header(k, val);
                    }
                }
                let r2 = req2.send().await?;
                if r2.status().is_success() {
                    Ok(())
                } else {
                    Err(anyhow!("health check failed: HTTP {}", r2.status()))
                }
            }
            Ok(r) => Err(anyhow!("health check failed: HTTP {}", r.status())),
            Err(e) => Err(anyhow!("health check failed: {e}")),
        }
    } else {
        // Unknown connector types: no-op pass for now (validated structurally)
        Ok(())
    }
}

pub async fn cmd_auth_login(mode: OutputMode, token: &str, admin: bool) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let mut row_opt = cp_auth::resolve_active_token(&ctx.pool, token).await?;
    if row_opt.is_none() && admin {
        if let Ok(env_admin) = std::env::var("TALAINT_ADMIN_TOKEN") {
            if env_admin == token {
                row_opt = cp_auth::resolve_active_token(&ctx.pool, token).await?;
                if row_opt.is_none() {
                    row_opt = Some(cp_auth::create_admin_token(&ctx.pool, token).await?);
                }
            }
        }
    }
    let row = row_opt.ok_or_else(|| anyhow!("token is invalid or revoked"))?;
    let expected = if admin { "admin" } else { "tenant" };
    if row.kind != expected {
        return Err(anyhow!(
            "token kind mismatch: expected {expected}, got {}",
            row.kind
        ));
    }
    let mut tenant_name = None;
    if let Some(tid) = row.tenant_id {
        tenant_name = repos::get_tenant_by_id(&ctx.pool, tid)
            .await?
            .map(|t| t.name);
    }
    let stored = StoredCredentials {
        token: token.to_string(),
        kind: row.kind.clone(),
        tenant_id: row.tenant_id.map(|u| u.to_string()),
        tenant_name,
    };
    let path = creds::save_credentials(&stored)?;
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "kind": stored.kind, "tenant_id": stored.tenant_id, "credentials_path": path}),
        )
    } else {
        println!(
            "Logged in as {}. Credentials saved to {}",
            stored.kind,
            path.display()
        );
        Ok(())
    }
}

pub async fn cmd_token_rotate(mode: OutputMode) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _tok) = require_tenant(&ctx.pool).await?;
    let (tenant, new_token) =
        cp_auth::rotate_tenant_token_by_name_or_id(&ctx.pool, &tenant.id.to_string()).await?;
    let stored = StoredCredentials {
        token: new_token.clone(),
        kind: "tenant".to_string(),
        tenant_id: Some(tenant.id.to_string()),
        tenant_name: Some(tenant.name.clone()),
    };
    let path = creds::save_credentials(&stored)?;
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "tenant_id": tenant.id, "token": new_token, "credentials_path": path}),
        )
    } else {
        println!(
            "Tenant token rotated for '{}'. New token (shown once): {}",
            tenant.name, new_token
        );
        println!("Credentials updated at {}", path.display());
        Ok(())
    }
}

pub async fn cmd_admin_tenant_create(mode: OutputMode, name: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    require_admin(&ctx.pool).await?;
    let (tenant, token) = cp_auth::create_tenant_with_token(&ctx.pool, name).await?;
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "tenant": {"id": tenant.id, "name": tenant.name}, "token": token}),
        )
    } else {
        println!("Tenant created: {} ({})", tenant.name, tenant.id);
        println!("One-time access token (show once): {token}");
        Ok(())
    }
}

pub async fn cmd_admin_tenant_list(mode: OutputMode) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    require_admin(&ctx.pool).await?;
    let tenants = repos::list_tenants(&ctx.pool).await?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "tenants": tenants}));
    }
    if tenants.is_empty() {
        println!("No tenants.");
        return Ok(());
    }
    for t in tenants {
        println!("{}\t{}\t{}", t.id, t.name, t.created_at);
    }
    Ok(())
}

pub async fn cmd_admin_token_rotate(mode: OutputMode, tenant_ref: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    require_admin(&ctx.pool).await?;
    let (tenant, token) = cp_auth::rotate_tenant_token_by_name_or_id(&ctx.pool, tenant_ref).await?;
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "tenant": {"id": tenant.id, "name": tenant.name}, "token": token}),
        )
    } else {
        println!(
            "Rotated token for tenant '{}' ({}). New token (show once): {}",
            tenant.name, tenant.id, token
        );
        Ok(())
    }
}

pub async fn cmd_secrets_set(mode: OutputMode, name: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let row = cp_secrets::set_secret_from_env(&ctx.pool, tenant.id, name).await?;
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "secret": {"name": row.name, "key_version": row.key_version, "updated_at": row.updated_at}}),
        )
    } else {
        println!(
            "Secret '{}' stored (key_version {}).",
            row.name, row.key_version
        );
        Ok(())
    }
}

pub async fn cmd_secrets_list(mode: OutputMode) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let rows = repos::list_secret_metas(&ctx.pool, tenant.id).await?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "secrets": rows}));
    }
    if rows.is_empty() {
        println!("No secrets.");
        return Ok(());
    }
    for s in rows {
        println!(
            "{}\tkey_version={}\tupdated_at={}",
            s.name, s.key_version, s.updated_at
        );
    }
    Ok(())
}

pub async fn cmd_secrets_delete(mode: OutputMode, name: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let deleted = repos::delete_secret(&ctx.pool, tenant.id, name).await?;
    if !deleted {
        return Err(anyhow!("secret '{}' not found", name));
    }
    if mode.is_json() {
        output::print_json(&json!({"ok": true, "deleted": name}))
    } else {
        println!("Secret '{}' deleted.", name);
        Ok(())
    }
}

pub async fn cmd_validate(mode: OutputMode, file: &str) -> Result<()> {
    let (cfg, _) = load_pipeline_file(file)?;
    validate_pipeline_cfg(&cfg)?;
    let _ = dotenvy::dotenv();
    if std::env::var("DATABASE_URL").is_ok() {
        let ctx = AppCtx::connect().await?;
        let (tenant, _) = require_tenant(&ctx.pool).await?;
        let refs = cfg.collect_secret_refs();
        ensure_secret_refs(&ctx.pool, tenant.id, &refs).await?;
    }
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "valid": true, "intent": cfg.intent, "sources": cfg.sources.len()}),
        )
    } else {
        println!(
            "Config is valid (intent={}, sources={}).",
            cfg.intent,
            cfg.sources.len()
        );
        Ok(())
    }
}

pub async fn cmd_apply(mode: OutputMode, file: &str, dry_run: bool, force: bool) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let (cfg, _) = load_pipeline_file(file)?;
    validate_pipeline_cfg(&cfg)?;
    let refs = cfg.collect_secret_refs();
    ensure_secret_refs(&ctx.pool, tenant.id, &refs).await?;

    // health checks
    let mut health_errors: Vec<String> = Vec::new();
    for s in &cfg.sources {
        if let Err(e) = health_check_source(s).await {
            health_errors.push(format!("{}: {}", s.name, e));
        }
    }
    if !health_errors.is_empty() && !force && !dry_run {
        return Err(anyhow!(
            "source health checks failed:\n- {} (use --force to override)",
            health_errors.join("\n- ")
        ));
    }

    if dry_run {
        let plan = json!({
            "ok": true,
            "dry_run": true,
            "intent": cfg.intent,
            "dataset": cfg.intent.dataset_name(),
            "sources": cfg.sources.iter().map(|s| json!({"name": s.name, "type": s.source_type, "interval": s.schedule.interval, "initial_sync": s.initial_sync})).collect::<Vec<_>>(),
            "health_warnings": health_errors,
            "action": "upsert pipeline and fully reconcile sources"
        });
        if mode.is_json() {
            return output::print_json(&plan);
        }
        println!(
            "Dry run: intent={} dataset={} sources={}",
            cfg.intent,
            cfg.intent.dataset_name(),
            cfg.sources.len()
        );
        for s in &cfg.sources {
            println!(
                "  - {} ({}, interval={})",
                s.name, s.source_type, s.schedule.interval
            );
        }
        if !health_errors.is_empty() {
            println!("Health warnings:\n- {}", health_errors.join("\n- "));
        }
        return Ok(());
    }

    let pipeline_cfg = json!({"processing": cfg.processing});
    let pipeline =
        repos::upsert_pipeline(&ctx.pool, tenant.id, cfg.intent.as_str(), pipeline_cfg).await?;
    let desired = to_source_upserts(&cfg);
    let (upserted, removed) = repos::reconcile_sources(&ctx.pool, pipeline.id, &desired).await?;
    let result = json!({
        "ok": true,
        "intent": cfg.intent,
        "pipeline_id": pipeline.id,
        "upserted_sources": upserted.iter().map(|s| &s.name).collect::<Vec<_>>(),
        "removed_sources": removed.iter().map(|s| &s.name).collect::<Vec<_>>(),
        "health_warnings": health_errors,
    });
    if mode.is_json() {
        return output::print_json(&result);
    }
    println!(
        "Applied pipeline intent={} (sources upserted={}, removed={}).",
        cfg.intent,
        upserted.len(),
        removed.len()
    );
    if !health_errors.is_empty() {
        println!(
            "Health warnings (forced):\n- {}",
            health_errors.join("\n- ")
        );
    }
    Ok(())
}

pub async fn cmd_source_test(mode: OutputMode, file: &str) -> Result<()> {
    let (cfg, _) = load_pipeline_file(file)?;
    validate_pipeline_cfg(&cfg)?;
    let mut results = Vec::new();
    let mut ok_all = true;
    for s in &cfg.sources {
        let (ok, msg) = match health_check_source(s).await {
            Ok(_) => (true, "ok".to_string()),
            Err(e) => {
                ok_all = false;
                (false, e.to_string())
            }
        };
        results.push(json!({"name": s.name, "type": s.source_type, "ok": ok, "message": msg}));
    }
    if mode.is_json() {
        output::print_json(&json!({"ok": ok_all, "results": results}))?;
        if !ok_all {
            std::process::exit(1);
        }
        return Ok(());
    }
    for r in &results {
        println!(
            "{}: {} ({})",
            r["name"].as_str().unwrap_or("?"),
            r["message"].as_str().unwrap_or(""),
            if r["ok"].as_bool().unwrap_or(false) {
                "ok"
            } else {
                "fail"
            }
        );
    }
    if !ok_all {
        return Err(anyhow!("one or more source health checks failed"));
    }
    Ok(())
}

pub async fn cmd_pipelines_list(mode: OutputMode) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let rows = repos::list_pipelines(&ctx.pool, tenant.id).await?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "pipelines": rows}));
    }
    if rows.is_empty() {
        println!("No pipelines.");
        return Ok(());
    }
    for p in rows {
        let sources = repos::list_sources(&ctx.pool, p.id)
            .await
            .unwrap_or_default();
        println!(
            "intent={}\tsources={}\tupdated_at={}",
            p.intent,
            sources.len(),
            p.updated_at
        );
    }
    Ok(())
}

pub async fn cmd_pipelines_show(mode: OutputMode, intent: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let p = repos::get_pipeline_by_intent(&ctx.pool, tenant.id, intent)
        .await?
        .ok_or_else(|| anyhow!("pipeline not found for intent '{intent}'"))?;
    let sources = repos::list_sources(&ctx.pool, p.id).await?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "pipeline": p, "sources": sources}));
    }
    println!(
        "intent: {}\nconfig: {}\nupdated_at: {}",
        p.intent, p.config, p.updated_at
    );
    for s in sources {
        println!(
            "  - {} type={} interval={} paused={} initial_sync={}",
            s.name, s.r#type, s.schedule_interval, s.paused, s.initial_sync
        );
    }
    Ok(())
}

pub async fn cmd_pipelines_delete(mode: OutputMode, intent: &str, purge_data: bool) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let deleted = repos::delete_pipeline(&ctx.pool, tenant.id, intent).await?;
    if !deleted {
        return Err(anyhow!("pipeline not found for intent '{intent}'"));
    }
    let mut note = None;
    if purge_data {
        note = Some(
            "purge-data requested (dataset purge not yet implemented in M5; config deleted)"
                .to_string(),
        );
    }
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "deleted_intent": intent, "purge_requested": purge_data, "note": note}),
        )
    } else {
        println!(
            "Deleted pipeline intent '{}'.{}",
            intent,
            note.map(|n| format!(" {n}")).unwrap_or_default()
        );
        Ok(())
    }
}

pub async fn cmd_source_delete(
    mode: OutputMode,
    intent: &str,
    source_name: &str,
    purge_data: bool,
) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let p = repos::get_pipeline_by_intent(&ctx.pool, tenant.id, intent)
        .await?
        .ok_or_else(|| anyhow!("pipeline not found for intent '{intent}'"))?;
    let row = repos::delete_source(&ctx.pool, p.id, source_name)
        .await?
        .ok_or_else(|| anyhow!("source '{}' not found in intent '{}'", source_name, intent))?;
    let mut note = None;
    if purge_data {
        note = Some(
            "purge-data requested (dataset purge not yet implemented in M5; config deleted)"
                .to_string(),
        );
    }
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "deleted": row.name, "intent": intent, "purge_requested": purge_data, "note": note}),
        )
    } else {
        println!(
            "Deleted source '{}' from intent '{}'.{}",
            row.name,
            intent,
            note.map(|n| format!(" {n}")).unwrap_or_default()
        );
        Ok(())
    }
}

pub async fn cmd_pause_resume(
    mode: OutputMode,
    intent: &str,
    source_name: &str,
    paused: bool,
) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let p = repos::get_pipeline_by_intent(&ctx.pool, tenant.id, intent)
        .await?
        .ok_or_else(|| anyhow!("pipeline not found for intent '{intent}'"))?;
    let s = repos::get_source_by_name(&ctx.pool, p.id, source_name)
        .await?
        .ok_or_else(|| anyhow!("source '{}' not found in intent '{}'", source_name, intent))?;
    repos::set_source_paused(&ctx.pool, s.id, paused).await?;
    let action = if paused { "paused" } else { "resumed" };
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "intent": intent, "source": source_name, "paused": paused}),
        )
    } else {
        println!("Source '{}' ({}) {}.", source_name, intent, action);
        Ok(())
    }
}

pub async fn cmd_runs_list(
    mode: OutputMode,
    intent: Option<&str>,
    source: Option<&str>,
) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let rows = repos::list_jobs_for_tenant(&ctx.pool, tenant.id, intent, source, 100).await?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "runs": rows}));
    }
    if rows.is_empty() {
        println!("No runs.");
        return Ok(());
    }
    for r in rows {
        println!(
            "{}\t{}\t{}\tattempt={}\t{}",
            r.id, r.status, r.source_id, r.attempt, r.created_at
        );
    }
    Ok(())
}

pub async fn cmd_runs_show(mode: OutputMode, run_id: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let id = Uuid::parse_str(run_id).map_err(|_| anyhow!("invalid run id"))?;
    let row = repos::get_job_for_tenant(&ctx.pool, tenant.id, id)
        .await?
        .ok_or_else(|| anyhow!("run not found"))?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "run": row}));
    }
    println!("{}", serde_json::to_string_pretty(&row)?);
    Ok(())
}

pub async fn cmd_runs_logs(mode: OutputMode, run_id: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let id = Uuid::parse_str(run_id).map_err(|_| anyhow!("invalid run id"))?;
    let _row = repos::get_job_for_tenant(&ctx.pool, tenant.id, id)
        .await?
        .ok_or_else(|| anyhow!("run not found"))?;
    let logs = repos::list_run_logs(&ctx.pool, id, 1000).await?;
    if mode.is_json() {
        return output::print_json(&json!({"ok": true, "run_id": id, "logs": logs}));
    }
    if logs.is_empty() {
        println!("No logs.");
        return Ok(());
    }
    for l in logs {
        println!("{}\t{}\t{}", l.ts, l.level, l.message);
    }
    Ok(())
}

pub async fn cmd_runs_trigger(mode: OutputMode, intent: &str, source_name: &str) -> Result<()> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let p = repos::get_pipeline_by_intent(&ctx.pool, tenant.id, intent)
        .await?
        .ok_or_else(|| anyhow!("pipeline not found for intent '{intent}'"))?;
    let s = repos::get_source_by_name(&ctx.pool, p.id, source_name)
        .await?
        .ok_or_else(|| anyhow!("source '{}' not found in intent '{}'", source_name, intent))?;
    if jobs::has_active_or_queued_run(&ctx.pool, s.id).await? {
        // record skipped? simplest: inform user
        return Err(anyhow!(
            "source '{}' already has an active or queued run",
            source_name
        ));
    }
    let job = jobs::enqueue_job(&ctx.pool, s.id).await?;
    if mode.is_json() {
        output::print_json(
            &json!({"ok": true, "enqueued": true, "job_id": job.id, "source": source_name, "intent": intent}),
        )
    } else {
        println!(
            "Enqueued run {} for source '{}' (intent {}).",
            job.id, source_name, intent
        );
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedSource {
    pub name: String,
    pub source_type: String,
    pub url: String,
    pub method: Option<String>,
    pub interval: String,
    pub initial_sync: InitialSync,
    pub headers: BTreeMap<String, StringOrSecretRef>,
    pub record_path: Option<String>,
}

pub fn parse_source_spec(spec: &str) -> Result<GeneratedSource> {
    let mut name = None;
    let mut source_type = None;
    let mut url = None;
    let mut method = None;
    let mut interval = None;
    let mut initial_sync = None;
    let mut record_path = None;
    let mut headers = BTreeMap::new();

    for (index, pair) in spec.split(',').enumerate() {
        let (raw_key, raw_value) = pair.split_once('=').ok_or_else(|| {
            anyhow!(
                "source spec item {} must be key=value: '{}'",
                index + 1,
                pair
            )
        })?;
        let key = raw_key.trim();
        let value = raw_value.trim();
        if key.is_empty() || value.is_empty() {
            return Err(anyhow!(
                "source spec item {} must have a non-empty key and value: '{}'",
                index + 1,
                pair
            ));
        }
        match key {
            "name" => name = Some(value.to_string()),
            "type" => source_type = Some(value.to_string()),
            "url" => url = Some(value.to_string()),
            "method" => method = Some(value.to_string()),
            "interval" => interval = Some(value.to_string()),
            "initial_sync" => initial_sync = Some(value.to_string()),
            "record_path" => record_path = Some(value.to_string()),
            "secret" | "ref" => {
                headers.insert(
                    "Authorization".to_string(),
                    StringOrSecretRef::Secret(SecretRef {
                        r#ref: value.to_string(),
                    }),
                );
            }
            header_key if header_key.starts_with("header.") => {
                let header_name = &header_key["header.".len()..];
                if header_name.is_empty() {
                    return Err(anyhow!("header name must not be empty"));
                }
                headers.insert(
                    header_name.to_string(),
                    StringOrSecretRef::Secret(SecretRef {
                        r#ref: value.to_string(),
                    }),
                );
            }
            other => return Err(anyhow!("unknown source spec key '{other}'")),
        }
    }

    let name = name.ok_or_else(|| anyhow!("source spec requires name=<value>"))?;
    let source_type = source_type.unwrap_or_else(|| "http_json".to_string());
    if source_type != "http_json" {
        return Err(anyhow!(
            "unsupported source type '{source_type}' (v1 supports http_json)"
        ));
    }
    let url = url.ok_or_else(|| anyhow!("source spec requires url=<value>"))?;
    let parsed_url = reqwest::Url::parse(&url)
        .map_err(|e| anyhow!("invalid url '{url}' for source '{name}': {e}"))?;
    if !matches!(parsed_url.scheme(), "http" | "https") {
        return Err(anyhow!("url for source '{name}' must use http or https"));
    }
    if parsed_url.host_str().is_none() {
        return Err(anyhow!("url for source '{name}' must include a host"));
    }

    let method = if let Some(method) = method {
        let parsed = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|e| anyhow!("invalid HTTP method '{method}' for source '{name}': {e}"))?;
        Some(parsed.to_string())
    } else {
        None
    };

    let interval = interval.unwrap_or_else(|| "1m".to_string());
    if !is_valid_interval(&interval) {
        return Err(anyhow!(
            "invalid interval '{interval}' for source '{name}' (use 30s, 1m, 5m, 1h)"
        ));
    }

    let initial_sync = initial_sync.unwrap_or_else(|| "full".to_string());
    let initial_sync = InitialSync::from_str(&initial_sync).map_err(|e| anyhow!("{e}"))?;

    Ok(GeneratedSource {
        name,
        source_type,
        url,
        method,
        interval,
        initial_sync,
        headers,
        record_path,
    })
}

async fn stored_sources_for_intent(intent: &Intent) -> Result<(Vec<SourceConfig>, Option<Value>)> {
    let ctx = AppCtx::connect().await?;
    let (tenant, _) = require_tenant(&ctx.pool).await?;
    let pipeline = repos::get_pipeline_by_intent(&ctx.pool, tenant.id, intent.as_str())
        .await?
        .ok_or_else(|| {
            anyhow!(
                "no stored pipeline for intent '{}'; pass --source or copy config/pipeline.example.yaml",
                intent.as_str()
            )
        })?;
    let rows = repos::list_sources(&ctx.pool, pipeline.id).await?;
    let mut sources = Vec::new();

    for row in rows {
        let mut value = row.config;
        let obj = value
            .as_object_mut()
            .ok_or_else(|| anyhow!("stored config for source '{}' is not an object", row.name))?;
        obj.insert("name".into(), Value::String(row.name.clone()));
        obj.insert("type".into(), Value::String(row.r#type));
        obj.insert(
            "schedule".into(),
            json!({"interval": row.schedule_interval}),
        );
        obj.insert("initial_sync".into(), Value::String(row.initial_sync));
        let source: SourceConfig = serde_json::from_value(value)
            .with_context(|| format!("stored config for source '{}' is invalid", row.name))?;
        sources.push(source);
    }

    if sources.is_empty() {
        return Err(anyhow!(
            "stored pipeline for intent '{}' has no sources",
            intent.as_str()
        ));
    }

    let processing = pipeline
        .config
        .get("processing")
        .filter(|value| !value.is_null())
        .cloned();
    Ok((sources, processing))
}

fn build_source_config(source: GeneratedSource) -> SourceConfig {
    let mut extra = BTreeMap::new();
    extra.insert("url".to_string(), Value::String(source.url));
    if let Some(method) = source.method {
        extra.insert("method".to_string(), Value::String(method));
    }
    SourceConfig {
        name: source.name,
        source_type: source.source_type,
        schedule: SourceSchedule {
            interval: source.interval,
        },
        initial_sync: source.initial_sync,
        credentials: None,
        headers: source.headers,
        query_params: BTreeMap::new(),
        format: None,
        record_path: source.record_path,
        flatten: false,
        error_policy: "fail".to_string(),
        extra,
    }
}

pub async fn cmd_generate(
    mode: OutputMode,
    intent: &str,
    file: &str,
    source_specs: &[String],
    append: bool,
    force: bool,
) -> Result<()> {
    let intent = Intent::from_str(intent).map_err(|e| anyhow!("{e}"))?;
    let mut stored_processing = None;
    let mut generated_sources: Vec<SourceConfig> = if source_specs.is_empty() {
        let (sources, processing) = stored_sources_for_intent(&intent).await?;
        stored_processing = processing;
        sources
    } else {
        source_specs
            .iter()
            .map(|spec| parse_source_spec(spec))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .map(build_source_config)
            .collect()
    };

    let mut seen = BTreeSet::new();
    for source in &generated_sources {
        if !seen.insert(source.name.clone()) {
            return Err(anyhow!("duplicate source name '{}'", source.name));
        }
    }

    let file_path = Path::new(file);
    let existed = file_path.exists();
    let config = if append {
        if !existed {
            return Err(anyhow!("cannot append: file '{file}' does not exist"));
        }
        let (mut existing, _) = load_pipeline_file(file)?;
        if existing.intent != intent {
            return Err(anyhow!(
                "cannot append intent '{}' to file with intent '{}'",
                intent,
                existing.intent
            ));
        }
        for source in &existing.sources {
            if !seen.insert(source.name.clone()) {
                return Err(anyhow!("duplicate source name '{}'", source.name));
            }
        }
        existing.sources.append(&mut generated_sources);
        existing
    } else {
        if existed && !force {
            return Err(anyhow!(
                "file '{file}' already exists (use --force to overwrite)"
            ));
        }
        PipelineConfig {
            intent,
            sources: generated_sources,
            processing: stored_processing,
        }
    };

    validate_pipeline_cfg(&config)?;
    let yaml = config
        .to_yaml_string()
        .map_err(|e| anyhow!("failed to render YAML: {e}"))?;

    if let Some(parent) = file_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory '{}'", parent.display()))?;
        }
    }
    fs::write(file_path, yaml).with_context(|| format!("failed to write '{file}'"))?;

    let action = if append {
        "appended"
    } else if existed {
        "overwrote"
    } else {
        "wrote"
    };
    if mode.is_json() {
        output::print_json(&json!({
            "ok": true,
            "action": action,
            "file": file,
            "intent": config.intent.as_str(),
            "sources": config.sources.len(),
        }))
    } else {
        println!(
            "{} {} source(s) for intent {} to '{}'.",
            action,
            config.sources.len(),
            config.intent,
            file
        );
        Ok(())
    }
}

pub async fn cmd_init(mode: OutputMode, source: Option<&str>, intent: Option<&str>) -> Result<()> {
    let tpl = match (source, intent) {
        (Some(s), None) => init_template_source(s),
        (None, Some(i)) => init_template_intent(i),
        (Some(_), Some(_)) => return Err(anyhow!("specify either --source or --intent, not both")),
        (None, None) => return Err(anyhow!("specify --source <type> or --intent <intent>")),
    };
    if mode.is_json() {
        output::print_json(&json!({"ok": true, "template": tpl}))
    } else {
        print!("{tpl}");
        Ok(())
    }
}

pub async fn cmd_list(mode: OutputMode, kind: &str) -> Result<()> {
    let items: Vec<&str> = match kind {
        "sources" => list_sources_catalog(),
        "intents" => list_intents_catalog(),
        "formats" => list_formats_catalog(),
        _ => {
            return Err(anyhow!(
                "unknown list kind '{kind}' (use sources|intents|formats)"
            ))
        }
    };
    if mode.is_json() {
        output::print_json(&json!({"ok": true, "kind": kind, "items": items}))
    } else {
        for i in items {
            println!("{i}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_source_spec() {
        let source = parse_source_spec(
            "name=primary,url=http://localhost:8080/data,secret=api_token,interval=5m,initial_sync=latest",
        )
        .unwrap();
        assert_eq!(source.name, "primary");
        assert_eq!(source.url, "http://localhost:8080/data");
        assert_eq!(source.interval, "5m");
        assert_eq!(source.initial_sync, InitialSync::Latest);
        assert_eq!(source.headers.len(), 1);
    }

    #[test]
    fn rejects_invalid_source_specs() {
        assert!(parse_source_spec("name=x,url=not-a-url").is_err());
        assert!(parse_source_spec("name=x,url=http://localhost,interval=9x").is_err());
        assert!(parse_source_spec("name=x,url=http://localhost,type=sql").is_err());
        assert!(parse_source_spec("name=x,url=http://localhost,unknown=1").is_err());
    }
}
