CREATE DATABASE IF NOT EXISTS analytics;
SET allow_experimental_object_type = 1;;

CREATE TABLE analytics.events
(
    user_id UInt64,
    event_ts DateTime64(3),
    event_type LowCardinality(String),
    action LowCardinality(String),
    info JSON
)
ENGINE = MergeTree()
PARTITION BY toYYYYMM(event_ts)
ORDER BY (user_id, event_type, action, event_ts)
SETTINGS index_granularity = 8192;


CREATE TABLE analytics.queue
(
    user_id UInt64,
    event_ts DateTime64(3),
    event_type String,
    action String,
    info JSON
)
ENGINE = Kafka
SETTINGS
    kafka_broker_list = 'kafka-1:19092,kafka-2:19092,kafka-3:19092',
    kafka_topic_list = 'user-events',
    kafka_group_name = 'clickhouse_event_ingesters_main',
    kafka_format = 'JSONEachRow',
    kafka_num_consumers = 6,
    kafka_skip_broken_messages = 100,
    kafka_flush_interval_ms = 2000,
    kafka_max_block_size = 65536,
    kafka_thread_per_consumer = 1;

CREATE MATERIALIZED VIEW analytics.view TO analytics.events
(
    `user_id` UInt64,
    `event_ts` DateTime64(3),
    `event_type` LowCardinality(String),
    `action` LowCardinality(String),
    `info` JSON
) AS
SELECT
    user_id,
    event_ts,
    event_type,
    action,
    info
FROM analytics.queue;
