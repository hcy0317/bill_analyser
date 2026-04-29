"""Import reconciliation candidate and merge-ledger persistence helpers."""

# pylint: disable=too-many-arguments,too-many-locals

from __future__ import annotations

import json
import sqlite3
from difflib import SequenceMatcher
from typing import Any

from ..utils.logger import log_method
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso


class DatabaseReconciliationMixin(DatabaseFacadeBase):
    """Durable candidate truth for import-to-persisted-bill reconciliation."""

    _IMPORT_RECONCILIATION_FAMILY = "import_reconciliation"
    _VALID_RECONCILIATION_TYPES = {"transfer", "duplicate"}
    _PENDING_STATUS = "pending"
    _APPLIED_STATUSES = {"accepted", "merged"}
    _PARSER_DISPLAY_LABELS = {
        "wechat": "微信",
        "alipay": "支付宝",
        "abc": "农业银行",
        "ccb": "建设银行",
        "cmbc": "民生银行",
        "icbc": "工商银行",
        "generic": "通用来源",
    }

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
    def _parser_display_label(cls, parser_id: Any) -> str:
        normalized_parser_id = str(parser_id or "").strip().lower()
        if not normalized_parser_id:
            return ""
        return cls._PARSER_DISPLAY_LABELS.get(normalized_parser_id, normalized_parser_id)

    @staticmethod
    def _dedupe_text_items(items: list[str]) -> list[str]:
        deduped: list[str] = []
        for item in items:
            normalized_item = str(item or "").strip()
            if normalized_item and normalized_item not in deduped:
                deduped.append(normalized_item)
        return deduped

    @staticmethod
    def _split_description_segments(description: Any) -> list[str]:
        return [
            segment.strip()
            for segment in str(description or "").split("|")
            if segment.strip()
        ]

    @classmethod
    def _merge_description_values(cls, descriptions: list[Any]) -> str:
        segments: list[str] = []
        for description in descriptions:
            for segment in cls._split_description_segments(description):
                if any(
                    segment == existing_segment
                    or SequenceMatcher(None, segment, existing_segment).ratio() >= 0.92
                    for existing_segment in segments
                ):
                    continue
                segments.append(segment)
        return "|".join(segments)

    @classmethod
    def _normalize_tag_ids(cls, raw_value: Any) -> list[int]:
        if raw_value in (None, ""):
            return []

        raw_items: list[Any]
        if isinstance(raw_value, str):
            raw_items = [item.strip() for item in raw_value.split(",") if item.strip()]
        elif isinstance(raw_value, list):
            raw_items = list(raw_value)
        elif isinstance(raw_value, (tuple, set)):
            raw_items = list(raw_value)
        else:
            raw_items = [raw_value]

        tag_ids: list[int] = []
        for item in raw_items:
            raw_id = item.get("id") if isinstance(item, dict) else item
            normalized_id = cls._normalize_optional_int(raw_id)
            if normalized_id is not None and normalized_id not in tag_ids:
                tag_ids.append(normalized_id)
        return tag_ids

    @classmethod
    def _snapshot_tag_ids(cls, snapshot: dict[str, Any]) -> list[int]:
        tag_ids = cls._normalize_tag_ids(snapshot.get("tag_ids"))
        if tag_ids:
            return tag_ids
        return cls._normalize_tag_ids(snapshot.get("tags"))

    @classmethod
    def _source_label_from_import_snapshot(cls, snapshot: dict[str, Any]) -> str:
        for field_name in ("parser_id", "source", "payment_method"):
            raw_value = snapshot.get(field_name)
            if raw_value in (None, "", 0, "0"):
                continue
            label = cls._parser_display_label(raw_value)
            if label:
                return label
        return "导入"

    @staticmethod
    def _infer_bill_flow_role(snapshot: dict[str, Any]) -> str:
        bill_type = str(snapshot.get("type") or "").strip().lower()
        amount = float(snapshot.get("amount") or 0.0)
        if bill_type in {"支出", "expense"} or amount < 0:
            return "outgoing"
        if bill_type in {"收入", "income"} or amount > 0:
            return "incoming"
        return "primary"

    @classmethod
    def _build_reconciliation_projection_signal(
        cls,
        *,
        candidate_type: str,
        base_bill: dict[str, Any],
        import_snapshots: list[dict[str, Any]],
    ) -> dict[str, Any]:
        base_source = {
            "role": cls._infer_bill_flow_role(base_bill),
            "label": "人工",
            "source": "manual",
        }
        import_sources = [
            {
                "role": cls._infer_bill_flow_role(snapshot),
                "label": cls._source_label_from_import_snapshot(snapshot),
                "parser_id": str(snapshot.get("parser_id") or ""),
                "source": "parser",
            }
            for snapshot in import_snapshots
        ]

        if candidate_type == "transfer":
            sources_by_role: dict[str, list[str]] = {"outgoing": [], "incoming": []}
            for source in import_sources:
                role = str(source.get("role") or "primary")
                label = str(source.get("label") or "")
                if role in sources_by_role and label:
                    sources_by_role[role].append(label)
            base_role = str(base_source.get("role") or "primary")
            if base_role in sources_by_role:
                sources_by_role[base_role].append("人工")
            else:
                sources_by_role.setdefault(base_role, []).append("人工")

            side_labels = [
                "&".join(cls._dedupe_text_items(sources_by_role.get(role, [])))
                for role in ("outgoing", "incoming")
                if sources_by_role.get(role)
            ]
            if not side_labels:
                side_labels = ["人工"]
            signal_label = f"匹配：{'|'.join(side_labels)}"
        else:
            signal_label = "|".join(
                cls._dedupe_text_items(
                    ["人工", *[str(source.get("label") or "") for source in import_sources]]
                )
            )

        source_chain = [
            base_source,
            *import_sources,
        ]
        return {
            "signal_label": signal_label,
            "source_chain": source_chain,
        }

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
        group_metadata = DatabaseReconciliationMixin._json_loads(row.get("group_metadata_json"))
        projection = (
            dict(group_metadata.get("projection"))
            if isinstance(group_metadata.get("projection"), dict)
            else {}
        )
        fallback_signal = DatabaseReconciliationMixin._build_reconciliation_projection_signal(
            candidate_type=str(row.get("candidate_type") or ""),
            base_bill=DatabaseReconciliationMixin._json_loads(
                row.get("existing_bill_snapshot_json")
            ),
            import_snapshots=[
                DatabaseReconciliationMixin._json_loads(
                    row.get("import_bill_snapshot_json")
                )
            ],
        )
        signal_payload = projection or fallback_signal
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
            "group_id": DatabaseReconciliationMixin._normalize_optional_int(
                row.get("group_id")
            ),
            "group_status": str(row.get("group_status") or ""),
            "canonical_bill_id": DatabaseReconciliationMixin._normalize_optional_int(
                row.get("canonical_bill_id")
            ),
            "group_metadata": group_metadata,
            "signal_label": str(signal_payload.get("signal_label") or ""),
            "source_chain": (
                list(signal_payload.get("source_chain") or [])
                if isinstance(signal_payload.get("source_chain"), list)
                else []
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

    async def _get_reconciliation_candidate_with_group(
        self,
        conn: Any,
        *,
        candidate_id: str,
        user_id: int,
    ) -> dict[str, Any]:
        async with conn.execute(
            """
            SELECT c.*, g.id AS group_id, g.status AS group_status,
                   g.canonical_bill_id AS canonical_bill_id,
                   g.metadata_json AS group_metadata_json
            FROM bill_reconciliation_candidates c
            JOIN bill_merge_groups g
              ON g.user_id = c.user_id
             AND g.family = c.family
             AND g.group_key = c.group_key
            WHERE c.user_id = ? AND c.candidate_id = ?
            LIMIT 1
            """,
            (user_id, candidate_id),
        ) as cursor:
            row = await cursor.fetchone()
        if row is None:
            raise LookupError("Reconciliation candidate not found")
        return self._row_to_reconciliation_candidate(dict(row))

    async def _get_bill_projection_snapshot(
        self,
        conn: Any,
        *,
        bill_id: int,
        user_id: int,
    ) -> dict[str, Any]:
        async with conn.execute(
            "SELECT * FROM bills WHERE id = ? AND user_id = ?",
            (bill_id, user_id),
        ) as cursor:
            bill_row = await cursor.fetchone()
        if bill_row is None:
            raise LookupError("Bill not found")

        async with conn.execute(
            """
            SELECT bt.tag_id
            FROM bill_tags bt
            JOIN tags t ON t.id = bt.tag_id
            WHERE bt.bill_id = ? AND t.user_id = ?
            ORDER BY bt.tag_id
            """,
            (bill_id, user_id),
        ) as cursor:
            tag_rows = await cursor.fetchall()

        snapshot = dict(bill_row)
        snapshot["tag_ids"] = [int(row["tag_id"]) for row in tag_rows]
        return snapshot

    async def _filter_existing_tag_ids(
        self,
        conn: Any,
        tag_ids: list[int],
        *,
        user_id: int,
    ) -> list[int]:
        normalized_tag_ids = self._normalize_tag_ids(tag_ids)
        if not normalized_tag_ids:
            return []
        placeholders = ",".join("?" for _ in normalized_tag_ids)
        async with conn.execute(
            f"SELECT id FROM tags WHERE user_id = ? AND id IN ({placeholders})",
            (user_id, *normalized_tag_ids),
        ) as cursor:
            rows = await cursor.fetchall()
        existing_ids = {int(row["id"]) for row in rows}
        return [tag_id for tag_id in normalized_tag_ids if tag_id in existing_ids]

    async def _replace_bill_projection_tags(
        self,
        conn: Any,
        *,
        bill_id: int,
        tag_ids: list[int],
        user_id: int,
        now: str,
    ) -> list[int]:
        filtered_tag_ids = await self._filter_existing_tag_ids(
            conn,
            tag_ids,
            user_id=user_id,
        )
        await conn.execute("DELETE FROM bill_tags WHERE bill_id = ?", (bill_id,))
        if filtered_tag_ids:
            await conn.executemany(
                "INSERT OR IGNORE INTO bill_tags (bill_id, tag_id, created_at) VALUES (?, ?, ?)",
                [(bill_id, tag_id, now) for tag_id in filtered_tag_ids],
            )
        return filtered_tag_ids

    async def _find_reconciliation_preview_id(
        self,
        conn: Any,
        candidate: dict[str, Any],
        *,
        user_id: int,
    ) -> int | None:
        preview_id = self._normalize_optional_int(candidate.get("preview_id"))
        if preview_id is not None:
            return preview_id

        session_id = str(candidate.get("session_id") or "").strip()
        import_bill_key = str(candidate.get("import_bill_key") or "").strip()
        template_prefix = f"session:{session_id}:template:"
        if not session_id or not import_bill_key.startswith(template_prefix):
            return None
        template_id = self._normalize_optional_int(import_bill_key.removeprefix(template_prefix))
        if template_id is None:
            return None

        async with conn.execute(
            """
            SELECT id, dedup_source_ids
            FROM bills_preview
            WHERE session_id = ? AND user_id = ?
            ORDER BY id
            """,
            (session_id, user_id),
        ) as cursor:
            preview_rows = await cursor.fetchall()
        for row in preview_rows:
            source_ids = self._normalize_tag_ids(row["dedup_source_ids"])
            if template_id in source_ids:
                return int(row["id"])
        return None

    async def _set_reconciliation_preview_selected(
        self,
        conn: Any,
        *,
        preview_id: int | None,
        selected: bool,
        user_id: int,
    ) -> None:
        if preview_id is None:
            return
        await conn.execute(
            "UPDATE bills_preview SET preview_selected = ? WHERE id = ? AND user_id = ?",
            (1 if selected else 0, preview_id, user_id),
        )

    async def _load_group_candidates(
        self,
        conn: Any,
        *,
        group_key: str,
        user_id: int,
    ) -> list[dict[str, Any]]:
        async with conn.execute(
            """
            SELECT c.*, g.id AS group_id, g.status AS group_status,
                   g.canonical_bill_id AS canonical_bill_id,
                   g.metadata_json AS group_metadata_json
            FROM bill_reconciliation_candidates c
            JOIN bill_merge_groups g
              ON g.user_id = c.user_id
             AND g.family = c.family
             AND g.group_key = c.group_key
            WHERE c.user_id = ? AND c.group_key = ?
            ORDER BY c.id ASC
            """,
            (user_id, group_key),
        ) as cursor:
            rows = await cursor.fetchall()
        return [self._row_to_reconciliation_candidate(dict(row)) for row in rows]

    async def _recompute_reconciliation_projection(
        self,
        conn: Any,
        *,
        group_id: int,
        group_key: str,
        group_type: str,
        base_bill: dict[str, Any],
        user_id: int,
        now: str,
        metadata: dict[str, Any],
    ) -> dict[str, Any]:
        bill_id = int(base_bill.get("id") or 0)
        if bill_id <= 0:
            raise LookupError("Bill not found")

        group_candidates = await self._load_group_candidates(
            conn,
            group_key=group_key,
            user_id=user_id,
        )
        applied_candidates = [
            candidate
            for candidate in group_candidates
            if str(candidate.get("status") or "") in self._APPLIED_STATUSES
        ]
        import_snapshots = [
            dict(candidate.get("import_bill_snapshot") or {})
            for candidate in applied_candidates
        ]

        merged_description = self._merge_description_values(
            [
                base_bill.get("description", ""),
                *[snapshot.get("description", "") for snapshot in import_snapshots],
            ]
        )
        merged_tag_ids = self._normalize_tag_ids(base_bill.get("tag_ids"))
        for snapshot in import_snapshots:
            for tag_id in self._snapshot_tag_ids(snapshot):
                if tag_id not in merged_tag_ids:
                    merged_tag_ids.append(tag_id)

        await conn.execute(
            "UPDATE bills SET description = ?, updated_at = ? WHERE id = ? AND user_id = ?",
            (merged_description, now, bill_id, user_id),
        )
        merged_tag_ids = await self._replace_bill_projection_tags(
            conn,
            bill_id=bill_id,
            tag_ids=merged_tag_ids,
            user_id=user_id,
            now=now,
        )

        signal_payload = self._build_reconciliation_projection_signal(
            candidate_type=group_type,
            base_bill=base_bill,
            import_snapshots=import_snapshots,
        )
        projection = {
            **signal_payload,
            "bill_id": bill_id,
            "description": merged_description,
            "tag_ids": merged_tag_ids,
            "candidate_ids": [
                str(candidate.get("candidate_id") or "")
                for candidate in applied_candidates
                if str(candidate.get("candidate_id") or "")
            ],
        }
        next_metadata = {
            **metadata,
            "base_bill_snapshot": base_bill,
            "projection": projection,
        }
        next_status = "merged" if applied_candidates else self._PENDING_STATUS
        await conn.execute(
            """
            UPDATE bill_merge_groups
            SET status = ?, canonical_bill_id = ?, metadata_json = ?, updated_at = ?
            WHERE id = ? AND user_id = ?
            """,
            (
                next_status,
                bill_id if applied_candidates else None,
                self._json_dumps(next_metadata),
                now,
                group_id,
                user_id,
            ),
        )
        return projection

    async def _prepare_reconciliation_base_snapshot(
        self,
        conn: Any,
        candidate: dict[str, Any],
        *,
        user_id: int,
    ) -> tuple[dict[str, Any], dict[str, Any]]:
        metadata = dict(candidate.get("group_metadata") or {})
        base_bill = (
            dict(metadata.get("base_bill_snapshot"))
            if isinstance(metadata.get("base_bill_snapshot"), dict)
            else {}
        )
        if base_bill:
            return base_bill, metadata

        bill_id = int(candidate.get("existing_bill_id") or 0)
        base_bill = await self._get_bill_projection_snapshot(
            conn,
            bill_id=bill_id,
            user_id=user_id,
        )
        metadata["base_bill_snapshot"] = base_bill
        return base_bill, metadata

    async def _set_reconciliation_candidate_status(
        self,
        conn: Any,
        *,
        row_id: int,
        user_id: int,
        status: str,
        now: str,
        event_id: int | None = None,
    ) -> None:
        resolved_at = now if status in {"merged", "accepted", "rejected"} else None
        await conn.execute(
            """
            UPDATE bill_reconciliation_candidates
            SET status = ?, resolved_at = ?, resolution_event_id = ?,
                updated_at = ?
            WHERE id = ? AND user_id = ?
            """,
            (status, resolved_at, event_id, now, row_id, user_id),
        )

    @log_method
    async def accept_import_reconciliation_candidate(
        self,
        candidate_id: str,
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Apply a manual import-to-existing reconciliation candidate."""
        normalized_user_id = int(user_id)
        normalized_candidate_id = str(candidate_id or "").strip()
        if normalized_user_id <= 0 or not normalized_candidate_id:
            raise ValueError("Invalid reconciliation candidate")

        conn = await self._get_connection()
        try:
            await conn.execute("BEGIN IMMEDIATE")
            candidate = await self._get_reconciliation_candidate_with_group(
                conn,
                candidate_id=normalized_candidate_id,
                user_id=normalized_user_id,
            )
            group_id = int(candidate.get("group_id") or 0)
            base_bill, metadata = await self._prepare_reconciliation_base_snapshot(
                conn,
                candidate,
                user_id=normalized_user_id,
            )
            now = utc_now_iso()
            preview_id = await self._find_reconciliation_preview_id(
                conn,
                candidate,
                user_id=normalized_user_id,
            )
            preview_selection_before = None
            if preview_id is not None:
                async with conn.execute(
                    "SELECT preview_selected FROM bills_preview WHERE id = ? AND user_id = ?",
                    (preview_id, normalized_user_id),
                ) as cursor:
                    preview_row = await cursor.fetchone()
                preview_selection_before = bool(preview_row["preview_selected"]) if preview_row else None

            await self._set_reconciliation_candidate_status(
                conn,
                row_id=int(candidate["id"]),
                user_id=normalized_user_id,
                status="merged",
                now=now,
            )
            projection = await self._recompute_reconciliation_projection(
                conn,
                group_id=group_id,
                group_key=str(candidate.get("group_key") or ""),
                group_type=str(candidate.get("candidate_type") or ""),
                base_bill=base_bill,
                user_id=normalized_user_id,
                now=now,
                metadata=metadata,
            )
            await self._set_reconciliation_preview_selected(
                conn,
                preview_id=preview_id,
                selected=False,
                user_id=normalized_user_id,
            )
            event_id = await self._append_bill_merge_event(
                conn,
                user_id=normalized_user_id,
                group_id=group_id,
                candidate_id=normalized_candidate_id,
                event_type="merge_applied",
                payload={
                    "candidate_type": candidate.get("candidate_type"),
                    "base_bill": base_bill,
                    "projection": projection,
                    "preview_id": preview_id,
                    "preview_selected_before": preview_selection_before,
                },
                now=now,
            )
            await self._set_reconciliation_candidate_status(
                conn,
                row_id=int(candidate["id"]),
                user_id=normalized_user_id,
                status="merged",
                now=now,
                event_id=event_id,
            )
            await conn.commit()
            return {
                "candidate_id": normalized_candidate_id,
                "action": "accept",
                "group_id": group_id,
                "bill": {
                    **base_bill,
                    "description": projection.get("description", ""),
                    "tag_ids": list(projection.get("tag_ids") or []),
                },
                "projection": projection,
            }
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def reject_import_reconciliation_candidate(
        self,
        candidate_id: str,
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Reject a reconciliation candidate and undo its projection if needed."""
        normalized_user_id = int(user_id)
        normalized_candidate_id = str(candidate_id or "").strip()
        if normalized_user_id <= 0 or not normalized_candidate_id:
            raise ValueError("Invalid reconciliation candidate")

        conn = await self._get_connection()
        try:
            await conn.execute("BEGIN IMMEDIATE")
            candidate = await self._get_reconciliation_candidate_with_group(
                conn,
                candidate_id=normalized_candidate_id,
                user_id=normalized_user_id,
            )
            group_id = int(candidate.get("group_id") or 0)
            base_bill, metadata = await self._prepare_reconciliation_base_snapshot(
                conn,
                candidate,
                user_id=normalized_user_id,
            )
            now = utc_now_iso()
            await self._set_reconciliation_candidate_status(
                conn,
                row_id=int(candidate["id"]),
                user_id=normalized_user_id,
                status="rejected",
                now=now,
            )
            projection = await self._recompute_reconciliation_projection(
                conn,
                group_id=group_id,
                group_key=str(candidate.get("group_key") or ""),
                group_type=str(candidate.get("candidate_type") or ""),
                base_bill=base_bill,
                user_id=normalized_user_id,
                now=now,
                metadata=metadata,
            )
            event_id = await self._append_bill_merge_event(
                conn,
                user_id=normalized_user_id,
                group_id=group_id,
                candidate_id=normalized_candidate_id,
                event_type="candidate_rejected",
                payload={
                    "candidate_type": candidate.get("candidate_type"),
                    "projection": projection,
                },
                now=now,
            )
            await self._set_reconciliation_candidate_status(
                conn,
                row_id=int(candidate["id"]),
                user_id=normalized_user_id,
                status="rejected",
                now=now,
                event_id=event_id,
            )
            await conn.commit()
            return {
                "candidate_id": normalized_candidate_id,
                "action": "reject",
                "group_id": group_id,
                "projection": projection,
            }
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def clear_import_reconciliation_candidate(
        self,
        candidate_id: str,
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Clear a reconciliation decision and recompute the group projection."""
        normalized_user_id = int(user_id)
        normalized_candidate_id = str(candidate_id or "").strip()
        if normalized_user_id <= 0 or not normalized_candidate_id:
            raise ValueError("Invalid reconciliation candidate")

        conn = await self._get_connection()
        try:
            await conn.execute("BEGIN IMMEDIATE")
            candidate = await self._get_reconciliation_candidate_with_group(
                conn,
                candidate_id=normalized_candidate_id,
                user_id=normalized_user_id,
            )
            group_id = int(candidate.get("group_id") or 0)
            base_bill, metadata = await self._prepare_reconciliation_base_snapshot(
                conn,
                candidate,
                user_id=normalized_user_id,
            )
            now = utc_now_iso()
            preview_id = await self._find_reconciliation_preview_id(
                conn,
                candidate,
                user_id=normalized_user_id,
            )
            await self._set_reconciliation_candidate_status(
                conn,
                row_id=int(candidate["id"]),
                user_id=normalized_user_id,
                status=self._PENDING_STATUS,
                now=now,
            )
            projection = await self._recompute_reconciliation_projection(
                conn,
                group_id=group_id,
                group_key=str(candidate.get("group_key") or ""),
                group_type=str(candidate.get("candidate_type") or ""),
                base_bill=base_bill,
                user_id=normalized_user_id,
                now=now,
                metadata=metadata,
            )
            if preview_id is not None:
                group_candidates = await self._load_group_candidates(
                    conn,
                    group_key=str(candidate.get("group_key") or ""),
                    user_id=normalized_user_id,
                )
                same_preview_still_applied = False
                for group_candidate in group_candidates:
                    if str(group_candidate.get("status") or "") not in self._APPLIED_STATUSES:
                        continue
                    group_preview_id = await self._find_reconciliation_preview_id(
                        conn,
                        group_candidate,
                        user_id=normalized_user_id,
                    )
                    same_preview_still_applied = group_preview_id == preview_id
                    if same_preview_still_applied:
                        break
                if not same_preview_still_applied:
                    await self._set_reconciliation_preview_selected(
                        conn,
                        preview_id=preview_id,
                        selected=True,
                        user_id=normalized_user_id,
                    )

            event_id = await self._append_bill_merge_event(
                conn,
                user_id=normalized_user_id,
                group_id=group_id,
                candidate_id=normalized_candidate_id,
                event_type="merge_rolled_back",
                payload={
                    "candidate_type": candidate.get("candidate_type"),
                    "projection": projection,
                    "preview_id": preview_id,
                },
                now=now,
            )
            await self._set_reconciliation_candidate_status(
                conn,
                row_id=int(candidate["id"]),
                user_id=normalized_user_id,
                status=self._PENDING_STATUS,
                now=now,
                event_id=event_id,
            )
            await conn.commit()
            return {
                "candidate_id": normalized_candidate_id,
                "action": "clear",
                "group_id": group_id,
                "projection": projection,
            }
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def get_bill_reconciliation_projection(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        """Return the latest merge projection/provenance for a formal bill."""
        normalized_bill_id = int(bill_id)
        normalized_user_id = int(user_id)
        if normalized_bill_id <= 0 or normalized_user_id <= 0:
            return None

        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT g.*
            FROM bill_merge_groups g
            JOIN bill_merge_members m ON m.group_id = g.id
            WHERE g.user_id = ?
              AND g.family = ?
              AND m.bill_id = ?
              AND m.member_type = 'existing_bill'
            ORDER BY g.updated_at DESC, g.id DESC
            LIMIT 1
            """,
            (normalized_user_id, self._IMPORT_RECONCILIATION_FAMILY, normalized_bill_id),
        ) as cursor:
            group_row = await cursor.fetchone()
        if group_row is None:
            return None

        group = dict(group_row)
        metadata = self._json_loads(group.get("metadata_json"))
        projection = (
            dict(metadata.get("projection"))
            if isinstance(metadata.get("projection"), dict)
            else {}
        )
        candidate_ids = [
            str(candidate_id)
            for candidate_id in list(projection.get("candidate_ids") or [])
            if str(candidate_id)
        ]
        if not projection or not candidate_ids:
            return None
        return {
            "group_id": int(group.get("id") or 0),
            "group_type": str(group.get("group_type") or ""),
            "status": str(group.get("status") or ""),
            "canonical_bill_id": self._normalize_optional_int(group.get("canonical_bill_id")),
            "signal_label": str(projection.get("signal_label") or ""),
            "source_chain": (
                list(projection.get("source_chain") or [])
                if isinstance(projection.get("source_chain"), list)
                else []
            ),
            "candidate_ids": candidate_ids,
            "description": str(projection.get("description") or ""),
            "tag_ids": self._normalize_tag_ids(projection.get("tag_ids")),
        }

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
        async with conn.execute(
            """
            SELECT id
            FROM bill_merge_groups
            WHERE id = ? AND user_id = ?
            LIMIT 1
            """,
            (normalized_group_id, normalized_user_id),
        ) as cursor:
            group_row = await cursor.fetchone()
        if group_row is None:
            raise ValueError("Merge group not found")

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
