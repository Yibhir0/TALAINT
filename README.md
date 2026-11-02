# TALAINT

**A tenant-aware, AI-native data layer.**

TALAINT lets a tenant declare where its data comes from and what it is for, then
ingests, understands, models, and serves that data to LLMs, RAG pipelines, and
agents — with tenant isolation, deterministic behavior, and a full audit trail.

```
Configure -> Ingest -> Understand -> Model -> Embed/Index -> Serve
```

> **Status: early foundation (v0.1.0).** The repository currently contains the
> configuration domain model and a working CLI that validates and prints source
> configuration. Networking, detection, persistence, jobs, and serving are added
> one phase at a time. See [Current status](#current-status) and
> [`docs/architecture.md`](docs/architecture.md).

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
| `talaint configure --url ... --format ... --change-frequency ... --purpose ...` | Validates the arguments and prints the parsed configuration |
| `talaint configure --help` | Lists the command and all allowed values |
| Invalid arguments | Rejected with a suggestion; exit code `2` |

Domain model already implemented in `talaint-core`:

- `DataFormat`, `ChangeFrequency`, `Purpose` with `FromStr` parsing.
- Typed `ConfigError`.
- `HttpSourceConfig` (declared), `ObservedConfig`, `EffectiveConfig`, and a
  deterministic `merge` that records conflicts while keeping declared values
  authoritative.

**Not implemented yet:** HTTP fetching, format detection, Postgres/ClickHouse,
jobs, embeddings, API, web UI, authentication, and the agent.

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

### 4. Run

**From the source tree, without installing:**

```bash
cargo run -p talaint-cli -- version
```

```bash
cargo run -p talaint-cli -- configure \
  --url https://example.com/sales.csv \
  --format csv \
  --change-frequency daily \
  --purpose rag
```

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

### Configure an HTTP source

```bash
talaint configure \
  --url https://example.com/sales.csv \
  --format csv \
  --change-frequency daily \
  --purpose rag
```

Output:

```
configured HTTP source
  url              : https://example.com/sales.csv
  format           : csv
  change frequency : daily
  purpose          : rag
```

Allowed values:

| Flag | Values |
|---|---|
| `--format` | `csv`, `json`, `jsonl`, `parquet`, `xml` |
| `--change-frequency` | `once`, `hourly`, `daily`, `weekly`, `monthly` |
| `--purpose` | `analytics`, `rag`, `agent`, `search`, `other` |

Values are case-insensitive. Run `talaint configure --help` for the full list.

> Today `configure` only validates and prints the configuration. It does not
> fetch the URL, detect the format, persist anything, or start a job.

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

### Project layout

```
TALAINT/
├── Cargo.toml                  workspace (edition 2024, Rust 1.85+)
├── rust-toolchain.toml         pinned stable toolchain
├── crates/
│   ├── talaint-core/           domain types, errors, deterministic merge
│   └── talaint-cli/            the `talaint` binary (clap)
└── docs/
    └── architecture.md         full design, data model, roadmap, decisions
```

---

## Design principles

1. Start small; add one concept per phase.
2. Ports and adapters: domain code depends on traits, not stores.
3. Source is not format: one connector can serve many formats.
4. Declared / observed / effective: declared intent wins, conflicts are recorded.
5. Deterministic behavior and deterministic IDs.
6. Tenant isolation enforced in every store.
7. Secrets are references, never configuration.
8. At-least-once delivery with idempotency, not fragile exactly-once.

---

## Roadmap

| Phase | Build | Rust focus |
|---|---|---|
| 0 | Toolchain, workspace, config skeleton CLI | structs, enums, `Option`, `Result` |
| 1 | Richer config types, validation, tests | traits, `FromStr`, error types |
| 2 | Postgres: tenants + sources + RLS | `sqlx`, async/await, lifetimes |
| 3 | HTTP fetch + format detection + comparison | `Stream`, bytes, backpressure |
| 4 | Runs/jobs (simplified) + deduplication | tokio tasks, channels, retries |
| 5 | ClickHouse writer + embedder + serving | adapters, batching |
| 6 | HTTP API + web UI | axum, middleware |
| later | MCP tools, `talaint-agent`, agent-to-agent | tool schemas, agent loops |

The full design — data model, detection strategy, deduplication, security, and
the decisions log — lives in [`docs/architecture.md`](docs/architecture.md).

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
