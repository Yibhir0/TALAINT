# TALAINT

**A tenant-aware, AI-native data layer.**

TALAINT lets a tenant declare where its data comes from and what it is for, then
ingests, understands, models, and serves that data to LLMs, RAG pipelines, and
agents — with tenant isolation, deterministic behavior, and a full audit trail.

```
Configure -> Ingest -> Understand -> Model -> Embed/Index -> Serve
```

> **Status: early foundation (v0.1.0).** The repository currently contains the
> core domain primitives, a CLI skeleton (`talaint version`), and a containerized
> test stack (`talaint-net`). The descriptor-driven configuration CLI is the next
> step. See [Current status](#current-status),
> [`docs/requirements.md`](docs/requirements.md), and
> [`docs/configuration-phase.md`](docs/configuration-phase.md).

---

## Vision

TALAINT is built to be **AI-native from every corner**, not a traditional data
platform with an LLM bolted on:

- **Deterministic core.** Configuration, detection, merging, and deduplication
  are pure, testable functions. Same inputs always produce the same outputs.
- **Tenant isolation everywhere.** `tenant_id` is enforced in Postgres (RLS) and
  ClickHouse (row policies), not just in application code.
- **Agent-friendly by design.** The CLI is non-interactive, exposes a stable
  capability surface, and is meant to be driven by humans *and* agents.
- **API-first.** CLI, web UI, and agents are equal clients of the same
  capabilities. No logic lives only in the CLI.
- **Human-to-agent and agent-to-agent.** The long-term target is a first-party
  `talaint-agent` that can chat with users, explain exactly what it can do, and
  perform configuration and ingestion — and that other agents can call to ingest
  data, register events, and query datasets.
- **Governed by default.** Scoped credentials, approval gates for risky actions,
  SSRF protection, PII handling, quotas, and append-only audit.

### Maturity target

```
now      CLI (flags + machine-readable output)
later    REST API + MCP tools (capability manifest as the source of truth)
later    talaint-agent (conversational + autonomous operator)
later    agent-to-agent (clients' agents delegate to talaint-agent)
```

The deterministic core never depends on the agent layer. If the agent is down,
the CLI and API still work.

---

## Current status

What works today:

| Command | Behavior |
|---|---|
| `talaint version` | Prints `talaint 0.1.0` |

Core domain (`talaint-core`):

- `DataFormat`, `ChangeFrequency`, `Purpose` with `FromStr` parsing.
- Typed `ConfigError`.
- `version()` surfaced by the CLI.

Test stack:

- `docker compose up -d` starts the `talaint-net` network and the services
  `config-db`, `source-pg`, `source-mysql`, `http-source`, `dest-pg`, and
  `clickhouse`.
- Seed scripts are organized by purpose: `analytics`, `rag`, `agent`, `search`,
  `other`.

**In progress:** descriptor-driven configuration CLI (`connector list`,
`source add/list/test`, `destination add/list/test`, `plan`, `apply`).

**Not implemented yet:** ingest, transform, dataset/mapping, runs, serving, API,
web UI, authentication, and the agent.

Exit codes: `0` success, `1` runtime error, `2` usage error.

---

## Quick start (all OS)

### 1. Prerequisites

You need the Rust toolchain. Install it with [rustup](https://rustup.rs):

**Windows (PowerShell)**

```powershell
winget install Rustlang.Rustup
```

Or download and run `rustup-init.exe` from <https://rustup.rs>.

You also need the **MSVC C++ build tools**: install *Visual Studio Build Tools*
and select **Desktop development with C++** (the Windows SDK is included).

**macOS**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
xcode-select --install
```

**Linux (Debian/Ubuntu)**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo apt update && sudo apt install build-essential
```

**Linux (Fedora/RHEL)**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
sudo dnf groupinstall "Development Tools"
```

After installing, **open a new terminal** so `cargo` is on `PATH`, then verify:

```bash
rustc --version
cargo --version
```

> **Windows:** if `cargo` is not recognized, the current shell has a stale
> `PATH`. Run:
>
> ```powershell
> $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
> ```
>
> or open a new terminal.

You also need **Docker Desktop** (Windows/macOS) or Docker Engine + Compose
(Linux) for the local test stack:

```bash
docker --version
docker compose version
```

> **Windows/macOS:** start Docker Desktop and wait until `docker info` succeeds
> before running `docker compose`.

### 2. Get the code

```bash
git clone <repository-url>
cd TALAINT
```

If you already have the folder, just `cd` into it.

### 3. Build

```bash
cargo build --release
```

The binary is written to:

| OS | Binary |
|---|---|
| Windows | `target\release\talaint.exe` |
| macOS | `target/release/talaint` |
| Linux | `target/release/talaint` |

### 4. Start the test environment

The local stack runs on the `talaint-net` Docker network:

```bash
docker compose up -d --wait
```

Check it:

```bash
docker compose ps
docker network inspect talaint-net --format '{{.Name}}'
```

| Service | Host port | Role |
|---|---|---|
| `config-db` | 5432 | config store |
| `source-pg` | 5433 | SQL source |
| `source-mysql` | 3306 | SQL source |
| `http-source` | 8080 | HTTP source |
| `dest-pg` | 5434 | SQL destination |
| `clickhouse` | 8123 / 9000 | warehouse |

Init scripts under `docker/init/` seed the sources and prepare the destinations.
They run only on first initialization; reset everything with:

```bash
docker compose down -v
```

### 5. Run

**From the source tree, without installing:**

```bash
cargo run -p talaint-cli -- version
```

The configuration commands are being added next; see
[`docs/requirements.md`](docs/requirements.md).

**From the built binary:**

```powershell
# Windows (PowerShell)
.\target\release\talaint.exe version
```

```bash
# macOS / Linux
./target/release/talaint version
```

**Install it on your `PATH` (all OS, installs to `~/.cargo/bin`):**

```bash
cargo install --path crates/talaint-cli
talaint version
```

On Windows this installs `talaint.exe` to `%USERPROFILE%\.cargo\bin`, which is
normally already on `PATH`.

---

## Usage

### Print the version

```bash
talaint version
```

### Configuration (next step)

The descriptor-driven configuration commands are being implemented:

| Command | Purpose |
|---|---|
| `talaint connector list` | Show connector kinds and their fields |
| `talaint source add/list/test` | Create, list, and test sources |
| `talaint destination add/list/test` | Create, list, and test destinations |
| `talaint plan -f talaint.yaml` | Preview changes |
| `talaint apply -f talaint.yaml` | Commit changes |

See [`docs/configuration-phase.md`](docs/configuration-phase.md) for the design
and [`docs/requirements.md`](docs/requirements.md) for the phase plan.

---

## Development

Run the test suite:

```bash
cargo test
```

Format and lint (both are expected to pass with no warnings):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Build the whole workspace:

```bash
cargo build
```

### Test environment

```bash
docker compose up -d --wait
docker compose ps
docker compose down -v
```

### Project layout

```
TALAINT/
├── Cargo.toml                  workspace (edition 2024, Rust 1.85+)
├── rust-toolchain.toml         pinned stable toolchain
├── docker-compose.yml          talaint-net test stack
├── docker/init/                purpose-based seed scripts
├── crates/
│   ├── talaint-core/           domain types, errors
│   └── talaint-cli/            the `talaint` binary (clap)
└── docs/
    ├── requirements.md         what we build across phases
    ├── configuration-phase.md  phase 1 design
    └── architecture.md         target architecture
```

---

## Design principles

1. Start small; add one concept per phase.
2. Ports and adapters: domain code depends on traits, not stores.
3. Source is not format: one connector can serve many formats.
4. Descriptors are shared; configs are per tenant.
5. Deterministic behavior and deterministic IDs.
6. Tenant isolation enforced in every store.
7. Secrets are references, never configuration.
8. At-least-once delivery with idempotency, not fragile exactly-once.

---

## Roadmap

| Phase | Build |
|---|---|
| 1 | Configuration: descriptors, registry, source/destination configs, CLI, test stack |
| 2 | Dataset/mapping/transform: bind sources to destinations, derive mapping, execute, verify |
| 3 | Agent integration: expose the validated tools to an agent |

See [`docs/requirements.md`](docs/requirements.md) for the phase plan and
[`docs/architecture.md`](docs/architecture.md) for the target architecture.

---

## Distribution (planned)

Prebuilt binaries are not published yet. The plan is to ship:

- GitHub Releases with per-OS binaries and checksums,
- package managers (`winget`, `scoop`, Homebrew, `curl | sh`),
- a Docker image for the CLI and for the self-hosted data layer,
- `cargo install talaint-cli` for Rust users.

Normal users will never need Rust or `cargo`; they will download a native
binary, use a package manager, or use the hosted web UI/API.

---

## License

Apache-2.0.
