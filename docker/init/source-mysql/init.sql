USE source;

CREATE TABLE IF NOT EXISTS analytics_customers (
    customer_id INT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL,
    country VARCHAR(64) NOT NULL,
    created_at DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS analytics_orders (
    order_id INT PRIMARY KEY,
    customer_id INT NOT NULL,
    order_date DATE NOT NULL,
    status VARCHAR(32) NOT NULL,
    total_amount DECIMAL(12, 2) NOT NULL,
    CONSTRAINT fk_analytics_orders_customer FOREIGN KEY (customer_id) REFERENCES analytics_customers(customer_id)
);

CREATE TABLE IF NOT EXISTS rag_documents (
    document_id INT PRIMARY KEY,
    title VARCHAR(255) NOT NULL,
    source_url VARCHAR(512),
    content TEXT NOT NULL,
    created_at DATETIME NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_runs (
    run_id CHAR(36) PRIMARY KEY,
    agent_name VARCHAR(255) NOT NULL,
    started_at DATETIME NOT NULL,
    finished_at DATETIME,
    status VARCHAR(32) NOT NULL
);

CREATE TABLE IF NOT EXISTS search_products (
    product_id INT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    category VARCHAR(64) NOT NULL,
    price DECIMAL(12, 2) NOT NULL
);

CREATE TABLE IF NOT EXISTS other_events (
    event_id INT PRIMARY KEY,
    event_type VARCHAR(64) NOT NULL,
    payload JSON NOT NULL,
    occurred_at DATETIME NOT NULL
);

INSERT IGNORE INTO analytics_customers (customer_id, name, email, country, created_at) VALUES
    (1, 'Ada Lovelace', 'ada@example.com', 'UK', '2024-01-05 09:00:00'),
    (2, 'Grace Hopper', 'grace@example.com', 'US', '2024-01-07 10:30:00'),
    (3, 'Alan Turing', 'alan@example.com', 'UK', '2024-02-01 08:15:00'),
    (4, 'Katherine Johnson', 'katherine@example.com', 'US', '2024-02-14 12:00:00');

INSERT IGNORE INTO analytics_orders (order_id, customer_id, order_date, status, total_amount) VALUES
    (2001, 1, '2024-03-01', 'paid', 129.99),
    (2002, 2, '2024-03-07', 'shipped', 240.00),
    (2003, 3, '2024-03-09', 'pending', 15.75),
    (2004, 4, '2024-03-11', 'paid', 560.20),
    (2005, 2, '2024-03-15', 'paid', 45.00);

INSERT IGNORE INTO rag_documents (document_id, title, source_url, content, created_at) VALUES
    (1, 'TALAINT MySQL source', 'https://talaint.example/docs/mysql', 'This document lives in the MySQL source database.', '2024-04-01 09:00:00'),
    (2, 'Purpose-driven seeds', 'https://talaint.example/docs/seeds', 'Seed data is organized by purpose: analytics, rag, agent, search, other.', '2024-04-02 10:00:00');

INSERT IGNORE INTO agent_runs (run_id, agent_name, started_at, finished_at, status) VALUES
    ('b0000000-0000-0000-0000-000000000001', 'mysql-agent', '2024-05-01 08:00:00', '2024-05-01 08:00:30', 'succeeded'),
    ('b0000000-0000-0000-0000-000000000002', 'mysql-agent', '2024-05-01 09:00:00', NULL, 'running');

INSERT IGNORE INTO search_products (product_id, name, description, category, price) VALUES
    (601, 'Mechanical Keyboard', 'A compact mechanical keyboard with tactile switches.', 'electronics', 89.99),
    (602, 'USB-C Hub', 'Seven-port USB-C hub with HDMI and ethernet.', 'electronics', 30.01),
    (603, 'Noise-Cancelling Headphones', 'Over-ear headphones with active noise cancellation.', 'audio', 79.50),
    (604, 'Standing Desk', 'Height-adjustable standing desk with memory presets.', 'furniture', 80.00);

INSERT IGNORE INTO other_events (event_id, event_type, payload, occurred_at) VALUES
    (1, 'deploy', '{"service":"cli","version":"0.1.0"}', '2024-05-03 07:00:00'),
    (2, 'config.change', '{"source":"mysql-source","field":"database"}', '2024-05-03 07:30:00'),
    (3, 'alert', '{"severity":"info","message":"mysql source ready"}', '2024-05-03 08:00:00'),
    (4, 'deploy', '{"service":"agent","version":"0.1.0"}', '2024-05-03 08:30:00');
