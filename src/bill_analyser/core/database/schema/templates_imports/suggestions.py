"""Recurring suggestion schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaRecurringSuggestionsMixin:
    async def _init_recurring_suggestion_tables(self, conn: aiosqlite.Connection) -> None:
        # ── recurring_suggestions (auto-detected recurring patterns) ──
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS recurring_suggestions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                pattern_hash TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                type TEXT NOT NULL,
                amount REAL NOT NULL,
                source_account_id INTEGER,
                destination_account_id TEXT,
                counterparty TEXT,
                frequency TEXT NOT NULL,
                detected_interval_days REAL,
                confidence_score REAL NOT NULL DEFAULT 0,
                sample_count INTEGER NOT NULL DEFAULT 0,
                sample_bill_ids_json TEXT,
                first_occurrence TEXT,
                last_occurrence TEXT,
                suggested_next_date TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(user_id, pattern_hash),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_recurring_suggestions_user_status "
            "ON recurring_suggestions(user_id, status)"
        )
