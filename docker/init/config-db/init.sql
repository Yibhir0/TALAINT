CREATE TABLE IF NOT EXISTS sources (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id text NOT NULL,
    name text NOT NULL,
    kind text NOT NULL,
    settings jsonb NOT NULL DEFAULT '{}'::jsonb,
    secret_ref text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT sources_tenant_name_unique UNIQUE (tenant_id, name)
);

CREATE TABLE IF NOT EXISTS destinations (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id text NOT NULL,
    name text NOT NULL,
    kind text NOT NULL,
    settings jsonb NOT NULL DEFAULT '{}'::jsonb,
    secret_ref text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT destinations_tenant_name_unique UNIQUE (tenant_id, name)
);

CREATE INDEX IF NOT EXISTS sources_tenant_idx ON sources (tenant_id);
CREATE INDEX IF NOT EXISTS destinations_tenant_idx ON destinations (tenant_id);

INSERT INTO sources (tenant_id, name, kind, settings, secret_ref) VALUES
    ('demo', 'ops-postgres', 'postgres', '{"host":"source-pg","port":5432,"database":"source","user":"talaint","schema":"analytics"}'::jsonb, 'SOURCE_PG_PASSWORD')
ON CONFLICT DO NOTHING;

INSERT INTO destinations (tenant_id, name, kind, settings, secret_ref) VALUES
    ('demo', 'warehouse-postgres', 'postgres', '{"host":"dest-pg","port":5432,"database":"warehouse","user":"talaint","schema":"analytics","table":"orders","write_mode":"upsert","key_columns":["order_id"]}'::jsonb, 'DEST_PG_PASSWORD'),
    ('demo', 'warehouse-clickhouse', 'clickhouse', '{"url":"http://clickhouse:8123","database":"talaint_dw","user":"talaint","table":"analytics_orders","secure":false,"write_mode":"append"}'::jsonb, 'CLICKHOUSE_PASSWORD')
ON CONFLICT DO NOTHING;
