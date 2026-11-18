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
| `TALAINT_ADMIN_TOKEN`      | first admin login       | any long random string for local dev                                    |
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

### Windows PowerShell

```powershell
$cli = ".\target\debug\talaint-cli.exe"

& $cli --output json auth login --admin --token "<TALAINT_ADMIN_TOKEN>"

$tenant = & $cli --output json admin tenant create --name acme | ConvertFrom-Json
$tenant.token

& $cli --output json auth login --token $tenant.token
& $cli --output json secrets set api_token
& $cli --output json generate --intent rag --file pipeline.yaml
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
$CLI --output json secrets set api_token
$CLI --output json generate --intent rag --file pipeline.yaml
nano pipeline.yaml

$CLI --output json validate -f pipeline.yaml
$CLI --output json source test -f pipeline.yaml
$CLI --output json apply -f pipeline.yaml --dry-run
$CLI --output json apply -f pipeline.yaml
$CLI --output json pipelines show --intent rag
```

Edit `pipeline.yaml`:

- The generator writes two example sources by default; every `name` must be unique in the pipeline.
- Replace each `name` with a stable source name.
- Replace each `url` with a reachable JSON endpoint.
- Replace `api_token` in the `Authorization` header with your stored secret name, or remove the whole `headers` block if no secret is needed.
- Change `schedule.interval` per source; `1m` and `5m` are examples.
- Sources omitted from YAML are removed by `apply`.

Generate sources directly instead of editing by hand:

```powershell
& $cli --output json generate --intent rag --file pipeline.yaml --force --source 'name=primary,url=http://localhost:8080/data,secret=api_token' --source 'name=secondary,url=http://localhost:8080/other,interval=5m,initial_sync=latest'
```

```bash
$CLI --output json generate --intent rag --file pipeline.yaml --force --source 'name=primary,url=http://localhost:8080/data,secret=api_token' --source 'name=secondary,url=http://localhost:8080/other,interval=5m,initial_sync=latest'
```

Append one more source:

```powershell
& $cli --output json generate --intent rag --file pipeline.yaml --append --source 'name=third,url=http://localhost:8080/third,interval=30s'
```

```bash
$CLI --output json generate --intent rag --file pipeline.yaml --append --source 'name=third,url=http://localhost:8080/third,interval=30s'
```

If the endpoint is unavailable and you only want to store the config, add `--force` to the final `apply`.

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
