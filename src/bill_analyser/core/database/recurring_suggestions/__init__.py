"""Recurring suggestion persistence mixin."""

# pylint: disable=line-too-long

from __future__ import annotations

import json
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso


class DatabaseRecurringSuggestionsMixin(DatabaseFacadeBase):
    """Recurring auto-detection suggestion persistence."""

    @log_method
    async def detect_and_save_recurring_suggestions(
        self,
        user_id: int,
        patterns: list[Any],
    ) -> dict[str, int]:
        """Persist detected recurring patterns as suggestions.

        Returns dict with created/updated/skipped counts.
        """
        conn = await self._get_connection()
        now = utc_now_iso()
        created = 0
        updated = 0
        skipped = 0

        for pattern in patterns:
            sample_ids_json = (
                json.dumps(pattern.sample_bill_ids)
                if hasattr(pattern, "sample_bill_ids")
                else "[]"
            )

            # Check existing
            async with conn.execute(
                "SELECT id, status FROM recurring_suggestions WHERE user_id = ? AND pattern_hash = ?",
                (user_id, pattern.pattern_hash),
            ) as cursor:
                existing = await cursor.fetchone()

            if existing:
                row = dict(existing)
                if row["status"] in ("accepted", "rejected"):
                    skipped += 1
                    continue
                # Update pending suggestion with latest data
                await conn.execute(
                    """UPDATE recurring_suggestions SET
                        name = ?, description = ?, type = ?, amount = ?,
                        source_account_id = ?, destination_account_id = ?,
                        counterparty = ?, frequency = ?, detected_interval_days = ?,
                        confidence_score = ?, sample_count = ?, sample_bill_ids_json = ?,
                        first_occurrence = ?, last_occurrence = ?, suggested_next_date = ?,
                        updated_at = ?
                    WHERE id = ?""",
                    (
                        pattern.name,
                        pattern.description,
                        pattern.type,
                        pattern.amount,
                        pattern.source_account_id,
                        pattern.destination_account_id,
                        pattern.counterparty,
                        pattern.frequency,
                        pattern.detected_interval_days,
                        pattern.confidence_score,
                        pattern.sample_count,
                        sample_ids_json,
                        pattern.first_occurrence,
                        pattern.last_occurrence,
                        pattern.suggested_next_date,
                        now,
                        row["id"],
                    ),
                )
                updated += 1
            else:
                await conn.execute(
                    """INSERT INTO recurring_suggestions
                        (user_id, pattern_hash, name, description, type, amount,
                         source_account_id, destination_account_id, counterparty,
                         frequency, detected_interval_days, confidence_score,
                         sample_count, sample_bill_ids_json, first_occurrence,
                         last_occurrence, suggested_next_date, status,
                         created_at, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending', ?, ?)""",
                    (
                        user_id,
                        pattern.pattern_hash,
                        pattern.name,
                        pattern.description,
                        pattern.type,
                        pattern.amount,
                        pattern.source_account_id,
                        pattern.destination_account_id,
                        pattern.counterparty,
                        pattern.frequency,
                        pattern.detected_interval_days,
                        pattern.confidence_score,
                        pattern.sample_count,
                        sample_ids_json,
                        pattern.first_occurrence,
                        pattern.last_occurrence,
                        pattern.suggested_next_date,
                        now,
                        now,
                    ),
                )
                created += 1

        await conn.commit()
        return {"created": created, "updated": updated, "skipped": skipped}

    @log_method
    async def get_recurring_suggestions(
        self,
        user_id: int,
        status: str | None = None,
        limit: int = 200,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """List recurring suggestions with optional status filter."""
        conn = await self._get_connection()
        params: list[Any] = [user_id]
        query = "SELECT * FROM recurring_suggestions WHERE user_id = ?"
        if status:
            query += " AND status = ?"
            params.append(status)
        query += " ORDER BY confidence_score DESC, last_occurrence DESC LIMIT ? OFFSET ?"
        params.extend([limit, offset])

        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        results = []
        for row in rows:
            item = dict(row)
            if item.get("sample_bill_ids_json"):
                try:
                    item["sample_bill_ids"] = json.loads(item["sample_bill_ids_json"])
                except (json.JSONDecodeError, TypeError):
                    item["sample_bill_ids"] = []
            else:
                item["sample_bill_ids"] = []
            results.append(item)
        return results

    @log_method
    async def count_recurring_suggestions(
        self,
        user_id: int,
        status: str | None = None,
    ) -> int:
        """Count recurring suggestions."""
        conn = await self._get_connection()
        params: list[Any] = [user_id]
        query = "SELECT COUNT(*) FROM recurring_suggestions WHERE user_id = ?"
        if status:
            query += " AND status = ?"
            params.append(status)
        async with conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
        return int(row[0]) if row else 0

    @log_method
    async def accept_recurring_suggestion(
        self,
        suggestion_id: int,
        user_id: int,
    ) -> dict[str, Any] | None:
        """Accept a suggestion and create a recurring_bills rule from it.

        Returns the created recurring rule dict, or None if suggestion not
        found/already processed.
        """
        conn = await self._get_connection()
        now = utc_now_iso()

        async with conn.execute(
            "SELECT * FROM recurring_suggestions WHERE id = ? AND user_id = ?",
            (suggestion_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()

        if not row:
            return None

        suggestion = dict(row)
        if suggestion["status"] != "pending":
            return None

        # Create recurring rule
        cursor = await conn.execute(
            """INSERT INTO recurring_bills
                (user_id, name, description, type, amount, account, counterparty,
                 frequency, start_date, next_date, enabled, auto_create,
                 created_at, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, 0, ?, ?)""",
            (
                user_id,
                suggestion["name"],
                suggestion.get("description") or "",
                suggestion["type"],
                suggestion["amount"],
                str(suggestion.get("source_account_id") or ""),
                suggestion.get("counterparty") or "",
                suggestion["frequency"],
                suggestion.get("first_occurrence") or now[:10],
                suggestion.get("suggested_next_date") or now[:10],
                now,
                now,
            ),
        )
        recurring_id = cursor.lastrowid

        # Mark suggestion as accepted
        await conn.execute(
            "UPDATE recurring_suggestions SET status = 'accepted', updated_at = ? WHERE id = ?",
            (now, suggestion_id),
        )
        await conn.commit()

        return {
            "recurring_id": recurring_id,
            "suggestion_id": suggestion_id,
            "status": "accepted",
        }

    @log_method
    async def reject_recurring_suggestion(
        self,
        suggestion_id: int,
        user_id: int,
    ) -> bool:
        """Reject a recurring suggestion. Returns True if updated."""
        conn = await self._get_connection()
        now = utc_now_iso()

        async with conn.execute(
            "SELECT status FROM recurring_suggestions WHERE id = ? AND user_id = ?",
            (suggestion_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()

        if not row or dict(row)["status"] != "pending":
            return False

        await conn.execute(
            "UPDATE recurring_suggestions SET status = 'rejected', updated_at = ? WHERE id = ?",
            (now, suggestion_id),
        )
        await conn.commit()
        return True

    @log_method
    async def get_bills_linked_to_recurring(self, user_id: int) -> set[int]:
        """Get set of bill IDs that are already linked to recurring rules."""
        conn = await self._get_connection()
        # Bills linked via created_from_recurring field
        try:
            async with conn.execute(
                "SELECT id FROM bills WHERE user_id = ? AND created_from_recurring IS NOT NULL AND created_from_recurring > 0",
                (user_id,),
            ) as cursor:
                rows = await cursor.fetchall()
            return {int(row[0]) for row in rows}
        except Exception:  # pragma: no cover - graceful fallback if column missing
            return set()
