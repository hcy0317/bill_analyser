\set ON_ERROR_STOP on
\pset pager off

BEGIN;
SET LOCAL jit = off;

CREATE TEMP TABLE bench_signal_source ON COMMIT DROP AS
SELECT
    preview.id,
    preview.session_id,
    preview.user_id,
    preview.page_sort_key,
    flags.parser,
    flags.platform_duplicate,
    flags.transfer,
    flags.history,
    flags.learning,
    flags.llm
FROM import_preview_rows preview
CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(preview.preview_payload)) AS flags(
    parser BOOLEAN,
    platform_duplicate BOOLEAN,
    transfer BOOLEAN,
    history BOOLEAN,
    learning BOOLEAN,
    llm BOOLEAN
);
ANALYZE bench_signal_source;

CREATE TEMP TABLE bench_signal_target ON COMMIT DROP AS
SELECT session_id, COUNT(*)::BIGINT AS preview_rows
FROM bench_signal_source
GROUP BY session_id
ORDER BY COUNT(*) DESC, session_id
LIMIT 1;

CREATE TEMP TABLE bench_signal_boolean (
    id BIGINT PRIMARY KEY,
    session_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    page_sort_key TEXT NOT NULL,
    signal_parser BOOLEAN NOT NULL,
    signal_platform_duplicate BOOLEAN NOT NULL,
    signal_transfer BOOLEAN NOT NULL,
    signal_history BOOLEAN NOT NULL,
    signal_learning BOOLEAN NOT NULL,
    signal_llm BOOLEAN NOT NULL
) ON COMMIT DROP;
CREATE INDEX bench_signal_boolean_session_idx
    ON bench_signal_boolean (session_id, page_sort_key);
INSERT INTO bench_signal_boolean
SELECT
    id,
    session_id,
    user_id,
    page_sort_key,
    parser,
    platform_duplicate,
    transfer,
    history,
    learning,
    llm
FROM bench_signal_source;
ANALYZE bench_signal_boolean;

CREATE TEMP TABLE bench_signal_membership (
    preview_row_id BIGINT NOT NULL,
    session_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    page_sort_key TEXT NOT NULL,
    signal_family TEXT NOT NULL,
    PRIMARY KEY (preview_row_id, signal_family)
) ON COMMIT DROP;
CREATE INDEX bench_signal_membership_family_idx
    ON bench_signal_membership (session_id, signal_family, page_sort_key);
INSERT INTO bench_signal_membership
SELECT
    source.id,
    source.session_id,
    source.user_id,
    source.page_sort_key,
    family.signal_family
FROM bench_signal_source source
CROSS JOIN LATERAL (
    VALUES
        ('parser', source.parser),
        ('platform_duplicate', source.platform_duplicate),
        ('transfer', source.transfer),
        ('history', source.history),
        ('learning', source.learning),
        ('llm', source.llm)
) family(signal_family, enabled)
WHERE family.enabled;
ANALYZE bench_signal_membership;

CREATE TEMP TABLE bench_signal_target_rows ON COMMIT DROP AS
SELECT source.*
FROM bench_signal_source source
JOIN bench_signal_target target USING (session_id);
CREATE TEMP TABLE bench_signal_target_memberships ON COMMIT DROP AS
SELECT membership.*
FROM bench_signal_membership membership
JOIN bench_signal_target target USING (session_id);
ANALYZE bench_signal_target_rows;
ANALYZE bench_signal_target_memberships;

CREATE TEMP TABLE bench_write_base (
    id BIGINT PRIMARY KEY,
    session_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    page_sort_key TEXT NOT NULL
) ON COMMIT DROP;
CREATE INDEX bench_write_base_session_idx
    ON bench_write_base (session_id, page_sort_key);
CREATE TEMP TABLE bench_write_boolean
    (LIKE bench_signal_boolean INCLUDING ALL) ON COMMIT DROP;
CREATE TEMP TABLE bench_write_boolean_indexed
    (LIKE bench_signal_boolean INCLUDING ALL) ON COMMIT DROP;
CREATE INDEX bench_write_boolean_indexed_parser_idx
    ON bench_write_boolean_indexed (session_id, page_sort_key) WHERE signal_parser;
CREATE INDEX bench_write_boolean_indexed_platform_idx
    ON bench_write_boolean_indexed (session_id, page_sort_key) WHERE signal_platform_duplicate;
CREATE INDEX bench_write_boolean_indexed_transfer_idx
    ON bench_write_boolean_indexed (session_id, page_sort_key) WHERE signal_transfer;
CREATE INDEX bench_write_boolean_indexed_history_idx
    ON bench_write_boolean_indexed (session_id, page_sort_key) WHERE signal_history;
CREATE INDEX bench_write_boolean_indexed_learning_idx
    ON bench_write_boolean_indexed (session_id, page_sort_key) WHERE signal_learning;
CREATE INDEX bench_write_boolean_indexed_llm_idx
    ON bench_write_boolean_indexed (session_id, page_sort_key) WHERE signal_llm;
CREATE TEMP TABLE bench_write_membership_base
    (LIKE bench_write_base INCLUDING ALL) ON COMMIT DROP;
CREATE TEMP TABLE bench_write_membership_child
    (LIKE bench_signal_membership INCLUDING ALL) ON COMMIT DROP;
CREATE TEMP TABLE bench_signal_measurements (
    candidate TEXT NOT NULL,
    metric TEXT NOT NULL,
    iteration INTEGER NOT NULL,
    milliseconds NUMERIC NOT NULL
) ON COMMIT DROP;

DO $benchmark$
DECLARE
    iteration INTEGER;
    target_session_id BIGINT;
    started_at TIMESTAMPTZ;
    elapsed_ms NUMERIC;
BEGIN
    SELECT session_id INTO STRICT target_session_id FROM bench_signal_target;

    FOR iteration IN 0..5 LOOP
        started_at := clock_timestamp();
        PERFORM
            COUNT(*) FILTER (WHERE flags.parser),
            COUNT(*) FILTER (WHERE flags.platform_duplicate),
            COUNT(*) FILTER (WHERE flags.transfer),
            COUNT(*) FILTER (WHERE flags.history),
            COUNT(*) FILTER (WHERE flags.learning),
            COUNT(*) FILTER (WHERE flags.llm)
        FROM import_preview_rows preview
        CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(preview.preview_payload)) AS flags(
            parser BOOLEAN,
            platform_duplicate BOOLEAN,
            transfer BOOLEAN,
            history BOOLEAN,
            learning BOOLEAN,
            llm BOOLEAN
        )
        WHERE preview.session_id = target_session_id;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('json', 'metadata', iteration, elapsed_ms);
        END IF;

        started_at := clock_timestamp();
        PERFORM
            COUNT(*) FILTER (WHERE signal_parser),
            COUNT(*) FILTER (WHERE signal_platform_duplicate),
            COUNT(*) FILTER (WHERE signal_transfer),
            COUNT(*) FILTER (WHERE signal_history),
            COUNT(*) FILTER (WHERE signal_learning),
            COUNT(*) FILTER (WHERE signal_llm)
        FROM bench_signal_boolean
        WHERE session_id = target_session_id;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('boolean_minimal', 'metadata', iteration, elapsed_ms);
        END IF;

        started_at := clock_timestamp();
        PERFORM COUNT(*)
        FROM (
            SELECT signal_family, COUNT(*)
            FROM bench_signal_membership
            WHERE session_id = target_session_id
            GROUP BY signal_family
        ) grouped;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('membership', 'metadata', iteration, elapsed_ms);
        END IF;

        started_at := clock_timestamp();
        PERFORM id
        FROM (
            SELECT preview.id
            FROM import_preview_rows preview
            CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(preview.preview_payload)) AS flags(
                parser BOOLEAN,
                platform_duplicate BOOLEAN,
                transfer BOOLEAN,
                history BOOLEAN,
                learning BOOLEAN,
                llm BOOLEAN
            )
            WHERE preview.session_id = target_session_id AND flags.learning
            ORDER BY preview.page_sort_key
            LIMIT 100
        ) page;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('json', 'learning_page', iteration, elapsed_ms);
        END IF;

        started_at := clock_timestamp();
        PERFORM id
        FROM (
            SELECT id
            FROM bench_signal_boolean
            WHERE session_id = target_session_id AND signal_learning
            ORDER BY page_sort_key
            LIMIT 100
        ) page;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('boolean_minimal', 'learning_page', iteration, elapsed_ms);
        END IF;

        started_at := clock_timestamp();
        PERFORM preview_row_id
        FROM (
            SELECT preview_row_id
            FROM bench_signal_membership
            WHERE session_id = target_session_id AND signal_family = 'learning'
            ORDER BY page_sort_key
            LIMIT 100
        ) page;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('membership', 'learning_page', iteration, elapsed_ms);
        END IF;

        TRUNCATE bench_write_base;
        started_at := clock_timestamp();
        INSERT INTO bench_write_base
        SELECT id, session_id, user_id, page_sort_key FROM bench_signal_target_rows;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('base', 'write_target_session', iteration, elapsed_ms);
        END IF;

        TRUNCATE bench_write_boolean;
        started_at := clock_timestamp();
        INSERT INTO bench_write_boolean
        SELECT id, session_id, user_id, page_sort_key, parser, platform_duplicate,
               transfer, history, learning, llm
        FROM bench_signal_target_rows;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('boolean_minimal', 'write_target_session', iteration, elapsed_ms);
        END IF;

        TRUNCATE bench_write_boolean_indexed;
        started_at := clock_timestamp();
        INSERT INTO bench_write_boolean_indexed
        SELECT id, session_id, user_id, page_sort_key, parser, platform_duplicate,
               transfer, history, learning, llm
        FROM bench_signal_target_rows;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('boolean_six_partial_indexes', 'write_target_session', iteration, elapsed_ms);
        END IF;

        TRUNCATE bench_write_membership_base;
        TRUNCATE bench_write_membership_child;
        started_at := clock_timestamp();
        INSERT INTO bench_write_membership_base
        SELECT id, session_id, user_id, page_sort_key FROM bench_signal_target_rows;
        INSERT INTO bench_write_membership_child
        SELECT preview_row_id, session_id, user_id, page_sort_key, signal_family
        FROM bench_signal_target_memberships;
        elapsed_ms := EXTRACT(EPOCH FROM clock_timestamp() - started_at) * 1000;
        IF iteration > 0 THEN
            INSERT INTO bench_signal_measurements
            VALUES ('membership', 'write_target_session', iteration, elapsed_ms);
        END IF;
    END LOOP;
END
$benchmark$;

SELECT jsonb_pretty(jsonb_build_object(
    'target', (SELECT to_jsonb(target) FROM bench_signal_target target),
    'source_rows', (SELECT COUNT(*) FROM bench_signal_source),
    'membership_rows', (SELECT COUNT(*) FROM bench_signal_membership),
    'boolean_total_bytes', pg_total_relation_size('bench_signal_boolean'::regclass),
    'boolean_heap_bytes', pg_relation_size('bench_signal_boolean'::regclass),
    'boolean_index_bytes', pg_indexes_size('bench_signal_boolean'::regclass),
    'membership_total_bytes', pg_total_relation_size('bench_signal_membership'::regclass),
    'membership_index_bytes', pg_indexes_size('bench_signal_membership'::regclass)
));

SELECT jsonb_pretty(jsonb_agg(result ORDER BY metric, candidate))
FROM (
    SELECT
        metric,
        candidate,
        jsonb_build_object(
            'candidate', candidate,
            'metric', metric,
            'samples', COUNT(*),
            'min_ms', ROUND(MIN(milliseconds), 3),
            'p50_ms', ROUND(
                percentile_cont(0.5) WITHIN GROUP (ORDER BY milliseconds)::NUMERIC,
                3
            ),
            'p95_ms', ROUND(
                percentile_cont(0.95) WITHIN GROUP (ORDER BY milliseconds)::NUMERIC,
                3
            ),
            'max_ms', ROUND(MAX(milliseconds), 3)
        ) AS result
    FROM bench_signal_measurements
    GROUP BY metric, candidate
) summary;

EXPLAIN (ANALYZE, BUFFERS, WAL, TIMING OFF)
SELECT
    COUNT(*) FILTER (WHERE flags.parser),
    COUNT(*) FILTER (WHERE flags.platform_duplicate),
    COUNT(*) FILTER (WHERE flags.transfer),
    COUNT(*) FILTER (WHERE flags.history),
    COUNT(*) FILTER (WHERE flags.learning),
    COUNT(*) FILTER (WHERE flags.llm)
FROM import_preview_rows preview
CROSS JOIN LATERAL jsonb_to_record(import_preview_signal_flags(preview.preview_payload)) AS flags(
    parser BOOLEAN,
    platform_duplicate BOOLEAN,
    transfer BOOLEAN,
    history BOOLEAN,
    learning BOOLEAN,
    llm BOOLEAN
)
WHERE preview.session_id = (SELECT session_id FROM bench_signal_target);

EXPLAIN (ANALYZE, BUFFERS, WAL, TIMING OFF)
SELECT
    COUNT(*) FILTER (WHERE signal_parser),
    COUNT(*) FILTER (WHERE signal_platform_duplicate),
    COUNT(*) FILTER (WHERE signal_transfer),
    COUNT(*) FILTER (WHERE signal_history),
    COUNT(*) FILTER (WHERE signal_learning),
    COUNT(*) FILTER (WHERE signal_llm)
FROM bench_signal_boolean
WHERE session_id = (SELECT session_id FROM bench_signal_target);

EXPLAIN (ANALYZE, BUFFERS, WAL, TIMING OFF)
SELECT signal_family, COUNT(*)
FROM bench_signal_membership
WHERE session_id = (SELECT session_id FROM bench_signal_target)
GROUP BY signal_family;

ROLLBACK;
