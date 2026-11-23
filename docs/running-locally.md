# Running TALAINT Locally

This guide gives the prerequisites and exact commands to run the v1 CLI on Windows, macOS, and Linux. It assumes you are at the repository root.

For detailed command behavior after startup, see `docs/cli-guide.md`.

## 1. Prerequisites

### Required on all platforms

| Tool                            | Why it is needed                     | Verify                                       |
| ------------------------------- | ------------------------------------ | -------------------------------------------- |
| Git                             | clone/check out the repository       | `git --version`                              |
| Rust stable + Cargo             | build CLI and control-plane binaries | `rustc --version`, `cargo --version`         |
| Docker Engine or Docker Desktop | run local PostgreSQL                 | `docker --version`, `docker compose version` |
| Text editor                     | edit `.env` and pipeline YAML        | any editor                                   |

Rust must support edition 2021. Use a current stable toolchain. No global TALAINT installation is required.

### Platform notes

#### Windows 10/11

- Install Docker Desktop and make sure the engine is running.
- Use PowerShell 7 or Windows PowerShell 5.1.
- PowerShell examples below use `copy`, `$env:NAME`, and `Set-Content -Encoding UTF8`.
- If `docker compose` is not available, install Docker Compose v2 or try `docker-compose`.

#### macOS

- Install Docker Desktop and start it.
- Install the Xcode command line tools if Cargo asks for a C compiler:

```bash
xcode-select --install
```

- Install Rust with rustup, then verify `cargo --version`.

#### Linux

- Install Docker Engine and the Compose plugin.
- Install Rust with rustup.
- On Debian/Ubuntu, install native build dependencies:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev
```

- On Fedora/RHEL:

```bash
sudo dnf install -y gcc make pkg-config openssl-devel
```

- If `docker compose` is not available, install the Compose plugin or use `docker-compose`.

### Optional tools

| Tool    | Purpose                                                                             |
| ------- | ----------------------------------------------------------------------------------- |
| OpenSSL | generate KEK/admin/secret values on macOS/Linux; PowerShell alternative is provided |
| `psql`  | inspect PostgreSQL manually; you can also use `docker exec` instead                 |
| `jq`    | pretty-print JSON on macOS/Linux; not required                                      |

## 2. What gets started

`docker-compose.yml` starts one container:

- container: `talaint-postgres`
- image: `postgres:16`
- host port: `5432`
- database: `talaint`
- user: `talaint`
- password: `talaint`
- volume: `talaint_talaint_pgdata`

The CLI connects to PostgreSQL directly. There is no web server, API, or background worker in M2.

## 3. Start PostgreSQL

Run from the repository root:

```bash
docker compose up -d
```

Verify:

```bash
docker ps --filter name=talaint-postgres
```

Expected: container `talaint-postgres` is `Up`.

If port `5432` is already used, edit `docker-compose.yml`:

```yaml
ports:
  - "5433:5432"
```

Then set the matching database URL:

```dotenv
DATABASE_URL=postgres://talaint:talaint@localhost:5433/talaint
```

## 4. Create `.env`

### Windows PowerShell

```powershell
copy .env.example .env

$kekBytes = New-Object byte[] 32; [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($kekBytes); [Convert]::ToBase64String($kekBytes)

[guid]::NewGuid().ToString("N")

$secretBytes = New-Object byte[] 32; [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($secretBytes); [Convert]::ToBase64String($secretBytes)

notepad .env
```

PowerShell statements must be separated by newlines or semicolons. The random-value commands above are one-liners to avoid paste errors.

Paste values into `.env`:

```dotenv
DATABASE_URL=postgres://talaint:talaint@localhost:5432/talaint
TALAINT_ADMIN_TOKEN=<paste-hex-admin-token>
TALAINT_KEK_BASE64=<paste-base64-kek>
TALAINT_SECRET_API_TOKEN=<paste-base64-secret-value>
```

### macOS/Linux

```bash
cp .env.example .env

openssl rand -base64 32
openssl rand -hex 16
openssl rand -base64 32

nano .env
```

Paste values into `.env`:

```dotenv
DATABASE_URL=postgres://talaint:talaint@localhost:5432/talaint
TALAINT_ADMIN_TOKEN=<paste-hex-admin-token>
TALAINT_KEK_BASE64=<paste-base64-kek>
TALAINT_SECRET_API_TOKEN=<paste-base64-secret-value>
```

Variable meanings:

| Variable                   | Required                | Notes                                                                   |
| -------------------------- | ----------------------- | ----------------------------------------------------------------------- |
| `DATABASE_URL`             | all DB commands         | must match Docker Postgres credentials/port                             |
| `TALAINT_ADMIN_TOKEN`      | first admin login       | single-use bootstrap token; only one admin is allowed in the DB         |
| `TALAINT_KEK_BASE64`       | `secrets set`           | must decode to exactly 32 bytes                                         |
| `TALAINT_SECRET_API_TOKEN` | `secrets set api_token` | the plaintext value to encrypt; name mapping is `TALAINT_SECRET_<NAME>` |

The CLI loads `.env` from the current working directory. Process environment variables override `.env`.

Do not commit `.env`. It is already ignored by Git.

## 5. Run migrations

Windows PowerShell:

```powershell
cargo run -q -p talaint-control-plane --bin migrate
```

macOS/Linux:

```bash
cargo run -q -p talaint-control-plane --bin migrate
```

Expected output:

```text
migrations applied
```

## 6. Build and run the CLI

### Build once

Windows PowerShell:

```powershell
cargo build -q -p talaint-cli
.\target\debug\talaint-cli.exe --help
```

macOS/Linux:

```bash
cargo build -q -p talaint-cli
./target/debug/talaint-cli --help
```

### Run without a separate build

```bash
cargo run -q -p talaint-cli -- --help
cargo run -q -p talaint-cli -- list intents
```

Cargo warnings go to stderr. For clean JSON parsing, build once and call the binary directly, as shown below.

## 7. First-run workflow

### Auth flow

The CLI has no server or session. `auth login` saves the current token locally so later commands know who you are:

- Windows: `%APPDATA%\talaint\credentials.json`
- macOS/Linux: `~/.config/talaint/credentials.json`

Logging in again overwrites that file. Tenant commands fail while logged in as admin, and admin commands fail while logged in as a tenant.

1. **Admin login** bootstraps the single admin. Use the `TALAINT_ADMIN_TOKEN` value from `.env`. The first login stores a hash in the DB; later admin logins use the same token value. No other admin can be created.
2. **Admin creates or rotates a tenant token.** `admin tenant create --name <name>` prints a one-time tenant token. For an existing test tenant, use `admin token rotate --tenant <id|name>` to get a new token; this revokes the old tenant token.
3. **Tenant login** switches the CLI to that tenant. Pipeline, source, secret, and run commands require a tenant token.
4. **To return to admin commands**, run the admin login command again.

```powershell
$cli = ".\target\debug\talaint-cli.exe"

& $cli --output json auth login --admin --token "<TALAINT_ADMIN_TOKEN from .env>"

& $cli --output json admin tenant create --name acme
# or, for an existing tenant:
# & $cli --output json admin token rotate --tenant acme

& $cli --output json auth login --token "<one-time tenant token>"
```

Do not commit `.env` or the local credentials file.

### Configuration flow

A YAML file is a one-time input. `validate` checks it, `apply` extracts its values into Postgres, and the YAML text itself is never stored. You can delete the file after a successful apply.

1. **Store a secret value only if your source needs one.** With `TALAINT_SECRET_API_TOKEN` set in `.env`, run `secrets set api_token`. The CLI reads that environment value, encrypts it with the tenant key, and stores it in the `secrets` table. The YAML keeps only `{ ref: api_token }`. The default sample uses a public endpoint and needs no secret.

2. **Create the YAML file.** For a fresh public sample, copy `config/pipeline.example.yaml` to `pipeline.yaml`; it uses `https://jsonplaceholder.typicode.com/posts/` and needs no secret. Alternatively, pass sources with `generate --source 'name=...,url=...'`. Use `generate` with no `--source` only to rewrite a pipeline already stored in the DB. Use `--force` to replace an existing file or `--append` to add sources to one that already exists.

3. **Edit `pipeline.yaml`.** One file describes one pipeline for one `intent`. Each source needs a unique `name`, `type: http_json`, and `url`. `schedule.interval` accepts values like `30s`, `1m`, `5m`, `1h`. `initial_sync` is `full` or `latest`. Headers such as `Authorization: { ref: api_token }` reference stored secrets; never put raw secret values in the file. Removing a source means it will be removed from the pipeline on the next `apply`.

4. **Validate the file.** `validate -f pipeline.yaml` checks the schema, source names, intervals, and secret refs. Because `DATABASE_URL` is set, it also checks that referenced secrets exist for the logged-in tenant. Fix any errors before continuing.

5. **Test source connectivity.** `source test -f pipeline.yaml` performs a health check against each source URL. In v1 it sends literal headers only; secret refs are not resolved during this check. If the endpoint is unavailable, you can still store the config by using `--force` on `apply`.

6. **Preview the apply.** `apply -f pipeline.yaml --dry-run` shows the intent, target dataset, sources, and health warnings without writing anything.

7. **Apply the config.** `apply -f pipeline.yaml` persists the extracted values to Postgres: `intent` and `processing` on the pipeline row; source name/type/schedule/`initial_sync` and JSONB config on source rows. It fully reconciles sources, so anything omitted from YAML is removed from the pipeline config.

8. **Verify.** `pipelines show --intent rag` reads the stored config back from Postgres. You can also query the `pipelines` and `sources` tables in section 8.

9. **Update later.** Edit or regenerate `pipeline.yaml` and run `validate` then `apply` again. Run `secrets set api_token` again if you changed the secret value in `.env`.

### Windows PowerShell

```powershell
$cli = ".\target\debug\talaint-cli.exe"

& $cli --output json auth login --admin --token "<TALAINT_ADMIN_TOKEN>"

$tenant = & $cli --output json admin tenant create --name acme | ConvertFrom-Json
$tenant.token

& $cli --output json auth login --token $tenant.token
# Optional when your source needs an API secret:
# & $cli --output json secrets set api_token
Copy-Item config\pipeline.example.yaml pipeline.yaml
notepad pipeline.yaml

& $cli --output json validate -f pipeline.yaml
& $cli --output json source test -f pipeline.yaml
& $cli --output json apply -f pipeline.yaml --dry-run
& $cli --output json apply -f pipeline.yaml
& $cli --output json pipelines show --intent rag
```

### macOS/Linux

```bash
CLI=./target/debug/talaint-cli

$CLI --output json auth login --admin --token "<TALAINT_ADMIN_TOKEN>"

$CLI --output json admin tenant create --name acme
# copy the "token" value from the JSON output

$CLI --output json auth login --token "<TENANT_TOKEN>"
# Optional when your source needs an API secret:
# $CLI --output json secrets set api_token
cp config/pipeline.example.yaml pipeline.yaml
nano pipeline.yaml

$CLI --output json validate -f pipeline.yaml
$CLI --output json source test -f pipeline.yaml
$CLI --output json apply -f pipeline.yaml --dry-run
$CLI --output json apply -f pipeline.yaml
$CLI --output json pipelines show --intent rag
```

Edit `pipeline.yaml`:

- The checked-in sample has one public source; every `name` must be unique in the pipeline.
- Replace each `name` with a stable source name.
- Replace each `url` with a reachable JSON endpoint.
- The default source has no headers. If your endpoint needs a secret, add `headers: Authorization: { ref: api_token }` using a name you stored with `secrets set`.
- Change `schedule.interval` per source; `1m` and `5m` are examples.
- Sources omitted from YAML are removed by `apply`.

Generate sources directly instead of editing by hand:

```powershell
& $cli --output json generate --intent rag --file pipeline.yaml --force --source 'name=primary,url=https://api.example.com/data,secret=api_token' --source 'name=secondary,url=https://api.example.com/other,interval=5m,initial_sync=latest'
```

```bash
$CLI --output json generate --intent rag --file pipeline.yaml --force --source 'name=primary,url=https://api.example.com/data,secret=api_token' --source 'name=secondary,url=https://api.example.com/other,interval=5m,initial_sync=latest'
```

Append one more source:

```powershell
& $cli --output json generate --intent rag --file pipeline.yaml --append --source 'name=third,url=https://api.example.com/third,interval=30s'
```

```bash
$CLI --output json generate --intent rag --file pipeline.yaml --append --source 'name=third,url=https://api.example.com/third,interval=30s'
```

If the endpoint is unavailable and you only want to store the config, add `--force` to the final `apply`.

### Manual test checklist

Use this when returning to continue work. The current local credentials are tenant-scoped; admin commands require logging back in as admin first.

```powershell
$cli = ".\target\debug\talaint-cli.exe"

# Confirm the tenant token works; admin tokens cannot run this command.
& $cli --output json pipelines list

# Fresh rag config from the public, no-secret sample.
Copy-Item config\pipeline.example.yaml pipeline.yaml -Force
& $cli --output json validate -f pipeline.yaml
& $cli --output json source test -f pipeline.yaml
& $cli --output json apply -f pipeline.yaml --dry-run
& $cli --output json apply -f pipeline.yaml
& $cli --output json pipelines show --intent rag

# Rewrite YAML from the stored DB config.
& $cli --output json generate --intent rag --file pipeline.yaml --force
```

Expected result: `validate` and `source test` return `"ok": true`, `apply --dry-run` writes nothing, the real `apply` writes the pipeline/source rows, and `pipelines show --intent rag` returns the `jsonplaceholder-posts` source. The YAML file itself is never stored.

For an API-key endpoint later:

1. Put the raw key in `TALAINT_SECRET_API_TOKEN` in `.env`; never put it in YAML.
2. Run `& $cli --output json secrets set api_token`.
3. Change the source URL and add `headers: Authorization: { ref: api_token }` to `pipeline.yaml`.
4. Re-run `validate`, `source test`, `apply --dry-run`, `apply`, and `pipelines show`.

For admin work later:

```powershell
& $cli --output json auth login --admin --token "<TALAINT_ADMIN_TOKEN from .env>"
& $cli --output json admin tenant list
# Rotate a tenant token only if you need a new one:
& $cli --output json admin token rotate --tenant acme
& $cli --output json auth login --token "<new tenant token>"
```

Only one admin token can exist. Tenant token rotation revokes the old tenant token and does not update the local credentials file; log in again afterward.

## 8. Verify database contents

Optional manual check with psql inside the container:

```bash
docker exec -it talaint-postgres psql -U talaint -d talaint -c "\dt"
```

Useful queries:

```bash
docker exec -it talaint-postgres psql -U talaint -d talaint -c "SELECT id, name, created_at FROM tenants;"
docker exec -it talaint-postgres psql -U talaint -d talaint -c "SELECT id, intent, tenant_id FROM pipelines;"
docker exec -it talaint-postgres psql -U talaint -d talaint -c "SELECT name, type, schedule_interval, paused FROM sources;"
```

## 9. Run checks

```bash
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## 10. Stop or reset

Stop PostgreSQL but keep data:

```bash
docker compose down
```

Stop and delete local database data:

```bash
docker compose down -v
```

`docker compose down -v` deletes the Docker volume and all local TALAINT DB data.

## 11. Troubleshooting

| Symptom                                              | Fix                                                                |
| ---------------------------------------------------- | ------------------------------------------------------------------ |
| `cargo: command not found`                           | install Rust/restart shell; verify with `cargo --version`          |
| `docker: command not found`                          | install Docker and restart shell                                   |
| `Cannot connect to the Docker daemon`                | start Docker Desktop/Docker Engine                                 |
| `port 5432 already allocated`                        | change compose host port and `DATABASE_URL` together               |
| `DATABASE_URL is not set`                            | create `.env` or export env var; run from repo root                |
| `relation does not exist`                            | run the migrate command                                            |
| `token is invalid or revoked`                        | log in again with the current token                                |
| `this command requires a tenant token`               | run `auth login --token <tenant-token>`                            |
| `this command requires an admin token`               | run `auth login --admin --token <admin-token>`                     |
| `TALAINT_KEK_BASE64 must decode to 32 bytes`         | regenerate with `openssl rand -base64 32` or PowerShell equivalent |
| `environment variable TALAINT_SECRET_... is not set` | set the mapped env var before `secrets set <name>`                 |
| `failed to parse YAML` on Windows PowerShell         | write config with `Set-Content -Encoding UTF8`, not `>`            |
| `source health checks failed`                        | fix the YAML `url` or use `apply --force`                          |

## 12. Current limitations

- No HTTP API/server mode.
- No background worker; queued runs stay `queued`.
- No real connector execution; M2 only has `http_json` health checks.
- `--purge-data` does not delete dataset records yet.
- `chat` is a stub.
- Local credentials are stored in `<config-dir>/talaint/credentials.json` as plaintext; protect or delete it when done.

## 13. Next step for another day

- Start M3: implement the polling worker/executor. `runs trigger` currently only inserts a `queued` run; nothing consumes it yet.
- Worker scope: claim queued runs with heartbeat, fetch `http_json` sources using the stored DB config, store dataset rows/checkpoints, then mark runs completed or failed.
- Before coding, start PostgreSQL, rebuild the CLI, and run the manual test checklist above to confirm the local tenant/config flow still works.
