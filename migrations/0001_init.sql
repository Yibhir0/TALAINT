-- 0001_init.sql (foundation control plane)
-- To be applied with sqlx/migrations tooling during M1.

CREATE TABLE IF NOT EXISTS tenants (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS auth_tokens (
  id UUID PRIMARY KEY,
  kind TEXT NOT NULL CHECK (kind IN ('admin','tenant')),
  tenant_id UUID NULL REFERENCES tenants(id) ON DELETE CASCADE,
  token_hash TEXT NOT NULL UNIQUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ NULL,
  last_used_at TIMESTAMPTZ NULL
);

CREATE TABLE IF NOT EXISTS tenant_keys (
  tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
  wrapped_dek BYTEA NOT NULL,
  key_version INT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS secrets (
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  nonce BYTEA NOT NULL,
  ciphertext BYTEA NOT NULL,
  key_version INT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS pipelines (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  intent TEXT NOT NULL,
  config JSONB NOT NULL DEFAULT '{}'::jsonb,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (tenant_id, intent)
);

CREATE TABLE IF NOT EXISTS sources (
  id UUID PRIMARY KEY,
  pipeline_id UUID NOT NULL REFERENCES pipelines(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  type TEXT NOT NULL,
  config JSONB NOT NULL,
  schedule_interval TEXT NOT NULL DEFAULT '1m',
  initial_sync TEXT NOT NULL DEFAULT 'full',
  paused BOOLEAN NOT NULL DEFAULT false,
  next_run_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (pipeline_id, name)
);

CREATE TABLE IF NOT EXISTS pipeline_jobs (
  id UUID PRIMARY KEY,
  source_id UUID NOT NULL REFERENCES sources(id) ON DELETE CASCADE,
  status TEXT NOT NULL, -- queued|running|succeeded|failed|skipped
  attempt INT NOT NULL DEFAULT 0,
  heartbeat_at TIMESTAMPTZ NULL,
  started_at TIMESTAMPTZ NULL,
  finished_at TIMESTAMPTZ NULL,
  summary JSONB NULL,
  error TEXT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS pipeline_checkpoints (
  source_id UUID PRIMARY KEY REFERENCES sources(id) ON DELETE CASCADE,
  cursor_state JSONB NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS run_logs (
  id BIGSERIAL PRIMARY KEY,
  job_id UUID NOT NULL REFERENCES pipeline_jobs(id) ON DELETE CASCADE,
  ts TIMESTAMPTZ NOT NULL DEFAULT now(),
  level TEXT NOT NULL,
  message TEXT NOT NULL,
  data JSONB NULL
);

CREATE INDEX IF NOT EXISTS idx_sources_next_run_at ON sources(next_run_at);
CREATE INDEX IF NOT EXISTS idx_jobs_source_status_created ON pipeline_jobs(source_id, status, created_at);
