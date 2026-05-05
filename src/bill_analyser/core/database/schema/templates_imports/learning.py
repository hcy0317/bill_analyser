"""Import learning schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaImportLearningTablesMixin:
    async def _init_import_learning_sample_tables(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_annotation_samples (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                preview_id INTEGER NOT NULL,
                annotated_type TEXT,
                annotated_category_id INTEGER,
                annotated_source_account_id INTEGER,
                annotated_destination_account_id INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(session_id, preview_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_annotation_samples_session ON import_annotation_samples(session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_annotation_samples_user ON import_annotation_samples(user_id)"
        )

        await conn.execute(
            """
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
                UNIQUE(user_id, session_id, preview_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_user "
            "ON import_learning_corpus_samples(user_id, updated_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_corpus_hash "
            "ON import_learning_corpus_samples(user_id, composite_match_hash)"
        )

    async def _init_import_learning_rule_tables(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
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
                match_features_json TEXT,
                applied_count INTEGER NOT NULL DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rules_user_enabled "
            "ON import_learning_rules(user_id, enabled)"
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_learning_rule_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rule_id INTEGER,
                user_id INTEGER NOT NULL DEFAULT 1,
                action TEXT NOT NULL,
                match_type TEXT,
                match_value TEXT,
                normalized_match_value TEXT,
                session_id TEXT,
                preview_id INTEGER,
                payload_json TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (rule_id) REFERENCES import_learning_rules(id) ON DELETE SET NULL
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rule_logs_user "
            "ON import_learning_rule_logs(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_rule_logs_rule ON import_learning_rule_logs(rule_id)"
        )

    async def _init_import_learning_model_tables(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_learning_feedback_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                event_type TEXT NOT NULL,
                rule_id INTEGER,
                suggestion_id INTEGER,
                session_id TEXT,
                preview_id INTEGER,
                bill_id INTEGER,
                candidate_id TEXT,
                payload_json TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (rule_id) REFERENCES import_learning_rules(id) ON DELETE SET NULL
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_user "
            "ON import_learning_feedback_events(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_feedback_events_type "
            "ON import_learning_feedback_events(user_id, event_type)"
        )
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_learning_dataset_snapshots (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                corpus_sample_count INTEGER NOT NULL DEFAULT 0,
                filters_json TEXT,
                status TEXT NOT NULL DEFAULT 'draft',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_dataset_snapshots_user "
            "ON import_learning_dataset_snapshots(user_id, created_at DESC)"
        )
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS import_learning_model_registry (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                model_key TEXT NOT NULL,
                model_version TEXT NOT NULL,
                dataset_snapshot_id INTEGER,
                status TEXT NOT NULL DEFAULT 'draft',
                metrics_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, model_key, model_version),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (dataset_snapshot_id) REFERENCES import_learning_dataset_snapshots(id) ON DELETE SET NULL
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_model_registry_user "
            "ON import_learning_model_registry(user_id, status)"
        )

    async def _init_import_learning_suggestion_tables(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
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
                UNIQUE(user_id, concept_key, concept_type),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_import_learning_concept_stats_user "
            "ON import_learning_concept_stats(user_id, concept_type)"
        )

        await conn.execute(
            """
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
                existing_rule_id INTEGER,
                summary TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_learning_suggestions_user_status "
            "ON import_learning_suggestions(user_id, status)"
        )
