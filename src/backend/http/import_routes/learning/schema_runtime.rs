// 中文导读：导入 learning 运行态 schema 初始化。
// 维护重点：SQLite legacy/test 链路必须可独立运行，同时保持 lifecycle/event/suppression 表与 Postgres 权威 schema 对齐。
// 不变式：旧库升级必须幂等补列，不能破坏既有 learning rules/suggestions 数据。

fn init_import_learning_runtime_schema(
    runtime: &SqliteRuntime,
) -> Result<(), ImportV2RouteResponse> {
    init_import_runtime_schema(runtime)?;
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_learning_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                learned_type TEXT,
                learned_category_id INTEGER,
                learned_source_account_id INTEGER,
                learned_destination_account_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                source_session_id TEXT,
                source_preview_id INTEGER,
                parser_id TEXT,
                composite_match_hash TEXT,
                match_features_json TEXT,
                applied_count INTEGER NOT NULL DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_rules_user_enabled
                ON import_learning_rules(user_id, enabled);
            CREATE TABLE IF NOT EXISTS import_learning_rule_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                rule_id INTEGER,
                session_id TEXT,
                preview_id INTEGER,
                action TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            ",
        )
        .map_err(db_error_response)
}

fn init_global_learning_runtime_schema(
    runtime: &SqliteRuntime,
) -> Result<(), ImportV2RouteResponse> {
    init_import_learning_runtime_schema(runtime)?;
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS import_learning_corpus_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                session_id TEXT NOT NULL,
                preview_id INTEGER NOT NULL,
                parser_id TEXT,
                counterparty TEXT,
                description TEXT,
                payment_method TEXT,
                composite_match_hash TEXT,
                match_features_json TEXT,
                annotated_type TEXT,
                annotated_category_id INTEGER,
                annotated_source_account_id INTEGER,
                annotated_destination_account_id INTEGER,
                source_snapshot_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, session_id, preview_id)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_user
                ON import_learning_corpus_samples(user_id, updated_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_hash
                ON import_learning_corpus_samples(user_id, composite_match_hash);
            CREATE TABLE IF NOT EXISTS import_learning_feedback_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                event_type TEXT NOT NULL,
                rule_id INTEGER,
                suggestion_id INTEGER,
                lifecycle_id INTEGER,
                recommendation_key TEXT,
                session_id TEXT,
                preview_id INTEGER,
                bill_id INTEGER,
                candidate_id TEXT,
                previous_signal_state TEXT,
                next_signal_state TEXT,
                payload_json TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_user
                ON import_learning_feedback_events(user_id, created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_type
                ON import_learning_feedback_events(user_id, event_type);
            CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_key
                ON import_learning_feedback_events(user_id, recommendation_key);
            CREATE TABLE IF NOT EXISTS import_learning_lifecycle (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                recommendation_key TEXT NOT NULL,
                recommendation_type TEXT NOT NULL DEFAULT 'import_preview',
                status TEXT NOT NULL DEFAULT 'yellow',
                accepted_count INTEGER NOT NULL DEFAULT 0,
                rejected_count INTEGER NOT NULL DEFAULT 0,
                auto_applied_count INTEGER NOT NULL DEFAULT 0,
                auto_apply_enabled INTEGER NOT NULL DEFAULT 0,
                suppressed_until TEXT,
                last_feedback_at TEXT,
                metadata_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, recommendation_key)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_lifecycle_user_key
                ON import_learning_lifecycle(user_id, recommendation_key);
            CREATE TABLE IF NOT EXISTS import_learning_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                recommendation_key TEXT NOT NULL,
                suppression_reason TEXT NOT NULL,
                suppressed_until TEXT,
                metadata_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, recommendation_key)
            );
            CREATE TABLE IF NOT EXISTS import_learning_concept_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                concept_key TEXT NOT NULL,
                concept_type TEXT NOT NULL,
                sample_count INTEGER NOT NULL DEFAULT 0,
                accepted_count INTEGER NOT NULL DEFAULT 0,
                rejected_count INTEGER NOT NULL DEFAULT 0,
                auto_applied_count INTEGER NOT NULL DEFAULT 0,
                rollback_count INTEGER NOT NULL DEFAULT 0,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, concept_key, concept_type)
            );
            CREATE INDEX IF NOT EXISTS idx_import_learning_concept_stats_user
                ON import_learning_concept_stats(user_id, concept_type);
            CREATE TABLE IF NOT EXISTS import_learning_suggestions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                composite_match_hash TEXT,
                match_features_json TEXT,
                suggested_type TEXT,
                suggested_category_id INTEGER,
                suggested_source_account_id INTEGER,
                suggested_destination_account_id INTEGER,
                sample_count INTEGER NOT NULL DEFAULT 1,
                source_session_ids_json TEXT,
                source_preview_ids_json TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                recommendation_key TEXT,
                signal_state TEXT NOT NULL DEFAULT 'yellow',
                existing_rule_id INTEGER,
                summary TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            );
            CREATE INDEX IF NOT EXISTS idx_learning_suggestions_user_status
                ON import_learning_suggestions(user_id, status);
            ",
        )
        .map_err(db_error_response)?;
    for (table_name, column_name, definition) in [
        ("import_learning_rules", "parser_id", "TEXT"),
        ("import_learning_rules", "composite_match_hash", "TEXT"),
        ("import_learning_rules", "match_features_json", "TEXT"),
        ("import_learning_rule_logs", "match_type", "TEXT"),
        ("import_learning_rule_logs", "match_value", "TEXT"),
        (
            "import_learning_rule_logs",
            "normalized_match_value",
            "TEXT",
        ),
        ("import_learning_rule_logs", "payload_json", "TEXT"),
        (
            "import_learning_feedback_events",
            "lifecycle_id",
            "INTEGER",
        ),
        (
            "import_learning_feedback_events",
            "recommendation_key",
            "TEXT",
        ),
        (
            "import_learning_feedback_events",
            "previous_signal_state",
            "TEXT",
        ),
        (
            "import_learning_feedback_events",
            "next_signal_state",
            "TEXT",
        ),
        (
            "import_learning_lifecycle",
            "recommendation_type",
            "TEXT NOT NULL DEFAULT 'import_preview'",
        ),
        (
            "import_learning_lifecycle",
            "accepted_count",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "import_learning_lifecycle",
            "rejected_count",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "import_learning_lifecycle",
            "auto_applied_count",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        (
            "import_learning_lifecycle",
            "auto_apply_enabled",
            "INTEGER NOT NULL DEFAULT 0",
        ),
        ("import_learning_lifecycle", "suppressed_until", "TEXT"),
        ("import_learning_lifecycle", "last_feedback_at", "TEXT"),
        ("import_learning_lifecycle", "metadata_json", "TEXT"),
        ("import_learning_suppressions", "suppressed_until", "TEXT"),
        ("import_learning_suppressions", "metadata_json", "TEXT"),
        (
            "import_learning_suggestions",
            "recommendation_key",
            "TEXT",
        ),
        ("import_learning_suggestions", "signal_state", "TEXT"),
    ] {
        ensure_table_column(runtime.connection(), table_name, column_name, definition)?;
    }
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_table_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
    definition: &str,
) -> Result<(), ImportV2RouteResponse> {
    if table_has_column(connection, table_name, column_name).map_err(db_error_response)? {
        return Ok(());
    }
    connection
        .execute(
            &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {definition}"),
            [],
        )
        .map(|_| ())
        .map_err(db_error_response)
}

fn table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> rusqlite::Result<bool> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    for row in rows {
        if row? == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}
