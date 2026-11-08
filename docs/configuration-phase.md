# TALAINT — Configuration Phase Design

**Status:** design only — no code yet.
**Scope:** the first milestone — configuration + CLI + generic source/destination.
**Target (future):** [`architecture.md`](architecture.md).

---

## 1. Goal

Build the control plane: let a user declare **where data comes from** and **where it
goes**, for **any source** and **any warehouse**, through one small CLI.

This phase produces:

- a **registry** of built-in, self-describing connectors,
- a **generic configuration model** for sources and destinations,
- a **CLI tool surface** (`list`, `add`, `test`, `plan`, `apply`),
- a **containerized test stack** with real systems.

No ingestion, transformation, datasets, or serving yet.

---

## 2. Scope

**In:** descriptors, source/destination configs, CLI, test stack.
**Out:** ingest, transform, dataset, purpose, runs, serving, plugins, agent, auth.

We build step by step; each phase adds one concept.

---

## 3. Vocabulary

| Term | Meaning |
|---|---|
| **Descriptor** | The form for one connector kind: name, role, fields, capabilities. Shared. |
| **Registry** | Maps `kind → descriptor + connector`. The only place that knows what kinds exist. |
| **Config** | One configured source or destination: `name + kind + settings + secret_ref`. Per tenant. |
| **Config file** | YAML declaring many configs (the declarative input). |
| **Config store** | The database where configs are saved after `apply`. |
| **Connector** | The code that talks to a real system (`test` now, read/write later). |
| **Secret ref** | A pointer to a credential (env/keychain), never the secret itself. |

> **Descriptor ≠ config.** Descriptor = the blank form (shared, static). Config = the
> filled form (per tenant, dynamic). Descriptors live in the registry; configs live in
> the config store.

---

## 4. High-level design

```
                        HUMAN
                          │
                          ▼
                   ┌─────────────┐
                   │     CLI     │
                   └──────┬──────┘
                          │ reads kinds + fields
                          ▼
        ┌────────────────────────────────────────┐
        │ REGISTRY (kind → descriptor + connector)│
        │  http        role: source               │
        │  postgres    role: source + destination │
        │  clickhouse  role: destination          │
        └───────┬────────────────────────┬───────┘
                │ descriptor             │ connector
                ▼                        ▼
     ┌──────────────────────┐   ┌──────────────────┐
     │ CONFIGURATION        │   │ CONNECTORS       │
     │  source config       │   │  http            │
     │  destination config  │   │  postgres        │
     └──────────┬───────────┘   │  clickhouse      │
                │ save          └────────┬─────────┘
                ▼                        │ test / read / write
     ┌──────────────────────┐   ┌────────▼─────────┐
     │ CONFIG STORE         │   │ EXTERNAL SYSTEMS │
     │ (source + dest)      │   │ source / warehouse│
     └──────────────────────┘   └──────────────────┘
```

**Key rule:** the CLI and engine depend only on the **registry**, never on concrete
connectors. Adding a connector later = add a descriptor + connector; the CLI never changes.

---

## 5. Connectors and descriptors

One **descriptor per kind**, tagged with a **role**: `source`, `destination`, or `both`.
Source and destination lists are filtered views of the same registry.

### 5.1 Descriptor contents

- **Meta:** `kind`, `display_name`, `description`, `role`, `version`.
- **Capabilities:** `connect`, `read`, `write`, `upsert`, `create_table`, `infer_schema`, `describe`.
- **Fields:** each with `name`, `type`, `required`, `default`, `allowed values`,
  `group` (connection/target/options, for UX), `secret`, `help`.

### 5.2 Examples

```
http        role: source
  url (url, required) · method (enum, GET) · auth (enum, none) · auth_secret (secret)
  headers (map) · format (enum, auto) · verify_tls (bool, true) · timeout_seconds (int, 30)

postgres    role: source + destination
  host (required) · port (int, 5432) · database (required) · user (required)
  password (secret) · ssl_mode (enum, require) · schema (string, public)
  table (destination) · write_mode (enum, append) · key_columns (required-if upsert)

clickhouse  role: destination
  url (required) · database (required) · user (required) · password (secret)
  table (required) · secure (bool, true) · write_mode (enum, append)
```

Rules: role-scoped fields (`write_mode` only for destination); credentials are `secret`
fields, never inline; conditional requirements (`key_columns` only if `upsert`).

### 5.3 SQL family

SQL databases share a core (`host`, `port`, `database`, `user`, `password`, `tls`) but
differ in details (default port, schema model, auth, TLS params, upsert syntax). So we
keep **one descriptor per kind** on top of a shared SQL base — not one universal `sql`
kind. This keeps validation honest and the CLI simple.

---

## 6. Generic configuration model

Source and destination use the **same envelope**. Only `name`, `kind`, `secret_ref` are
universal; everything else is descriptor-declared `settings`.

```
SOURCE CONFIG                     DESTINATION CONFIG
  name:        ops                  name:        dw
  kind:        postgres             kind:        postgres
  settings:    { ... }              settings:    { ... }
  secret_ref:  OPS_PG_PASSWORD      secret_ref:  DW_PG_PASSWORD
```

### 6.1 `settings` is descriptor-declared (examples)

| Kind | `settings` |
|---|---|
| `http` (source) | `url`, `method`, `auth`, `headers`, `format`, `timeout` |
| `postgres` | `host`, `port`, `database`, `user`, `schema`, `table`, `write_mode`, `key_columns` |
| `clickhouse` (dest) | `url`, `database`, `user`, `table`, `secure`, `write_mode` |
| `s3` (later) | `bucket`, `region`, `prefix`, `format` |
| `bigquery` (later) | `project`, `location`, `dataset`, `table`, `write_mode` |

"Location" and "target" are **not** universal: a URL, a host/db/table, a bucket/prefix,
or a collection. That is why they live in `settings`, not as fixed columns.

### 6.2 Descriptor vs config vs purpose

| Layer | Answers | Scope |
|---|---|---|
| **Descriptor** | How to connect this kind? | shared, per kind |
| **Config** | Where is this tenant's instance? | per tenant |
| **Purpose** | What is this data for? | per dataset (later) |

Purpose is **not** in the descriptor; it belongs to the Dataset in a later phase.

### 6.3 Config store

**Postgres**, with flexible parts as JSONB:

```sql
sources(id, tenant_id, name, kind, settings jsonb, secret_ref, created_at, updated_at)
destinations(id, tenant_id, name, kind, settings jsonb, secret_ref, created_at, updated_at)
```

- `tenant_id` is the partition/isolation key.
- The store sits behind a **repository trait**; SQLite can serve a zero-infra v1.

---

## 7. Registry

```
"http"       → descriptor (source)      + HTTP connector
"postgres"   → descriptor (source+dest) + Postgres connector
"clickhouse" → descriptor (destination) + ClickHouse connector
"mysql"      → descriptor (source)      + MySQL connector   (later)
```

One descriptor + one connector per kind; the role decides which list it appears in.

---

## 8. CLI

| Tool | Input | Output | Idempotent |
|---|---|---|---|
| `connector.list` | — | kinds + fields | — |
| `source.add` | kind + required fields | source | yes |
| `source.test` | name | ok / error | yes |
| `source.list` | — | sources | — |
| `destination.add` | kind + fields | destination | yes |
| `destination.test` | name | ok / error | yes |
| `destination.list` | — | destinations | — |
| `plan` | config file | what would change | — |
| `apply` | config file | summary / diff | yes |

### 8.1 plan / apply / test

- **`test`** — connect without moving data; return `ok` or a clear error.
- **`plan`** — dry run: compare config file to store, show `+ create` / `~ update` / `= no change`.
- **`apply`** — make reality match the file; idempotent create/update.

```
write config file → plan (preview) → apply (commit) → test (verify)
```

The CLI's own config-store connection is bootstrapped from env / `talaint.toml`.

### 8.2 What the CLI asks

- **Source:** `name`, `kind`, location, credentials, plus options (`format`, TLS...).
- **Destination:** `name`, `kind`, location, credentials, target, write mode/keys.
- Purpose is **not** asked here — it belongs to the dataset.

### 8.3 Handoff to the dataset phase

We generate and store, per config: **stable `id`**, normalized `settings`, a
**capabilities snapshot**, descriptor version, config revision, and last test status.
The dataset phase then builds `source_id + destination_id + purpose` and checks
capabilities (e.g. `infer_schema`, `create_table`, `upsert`). `talaint list --json`
exposes this inventory to humans now and to the agent later.

---

## 9. Test stack

```
 HOST ── talaint CLI ── localhost:<port>
 │
 ╔══════════════ docker network: talaint-net ══════════════╗
 ║ CONTROL   config-db (pg16)          :5432  config store ║
 ║ SOURCES   source-pg (pg16)          :5433  seeded       ║
 ║           source-mysql (8)          :3306  seeded       ║
 ║           http-source (nginx)       :8080  sample files ║
 ║ DESTS     dest-pg (pg16)            :5434  empty        ║
 ║           clickhouse (24.x)         :8123  warehouse    ║
 ╚═════════════════════════════════════════════════════════╝
```

| Container | Role | Exercised by |
|---|---|---|
| `config-db` | config store | `apply`, `list`, `plan`, `test` |
| `source-pg` | SQL source | `postgres` connector |
| `source-mysql` | second SQL source | `mysql` connector (later) |
| `http-source` | HTTP source | `http` connector |
| `dest-pg` | SQL destination | `postgres` connector |
| `clickhouse` | warehouse | `clickhouse` connector |

Workflow: `docker compose up -d` → `talaint apply -f talaint.yaml` →
`talaint source test ...` / `destination test ...` → `docker compose down -v`.

Notes: keep config and data in separate DBs; health checks + seed scripts; move MinIO
off `:9000` later; the same stack runs in CI.

---

## 10. Deferred and next steps

**Deferred:** JSON schemas per tool, structured errors, `plan` as an approval gate,
non-interactive flags, scoped credentials.

**Next design steps:** finalize descriptor fields (`http`, `postgres`, `clickhouse`),
finalize the YAML config schema, finalize CLI names/flags/output — then implement in
order: core types → registry → repository → CLI → connectors → test stack.

**Idea (deferred with the dataset phase): derive the pipeline from the configs.**
Once the client has configured the source(s) and destination(s), we can *propose* the
transformation instead of asking for it up front:

- introspect both ends (`infer_schema` / `describe` capabilities),
- match columns by name and type, infer keys and write mode,
- use `purpose` to pick a default strategy (analytics → union/upsert, rag → chunk/embed,
  search → index, agent → append),
- show the result as a `plan` (`+ create`, `~ update`, `= no change`) for the client to
  review and override.

This is inference, not truth: business rules, joins across sources, unit conversions and
PII handling still need explicit mapping. The configs give us enough to start; `plan`
makes the assumption visible and correctable.

