"""Import reconciliation candidate and merge-ledger persistence helpers."""

# pylint: disable=too-many-arguments,too-many-locals

from __future__ import annotations

import json
import sqlite3
from typing import Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseReconciliationMixin(DatabaseFacadeBase):
    """Durable candidate truth for import-to-persisted-bill reconciliation."""

    _IMPORT_RECONCILIATION_FAMILY = "import_reconciliation"
    _VALID_RECONCILIATION_TYPES = {"transfer", "duplicate"}
    _PENDING_STATUS = "pending"

    @staticmethod
    def _json_dumps(payload: Any) -> str:
        if not isinstance(payload, (dict, list)):
            payload = {}
        return json.dumps(payload, ensure_ascii=False, sort_keys=True)

    @staticmethod
    def _json_loads(raw_payload: Any) -> dict[str, Any]:
        if isinstance(raw_payload, dict):
            return dict(raw_payload)
        if raw_payload in (None, ""):
            return {}
        try:
            payload = json.loads(str(raw_payload))
        except (TypeError, ValueError, json.JSONDecodeError):
            return {}
        return dict(payload) if isinstance(payload, dict) else {}

    @staticmethod
    def _normalize_optional_int(raw_value: Any) -> int | None:
        if raw_value in (None, "", 0, "0"):
            return None
        try:
            normalized_value = int(raw_value)
        except (TypeError, ValueError):
            return None
        return normalized_value if normalized_value > 0 else None

    @classmethod
    def _normalize_candidate_type(cls, raw_value: Any) -> str:
        candidate_type = str(raw_value or "").strip().lower()
        if candidate_type not in cls._VALID_RECONCILIATION_TYPES:
            raise ValueError("Invalid reconciliation candidate type")
        return candidate_type

    @classmethod
    def _normalize_candidate_payload(cls, candidate: dict[str, Any]) -> dict[str, Any]:
        candidate_type = cls._normalize_candidate_type(candidate.get("candidate_type"))
        candidate_id = str(candidate.get("candidate_id") or "").strip()
        import_bill_key = str(candidate.get("import_bill_key") or "").strip()
        existing_bill_id = cls._normalize_optional_int(candidate.get("existing_bill_id"))
        if not candidate_id or not import_bill_key or existing_bill_id is None:
            raise ValueError("Invalid reconciliation candidate")

        amount_abs = float(candidate.get("amount_abs") or candidate.get("amount") or 0.0)
        if amount_abs < 0:
            amount_abs = abs(amount_abs)

        group_key = str(candidate.get("group_key") or "").strip()
        if not group_key:
            group_key = (
                f"{cls._IMPORT_RECONCILIATION_FAMILY}:"
                f"{candidate_type}:bill:{existing_bill_id}:amount:{amount_abs:.2f}"
            )

        score = float(candidate.get("score") or 0.0)
        if score < 0:
            score = 0.0
        if score > 1:
            score = 1.0

        return {
            "family": cls._IMPORT_RECONCILIATION_FAMILY,
            "candidate_id": candidate_id,
            "candidate_type": candidate_type,
            "session_id": str(candidate.get("session_id") or "").strip() or None,
            "preview_id": cls._normalize_optional_int(candidate.get("preview_id")),
            "import_bill_key": import_bill_key,
            "existing_bill_id": existing_bill_id,
            "group_key": group_key,
            "amount_abs": amount_abs,
            "time_diff_seconds": cls._normalize_optional_int(candidate.get("time_diff_seconds")),
            "score": score,
            "level": str(candidate.get("level") or "").strip(),
            "reason": str(candidate.get("reason") or "").strip(),
            "import_bill_snapshot": (
                dict(candidate.get("import_bill_snapshot"))
                if isinstance(candidate.get("import_bill_snapshot"), dict)
                else {}
            ),
            "existing_bill_snapshot": (
                dict(candidate.get("existing_bill_snapshot"))
                if isinstance(candidate.get("existing_bill_snapshot"), dict)
                else {}
            ),
            "source_payload": (
                dict(candidate.get("source_payload"))
                if isinstance(candidate.get("source_payload"), dict)
                else {}
            ),
        }

    @staticmethod
    def _row_to_reconciliation_candidate(row: dict[str, Any]) -> dict[str, Any]:
        return {
            "id": int(row.get("id") or 0),
            "user_id": int(row.get("user_id") or 0),
            "family": str(row.get("family") or ""),
            "candidate_id": str(row.get("candidate_id") or ""),
            "candidate_type": str(row.get("candidate_type") or ""),
            "status": str(row.get("status") or ""),
            "session_id": str(row.get("session_id") or ""),
            "preview_id": DatabaseReconciliationMixin._normalize_optional_int(
                row.get("preview_id")
            ),
            "import_bill_key": str(row.get("import_bill_key") or ""),
            "existing_bill_id": int(row.get("existing_bill_id") or 0),
            "group_key": str(row.get("group_key") or ""),
            "amount_abs": float(row.get("amount_abs") or 0.0),
            "time_diff_seconds": DatabaseReconciliationMixin._normalize_optional_int(
                row.get("time_diff_seconds")
            ),
            "score": float(row.get("score") or 0.0),
            "level": str(row.get("level") or ""),
            "reason": str(row.get("reason") or ""),
            "import_bill_snapshot": DatabaseReconciliationMixin._json_loads(
                row.get("import_bill_snapshot_json")
            ),
            "existing_bill_snapshot": DatabaseReconciliationMixin._json_loads(
                row.get("existing_bill_snapshot_json")
            ),
            "source_payload": DatabaseReconciliationMixin._json_loads(
                row.get("source_payload_json")
            ),
            "seen_count": int(row.get("seen_count") or 0),
            "first_seen_at": str(row.get("first_seen_at") or ""),
            "last_seen_at": str(row.get("last_seen_at") or ""),
            "resolved_at": str(row.get("resolved_at") or ""),
            "resolution_event_id": DatabaseReconciliationMixin._normalize_optional_int(
                row.get("resolution_event_id")
            ),
            "created_at": str(row.get("created_at") or ""),
            "updated_at": str(row.get("updated_at") or ""),
        }

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
            SELECT id
            FROM bill_merge_groups
            WHERE user_id = ? AND family = ? AND group_key = ?
            LIMIT 1
            """,
            (user_id, candidate["family"], candidate["group_key"]),
        ) as cursor:
            existing_group = await cursor.fetchone()

        metadata_json = self._json_dumps(
            {
                "candidate_type": candidate["candidate_type"],
                "amount_abs": candidate["amount_abs"],
                "session_id": candidate["session_id"],
            }
        )
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
        cursor = await conn.execute(
            """
            UPDATE bill_merge_members
            SET candidate_id = ?, snapshot_json = ?, role = ?, updated_at = ?
            WHERE group_id = ? AND member_key = ? AND user_id = ?
            """,
            (
                candidate_id,
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
                bill_id,
                import_bill_key,
                candidate_id,
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
            "SELECT * FROM bill_reconciliation_candidates "
            f"WHERE {' AND '.join(conditions)} "
            "ORDER BY COALESCE(last_seen_at, created_at) DESC, id DESC "
            "LIMIT ?"
        )
        params.append(normalized_limit)

        conn = await self._get_connection()
        async with conn.execute(query, params) as cursor:
            rows = await cursor.fetchall()

        return [self._row_to_reconciliation_candidate(dict(row)) for row in rows]

    @log_method
    async def record_bill_merge_event(
        self,
        *,
        group_id: int,
        candidate_id: str,
        event_type: str,
        payload: dict[str, Any] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Append a merge-ledger event without mutating bill data."""
        normalized_group_id = int(group_id)
        normalized_user_id = int(user_id)
        normalized_candidate_id = str(candidate_id or "").strip()
        normalized_event_type = str(event_type or "").strip().lower()
        if normalized_group_id <= 0 or normalized_user_id <= 0 or not normalized_event_type:
            raise ValueError("Invalid merge event")

        now = utc_now_iso()
        conn = await self._get_connection()
        event_id = await self._append_bill_merge_event(
            conn,
            user_id=normalized_user_id,
            group_id=normalized_group_id,
            candidate_id=normalized_candidate_id,
            event_type=normalized_event_type,
            payload=payload or {},
            now=now,
        )
        await conn.commit()
        return {
            "id": event_id,
            "group_id": normalized_group_id,
            "candidate_id": normalized_candidate_id,
            "event_type": normalized_event_type,
            "payload": payload or {},
            "created_at": now,
        }
