# TALAINT

TALAINT is a tenant-aware AI data ingestion and preparation platform. The v1 foundation uses a local CLI, PostgreSQL as the control plane, pipeline YAML configs, and tenant-scoped secrets.

Status: M1–M2 foundation. The CLI can manage tenants, tokens, secrets, pipeline configs, source health checks, and queued runs. The polling worker, connector execution, dataset purge, and conversational agent are planned for later milestones.

## Quick start

Full prerequisites and OS-specific commands are in `docs/running-locally.md`.

```bash
docker compose up -d
cargo run -q -p talaint-control-plane --bin migrate
cp .env.example .env
cargo build -q -p talaint-cli
cp config/pipeline.example.yaml pipeline.yaml
cargo run -q -p talaint-cli -- --help
```

On Windows, use `copy .env.example .env` and `copy config\pipeline.example.yaml pipeline.yaml` instead of `cp`.

Copy `config/pipeline.example.yaml` for a public no-secret sample, or pass sources with `generate --source ...`. With no `--source`, `generate` reads the stored pipeline from the DB and rewrites it to YAML. It checks intent, interval, URL, and source-name mistakes before writing.

Edit `.env` before using secrets or admin bootstrap. At minimum, set:

- `DATABASE_URL`
- `TALAINT_ADMIN_TOKEN`
- `TALAINT_KEK_BASE64`
- `TALAINT_SECRET_<NORMALIZED_NAME>` for each secret you want to ingest

## Documentation

- `docs/running-locally.md` — prerequisites, environment setup, and OS-specific run commands
- `docs/cli-guide.md` — complete CLI usage guide, command reference, examples, and troubleshooting
- `docs/requirements.md` — product requirements and v1 decisions
- `docs/architecture.md` — architecture and milestone plan
- `docs/spec-v1.md` — YAML schema, data model, and CLI contract
