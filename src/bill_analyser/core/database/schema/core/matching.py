"""Matching, suppression, reconciliation, and merge schema fragments."""

from __future__ import annotations

# pylint: disable=line-too-long
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaCoreMatchingSchemaMixin:
    async def _init_pairing_suppression_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_pair_links (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                pair_type TEXT NOT NULL,
                left_bill_id INTEGER NOT NULL,
                right_bill_id INTEGER NOT NULL,
                source TEXT NOT NULL DEFAULT 'manual',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(left_bill_id < right_bill_id),
                UNIQUE(user_id, pair_type, left_bill_id, right_bill_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (left_bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (right_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_pair_feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL,
                candidate_id TEXT NOT NULL,
                action TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_transfer_pair_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                left_bill_id INTEGER NOT NULL,
                right_bill_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                CHECK(left_bill_id < right_bill_id),
                UNIQUE(user_id, left_bill_id, right_bill_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (left_bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (right_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_investment_pair_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                left_bill_id INTEGER NOT NULL,
                right_bill_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                CHECK(left_bill_id < right_bill_id),
                UNIQUE(user_id, left_bill_id, right_bill_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (left_bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (right_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_learning_rule_suppressions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                bill_id INTEGER NOT NULL,
                rule_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                UNIQUE(user_id, bill_id, rule_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE,
                FOREIGN KEY (rule_id) REFERENCES import_learning_rules(id) ON DELETE CASCADE
            )
            """
        )

    async def _init_reconciliation_merge_schema(self, conn: aiosqlite.Connection) -> None:
        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_reconciliation_candidates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                family TEXT NOT NULL DEFAULT 'import_reconciliation',
                candidate_id TEXT NOT NULL,
                candidate_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                session_id TEXT,
                preview_id INTEGER,
                import_bill_key TEXT NOT NULL,
                existing_bill_id INTEGER NOT NULL,
                group_key TEXT NOT NULL,
                amount_abs REAL NOT NULL DEFAULT 0,
                time_diff_seconds INTEGER,
                score REAL NOT NULL DEFAULT 0,
                level TEXT NOT NULL DEFAULT '',
                reason TEXT NOT NULL DEFAULT '',
                import_bill_snapshot_json TEXT NOT NULL DEFAULT '{}',
                existing_bill_snapshot_json TEXT NOT NULL DEFAULT '{}',
                source_payload_json TEXT NOT NULL DEFAULT '{}',
                seen_count INTEGER NOT NULL DEFAULT 1,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                resolved_at TEXT,
                resolution_event_id INTEGER,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(candidate_type IN ('transfer', 'duplicate')),
                CHECK(status IN ('pending', 'accepted', 'rejected', 'merged', 'rolled_back', 'superseded')),
                UNIQUE(user_id, candidate_id),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (existing_bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_merge_groups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                family TEXT NOT NULL DEFAULT 'import_reconciliation',
                group_key TEXT NOT NULL,
                group_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                canonical_bill_id INTEGER,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(group_type IN ('transfer', 'duplicate')),
                CHECK(status IN ('pending', 'accepted', 'merged', 'rolled_back', 'superseded')),
                UNIQUE(user_id, family, group_key),
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (canonical_bill_id) REFERENCES bills(id) ON DELETE SET NULL
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_merge_members (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_id INTEGER NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                member_key TEXT NOT NULL,
                member_type TEXT NOT NULL,
                bill_id INTEGER,
                import_bill_key TEXT,
                candidate_id TEXT,
                role TEXT NOT NULL DEFAULT 'candidate',
                snapshot_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                CHECK(member_type IN ('existing_bill', 'import_bill')),
                UNIQUE(group_id, member_key),
                FOREIGN KEY (group_id) REFERENCES bill_merge_groups(id) ON DELETE CASCADE,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (bill_id) REFERENCES bills(id) ON DELETE CASCADE
            )
            """
        )

        await conn.execute(
            """
            CREATE TABLE IF NOT EXISTS bill_merge_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                group_id INTEGER NOT NULL,
                candidate_id TEXT,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                FOREIGN KEY (group_id) REFERENCES bill_merge_groups(id) ON DELETE CASCADE
            )
            """
        )
