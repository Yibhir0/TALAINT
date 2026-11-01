# TALAINT Architecture

Status: **v0.2 — reduced scope** (Phase 0 skeleton)
License: Apache-2.0

## 0. Current scope (start small)

We build in slices. **Now:**

- Sources: **HTTP only**. (File uploads, S3, Postgres, MCP come later.)
- Databases: **Postgres** (the layer's own DB) + **ClickHouse** (AI-native warehouse).
- **MinIO / Parquet lake is deferred.**
- Code is a **skeleton**: configuration types + a CLI that parses parameters.
  Detection, networking, persistence, jobs, and events are added one phase at a time.

The wider design below is the target; anything marked *deferred* is not built yet.

## 1. Vision

TALAINT is a tenant-aware AI data layer. A tenant configures where data comes
from and what it is used for; TALAINT ingests it, understands it, models it for
AI, and serves it to LLMs and agents.

```
Configure -> Ingest -> Understand -> Model -> Embed/Index -> Serve
```

## 2. Principles

1. Start small; add one concept per phase.
2. Sync at the edge, async inside (later).
3. Ports and adapters: domain code depends on traits, not stores.
4. Config is data: stored in Postgres, validated by Rust types.
5. Deterministic behavior: same inputs, same outputs.
6. Tenant isolation enforced in every store.
7. Secrets are references, never config.

## 3. Configuration flow (current slice)

A tenant configures a source with parameters:

| Parameter | Meaning | Example |
|---|---|---|
| `url` | where the data lives | `https://example.com/sales.csv` |
| `format` | declared format | `csv` |
| `change_frequency` | how often data changes | `daily` |
| `purpose` | what the data is for | `rag` |

```
talaint configure \
  --url https://example.com/sales.csv \
  --format csv \
  --change-frequency daily \
  --purpose rag
```

Today this only parses and prints the config. Later phases:

- persist it in Postgres, attached to a tenant;
- fetch the URL and **detect** the real format;
- compare declared vs observed and record conflicts;
- create jobs/events based on `change_frequency`.

## 4. Logical architecture (target)

```
┌──────────────────────── Interfaces ────────────────────────┐
│      CLI (clap)        HTTP API (axum)        Web UI       │
└───────────────────────────────┬────────────────────────────┘
                                │
┌───────────────────────────────▼────────────────────────────┐
│                       Application                          │
│  Configure · Ingest · Detect · Catalog · Model · Embed ·   │
│  Serve                                                     │
└───────┬───────────────┬───────────────┬────────────────────┘
        │               │               │
┌───────▼──────┐ ┌──────▼───────┐ ┌─────▼──────┐
│  Job Engine  │ │  Catalog     │ │  Serving   │
│ sched/events │ │ infer+merge  │ │ RAG/LLM    │
└───────┬──────┘ └──────┬───────┘ └─────┬──────┘
        │               │               │
┌───────▼───────────────▼───────────────▼────────────────────┐
│                     Ingest Pipeline                        │
│   HTTP Source ─► Detector ─► Decoder ─► Normalizer ─► Record│
└───────┬───────────────────────────────┬────────────────────┘
        │                               │
┌───────▼────────┐              ┌───────▼──────────────┐
│   Postgres     │              │     ClickHouse       │
│ layer DB       │              │ AI-native warehouse  │
│ config + RLS   │              │ + vector index       │
└────────────────┘              └──────────────────────┘
```

## 5. Data model (target, Postgres)

```
tenants(id, slug, name, status, plan, created_at, updated_at)

secrets(id, tenant_id, name, kind, ciphertext, key_id, created_at, updated_at)

data_sources(id, tenant_id, name, kind, connection jsonb, secret_id,
             ingestion_mode, schedule, enabled, created_at, updated_at)

format_specs(id, tenant_id, data_source_id, declared_format, options jsonb,
             detection_policy, created_at, updated_at)

datasets(id, tenant_id, data_source_id, name, purpose, purpose_detail,
         observed_format, detection_confidence, detection_evidence,
         status, created_at, updated_at)

schema_snapshots(id, tenant_id, dataset_id, origin, version, fields jsonb, created_at)

audit_log(id, tenant_id, actor_id, action, entity_type, entity_id,
          before jsonb, after jsonb, created_at)
```

Not built yet. This is what Phase 2 will create.

### Declared / observed / effective

- **Declared**: what the tenant configured.
- **Observed**: what TALAINT detected from the bytes.
- **Effective**: merged result used at runtime.

Merge rule (deterministic): declared wins; observed fills gaps; conflicts are
recorded; output ordering is stable.

## 6. Detection (deferred to Phase 3)

```
declared hint -> magic bytes -> structural sniff (bounded prefix)
             -> {format, confidence, evidence} -> tenant confirmation
```

Bounded prefix only; never buffer the whole stream. Text formats (CSV/JSON/JSONL/
XML) are sniffed structurally; binary formats (Parquet) by magic bytes.

## 7. AI-native warehouse (target, ClickHouse)

```
documents  (tenant_id, dataset_id, source_id, doc_id, content, metadata, ingested_at)
chunks     (tenant_id, dataset_id, doc_id, chunk_id, text, offset, metadata)
embeddings (tenant_id, dataset_id, doc_id, chunk_id, model, dim, vector)
entities   (tenant_id, dataset_id, entity_id, type, name, props)
relations  (tenant_id, dataset_id, from_id, to_id, kind, props)
```

- `tenant_id` first in `ORDER BY`; row policies enforce isolation.
- Vector search starts with `cosineDistance`, ANN later.
- Store `model` + `dim` per vector.

### How LLMs and agents use it

| Mode | Capability |
|---|---|
| Semantic retrieval | RAG over chunks |
| Structured query | NL2SQL over governed tables |
| Agent memory | episodic/semantic memory |
| AI observability | prompts, tokens, evals, cost |

## 8. Security and multi-tenancy (target)

| Layer | Mechanism |
|---|---|
| Identity | OIDC/JWT; tenant from token claim |
| Authorization | RBAC + resource scopes |
| Postgres | RLS: `tenant_id = current_setting('app.tenant_id')::uuid` |
| ClickHouse | `tenant_id` in ORDER BY + row policies |
| Secrets | envelope encryption + zeroize |
| PII | detect, mask before embedding |
| Audit | append-only log |
| Agents | least-privilege tokens, approval gates |

## 9. Job engine (deferred to Phase 4)

At-least-once + idempotency + checkpoints + lease/heartbeat + retries/backoff.
Simplified implementation first; scheduler, streaming, and DLQ are hooks.

Triggers: on-demand, scheduled, event/webhook, streaming.

## 10. Component contracts (introduced gradually)

```rust
trait Source   { /* open a byte stream */ }
trait Detector { /* bytes prefix -> detected format */ }
trait Decoder  { /* bytes -> records */ }
trait Embedder { /* text -> vectors */ }
trait Store    { /* persist config / vectors */ }
trait EventBus { /* publish / subscribe */ }
```

Only the config skeleton exists today. Traits arrive with their phases.

## 11. Technology stack

- Rust (stable, edition 2024), tokio, axum, clap.
- Postgres + sqlx, RLS.
- ClickHouse (vector search).
- MinIO / Parquet lake: **deferred**.
- NATS JetStream: **deferred** (in-process first).
- Embeddings: local ONNX or hosted API (later).

## 12. Roadmap

| Phase | Build | Rust focus |
|---|---|---|
| 0 | toolchain + workspace + config skeleton CLI | structs, enums, `Option`, `Result` |
| 1 | richer config types + validation + tests | traits, `FromStr`, error types |
| 2 | Postgres: tenants + sources + RLS | sqlx, async/await, lifetimes |
| 3 | HTTP fetch + format detection + comparison | `Stream`, bytes, backpressure |
| 4 | jobs/events (simplified) | tokio tasks, channels, retries |
| 5 | ClickHouse writer + embedder + serving | adapters, batching |
| 6 | HTTP API + web UI | axum, middleware |

## 13. Decisions log

| # | Decision | Rationale |
|---|---|---|
| 1 | Modular monolith first | Avoid distributed-systems cost while learning |
| 2 | Ports and adapters | Swappable stores, testable with fakes |
| 3 | Source != Format | Avoid N x M connector explosion |
| 4 | Declared/observed/effective | Accuracy via confirmation, not guessing |
| 5 | Declared wins, conflicts recorded | Tenant intent is authoritative |
| 6 | Postgres layer DB + RLS | Transactional config, strong isolation |
| 7 | ClickHouse AI-native warehouse | Free OSS, real-time, vector search |
| 8 | At-least-once + idempotency | Simpler than exactly-once, safe reruns |
| 9 | HTTP-only + Postgres/ClickHouse first | Reduce scope so each step is learnable |
| 10 | MinIO, S3, file, DB, MCP deferred | Add only when the HTTP path works |
