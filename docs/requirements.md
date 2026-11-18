# Talaint Requirements

v1 foundation requirements. Only agreed decisions are recorded here. Items explicitly out of scope are listed in §21.

## Problem and Core Requirements

**Problem:** Companies have data spread across many sources and need it AI-ready, but building ingestion and preparation pipelines is expensive and repetitive.

**Core requirements**
1. Multi-tenant: isolate each tenant's data, secrets, and storage.
2. Configure via CLI YAML; configs stored in PostgreSQL (no stored files in git).
3. Connect sources and read incrementally from checkpoints with correct deduplication.
4. Declare one explicit intent per pipeline: RAG, fine-tuning, ML, multimodal.
5. Run autonomously per source schedule with reliable, non-duplicated jobs.
6. Prepare data per intent: parse, clean, chunk, PII-mask, embed, format, cast.
7. Store AI-ready datasets in tenant-isolated LanceDB, one dataset per intent.
8. Secure secrets and config end to end.
9. Let downstream AI apps consume the prepared datasets.

## 1. Core Model

```text
Tenant
  └── Pipeline (one intent, one destination dataset)
        └── Sources (many; each has its own config, credentials ref, schedule, checkpoint)
```

- Tenant identity is derived from the authenticated CLI token (never declared in YAML). Tenants are created by an admin and issued a one-time access token (§10).
- A tenant can have **multiple pipelines**, but **at most one pipeline per intent** (`rag`, `fine_tuning`, `ml_training`, `multimodal`).
- A **pipeline** is identified by `(tenant, intent)` and maps to exactly one destination dataset for that intent.
- A pipeline can have **many sources**; all feed the pipeline's single intent dataset. **Job = one source run**. Sources are scheduled independently.
- Each **source** has a unique `name` within its pipeline and belongs to exactly one pipeline.
- PostgreSQL is the source of truth; runtime always reads configuration from the DB, not YAML files.

## 2. Intents

Intent is **explicit**, never inferred.

| Intent        | Dataset             | Output schema                                      |
|---------------|---------------------|----------------------------------------------------|
| `rag`         | `rag_documents`     | `id, text_chunk, metadata, vector, source_ref`     |
| `fine_tuning` | `finetune_datasets` | `prompt, completion` (+ optional `system`/metadata)|
| `ml_training` | `tabular_features`  | typed tabular columns + optional label             |
| `multimodal`  | `multimodal_assets` | `id, media_ref, text, vector, metadata`            |

- Each intent maps to exactly one dataset in the tenant's LanceDB.
- v1 multimodal scope is **images + text** (OCR-extracted text stored in `text`); audio/transcripts and AI captions are deferred.
- Supporting multiple intents per pipeline is deferred.

## 3. Configuration & Apply Semantics

- **YAML:** one file defines one pipeline. Required top-level fields: `intent`, `sources` (non-empty list). Tenant is never declared in YAML.
- **Source (v1 common fields):** `name` (required, unique within pipeline), `type` (required), source-specific connection/location fields (e.g. `url` for HTTP), optional `schedule`, `initial_sync`, `credentials`, and parsing/ingestion options per §§4,11.
- **Flow:** user writes YAML -> `talaint apply -f <file>` validates using the shared schema -> core runs health checks -> core encrypts/stores referenced secrets as needed by secret commands/config refs -> persists config in PostgreSQL.
- **Re-apply (override):** `apply` upserts the pipeline for the given `intent` and **fully reconciles sources** to the YAML: sources present are added/updated by `name`; sources in DB but not in YAML are removed.
- **On source removal or rename:** scheduling/checkpoint/config state for that source is removed and it stops running. Renaming is treated as remove + add (checkpoint resets). Contributed dataset records are **kept by default**; removing data is an explicit user action (`--purge-data`, §9, §18). Run/job history may be retained.
- **In-place updates:** foundation uses in-place config updates in PostgreSQL (no pipeline version history/rollback in v1).
- **v1 mode:** CLI runs in-process with the core (no HTTP API/server mode).

## 4. Parsing Configuration

Parsing lives on the **source**, not the pipeline or intent.

- **Required (general):** `name`, `type`, and the connector's location/connection field(s) (e.g. `url` for HTTP).
- **Common options:** `format` (inferred by default), `record_selector`/record path, `encoding`, CSV dialect, `include`/`exclude`, `metadata_map`, `flatten`.
- **Advanced options (as supported):** parser engine, OCR/page range, size limits/timeouts.
- **Error policy:** default `fail` (record/parse/processing error fails the run). `skip` and `max_errors` are deferred in v1.
- Defaults must cover common cases; advanced fields stay optional.

## 5. Templates and Discovery UX

- **Templates:** `init --source <type>`, `init --intent <intent>`
- **Discovery:** `list sources`, `list intents`, `list formats`
- **Validation:** `validate -f <file>`
- **Preview:** `apply --dry-run -f <file>` (shows plan/parse intent, destination dataset, changes)
- **Health check:** `source test -f <file>`

Templates include commented defaults. Validation errors state what is missing and list valid options.

## 6. Secrets

- Secrets are **named and tenant-scoped** (`secrets(tenant_id, name, ...)`); one secret may be shared by many sources.
- A source references a secret by name (e.g. `credentials: { ref: "<name>" }` or header/query value refs). Each connector declares required keys; validation checks references exist and contain required keys.
- **Env var mapping:** secret name maps to `TALAINT_SECRET_<NORMALIZED_NAME>` where `<NORMALIZED_NAME>` is uppercased with non-alphanumeric characters replaced by `_` (e.g. `aws-prod` -> `TALAINT_SECRET_AWS_PROD`).
- **`secrets set <name>`:** reads the value from the mapped env var, encrypts it in-process, and stores/upserts it in PostgreSQL. `apply` requires referenced secrets to already exist in DB.
- **Protection (envelope encryption):** KEK from environment/KMS; per-tenant DEK generated once and stored wrapped in PostgreSQL. Secrets encrypted with **AES-256-GCM** storing `nonce`, `ciphertext`, `key_version`.
- Decrypt only in the worker/runtime, in memory, per job. Never log secrets or expose them through outputs/LLM context. YAML contains only secret references.
- Minimal storage: `tenant_keys(tenant_id, wrapped_dek, key_version)`, `secrets(tenant_id, name, nonce, ciphertext, key_version)`.
- `key_version` enables future rotation/KMS migration. Audit secret access.

## 7. Health Checks

- `source test` and `apply --dry-run` run a lightweight connector check per source (as applicable to source type).
- `apply` runs the same checks and **fails by default** if a source is unhealthy.
- Errors identify the source, the failed check, and how to fix it.
- `apply --force` allows proceeding despite a failed health check.

## 8. Scheduling

- Each source has its own schedule and is scheduled independently.
- **v1 (simple polling):** `schedule.interval` is an optional duration string; default `1m`. Supported units: `s`, `m`, `h` (e.g. `30s`, `1m`, `5m`, `1h`).
- A scheduler polls PostgreSQL for due sources and inserts jobs into `pipeline_jobs` (no `PG NOTIFY` in v1).
- Workers claim jobs via Postgres `SKIP LOCKED`. One active run per source: if previous run is active at tick time, skip the tick and record it as skipped.
- **Pause/resume:** pause stops scheduling new runs; an active run continues. Resume schedules from the next tick.

## 9. Storage and Concurrency (LanceDB)

- Each tenant has isolated storage: `tenants/{tenant_id}/ai_store.lance/` with one dataset per intent (`rag_documents`, `finetune_datasets`, `tabular_features`, `multimodal_assets`).
- One pipeline per `(tenant, intent)` -> one dataset. Reads are snapshot-isolated; writes are versioned with optimistic concurrency.
- Parallelism across tenants and across intents is fine. **One writer per `(tenant, intent)` dataset** (sources may fetch/prepare in parallel, final write serialized per dataset).
- **Updates replace per record:** when a record changes, delete existing rows by record identifier and insert new rows. **Deletes** propagate only when the source reports them (tombstone/deleted flag); otherwise data is left untouched. No append-only history.
- **Data removal is explicit/user-driven:** removing a source or pipeline config does **not** purge contributed data by default. `pipelines delete --intent <intent> --purge-data` deletes that intent dataset; `source delete --intent <intent> --source <name> --purge-data` deletes records attributable to that source from the intent dataset.

## 10. Control Plane: Tenants & Auth

- Tenants are **created by admins** (not self-serve) and provided a one-time access token to use the CLI.
- One admin account manages tenants and tokens. Admin authenticates via separate admin token (`TALAINT_ADMIN_TOKEN` or `auth login --admin`).
- Each tenant has **one access token** for CLI access; tenant context comes from the authenticated token.
- Access and admin tokens are **hashed at rest** and shown only once at creation.
- Tenant can rotate its own token while authenticated (old token invalidated immediately). Admin can rotate/issue a tenant token if locked out.
- Tenant limits are deferred.

## 11. Ingestion: Polling (v1)

- v1 ingestion is **polling-only**. Webhooks and continuous streams/Kafka/CDC are deferred.
- **Initial connector focus:** configurable HTTP JSON API polling with correct incremental handling and deduplication.
- **Minimal HTTP JSON source config:**
  - Required: `name`, `type: http_json`, `url`
  - Optional (defaults): `method` (`GET`), `headers` (map; values may be literals or secret refs), `query_params` (map), `body`, `record_path` (auto-detect common JSON paths if omitted), `id_field`/`cursor_field` (connector hints), `pagination` (connector-supported config), `flatten` (false), `schedule.interval` (`1m`), `initial_sync` (`full`|`latest`, default `full`), `credentials`/secret refs as needed
- **Connector-owned internals:** cursor management, stable ID/content hash, format detection, incremental logic, and health-check behavior.
- **Incrementality:** use cursor/timestamp when supported; otherwise deduplicate by stable ID + content hash. `initial_sync`: `full` (default) or `latest`.
- **Execution:** one active run per source; skip-and-record if a tick fires while running. On failure retry **3 times with exponential backoff**, then mark failed. Save checkpoint only after the **whole source run** succeeds; retries reprocess from previous checkpoint and record replacement keeps runs idempotent.

## 12. Webhooks and Subscriptions (Deferred)

- **Webhooks:** deferred in v1 (no HTTP receiver/runtime). Future: per-source webhook URL/signing secret, configurable payload/event-id extraction, hash fallback.
- **Subscriptions (Kafka/CDC):** deferred. Future: source-native positions as cursors (e.g. Kafka offsets, Postgres LSN).

## 13. Processing: RAG

Default chain: **parse -> clean -> PII mask -> chunk -> embed -> store**.

- **PII masking:** on by default; user can disable/adjust when supported.
- **Chunking:** structure-aware recursive splitting (headings -> paragraphs -> sentences -> token limit). Defaults ~800 tokens/chunk, ~15% overlap; configurable (size, overlap, unit, boundaries) as supported.
- **Embeddings:** fixed local in-process model for v1 (BGE-M3, 1024-dim) via ONNX/fastembed (no external service).
- **Chunk metadata:** includes `text`, `vector`, `source_ref`, `record_id`/chunk id, `chunk_index`, `content_hash`, `title`/`section` (when available), `updated_at`. Tenant/pipeline isolation via storage path, not per-row tenant IDs.

## 14. Processing: Fine-Tuning

- `prompt`, `completion`, and optional `system` defined in a template file referenced by config (Jinja2 placeholders).
- If a record cannot be rendered (missing field, empty prompt/completion), the **run fails**.
- Duplicate prompt/completion pairs are deduplicated by content hash.
- Output is one flat dataset by default (deterministic split/train-validation views deferred).

## 15. Processing: ML Training

- v1 tabular sources: CSV, Parquet, SQL rows (nested JSON flattening deferred).
- User **declares schema** (columns, types, optional label); schema is fixed/strict (validation may suggest from sample).
- Generic deterministic prep: cast types, enforce schema, dedupe, keep columns consistent. Model-specific transforms left to training pipelines.
- Null handling: drop rows where all values are null; drop columns where all values are null; keep partial nulls.

## 16. Processing: Multimodal

- v1: **images + text** (audio/transcripts deferred).
- Run **OCR** to extract text; generate embeddings in a shared local multimodal space (e.g. CLIP/SigLIP) for image/text search. AI captions deferred.
- Store: `media_ref`, OCR-extracted `text`, `vector`, and `metadata` (per §2).

## 17. Logs & Runs

- Each run exposes a **summary**: records read, written, skipped, failed, duration, status.
- Detailed logs available on demand; stored in PostgreSQL for now.
- **Job statuses:** `queued -> running -> succeeded | failed`, plus `skipped` for missed ticks.
- Workers send heartbeat every **30s**; job considered stuck after **~2 minutes** without heartbeat (configurable). Stuck jobs requeued up to **3 times**, then marked `failed`.

## 18. CLI (v1)

All commands run in-process with core, scoped to authenticated tenant/admin context, and support `--output json` for structured output (in addition to human-readable default) where applicable.

- **Auth:** `auth login`, `token rotate`
- **Templates/discovery:** `init --source <type>`, `init --intent <intent>`, `list sources|intents|formats`
- **Config & validation:** `validate -f <file>`, `apply -f <file> [--dry-run] [--force]`, `source test -f <file>`
- **Pipelines:** `pipelines list`, `pipelines show --intent <intent>`, `pipelines delete --intent <intent> [--purge-data]`
- **Sources:** `source delete --intent <intent> --source <name> [--purge-data]`, `pause --intent <intent> --source <name>`, `resume --intent <intent> --source <name>`
- **Runs/logs:** `runs list [--intent <intent>] [--source <name>]`, `runs show <run_id>`, `runs logs <run_id>`, `runs trigger --intent <intent> --source <name>`
- **Secrets:** `secrets set <name>`, `secrets list`, `secrets delete <name>`
- **Agent:** `chat` (see §22)
- **Admin:** `admin tenant create`, `admin tenant list`, `admin token rotate <tenant>`

## 19. Admin

- Admin authenticates with a separate admin token (not a password), via `TALAINT_ADMIN_TOKEN` or `auth login --admin`.
- Admin commands: `admin tenant create`, `admin tenant list`, `admin token rotate <tenant>`.

## 20. Jobs and Queue

- Job statuses: `queued -> running -> succeeded | failed`, plus `skipped`.
- Claim via PostgreSQL `SKIP LOCKED`; one active run per source.
- Heartbeat every 30s; stuck after ~2m without heartbeat (configurable). Requeue stuck jobs up to 3 times, then `failed`; retain logs.

## 21. Deferred / Out of Scope (v1 foundation)

- Pipeline version history/immutability and rollback (in-place updates only).
- Webhook ingestion/runtime and continuous streams/Kafka/CDC.
- `PG NOTIFY`, HTTP API/server mode, remote CLI-over-TLS apply.
- Multiple intents per pipeline.
- Complex scheduling/triggers beyond simple interval polling.
- `skip`/`max_errors` error policies; advanced parsing/processing options beyond those supported.
- Downstream AI consumption/access model.
- Tenant/resource limits and quotas.

## 22. Conversational Agent Harness

`talaint chat` is a conversational configuration interface built into the CLI/core (in-process in v1). It is an interaction layer over deterministic CLI/core operations, not a bypass.

- **Boundaries:** assists with discovery, drafting/refining config, validation, dry-run, and explaining errors/options. Never accesses PostgreSQL, LanceDB, or secret values directly; all actions go through sandboxed, tenant-scoped internal operations. Secret values are never exposed to the LLM.
- **Tools & safety:** read-only operations auto-approved (discovery, inspect, validate, dry-run/preview, health checks, list/show). Mutating operations require explicit confirmation `[y/N]` with a clear summary/diff before execution (`apply`, delete/purge, `secrets set/delete`, pause/resume, `runs trigger`). Tool calls are strictly typed/whitelisted (no raw shell) and tenant-scoped.
- **Execution:** bounded agent loop with max-step limit; structured CLI/core errors fed back for self-correction; context distillation keeps prompts bounded (system rules, current draft, recent turns).
- **LLM backend:** configurable OpenAI-compatible endpoint (e.g. local Ollama or cloud). Provider/model must not change safety, scoping, or confirmation behavior.
- **Evaluation:** maintain eval scenarios verifying inspect/validate-before-mutate, valid intent configs, confirmation gates, and safe error handling.
