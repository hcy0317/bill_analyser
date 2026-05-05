"""Import reconciliation candidate and merge-ledger persistence helpers."""

# pylint: disable=too-many-arguments,too-many-locals

from __future__ import annotations

import json
import sqlite3
from difflib import SequenceMatcher
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso


class ReconciliationPersistenceMixin(object):
        async def _ensure_bill_merge_group(
            self,
            conn: Any,
            candidate: dict[str, Any],
            *,
            user_id: int,
            now: str,
        ) -> int:
            async with conn.execute(
                """
                SELECT id, metadata_json
                FROM bill_merge_groups
                WHERE user_id = ? AND family = ? AND group_key = ?
                LIMIT 1
                """,
                (user_id, candidate["family"], candidate["group_key"]),
            ) as cursor:
                existing_group = await cursor.fetchone()

            metadata = (
                self._json_loads(existing_group["metadata_json"])
                if existing_group
                else {}
            )
            metadata.update(
                {
                    "candidate_type": candidate["candidate_type"],
                    "amount_abs": candidate["amount_abs"],
                    "session_id": candidate["session_id"],
                }
            )
            metadata_json = self._json_dumps(metadata)
            if existing_group:
                group_id = int(existing_group["id"])
                await conn.execute(
                    """
                    UPDATE bill_merge_groups
                    SET group_type = ?, metadata_json = ?, updated_at = ?
                    WHERE id = ? AND user_id = ?
                    """,
                    (candidate["candidate_type"], metadata_json, now, group_id, user_id),
                )
                return group_id

            cursor = await conn.execute(
                """
                INSERT INTO bill_merge_groups (
                    user_id, family, group_key, group_type, status, canonical_bill_id,
                    metadata_json, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?)
                """,
                (
                    user_id,
                    candidate["family"],
                    candidate["group_key"],
                    candidate["candidate_type"],
                    self._PENDING_STATUS,
                    metadata_json,
                    now,
                    now,
                ),
            )
            return int(cursor.lastrowid or 0)

        async def _ensure_bill_merge_member(
            self,
            conn: Any,
            *,
            group_id: int,
            user_id: int,
            member_key: str,
            member_type: str,
            role: str,
            snapshot: dict[str, Any],
            bill_id: int | None = None,
            import_bill_key: str | None = None,
            candidate_id: str | None = None,
            now: str,
        ) -> None:
            snapshot_json = self._json_dumps(snapshot)
            normalized_bill_id = self._normalize_optional_int(bill_id)
            normalized_import_bill_key = str(import_bill_key or "").strip() or None
            normalized_candidate_id = str(candidate_id or "").strip() or None
            cursor = await conn.execute(
                """
                UPDATE bill_merge_members
                SET candidate_id = COALESCE(?, candidate_id),
                    bill_id = COALESCE(?, bill_id),
                    import_bill_key = COALESCE(?, import_bill_key),
                    snapshot_json = ?,
                    role = ?,
                    updated_at = ?
                WHERE group_id = ? AND member_key = ? AND user_id = ?
                """,
                (
                    normalized_candidate_id,
                    normalized_bill_id,
                    normalized_import_bill_key,
                    snapshot_json,
                    role,
                    now,
                    group_id,
                    member_key,
                    user_id,
                ),
            )
            if int(cursor.rowcount or 0) > 0:
                return

            await conn.execute(
                """
                INSERT INTO bill_merge_members (
                    group_id, user_id, member_key, member_type, bill_id, import_bill_key,
                    candidate_id, role, snapshot_json, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    group_id,
                    user_id,
                    member_key,
                    member_type,
                    normalized_bill_id,
                    normalized_import_bill_key,
                    normalized_candidate_id,
                    role,
                    snapshot_json,
                    now,
                    now,
                ),
            )

        async def _append_bill_merge_event(
            self,
            conn: Any,
            *,
            user_id: int,
            group_id: int,
            candidate_id: str,
            event_type: str,
            payload: dict[str, Any],
            now: str,
        ) -> int:
            cursor = await conn.execute(
                """
                INSERT INTO bill_merge_events (
                    user_id, group_id, candidate_id, event_type, payload_json, created_at
                ) VALUES (?, ?, ?, ?, ?, ?)
                """,
                (user_id, group_id, candidate_id, event_type, self._json_dumps(payload), now),
            )
            return int(cursor.lastrowid or 0)

        @log_method
        async def persist_import_reconciliation_candidates(
            self,
            candidates: list[dict[str, Any]],
            *,
            user_id: int = 1,
        ) -> list[dict[str, Any]]:
            """Upsert import reconciliation candidates and append merge-ledger events."""
            normalized_user_id = int(user_id)
            if normalized_user_id <= 0:
                raise ValueError("Invalid userId")
            normalized_candidates = [
                self._normalize_candidate_payload(candidate)
                for candidate in candidates
                if isinstance(candidate, dict)
            ]
            if not normalized_candidates:
                return []

            conn = await self._get_connection()
            persisted: list[dict[str, Any]] = []
            try:
                await conn.execute("BEGIN IMMEDIATE")
                for candidate in normalized_candidates:
                    now = utc_now_iso()
                    async with conn.execute(
                        """
                        SELECT id, status, seen_count, first_seen_at
                        FROM bill_reconciliation_candidates
                        WHERE user_id = ? AND candidate_id = ?
                        LIMIT 1
                        """,
                        (normalized_user_id, candidate["candidate_id"]),
                    ) as cursor:
                        existing_row = await cursor.fetchone()

                    snapshot_values = (
                        candidate["family"],
                        candidate["candidate_type"],
                        candidate["session_id"],
                        candidate["preview_id"],
                        candidate["import_bill_key"],
                        candidate["existing_bill_id"],
                        candidate["group_key"],
                        candidate["amount_abs"],
                        candidate["time_diff_seconds"],
                        candidate["score"],
                        candidate["level"],
                        candidate["reason"],
                        self._json_dumps(candidate["import_bill_snapshot"]),
                        self._json_dumps(candidate["existing_bill_snapshot"]),
                        self._json_dumps(candidate["source_payload"]),
                    )
                    if existing_row:
                        candidate_row_id = int(existing_row["id"])
                        await conn.execute(
                            """
                            UPDATE bill_reconciliation_candidates
                            SET family = ?, candidate_type = ?, session_id = ?, preview_id = ?,
                                import_bill_key = ?, existing_bill_id = ?, group_key = ?,
                                amount_abs = ?, time_diff_seconds = ?, score = ?, level = ?,
                                reason = ?, import_bill_snapshot_json = ?,
                                existing_bill_snapshot_json = ?, source_payload_json = ?,
                                seen_count = seen_count + 1, last_seen_at = ?, updated_at = ?
                            WHERE id = ? AND user_id = ?
                            """,
                            (
                                *snapshot_values,
                                now,
                                now,
                                candidate_row_id,
                                normalized_user_id,
                            ),
                        )
                        event_type = "candidate_seen"
                    else:
                        cursor = await conn.execute(
                            """
                            INSERT INTO bill_reconciliation_candidates (
                                user_id, family, candidate_id, candidate_type, status,
                                session_id, preview_id, import_bill_key, existing_bill_id,
                                group_key, amount_abs, time_diff_seconds, score, level,
                                reason, import_bill_snapshot_json, existing_bill_snapshot_json,
                                source_payload_json, seen_count, first_seen_at, last_seen_at,
                                created_at, updated_at
                            ) VALUES (
                                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                                1, ?, ?, ?, ?
                            )
                            """,
                            (
                                normalized_user_id,
                                candidate["family"],
                                candidate["candidate_id"],
                                candidate["candidate_type"],
                                self._PENDING_STATUS,
                                candidate["session_id"],
                                candidate["preview_id"],
                                candidate["import_bill_key"],
                                candidate["existing_bill_id"],
                                candidate["group_key"],
                                candidate["amount_abs"],
                                candidate["time_diff_seconds"],
                                candidate["score"],
                                candidate["level"],
                                candidate["reason"],
                                self._json_dumps(candidate["import_bill_snapshot"]),
                                self._json_dumps(candidate["existing_bill_snapshot"]),
                                self._json_dumps(candidate["source_payload"]),
                                now,
                                now,
                                now,
                                now,
                            ),
                        )
                        candidate_row_id = int(cursor.lastrowid or 0)
                        event_type = "candidate_discovered"

                    group_id = await self._ensure_bill_merge_group(
                        conn,
                        candidate,
                        user_id=normalized_user_id,
                        now=now,
                    )
                    await self._ensure_bill_merge_member(
                        conn,
                        group_id=group_id,
                        user_id=normalized_user_id,
                        member_key=f"bill:{candidate['existing_bill_id']}",
                        member_type="existing_bill",
                        bill_id=candidate["existing_bill_id"],
                        role="anchor",
                        snapshot=candidate["existing_bill_snapshot"],
                        now=now,
                    )
                    await self._ensure_bill_merge_member(
                        conn,
                        group_id=group_id,
                        user_id=normalized_user_id,
                        member_key=f"import:{candidate['import_bill_key']}",
                        member_type="import_bill",
                        import_bill_key=candidate["import_bill_key"],
                        candidate_id=candidate["candidate_id"],
                        role="candidate",
                        snapshot=candidate["import_bill_snapshot"],
                        now=now,
                    )
                    event_id = await self._append_bill_merge_event(
                        conn,
                        user_id=normalized_user_id,
                        group_id=group_id,
                        candidate_id=candidate["candidate_id"],
                        event_type=event_type,
                        payload={
                            "candidate_row_id": candidate_row_id,
                            "candidate_type": candidate["candidate_type"],
                            "existing_bill_id": candidate["existing_bill_id"],
                            "import_bill_key": candidate["import_bill_key"],
                            "status": self._PENDING_STATUS,
                        },
                        now=now,
                    )
                    persisted.append(
                        {
                            **candidate,
                            "id": candidate_row_id,
                            "user_id": normalized_user_id,
                            "status": self._PENDING_STATUS,
                            "group_id": group_id,
                            "event_id": event_id,
                        }
                    )

                await conn.commit()
                return persisted
            except sqlite3.IntegrityError:
                await conn.rollback()
                raise
            except Exception:
                await conn.rollback()
                raise

        @log_method
        async def list_import_reconciliation_candidates(
            self,
            *,
            user_id: int = 1,
            session_id: str | None = None,
            preview_id: int | None = None,
            existing_bill_id: int | None = None,
            candidate_type: str | None = None,
            status: str | None = None,
            limit: int = 200,
        ) -> list[dict[str, Any]]:
            """List persisted import reconciliation candidates for read-only review."""
            normalized_user_id = int(user_id)
            if normalized_user_id <= 0:
                return []

            conditions = ["user_id = ?", "family = ?"]
            params: list[Any] = [normalized_user_id, self._IMPORT_RECONCILIATION_FAMILY]

            if session_id not in (None, ""):
                conditions.append("session_id = ?")
                params.append(str(session_id))
            if preview_id not in (None, "", 0, "0"):
                conditions.append("preview_id = ?")
                params.append(int(preview_id))
            if existing_bill_id not in (None, "", 0, "0"):
                conditions.append("existing_bill_id = ?")
                params.append(int(existing_bill_id))
            if candidate_type not in (None, ""):
                conditions.append("candidate_type = ?")
                params.append(self._normalize_candidate_type(candidate_type))
            if status not in (None, ""):
                conditions.append("status = ?")
                params.append(str(status).strip().lower())

            normalized_limit = max(1, min(int(limit or 200), 500))
            query = (
                "SELECT c.*, g.id AS group_id, g.status AS group_status, "
                "g.canonical_bill_id AS canonical_bill_id, "
                "g.metadata_json AS group_metadata_json "
                "FROM bill_reconciliation_candidates c "
                "LEFT JOIN bill_merge_groups g "
                "ON g.user_id = c.user_id AND g.family = c.family "
                "AND g.group_key = c.group_key "
                f"WHERE {' AND '.join('c.' + condition for condition in conditions)} "
                "ORDER BY COALESCE(c.last_seen_at, c.created_at) DESC, c.id DESC "
                "LIMIT ?"
            )
            params.append(normalized_limit)

            conn = await self._get_connection()
            async with conn.execute(query, params) as cursor:
                rows = await cursor.fetchall()

            return [self._row_to_reconciliation_candidate(dict(row)) for row in rows]
