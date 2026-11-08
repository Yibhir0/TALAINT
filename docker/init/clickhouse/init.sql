CREATE TABLE IF NOT EXISTS talaint_dw.analytics_orders (
    order_id UInt32,
    customer_id UInt32,
    order_date Date,
    status String,
    total_amount Decimal(12, 2)
) ENGINE = MergeTree()
ORDER BY (order_date, order_id);

CREATE TABLE IF NOT EXISTS talaint_dw.rag_documents (
    document_id UInt32,
    title String,
    source_url String,
    content String,
    created_at DateTime
) ENGINE = MergeTree()
ORDER BY (document_id);

CREATE TABLE IF NOT EXISTS talaint_dw.agent_runs (
    run_id String,
    agent_name String,
    started_at DateTime,
    finished_at Nullable(DateTime),
    status String
) ENGINE = MergeTree()
ORDER BY (started_at, run_id);

CREATE TABLE IF NOT EXISTS talaint_dw.search_products (
    product_id UInt32,
    name String,
    description String,
    category String,
    price Decimal(12, 2)
) ENGINE = MergeTree()
ORDER BY (product_id);

CREATE TABLE IF NOT EXISTS talaint_dw.other_events (
    event_id UInt32,
    event_type String,
    payload String,
    occurred_at DateTime
) ENGINE = MergeTree()
ORDER BY (occurred_at, event_id);
