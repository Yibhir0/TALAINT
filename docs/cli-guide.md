# TALAINT CLI Guide

This guide documents the M2 command-line interface: local setup, authentication, tenant/token lifecycle, secrets, pipeline YAML, apply semantics, run inspection, JSON output, and current limitations.

Related documents:

- `docs/running-locally.md` — prerequisites and OS-specific setup/run commands
- `docs/requirements.md` — product requirements and v1 decisions
- `docs/architecture.md` — architecture and milestone plan
- `docs/spec-v1.md` — YAML schema, data model, and CLI contract

## 1. What the CLI does

The CLI is the only user-facing surface in v1. It runs in-process and talks directly to PostgreSQL through the `talaint-control-plane` crate.

Current M2 scope:

- Authenticate as an admin or tenant.
- Create tenants and rotate tenant tokens.
- Store tenant-scoped secrets with envelope encryption.
- Validate and apply pipeline YAML configs.
- Health-check `http_json` sources.
- Pause/resume sources.
- Enqueue and inspect run jobs.
- Emit machine-readable JSON with `--output json`.

Current M2 non-goals:

- No HTTP server/API.
- No polling worker/executor yet; `runs trigger` enqueues a job but no runner consumes it in M2.
- No actual source connector processing; `http_json` health checks exist, but ingestion/transformation is deferred.
- No dataset purge; `--purge-data` is accepted but only config is deleted.
- No conversational `chat` agent yet; the command is a stub.

## 2. Build and invoke

From the repository root:

```powershell
cargo build -p talaint-cli
```

Run without building separately:

```powershell
cargo run -q -p talaint-cli -- <COMMAND> [ARGS]
```

After building, the binary is:

```powershell
.\target\debug\talaint-cli.exe <COMMAND> [ARGS]
```

Get help:

```powershell
cargo run -q -p talaint-cli -- --help
cargo run -q -p talaint-cli -- apply --help
```

The global option is:

```text
--output <text|json>    default: text
```

It can be placed before or after the subcommand:

```powershell
cargo run -q -p talaint-cli -- --output json pipelines list
cargo run -q -p talaint-cli -- pipelines list --output json
```

## 3. Local setup

For a full prerequisite checklist and OS-specific commands, see `docs/running-locally.md`.

### 3.1 Start PostgreSQL

```powershell
docker compose up -d
```

The compose file starts `talaint-postgres` on `localhost:5432` with:

- database: `talaint`
- user: `talaint`
- password: `talaint`

### 3.2 Configure environment

Copy the example file:

```powershell
copy .env.example .env
```

Environment variables used by the CLI:

| Variable | Required for | Purpose |
|---|---|---|
| `DATABASE_URL` | all DB-backed commands | PostgreSQL connection string |
| `TALAINT_ADMIN_TOKEN` | first admin login | Bootstrap token for `auth login --admin` |
| `TALAINT_KEK_BASE64` | `secrets set` | Base64-encoded 32-byte key encryption key |
| `TALAINT_SECRET_<NORMALIZED_NAME>` | `secrets set <name>` | Plaintext secret value to ingest |

The CLI loads `.env` via `dotenvy`; values already present in the process environment win.

Example `.env`:

```dotenv
DATABASE_URL=postgres://talaint:talaint@localhost:5432/talaint
TALAINT_ADMIN_TOKEN=replace-with-a-long-random-admin-token

TALAINT_KEK_BASE64=replace-with-base64-of-32-random-bytes
TALAINT_SECRET_API_TOKEN=replace-with-the-secret-value
```

Generate a KEK with OpenSSL:

```powershell
openssl rand -base64 32
```

`TALAINT_KEK_BASE64` must decode to exactly 32 bytes. Secrets already stored with a different KEK cannot be unwrapped after the KEK changes.

### 3.3 Apply migrations

```powershell
cargo run -q -p talaint-control-plane --bin migrate
```

Expected output:

```text
migrations applied
```

## 4. Authentication model

There are two token kinds:

| Kind | Created by | Used for |
|---|---|---|
| `admin` | `TALAINT_ADMIN_TOKEN` bootstrap | tenant/admin management only |
| `tenant` | `admin tenant create` or token rotation | all pipeline, source, secret, and run commands |

Tokens are hashed with SHA-256 before storage. Plaintext tokens are shown only when created or rotated.

The active token is saved locally. The CLI stores it in the platform config directory:

```text
<config-dir>/talaint/credentials.json
```

Typical locations:

- Windows: `%APPDATA%\talaint\credentials.json`
- Linux: `$XDG_CONFIG_HOME/talaint/credentials.json` or `~/.config/talaint/credentials.json`
- macOS: `~/Library/Application Support/talaint/credentials.json`

The token is stored in plaintext locally. Treat this file as a secret. There is no `logout` command; delete the file to clear stored credentials. Logging in again overwrites it.

### 4.1 Bootstrap admin login

Set `TALAINT_ADMIN_TOKEN` in the environment or `.env`, then log in:

```powershell
cargo run -q -p talaint-cli -- --output json auth login --admin --token "<admin-token>"
```

On first successful admin login, the CLI inserts a hashed admin token into the database. Later logins with the same token resolve against the stored hash.

Only one admin token is allowed in the database. Once bootstrapped, attempts to create another admin token are rejected by both the CLI logic and a database unique index.

If `TALAINT_ADMIN_TOKEN` is changed after bootstrap, the old admin token remains active; there is no admin token rotation command in M2.

### 4.2 Create a tenant

Admin credentials are required:

```powershell
cargo run -q -p talaint-cli -- --output json admin tenant create --name acme
```

Example response:

```json
{
  "ok": true,
  "tenant": {
    "id": "a6e17b94-cfc3-4b94-8a1b-8e66ca449b92",
    "name": "acme"
  },
  "token": "one-time-tenant-token"
}
```

Save the `token` value; it is shown once.

### 4.3 Log in as the tenant

```powershell
cargo run -q -p talaint-cli -- --output json auth login --token "<tenant-token>"
```

Tenant commands fail if the stored token is admin, and admin commands fail if the stored token is tenant.

List tenants as admin:

```powershell
cargo run -q -p talaint-cli -- --output json admin tenant list
```

### 4.4 Rotate tokens

Rotate the current tenant token and update local credentials:

```powershell
cargo run -q -p talaint-cli -- --output json token rotate
```

Rotate a tenant token as admin, by tenant id or name:

```powershell
cargo run -q -p talaint-cli -- --output json admin token rotate --tenant acme
```

`admin token rotate` revokes all active tokens for that tenant and prints the new token once. It does not update local credentials; log in again with the new token.

## 5. Secrets

Secrets are tenant-scoped. They are never stored in pipeline YAML as values; YAML stores references.

### 5.1 Name normalization

`secrets set <name>` reads the value from:

```text
TALAINT_SECRET_<NORMALIZED_NAME>
```

Normalization: uppercase every character and replace each non-alphanumeric character with `_`.

Examples:

| Secret name | Environment variable |
|---|---|
| `api_token` | `TALAINT_SECRET_API_TOKEN` |
| `aws-prod` | `TALAINT_SECRET_AWS_PROD` |
| `db.password` | `TALAINT_SECRET_DB_PASSWORD` |

### 5.2 Set a secret

Requires tenant credentials and `TALAINT_KEK_BASE64`.

```powershell
$env:TALAINT_SECRET_API_TOKEN = "super-secret-value"
cargo run -q -p talaint-cli -- --output json secrets set api_token
```

The CLI creates a per-tenant data encryption key on first use, wraps it with the KEK, and encrypts the secret with AES-256-GCM.

### 5.3 List and delete secrets

```powershell
cargo run -q -p talaint-cli -- --output json secrets list
cargo run -q -p talaint-cli -- --output json secrets delete api_token
```

`secrets list` returns names and metadata only, never values.

M2 note: `secrets delete` does not check whether a stored pipeline still references the secret. Deleting a referenced secret makes later `validate`/`apply` fail until the reference is removed or the secret is set again.

## 6. Pipeline YAML

One YAML file describes one pipeline for the authenticated tenant. The tenant is never in YAML.

### 6.1 Minimal example

```yaml
intent: rag
sources:
  - name: tickets-api
    type: http_json
    url: http://localhost:8080/tickets
    schedule:
      interval: 1m
    initial_sync: full
```

### 6.2 Example with secret reference

```yaml
intent: rag
sources:
  - name: tickets-api
    type: http_json
    url: http://localhost:8080/tickets
    headers:
      Authorization: { ref: api_token }
    schedule:
      interval: 5m
    initial_sync: full
```

The referenced secret `api_token` must already exist for this tenant.

### 6.3 Top-level fields

| Field | Required | Default | Notes |
|---|---|---|---|
| `intent` | yes | - | `rag`, `fine_tuning`, `ml_training`, or `multimodal` |
| `sources` | yes | - | non-empty list; source names must be unique |
| `processing` | no | `null` | intent-specific object; stored by M2 but not executed |

### 6.4 Source fields

| Field | Required | Default | Notes |
|---|---|---|---|
| `name` | yes | - | stable source key within the pipeline |
| `type` | yes | - | M2 supports structurally: `http_json` |
| `schedule.interval` | no | `1m` | duration like `30s`, `1m`, `5m`, `1h` |
| `initial_sync` | no | `full` | `full` or `latest` |
| `credentials.ref` | no | - | tenant-scoped secret reference |
| `headers` | no | `{}` | literal string or `{ ref: name }` |
| `query_params` | no | `{}` | string map |
| `format` | no | inferred | connector hint |
| `record_path` | no | auto | path to records array/item |
| `flatten` | no | `false` | connector hint |
| `error_policy` | no | `fail` | M2 supports only `fail` |
| `url` | yes for `http_json` | - | HTTP endpoint to poll |

Connector-specific fields are preserved in the source config stored in PostgreSQL.

### 6.5 Apply semantics

- Pipeline key: `(tenant_id, intent)`.
- Source key: `name` within the pipeline.
- `apply` upserts the pipeline and fully reconciles sources.
- Sources omitted from YAML are removed from the pipeline config.
- Removing a source stops scheduling and removes checkpoint/config state.
- Dataset records are kept unless a future M5 dataset purge is implemented.
- Renaming a source is remove + add.
- The `paused` flag is preserved across `apply`; use `resume` if you want a source active again.

### 6.6 Templates and discovery

For a fresh no-secret sample, copy the checked-in template:

```powershell
Copy-Item config\pipeline.example.yaml pipeline.yaml
```

```bash
cp config/pipeline.example.yaml pipeline.yaml
```

The template points at `https://jsonplaceholder.typicode.com/posts/`. To define sources directly, repeat `--source` with comma-separated `key=value` fields:

```powershell
cargo run -q -p talaint-cli -- generate --intent rag --file pipeline.yaml --source 'name=primary,url=https://api.example.com/data,secret=api_token' --source 'name=secondary,url=https://api.example.com/other,interval=5m,initial_sync=latest'
```

```bash
cargo run -q -p talaint-cli -- generate --intent rag --file pipeline.yaml --source 'name=primary,url=https://api.example.com/data,secret=api_token' --source 'name=secondary,url=https://api.example.com/other,interval=5m,initial_sync=latest'
```

Running `generate` with no `--source` loads the stored pipeline for the tenant from Postgres and rewrites it to YAML. It fails if no pipeline is stored for that intent, rather than inventing a URL in code.

Supported source fields:

- `name` (required, unique in the pipeline)
- `url` (required for `http_json`)
- `type` (default `http_json`)
- `interval` (default `1m`; examples `30s`, `5m`, `1h`)
- `initial_sync` (default `full`; also `latest`)
- `secret` (shortcut for an `Authorization` header secret ref)
- `header.<NAME>` (header secret ref, e.g. `header.X-Api-Key=api_token`)
- `method` (HTTP method)
- `record_path`

The generator rejects invalid intents, URLs, intervals, `initial_sync` values, duplicate names, and unsupported source types. It refuses to overwrite an existing file unless `--force` is set. Use `--append` to add sources to an existing file:

```powershell
cargo run -q -p talaint-cli -- generate --intent rag --file pipeline.yaml --append --source 'name=third,url=https://api.example.com/third,interval=30s'
```

```bash
cargo run -q -p talaint-cli -- generate --intent rag --file pipeline.yaml --append --source 'name=third,url=https://api.example.com/third,interval=30s'
```

A checked-in template is also available at `config/pipeline.example.yaml`. You can also print a basic template from the CLI:

```powershell
cargo run -q -p talaint-cli -- init --intent rag
cargo run -q -p talaint-cli -- init --source http_json
```

In Windows PowerShell 5.1, `>` writes UTF-16. Use:

```powershell
cargo run -q -p talaint-cli -- init --intent rag | Set-Content -Encoding UTF8 pipeline.yaml
```

The CLI strips a UTF-8 BOM from config files.

Discovery lists:

```powershell
cargo run -q -p talaint-cli -- list intents
cargo run -q -p talaint-cli -- list sources
cargo run -q -p talaint-cli -- list formats
```

In JSON mode, `init` returns the template as a string in the `template` field.

## 7. Validation and health checks

### 7.1 Validate

```powershell
cargo run -q -p talaint-cli -- --output json validate -f pipeline.yaml
```

`validate` always checks YAML schema. If `DATABASE_URL` is available, it also requires tenant credentials and checks that referenced secrets exist in PostgreSQL.

To force schema-only validation, run without `DATABASE_URL` in the environment or `.env`.

Validation errors are collected and returned as a single error string, for example:

```json
{
  "ok": false,
  "error": "validation failed:\n- sources[0] (tickets-api): url is required for type 'http_json'"
}
```

### 7.2 Source health check

```powershell
cargo run -q -p talaint-cli -- --output json source test -f pipeline.yaml
```

For `http_json`, the CLI sends a `HEAD` request. On `405` or another client/server error, it retries with `GET`. A `2xx` response means healthy.

M2 note: only literal header values are sent during the health check. Header secret references are not resolved in the health check.

`source test` does not require database access or authentication.

In JSON mode, a failed health check prints `{"ok": false, "results": [...]}` and exits with code `1`.

## 8. Apply

Requires tenant credentials.

Preview without writing:

```powershell
cargo run -q -p talaint-cli -- --output json apply -f pipeline.yaml --dry-run
```

Apply:

```powershell
cargo run -q -p talaint-cli -- --output json apply -f pipeline.yaml
```

Override unhealthy source checks:

```powershell
cargo run -q -p talaint-cli -- --output json apply -f pipeline.yaml --force
```

Behavior:

1. Load and parse YAML.
2. Validate schema.
3. Check referenced secrets exist for the tenant.
4. Run source health checks.
5. In `--dry-run`, print the plan and stop; health failures are warnings only.
6. In a normal apply, fail on health errors unless `--force` is used.
7. Upsert the pipeline by intent.
8. Reconcile sources by name.

`--force` only overrides health-check failures. It does not bypass schema validation, authentication, or secret-reference checks.

## 9. Pipelines and sources

List pipelines:

```powershell
cargo run -q -p talaint-cli -- --output json pipelines list
```

Show one pipeline:

```powershell
cargo run -q -p talaint-cli -- --output json pipelines show --intent rag
```

Delete a pipeline config:

```powershell
cargo run -q -p talaint-cli -- --output json pipelines delete --intent rag
```

Delete a source config:

```powershell
cargo run -q -p talaint-cli -- --output json source delete --intent rag --source tickets-api
```

The `--purge-data` flag is accepted on both delete commands:

```powershell
cargo run -q -p talaint-cli -- --output json pipelines delete --intent rag --purge-data
cargo run -q -p talaint-cli -- --output json source delete --intent rag --source tickets-api --purge-data
```

M2 note: dataset purge is not implemented. The response includes a note, and only config rows are deleted.

Pause or resume scheduling:

```powershell
cargo run -q -p talaint-cli -- --output json pause --intent rag --source tickets-api
cargo run -q -p talaint-cli -- --output json resume --intent rag --source tickets-api
```

## 10. Runs and logs

Trigger a run:

```powershell
cargo run -q -p talaint-cli -- --output json runs trigger --intent rag --source tickets-api
```

A source can have at most one `queued` or `running` job at a time. Triggering again while one is active returns an error.

List runs:

```powershell
cargo run -q -p talaint-cli -- --output json runs list
cargo run -q -p talaint-cli -- --output json runs list --intent rag
cargo run -q -p talaint-cli -- --output json runs list --intent rag --source tickets-api
```

Show a run:

```powershell
cargo run -q -p talaint-cli -- --output json runs show <RUN_ID>
```

Show logs:

```powershell
cargo run -q -p talaint-cli -- --output json runs logs <RUN_ID>
```

M2 note: no runner consumes queued jobs yet, so newly triggered runs remain `queued` and logs are empty until a worker is implemented.

## 11. JSON output contract

Use `--output json` for automation.

Success responses are pretty-printed JSON objects. Most include `"ok": true`, for example:

```json
{
  "ok": true,
  "valid": true,
  "intent": "rag",
  "sources": 1
}
```

Command errors from the CLI are emitted as:

```json
{
  "ok": false,
  "error": "message"
}
```

and the process exits with code `1`.

Text mode prints human-readable output to stdout and errors to stderr.

Exit codes:

| Code | Meaning |
|---|---|
| `0` | success |
| `1` | command error, failed health check, or JSON-mode stub error |

`source test --output json` is special: the normal result object contains `"ok": false` on failure and exits `1`; it does not emit the standard error object.

## 12. Command reference

All commands support the global `--output text|json` option.

| Command | Auth | Requires DB | Description |
|---|---|---|---|
| `auth login --token <token>` | none | yes | store tenant token locally |
| `auth login --admin --token <token>` | none | yes | store admin token; bootstrap from `TALAINT_ADMIN_TOKEN` if needed |
| `token rotate` | tenant | yes | rotate current tenant token and update local credentials |
| `init --intent <intent>` | none | no | print intent template |
| `init --source <type>` | none | no | print source template |
| `generate --intent <intent> -f <file> [--source <spec>]... [--append] [--force]` | tenant when no `--source` | when no `--source` | generate and validate a YAML pipeline file; no-source mode reads stored sources from the DB |
| `list sources` | none | no | list connector types |
| `list intents` | none | no | list supported intents |
| `list formats` | none | no | list known formats |
| `validate -f <file>` | none or tenant | optional | schema check; plus DB secret-ref check when `DATABASE_URL` is set |
| `apply -f <file> [--dry-run] [--force]` | tenant | yes | validate, health-check, persist, reconcile |
| `source test -f <file>` | none | no | health-check sources in YAML |
| `source delete --intent <intent> --source <name> [--purge-data]` | tenant | yes | delete one source config |
| `pipelines list` | tenant | yes | list tenant pipelines |
| `pipelines show --intent <intent>` | tenant | yes | show pipeline and sources |
| `pipelines delete --intent <intent> [--purge-data]` | tenant | yes | delete pipeline config |
| `pause --intent <intent> --source <name>` | tenant | yes | set source `paused=true` |
| `resume --intent <intent> --source <name>` | tenant | yes | set source `paused=false` |
| `runs trigger --intent <intent> --source <name>` | tenant | yes | enqueue one run |
| `runs list [--intent <intent>] [--source <name>]` | tenant | yes | list runs |
| `runs show <run_id>` | tenant | yes | show one run |
| `runs logs <run_id>` | tenant | yes | list run logs |
| `secrets set <name>` | tenant | yes | ingest secret from `TALAINT_SECRET_<NORMALIZED_NAME>` |
| `secrets list` | tenant | yes | list secret metadata |
| `secrets delete <name>` | tenant | yes | delete secret |
| `admin tenant create --name <name>` | admin | yes | create tenant and print one-time token |
| `admin tenant list` | admin | yes | list tenants |
| `admin token rotate --tenant <id\|name>` | admin | yes | rotate a tenant token |
| `chat` | none | no | stub; planned for M6 |

## 13. End-to-end example

```powershell
docker compose up -d
cargo run -q -p talaint-control-plane --bin migrate

copy .env.example .env

$env:DATABASE_URL = "postgres://talaint:talaint@localhost:5432/talaint"
$env:TALAINT_ADMIN_TOKEN = "a-long-random-admin-token"
$env:TALAINT_KEK_BASE64 = "base64-of-32-random-bytes"
$env:TALAINT_SECRET_API_TOKEN = "super-secret-value"

cargo run -q -p talaint-cli -- --output json auth login --admin --token $env:TALAINT_ADMIN_TOKEN

$created = cargo run -q -p talaint-cli -- --output json admin tenant create --name acme | ConvertFrom-Json
$created.token

cargo run -q -p talaint-cli -- --output json auth login --token $created.token
# Optional when your source needs an API secret:
# cargo run -q -p talaint-cli -- --output json secrets set api_token
Copy-Item config\pipeline.example.yaml pipeline.yaml
cargo run -q -p talaint-cli -- --output json validate -f pipeline.yaml
cargo run -q -p talaint-cli -- --output json source test -f pipeline.yaml
cargo run -q -p talaint-cli -- --output json apply -f pipeline.yaml --dry-run
cargo run -q -p talaint-cli -- --output json apply -f pipeline.yaml
cargo run -q -p talaint-cli -- --output json pipelines show --intent rag
cargo run -q -p talaint-cli -- --output json runs trigger --intent rag --source jsonplaceholder-posts
cargo run -q -p talaint-cli -- --output json runs list --intent rag
```

The checked-in sample uses a public endpoint and needs no secret. Edit `pipeline.yaml` so each source `url`, `name`, and secret reference match your environment before applying. Add a `headers` block only if your endpoint needs one.

## 14. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `DATABASE_URL is not set` | env/.env missing | set `DATABASE_URL` |
| connection refused | Postgres not running | `docker compose up -d` |
| relation does not exist | migrations not applied | run migrate binary |
| `token is invalid or revoked` | stored token rotated/revoked | log in with current token |
| `this command requires a tenant token` | logged in as admin | `auth login --token <tenant-token>` |
| `this command requires an admin token` | logged in as tenant | `auth login --admin --token <admin-token>` |
| `TALAINT_KEK_BASE64 is not set` | KEK missing | set 32-byte base64 KEK |
| `TALAINT_KEK_BASE64 must decode to 32 bytes` | wrong key length | generate a new 32-byte key |
| `environment variable TALAINT_SECRET_... is not set` | wrong secret env name | check normalization and set env var |
| `missing secret(s) in database` | YAML ref not stored for tenant | `secrets set <name>` |
| `failed to parse YAML: missing field ...` | malformed config | check required fields; BOM is supported |
| `source health checks failed` | endpoint unavailable | fix URL/network or use `--force` |
| `already has an active or queued run` | run dedupe active | wait for runner or inspect `runs list` |

## 15. Security notes

- Do not commit `.env`, tokens, or secret values.
- Keep `TALAINT_KEK_BASE64` stable and outside the repository in real deployments.
- The local credentials file contains a plaintext token; protect it and delete it when finished.
- Tokens are hashed in PostgreSQL and are shown only at creation/rotation.
- Secret values are encrypted with AES-256-GCM and never printed by the CLI.
- M2 has no logout, admin-token rotation, or token expiry workflow yet.
