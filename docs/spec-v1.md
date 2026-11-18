# Talaint v1 Spec (Foundation)

Concrete spec for v1 foundation, aligned with `docs/requirements.md` and `docs/architecture.md`. Keep it simple; expand later.

## 1. YAML Config Schema

- **One file = one pipeline** for the authenticated tenant (tenant never in YAML).
- **Apply is by `intent`** (one pipeline per `(tenant,intent)`), with **full source reconciliation** by source `name`.

### 1.1 Top-level (pipeline)

| Field | Type | Required | Notes |
|---|---|---|---|
| `intent` | enum | yes | `rag` \| `fine_tuning` \| `ml_training` \| `multimodal` |
| `sources` | list[Source] | yes | non-empty; each `name` unique within pipeline |
| `processing` | object | no | intent-specific options (defaults apply) |

### 1.2 Source (common)

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `name` | string | yes | - | unique within pipeline; stable id for updates/reconcile |
| `type` | string | yes | - | connector type (v1: `http_json`) |
| `schedule.interval` | duration | no | `1m` | simple interval, units `s`,`m`,`h` (e.g. `30s`,`1m`,`5m`) |
| `initial_sync` | enum | no | `full` | `full` \| `latest` |
| `credentials.ref` | string | no | - | tenant-scoped secret name (must exist in DB on apply) |
| `headers` | map[string,string\|SecretRef] | no | `{}` | values literal or `{ ref: "<secret>" }` (as supported) |
| `format` | string | no | inferred | connector may infer |
| `record_path` | string | no | auto | path/selector to records array/items when needed |
| `flatten` | bool | no | `false` | optional flatten after extraction |
| `error_policy` | enum | no | `fail` | v1: `fail` only (`skip`/`max_errors` deferred) |

`SecretRef` object: `{ "ref": "<secret_name>" }`.

### 1.3 HTTP JSON source (`type: http_json`)

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `url` | string | yes | - | HTTP endpoint to poll |
| `method` | string | no | `GET` | e.g. GET/POST |
| `headers` | map | no | `{}` | request headers; may include secret refs |
| `query_params` | map[string,string] | no | `{}` | query parameters |
| `body` | any | no | - | request body for non-GET if used |
| `record_path` | string | no | auto-detect | JSON path to record list/item |
| `id_field` | string | no | connector | hint for stable record id |
| `cursor_field` | string | no | connector | hint for cursor/timestamp |
| `pagination` | object | no | none | connector-supported pagination config (TBD details per connector) |

Connector owns cursor/hash/incremental/health-check internals and dedupes by cursor or `stable_id + content_hash`.

### 1.4 Intent-specific `processing` (v1 minimal)

- **`rag`** (defaults): parse -> clean -> PII mask (on) -> chunk (~800 tokens, ~15% overlap, structure-aware) -> embed (local BGE-M3, 1024-dim). Optional overrides (as implemented): chunk size/overlap, pii on/off.
- **`fine_tuning`**: `processing.template` (required path/ref to template file) with Jinja2 placeholders for `prompt`, `completion`, optional `system`; fail on unrenderable/empty required fields; dedupe by content hash.
- **`ml_training`**: `processing.schema` required, declared and strict — `columns: [{name, type, nullable?}]`, optional `label`. Tabular inputs (CSV/Parquet/SQL rows); null rules per requirements.
- **`multimodal`** (images+text): OCR -> `text`, local multimodal embedding -> `vector`, store `media_ref`, `metadata` (mostly defaults in v1).

### 1.5 Examples

**RAG over HTTP JSON (minimal):**
```yaml
intent: rag
sources:
  - name: tickets-api
    type: http_json
    url: http://localhost:8080/tickets
    method: GET
    schedule:
      interval: 1m
    initial_sync: full
    # credentials:
    #   ref: tickets_api
    # headers:
    #   Authorization: { ref: tickets_api }  # if connector maps ref->value
```

**Fine-tuning (sketch):**
```yaml
intent: fine_tuning
sources:
  - name: support-export
    type: http_json
    url: http://localhost:8080/export
    schedule: { interval: 5m }
processing:
  template: ./templates/support_ft.jinja
```

**ML training (sketch):**
```yaml
intent: ml_training
sources:
  - name: orders-csv-api
    type: http_json
    url: http://localhost:8080/orders
processing:
  schema:
    columns:
      - { name: order_id, type: string }
      - { name: amount, type: float }
      - { name: churned, type: int }
    label: churned
```

## 2. Apply & Identity Semantics

- **Pipeline key:** `(tenant_id, intent)`; apply upserts pipeline for `intent`.
- **Source key:** `name` within pipeline. Reconcile = add/update sources in YAML, **remove** DB sources not in YAML.
- **Rename:** treated as remove + add (checkpoint/schedule state reset for old name).
- **Removal side effects:** removed source stops scheduling and loses its checkpoint/config state; run history may remain; **dataset records kept by default**.
- **Explicit data deletion:** `pipelines delete --intent <intent> --purge-data` or `source delete --intent <intent> --source <name> --purge-data`.
- **Tenant:** always from authenticated token; never in YAML. **Validation:** schema + secret refs exist + connector checks; `apply` fails on unhealthy source unless `--force`.

## 3. PostgreSQL Schema (logical DDL)

```sql
-- tenants & auth
CREATE TABLE tenants (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE auth_tokens (
  id UUID PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('admin','tenant')),
  tenant_id UUID NULL REFERENCES tenants(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ NULL,
  last_used_at TIMESTAMPTZ NULL
);

-- secrets (envelope encryption)
CREATE TABLE tenant_keys (
  tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
  wrapped_dek BYTEA NOT NULL,
  key_version INT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE secrets (
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  nonce BYTEA NOT NULL,
  ciphertext BYTEA NOT NULL,
  key_version INT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, name)
);

-- config
CREATE TABLE pipelines (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  intent TEXT NOT NULL,
  config JSONB NOT NULL DEFAULT '{}'::jsonb, -- pipeline-level/processing
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, intent)
);

CREATE TABLE sources (
  id UUID PRIMARY KEY,
  pipeline_id UUID NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  config JSONB NOT NULL,                -- source-specific config
  schedule_interval TEXT NOT NULL DEFAULT '1m',
  initial_sync TEXT NOT NULL DEFAULT 'full',
  paused BOOLEAN NOT NULL DEFAULT false,
  next_run_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (pipeline_id, name)
);

-- execution
CREATE TABLE pipeline_jobs (
  id UUID PRIMARY KEY,
  source_id UUID NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  status TEXT NOT NULL,                 -- queued|running|succeeded|failed|skipped
  attempt INT NOT NULL DEFAULT 0,
  heartbeat_at TIMESTAMPTZ NULL,
  started_at TIMESTAMPTZ NULL,
  finished_at TIMESTAMPTZ NULL,
  summary JSONB NULL,                   -- read/written/skipped/failed/duration
  error TEXT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE pipeline_checkpoints (
  source_id UUID PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  cursor_state JSONB NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- logs (detailed, on demand)
CREATE TABLE run_logs (
  id BIGSERIAL PRIMARY KEY,
  job_id UUID NOT NULL REFERENCES pipeline_jobs(id) ON DELETE CASCADE,
  ts TIMESTAMPTZ NOT NULL DEFAULT now(),
  level TEXT NOT NULL,
  message TEXT NOT NULL,
  data JSONB NULL
);
```

**Notes/indexes (implementation):** index `sources(next_run_at)` for scheduler, `pipeline_jobs(source_id,status,created_at)` and claim/heartbeat queries; enforce one-active-run-per-source in claim logic/constraints as appropriate; `config` JSONB holds flexible source/processing fields.

## 4. CLI Contract (v1)

- **Global:** `--output text|json` (default `text`). JSON mode emits machine-readable objects/arrays and stable error shape for agent/tooling.
- **Auth context:** `auth login` stores token locally (e.g. user config/credentials file); tenant/admin resolved from token. `auth login --admin` for admin token. `token rotate` rotates current tenant token.

| Command | Key args/options | Behavior |
|---|---|---|
| `auth login` | `[--admin] --token <token>` | store token for subsequent commands |
| `token rotate` | - | rotate current tenant token |
| `init` | `--source <type>` \| `--intent <intent>` | print template (YAML) to stdout |
| `list` | `sources\|intents\|formats` | discovery |
| `validate` | `-f <file>` | schema + ref checks (no persist) |
| `apply` | `-f <file> [--dry-run] [--force]` | validate/health-check/persist+reconcile; dry-run shows plan |
| `source test` | `-f <file>` | connector health checks for sources in file |
| `pipelines list` | - | list tenant pipelines (by intent) |
| `pipelines show` | `--intent <intent>` | show pipeline + sources config/state |
| `pipelines delete` | `--intent <intent> [--purge-data]` | delete pipeline/config; purge dataset if flag |
| `source delete` | `--intent <intent> --source <name> [--purge-data]` | remove source; purge its data if flag |
| `pause` | `--intent <intent> --source <name>` | stop scheduling new runs |
| `resume` | `--intent <intent> --source <name>` | resume scheduling |
| `runs list` | `[--intent <intent>] [--source <name>]` | list runs/jobs |
| `runs show` | `<run_id>` | run summary/status |
| `runs logs` | `<run_id>` | detailed logs |
| `runs trigger` | `--intent <intent> --source <name>` | enqueue immediate run (subject to one-active-run rules) |
| `secrets set` | `<name>` | read value from `TALAINT_SECRET_<NORMALIZED_NAME>` env, encrypt+upsert |
| `secrets list` | - | list secret names/metadata (no values) |
| `secrets delete` | `<name>` | delete secret (fails if referenced, or as defined) |
| `chat` | - | interactive agent harness |
| `admin tenant create` | `--name <name>` | create tenant, emit one-time token once |
| `admin tenant list` | - | list tenants |
| `admin token rotate` | `--tenant <id\|name>` | issue/rotate tenant token |

## 5. Operational Defaults (v1)

- Schedule: `1m` interval polling if omitted; scheduler DB-poll loop (no `PG NOTIFY`).
- Job retry: 3 attempts with exponential backoff, then `failed`; checkpoint only on full success.
- Heartbeat: every 30s; stuck after ~2m; requeue up to 3 times.
- Error policy: `fail`.
- Source removal/rename: config/checkpoint removed, data kept unless `--purge-data`.
- Initial sync: `full`.

## 6. Out of Scope (v1)

Version history/rollback, webhooks/streams/CDC, HTTP server mode/`PG NOTIFY`, multiple intents per pipeline, cron/complex triggers, `skip`/`max_errors`, tenant quotas, downstream consumption API. Expand incrementally.
