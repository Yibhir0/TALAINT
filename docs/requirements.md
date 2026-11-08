# TALAINT — Requirements

**Status:** living draft.
**Purpose:** one place for everything we intend to build, across phases.
**Design detail:** see [`configuration-phase.md`](configuration-phase.md) (phase 1) and
[`architecture.md`](architecture.md) (target architecture).

---

## 1. Vision

TALAINT lets a client declare:

- **where data comes from** — source: location, format, credentials;
- **what it is for** — purpose;
- **where it should go** — destination: database, schema, table;

then derives and performs the transformation that maps source data into the destination,
and stores it in the right place.

---

## 2. Phases

| Phase | Focus | Status |
|---|---|---|
| 1 | Configuration: descriptors, registry, source/destination configs, CLI, test stack | current |
| 2 | Dataset/mapping/transform: bind sources to destinations, derive mapping, execute, verify | later |
| 3 | Agent integration: expose the validated tools to an agent | last |

---

## 3. Functional requirements

### 3.1 Phase 1 — Configuration

**Descriptors**

- R1.1 One descriptor per connector kind, tagged with a role: `source`, `destination`, or `both`.
- R1.2 Descriptor meta: `kind`, `display_name`, `description`, `role`, `version`.
- R1.3 Capabilities: `connect`, `read`, `write`, `upsert`, `create_table`, `infer_schema`, `describe`.
- R1.4 Fields: `name`, `type`, `required`, `default`, allowed values, `group`, `secret`, `help`, conditional rules.
- R1.5 Built-in kinds: `http` (source), `postgres` (both), `clickhouse` (destination); `mysql` later.

**Registry**

- R1.6 Registry maps `kind → descriptor + connector`.
- R1.7 CLI and engine depend only on the registry; adding a kind does not change the CLI.

**Configuration model**

- R1.8 Source and destination share one envelope: `name + kind + settings + secret_ref`.
- R1.9 `settings` is descriptor-declared (JSONB in the store); there is no universal location/target.
- R1.10 Credentials are referenced by `secret_ref`, never stored inline.
- R1.11 Configs are per tenant (`tenant_id`).
- R1.12 Purpose is not part of an endpoint config; it belongs to the dataset (phase 2).

**Config store**

- R1.13 Store `sources` and `destinations` in Postgres with `settings jsonb`.
- R1.14 Access through a repository trait; SQLite may serve a zero-infra v1.
- R1.15 Bootstrap the store connection from env / `talaint.toml`.

**CLI**

- R1.16 `connector list` — kinds + fields.
- R1.17 `source add`, `source list`, `source test`.
- R1.18 `destination add`, `destination list`, `destination test`.
- R1.19 `plan -f` — dry-run diff (`+ create`, `~ update`, `= no change`).
- R1.20 `apply -f` — idempotent commit.
- R1.21 `test` — connection check without moving data.
- R1.22 Handoff to phase 2: stable `id`, normalized `settings`, capabilities snapshot, descriptor version, config revision, last test status.

**Test stack**

- R1.23 Docker network `talaint-net`.
- R1.24 Containers: `config-db`, `source-pg`, `source-mysql`, `http-source`, `dest-pg`, `clickhouse`.
- R1.25 Seed data organized by purpose: `analytics`, `rag`, `agent`, `search`, `other`.
- R1.26 Destinations initialized with purpose schemas/tables but no rows.

### 3.2 Phase 2 — Dataset / mapping / transform

**Binding**

- R2.1 A dataset binds 1..N inputs (sources) to 1..N outputs (destinations).
- R2.2 A dataset has one `purpose`: `analytics`, `rag`, `agent`, `search`, `other`.
- R2.3 A dataset has a `strategy`: `mirror`, `union`, `join`, `upsert`, `embed`, `split`.
- R2.4 Inputs/outputs may carry selectors/targets (table, path, collection).
- R2.5 Cardinality is expressed by the input/output lists (1:1, N:1, 1:N, N:M).

**Derivation**

- R2.6 Propose the mapping from the configured endpoints: introspect both sides, match columns by name/type, infer keys and write mode.
- R2.7 Use `purpose` to pick a default strategy.
- R2.8 Show the proposal as a plan for review and override.
- R2.9 Explicit mapping for business rules, cross-source joins, unit conversions, PII handling.

**Execution and verification**

- R2.10 Execute only a validated plan.
- R2.11 Idempotent writes.
- R2.12 Verify row counts, schema, and nulls after write.
- R2.13 Store the transformation spec and run metadata for reproducibility and audit.

### 3.3 Phase 3 — Agent integration

- R3.1 The agent is a separate integration layer, built after the CLI and core pipeline logic.
- R3.2 Tools are self-describing: name, description, input/output/error schemas.
- R3.3 The agent selects the tool, structures arguments, proposes the mapping, and recovers from structured errors.
- R3.4 Tools enforce validation, capability checks, plan/apply, verification, and storage.
- R3.5 Non-interactive flags, structured errors, plan as an approval gate, scoped credentials.
- R3.6 MCP wrapper (or equivalent) for agent integration.

---

## 4. Non-functional requirements

- N1 **Idempotency:** repeated `apply`/runs do not duplicate data.
- N2 **Reproducibility:** configs, descriptor versions, and run specs are stored.
- N3 **Auditability:** config revisions, test status, run metadata.
- N4 **Tenant isolation:** `tenant_id` on configs and datasets.
- N5 **Secrets:** only references, never inline.
- N6 **Extensibility:** add a kind = descriptor + connector.
- N7 **Safety:** plan before apply; capability checks; post-write verification.
- N8 **Portability:** config store behind a repository trait (Postgres/SQLite).
- N9 **Testability:** containerized, seeded test stack.

---

## 5. Data model (target)

```sql
sources(id, tenant_id, name, kind, settings jsonb, secret_ref, created_at, updated_at)
destinations(id, tenant_id, name, kind, settings jsonb, secret_ref, created_at, updated_at)

datasets(id, tenant_id, name, purpose, strategy, created_at, updated_at)
dataset_inputs(dataset_id, source_id, selector, position)
dataset_outputs(dataset_id, destination_id, target, write_mode, position)
runs(id, dataset_id, status, started_at, finished_at, stats jsonb)
```

---

## 6. Out of scope (phase 1)

Ingest, transform, dataset, purpose, runs, serving, plugins, agent, auth.

---

## 7. Open decisions

- Config-store bootstrap: env vs `talaint.toml`.
- CLI style: flags vs interactive.
- YAML schema shape.
- Dataset naming: `Dataset` vs `Pipeline` / `Flow` / `Binding`.
- Binding granularity: connection vs object.
- Strategy set: closed set vs arbitrary transforms.
- One purpose per dataset vs per output.
- Whether to collect purpose in phase 1 or defer.

---

## 8. References

- [`configuration-phase.md`](configuration-phase.md)
- [`architecture.md`](architecture.md)

---

## 9. Next step (phase 1 implementation)

Use the `talaint-net` test environment to build the generic configuration logic for the CLI.

1. **Bootstrap** — `TALAINT_CONFIG_DB_URL` env → `config-db`.
2. **Core types** — `ConfigEnvelope`, `SourceConfig`, `DestinationConfig`, `Role`, `Descriptor`, `FieldSpec`, `Capabilities`.
3. **Registry** — `http`, `postgres`, `clickhouse`.
4. **Repository trait + Postgres impl** — save/load configs.
5. **CLI** — `connector list`, `source add/list`, `destination add/list`.

Then `test`, `plan`, `apply` on top.

Defaults unless changed: env-based bootstrap, flags + `--json` output (interactive prompts later).
