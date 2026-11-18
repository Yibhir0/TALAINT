# Talaint Architecture (v1 Foundation)

Implementation blueprint aligned with `docs/requirements.md`. v1 is CLI in-process with core, PostgreSQL control plane, simple polling ingestion, tenant-isolated LanceDB, and a conversational agent harness. Versioning/rollback, webhooks, streams/CDC, `PG NOTIFY`, and HTTP server mode are deferred.

## 1. End-to-End Architecture (v1)

```text
================================================================================
  LAYER 0: INTERACTION (CLI + Conversational Agent)            [crates/cli, agent]
================================================================================
  User (terminal)
    |
    +-- Deterministic CLI: auth/init/list/validate/apply/source test
    |                     pipelines/source/runs/secrets/pause/resume/chat/admin
    |                     (all support structured --output json where applicable)
    |
    +-- `talaint chat` (Agent Harness, req §22)
          +------------------------------------------------------------------+
          | 1) Prompt & State (system rules, draft config, recent turns)     |
          | 2) LLM Client (configurable OpenAI-compatible endpoint)          |
          | 3) Sandboxed Tool Router (typed, whitelisted internal ops only)  |
          | 4) Bounded loop + self-correction (structured errors fed back)   |
          | 5) Human-in-the-loop gate for mutating ops (summary/diff + y/N)  |
          +------------------------------------------------------------------+
                             | in-process, tenant-scoped calls (no raw shell)
                             | never direct DB/LanceDB/secret-value access
                             v
================================================================================
  LAYER 1: CORE / CONTROL LOGIC (in-process library)   [crates/core, control-plane]
================================================================================
  +------------------------------------------------------------------------------+
  | Config schema & validation        | Pipeline/source management (in-place)    |
  | Secret vault (KEK env/KMS,        | Scheduler (poll DB for due sources)      |
  |  per-tenant DEK, AES-256-GCM)     | Job lifecycle, retries, heartbeats       |
  | Health checks & dry-run planning  | Tenant/auth/admin token verification     |
  +-----------------------------------+------------------------------------------+
                                      | sqlx
                                      v
================================================================================
  LAYER 2: CONTROL PLANE (PostgreSQL, source of truth)
================================================================================
  tenants, tenant_keys, secrets,
  pipelines (per tenant+intent), sources (unique name per pipeline),
  schedules, pipeline_jobs, pipeline_checkpoints,
  run_summaries/logs, token/admin records, (audit as needed)

  - Job claim: FOR UPDATE SKIP LOCKED        - One active run per source
  - Heartbeat ~30s, stuck after ~2m, requeue up to 3x then failed
  - Scheduler polls DB for due work (no PG NOTIFY in v1)

================================================================================
  LAYER 3: DATA PLANE / WORKER RUNTIME (Rust, tokio)  [sources, processors, dest]
================================================================================
  [Worker/Supervisor]
     | claim job -> hydrate TenantContext (tenant, pipeline intent/config,
     | decrypted secrets in-memory, checkpoint, schedule)
     v
  [Source Connector (polling-first, e.g. HTTP JSON)] --(bounded stream/backpressure)-->
     |  reads from cursor; incremental by cursor/timestamp or stable ID+content hash
     v
  [Intent Router & Processors (per intent)]
     |  RAG: parse -> clean -> PII mask -> chunk -> embed (local BGE-M3) -> store
     |  Fine-tuning: render template -> validate -> dedupe -> store
     |  ML (tabular): enforce declared schema/cast/dedupe/null rules -> store
     |  Multimodal (images+text): OCR -> text + local multimodal embed -> store
     v
  [Dataset Writer]
     - per-record replace (delete by record id + insert), idempotent retries
     - serialize final write per (tenant,intent); commit checkpoint only on full success

================================================================================
  LAYER 4: STORAGE (Tenant-Isolated LanceDB)
================================================================================
  tenants/{tenant_id}/ai_store.lance/
    ├── rag_documents.lance
    ├── finetune_datasets.lance
    ├── tabular_features.lance
    └── multimodal_assets.lance

  - one pipeline per (tenant,intent) -> one dataset; snapshot-isolated reads
  - parallel across tenants/intents; writes serialized per (tenant,intent) dataset
```

## 2. Key Flows

- **Config/apply (override):** YAML (one pipeline: `intent` + `sources`, tenant from auth, secret refs only) -> validate -> health checks/dry-run -> persist pipeline and fully reconcile sources in Postgres (add/update by name, remove missing). Source removal/rename drops source scheduling/checkpoint/config (rename = reset checkpoint); contributed data kept unless explicitly purged via CLI.
- **Schedule & run:** scheduler polls DB for due sources (simple interval, default 1m) -> enqueues `pipeline_jobs`; worker claims with `SKIP LOCKED`; skip+record if source already running; pause stops new scheduling only. Manual trigger: `runs trigger --intent <intent> --source <name>`.
- **Execution:** hydrate context + prior checkpoint -> poll source incrementally -> process per intent -> write dataset -> commit checkpoint and mark succeeded/failed (retry 3 w/ exponential backoff; reprocess from last checkpoint, idempotent via replacement/dedup).
- **Secrets:** `secrets set <name>` reads from env `TALAINT_SECRET_<NORMALIZED_NAME>`, encrypts (KEK -> per-tenant DEK, AES-256-GCM) and stores; `apply` requires referenced secrets exist. Decrypted only in-memory per job.
- **Data removal:** explicit only — `pipelines delete --intent <intent> [--purge-data]`, `source delete --intent <intent> --source <name> [--purge-data]`.
- **Chat:** user intent -> bounded sandboxed tool loop over tenant-scoped ops -> validate/dry-run -> confirm `[y/N]` before mutating actions -> same core path as CLI.

## 3. v1 Decisions (resolved)

- **Pipeline identity:** keyed by `(tenant, intent)`; one pipeline per intent per tenant. YAML requires `intent`, `sources`.
- **Source identity:** unique `name` within pipeline. `apply` fully reconciles sources to YAML; rename = remove+add. Removed sources keep contributed data by default.
- **Schedule:** simple interval polling, `schedule.interval` with `s|m|h` units, default `1m`. No cron/`PG NOTIFY`.
- **Manual trigger:** `runs trigger --intent <intent> --source <name>`.
- **Error policy:** default `fail` (`skip`/`max_errors` deferred).
- **Secrets:** `secrets set <name>` reads `TALAINT_SECRET_<NORMALIZED_NAME>` env var and upserts; refs must exist on apply.
- **CLI output:** `--output json` supported for agent/tooling; human-readable default.
- **Initial source:** HTTP JSON polling (`type: http_json`, required `name`, `type`, `url`; method/headers/params/record path/interval/initial_sync configurable with defaults).
- **Multimodal:** images + OCR text stored as `text` with `media_ref`, `vector`, `metadata`.

## 4. Deferred (not in v1)

- Pipeline version history/rollback (in-place config only)
- HTTP API/server mode, remote CLI-over-TLS, `PG NOTIFY`
- Webhook ingestion/runtime, continuous streams/Kafka/CDC
- Multiple intents per pipeline, tenant/resource limits
- Complex scheduling/triggers, `skip`/`max_errors`, downstream consumption model

## 5. Workspace Mapping (target)

```text
crates/
  cli/            # clap entrypoint, commands, --output json, admin/auth UX
  agent/          # harness loop, tool schemas/router, sandbox, distiller, LLM client
  core/           # config schema/models, intent contracts, TenantContext, traits, errors
  control-plane/  # sqlx repos, migrations helpers, scheduler, queue/claim, secrets vault
  sources/        # connectors (polling-first, HTTP JSON), health/incremental/checkpoint logic
  processors/     # parse/clean/PII/chunk/embed & per-intent transformers
  destinations/   # LanceDB writer, per-(tenant,intent) write serialization
migrations/       # PostgreSQL DDL
evals/            # agent/config scenario cases
docs/             # requirements, architecture, specs
```

## 6. Incremental Development Plan

### Milestone 0 - Foundations
- Initialize Cargo workspace and crate skeletons (`cli`, `core`, `control-plane`, `sources`, `processors`, `destinations`, `agent`) + `migrations/`, `evals/`.
- Add shared config schema/types (pipeline intent, sources, schedule, secret refs) and common error/result types in `core`.
- Define local dev conventions (Postgres, KEK env, `TALAINT_SECRET_*`).

### Milestone 1 - Control Plane & Persistence
- PostgreSQL migrations: tenants/admin/tokens (hashed), tenant_keys, secrets, pipelines (by tenant+intent), sources, schedules, jobs, checkpoints, run summaries/logs.
- `control-plane`: repositories/transactions, job states, `SKIP LOCKED` claim, heartbeat/stuck detection/requeue, one-active-run guards.

### Milestone 2 - Config, Secrets, and Deterministic CLI
- YAML parsing/validation, templates/init, discovery, `--output json`, dry-run/plan output.
- Envelope encryption and `secrets set|list|delete` (env-based ingestion) + secret-ref resolution.
- `apply` (reconcile/upsert), `validate`, `source test`, pipeline/source show/list/delete (incl. `--purge-data`), pause/resume, runs list/show/logs/trigger, auth/admin - in-process.

### Milestone 3 - Scheduling & Polling Ingestion
- DB-polling scheduler for due sources (interval-based) with skip-on-active.
- Polling source framework + HTTP JSON connector: request/pagination, record extraction, stable ID/hash or cursor incremental sync, `initial_sync`, checkpoints.

### Milestone 4 - Processing Pipelines (by intent)
- RAG: parse -> clean -> PII mask (default on) -> chunk -> local embed (BGE-M3) -> store.
- Fine-tuning: template render, fail-on-unrenderable, content-hash dedupe.
- ML (tabular): strict declared schema, cast/enforce/dedupe, null rules.
- Multimodal (images+text): OCR + local multimodal embeddings -> store.

### Milestone 5 - LanceDB Storage & Idempotent Writes
- Tenant-isolated LanceDB destination and per-intent datasets/schemas.
- Per-(tenant,intent) write serialization, per-record replace, delete propagation when signaled, checkpoint commit only after full success; explicit purge operations.

### Milestone 6 - Conversational Agent (`talaint chat`)
- `crates/agent`: typed tool schemas, sandbox/router with tenant scoping, context distiller, configurable LLM client, bounded self-correcting loop.
- Read-only auto-approve vs mutating confirmation (`[y/N]` with diff/summary), no DB/storage/secret-value exposure.
- `evals/` scenarios for tool order, valid configs, confirmations, error recovery.

### Milestone 7 - Hardening & E2E Verification
- Run summaries/detailed logs, audit for secret/admin actions, failure/retry/stuck-run validation.
- End-to-end tests per intent and incremental/idempotent re-runs; backpressure sanity checks.
- Capture finalized YAML/DB/CLI details in `docs/spec-v1.md` and keep docs in sync.

## 7. Definition of Ready to Start Coding

- `docs/requirements.md` and this architecture accepted as v1 foundation baseline.
- Decisions in §3 reflected in implementation (config schema, DB schema, CLI contract) — detail in `docs/spec-v1.md` during M1–M2.
- First slice: **M0 -> M1 -> M2** minimal `validate`/`apply`/config persistence for one pipeline intent with one HTTP JSON source.
