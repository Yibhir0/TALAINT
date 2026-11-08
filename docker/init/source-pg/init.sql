CREATE SCHEMA IF NOT EXISTS analytics;
CREATE SCHEMA IF NOT EXISTS rag;
CREATE SCHEMA IF NOT EXISTS agent;
CREATE SCHEMA IF NOT EXISTS search;
CREATE SCHEMA IF NOT EXISTS other;

CREATE TABLE IF NOT EXISTS analytics.customers (
    customer_id integer PRIMARY KEY,
    name text NOT NULL,
    email text NOT NULL,
    country text NOT NULL,
    created_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics.orders (
    order_id integer PRIMARY KEY,
    customer_id integer NOT NULL REFERENCES analytics.customers(customer_id),
    order_date date NOT NULL,
    status text NOT NULL,
    total_amount numeric(12, 2) NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics.order_items (
    order_id integer NOT NULL REFERENCES analytics.orders(order_id),
    line_no integer NOT NULL,
    product_id integer NOT NULL,
    quantity integer NOT NULL,
    unit_price numeric(12, 2) NOT NULL,
    PRIMARY KEY (order_id, line_no)
);

CREATE TABLE IF NOT EXISTS rag.documents (
    document_id integer PRIMARY KEY,
    title text NOT NULL,
    source_url text,
    content text NOT NULL,
    created_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS rag.document_chunks (
    chunk_id integer PRIMARY KEY,
    document_id integer NOT NULL REFERENCES rag.documents(document_id),
    chunk_index integer NOT NULL,
    content text NOT NULL,
    token_count integer NOT NULL
);

CREATE TABLE IF NOT EXISTS agent.agent_runs (
    run_id uuid PRIMARY KEY,
    agent_name text NOT NULL,
    started_at timestamptz NOT NULL,
    finished_at timestamptz,
    status text NOT NULL
);

CREATE TABLE IF NOT EXISTS agent.tool_calls (
    call_id integer PRIMARY KEY,
    run_id uuid NOT NULL REFERENCES agent.agent_runs(run_id),
    tool_name text NOT NULL,
    arguments jsonb NOT NULL,
    result jsonb,
    called_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS search.products (
    product_id integer PRIMARY KEY,
    name text NOT NULL,
    description text NOT NULL,
    category text NOT NULL,
    price numeric(12, 2) NOT NULL,
    search_vector tsvector GENERATED ALWAYS AS (to_tsvector('english', name || ' ' || description)) STORED
);

CREATE TABLE IF NOT EXISTS search.search_queries (
    query_id integer PRIMARY KEY,
    query_text text NOT NULL,
    result_count integer NOT NULL,
    searched_at timestamptz NOT NULL
);

CREATE TABLE IF NOT EXISTS other.misc_events (
    event_id integer PRIMARY KEY,
    event_type text NOT NULL,
    payload jsonb NOT NULL,
    occurred_at timestamptz NOT NULL
);

INSERT INTO analytics.customers (customer_id, name, email, country, created_at) VALUES
    (1, 'Ada Lovelace', 'ada@example.com', 'UK', '2024-01-05 09:00:00+00'),
    (2, 'Grace Hopper', 'grace@example.com', 'US', '2024-01-07 10:30:00+00'),
    (3, 'Alan Turing', 'alan@example.com', 'UK', '2024-02-01 08:15:00+00'),
    (4, 'Katherine Johnson', 'katherine@example.com', 'US', '2024-02-14 12:00:00+00'),
    (5, 'Linus Torvalds', 'linus@example.com', 'FI', '2024-03-02 16:45:00+00')
ON CONFLICT DO NOTHING;

INSERT INTO analytics.orders (order_id, customer_id, order_date, status, total_amount) VALUES
    (1001, 1, '2024-03-01', 'paid', 129.99),
    (1002, 1, '2024-03-05', 'paid', 79.50),
    (1003, 2, '2024-03-07', 'shipped', 240.00),
    (1004, 3, '2024-03-09', 'pending', 15.75),
    (1005, 4, '2024-03-11', 'paid', 560.20),
    (1006, 5, '2024-03-12', 'cancelled', 99.99),
    (1007, 2, '2024-03-15', 'paid', 45.00),
    (1008, 4, '2024-03-18', 'shipped', 320.10)
ON CONFLICT DO NOTHING;

INSERT INTO analytics.order_items (order_id, line_no, product_id, quantity, unit_price) VALUES
    (1001, 1, 501, 2, 49.99),
    (1001, 2, 502, 1, 30.01),
    (1002, 1, 503, 1, 79.50),
    (1003, 1, 504, 3, 80.00),
    (1004, 1, 505, 1, 15.75),
    (1005, 1, 506, 2, 200.10),
    (1005, 2, 507, 1, 160.00),
    (1006, 1, 508, 1, 99.99),
    (1007, 1, 509, 1, 45.00),
    (1008, 1, 510, 2, 160.05)
ON CONFLICT DO NOTHING;

INSERT INTO rag.documents (document_id, title, source_url, content, created_at) VALUES
    (1, 'TALAINT overview', 'https://talaint.example/docs/overview', 'TALAINT is a data integration platform that moves data from sources to warehouses.', '2024-04-01 09:00:00+00'),
    (2, 'Connector design', 'https://talaint.example/docs/connectors', 'Connectors are described by descriptors and resolved through a registry.', '2024-04-02 10:00:00+00'),
    (3, 'Configuration model', 'https://talaint.example/docs/config', 'Source and destination configs share the same envelope.', '2024-04-03 11:00:00+00')
ON CONFLICT DO NOTHING;

INSERT INTO rag.document_chunks (chunk_id, document_id, chunk_index, content, token_count) VALUES
    (1, 1, 0, 'TALAINT is a data integration platform that moves data from sources to warehouses.', 12),
    (2, 1, 1, 'It uses built-in connectors and a declarative configuration model.', 9),
    (3, 2, 0, 'A descriptor describes the fields and capabilities of a connector kind.', 11),
    (4, 2, 1, 'The registry maps each kind to its descriptor and connector.', 9),
    (5, 3, 0, 'A source config and a destination config share the same envelope.', 11),
    (6, 3, 1, 'Only name, kind, and secret_ref are universal; settings are descriptor-declared.', 11)
ON CONFLICT DO NOTHING;

INSERT INTO agent.agent_runs (run_id, agent_name, started_at, finished_at, status) VALUES
    ('a0000000-0000-0000-0000-000000000001', 'ingest-agent', '2024-05-01 08:00:00+00', '2024-05-01 08:00:42+00', 'succeeded'),
    ('a0000000-0000-0000-0000-000000000002', 'schema-agent', '2024-05-01 09:00:00+00', '2024-05-01 09:01:10+00', 'succeeded'),
    ('a0000000-0000-0000-0000-000000000003', 'repair-agent', '2024-05-01 10:00:00+00', NULL, 'running')
ON CONFLICT DO NOTHING;

INSERT INTO agent.tool_calls (call_id, run_id, tool_name, arguments, result, called_at) VALUES
    (1, 'a0000000-0000-0000-0000-000000000001', 'source.test', '{"name":"ops-postgres"}'::jsonb, '{"ok":true}'::jsonb, '2024-05-01 08:00:10+00'),
    (2, 'a0000000-0000-0000-0000-000000000001', 'destination.test', '{"name":"warehouse-postgres"}'::jsonb, '{"ok":true}'::jsonb, '2024-05-01 08:00:20+00'),
    (3, 'a0000000-0000-0000-0000-000000000001', 'apply', '{"file":"talaint.yaml"}'::jsonb, '{"created":2,"updated":0}'::jsonb, '2024-05-01 08:00:40+00'),
    (4, 'a0000000-0000-0000-0000-000000000002', 'connector.list', '{}'::jsonb, '{"kinds":["http","postgres","clickhouse"]}'::jsonb, '2024-05-01 09:00:30+00'),
    (5, 'a0000000-0000-0000-0000-000000000002', 'plan', '{"file":"talaint.yaml"}'::jsonb, '{"changes":0}'::jsonb, '2024-05-01 09:01:00+00'),
    (6, 'a0000000-0000-0000-0000-000000000003', 'source.test', '{"name":"ops-postgres"}'::jsonb, NULL, '2024-05-01 10:00:30+00')
ON CONFLICT DO NOTHING;

INSERT INTO search.products (product_id, name, description, category, price) VALUES
    (501, 'Mechanical Keyboard', 'A compact mechanical keyboard with tactile switches.', 'electronics', 89.99),
    (502, 'USB-C Hub', 'Seven-port USB-C hub with HDMI and ethernet.', 'electronics', 30.01),
    (503, 'Noise-Cancelling Headphones', 'Over-ear headphones with active noise cancellation.', 'audio', 79.50),
    (504, 'Standing Desk', 'Height-adjustable standing desk with memory presets.', 'furniture', 80.00),
    (505, 'Ergonomic Mouse', 'Vertical ergonomic mouse with wireless connectivity.', 'electronics', 15.75)
ON CONFLICT DO NOTHING;

INSERT INTO search.search_queries (query_id, query_text, result_count, searched_at) VALUES
    (1, 'mechanical keyboard', 1, '2024-05-02 08:00:00+00'),
    (2, 'usb c hub', 1, '2024-05-02 08:05:00+00'),
    (3, 'headphones', 1, '2024-05-02 08:10:00+00'),
    (4, 'standing desk', 1, '2024-05-02 08:15:00+00'),
    (5, 'ergonomic mouse', 1, '2024-05-02 08:20:00+00')
ON CONFLICT DO NOTHING;

INSERT INTO other.misc_events (event_id, event_type, payload, occurred_at) VALUES
    (1, 'deploy', '{"service":"cli","version":"0.1.0"}'::jsonb, '2024-05-03 07:00:00+00'),
    (2, 'config.change', '{"source":"ops-postgres","field":"schema"}'::jsonb, '2024-05-03 07:30:00+00'),
    (3, 'alert', '{"severity":"warning","message":"slow source"}'::jsonb, '2024-05-03 08:00:00+00'),
    (4, 'deploy', '{"service":"agent","version":"0.1.0"}'::jsonb, '2024-05-03 08:30:00+00'),
    (5, 'config.change', '{"destination":"warehouse-clickhouse","field":"table"}'::jsonb, '2024-05-03 09:00:00+00')
ON CONFLICT DO NOTHING;
