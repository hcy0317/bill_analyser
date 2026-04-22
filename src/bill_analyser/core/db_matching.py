"""Historical bill matching persistence helpers."""

from __future__ import annotations

import json
import sqlite3
from typing import Any

from ..utils.logger import log_method
from .bill_date_utils import parse_bill_datetime
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso
from .investment_matching import score_investment_candidate
from .investment_settings import (
    build_user_investment_keyword_settings,
    serialize_keyword_list,
)
from .matching import build_transfer_pair_candidate, build_transfer_pair_candidates
from .matching.candidate_ids import build_learning_rule_revision, normalize_learning_rule_revision


class DatabaseMatchingMixin(DatabaseFacadeBase):
    """Persistence helpers for historical bill transfer pairing."""

    _TRANSFER_PAIR_TYPE = "transfer"
    _INVESTMENT_PAIR_TYPE = "investment"
    _TRANSFER_PAIR_LOOKBACK_DAYS = 3
    _INVESTMENT_PAIR_LOOKBACK_DAYS = 3
    _MANUAL_PAIR_SOURCE = "manual"
    _UNSET = object()

    @staticmethod
    def _build_bill_snapshot_from_row(
        row: dict[str, Any],
        prefix: str,
    ) -> dict[str, Any]:
        return {
            "id": int(row.get(f"{prefix}_id") or 0),
            "date": str(row.get(f"{prefix}_date") or ""),
            "type": str(row.get(f"{prefix}_type") or ""),
            "amount": float(row.get(f"{prefix}_amount") or 0.0),
            "counterparty": str(row.get(f"{prefix}_counterparty") or ""),
            "description": str(row.get(f"{prefix}_description") or ""),
            "payment_method": str(row.get(f"{prefix}_payment_method") or ""),
            "main_category": str(row.get(f"{prefix}_main_category") or ""),
            "sub_category": str(row.get(f"{prefix}_sub_category") or ""),
            "source_account_id": int(row.get(f"{prefix}_source_account_id") or 0),
            "destination_account_id": int(row.get(f"{prefix}_destination_account_id") or 0),
        }

    @staticmethod
    def _normalize_transfer_pair_bill_ids(
        bill_id: int,
        candidate_bill_id: int,
    ) -> tuple[int, int]:
        normalized_bill_id = int(bill_id)
        normalized_candidate_bill_id = int(candidate_bill_id)
        if normalized_bill_id == normalized_candidate_bill_id:
            raise ValueError("billId and candidateBillId must be different")
        return (
            min(normalized_bill_id, normalized_candidate_bill_id),
            max(normalized_bill_id, normalized_candidate_bill_id),
        )

    @staticmethod
    def _has_opposite_matching_amounts(
        left_bill: dict[str, Any],
        right_bill: dict[str, Any],
    ) -> bool:
        left_amount = float(left_bill.get("amount") or 0.0)
        right_amount = float(right_bill.get("amount") or 0.0)
        return abs(abs(left_amount) - abs(right_amount)) <= 0.01 and left_amount * right_amount < 0

    @staticmethod
    def _has_distinct_valid_source_account_ids(
        left_bill: dict[str, Any],
        right_bill: dict[str, Any],
    ) -> bool:
        left_source_account_id = int(left_bill.get("source_account_id") or 0)
        right_source_account_id = int(right_bill.get("source_account_id") or 0)
        return (
            left_source_account_id > 0
            and right_source_account_id > 0
            and left_source_account_id != right_source_account_id
        )

    @staticmethod
    def _is_explicit_transfer_type(raw_type: Any) -> bool:
        return str(raw_type or "").strip().lower() in {"转账", "transfer"}

    @classmethod
    def _is_investment_like_bill(
        cls,
        bill: dict[str, Any],
        keyword_config: dict[str, list[str]],
    ) -> bool:
        return score_investment_candidate(
            bill,
            allow_existing_investment=True,
            keyword_config=keyword_config,
        ) is not None

    async def _get_user_investment_keyword_config(
        self,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> dict[str, list[str]]:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT investment_platform_keywords, investment_product_keywords, investment_exclude_keywords
            FROM users
            WHERE id = ?
            LIMIT 1
            """,
            (user_id,),
        ) as cursor:
            user_row = await cursor.fetchone()
        return build_user_investment_keyword_settings(
            dict(user_row) if user_row else None,
        )

    @log_method
    async def get_pairing_investment_settings(
        self,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        """Return pairing-center investment settings backed by the current user row."""
        normalized_user_id = int(user_id)
        if normalized_user_id <= 0:
            return None

        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT id, import_learning_enabled,
                   investment_platform_keywords, investment_product_keywords, investment_exclude_keywords
            FROM users
            WHERE id = ?
            LIMIT 1
            """,
            (normalized_user_id,),
        ) as cursor:
            user_row = await cursor.fetchone()

        if not user_row:
            return None

        row_dict = dict(user_row)
        keyword_settings = build_user_investment_keyword_settings(row_dict)
        return {
            "user_id": normalized_user_id,
            "import_learning_enabled": bool(row_dict.get("import_learning_enabled", True)),
            "investment_platform_keywords": keyword_settings["platform_keywords"],
            "investment_product_keywords": keyword_settings["product_keywords"],
            "investment_exclude_keywords": keyword_settings["exclude_keywords"],
        }

    @log_method
    async def update_pairing_investment_settings(
        self,
        *,
        user_id: int = 1,
        import_learning_enabled: Any = _UNSET,
        investment_platform_keywords: Any = _UNSET,
        investment_product_keywords: Any = _UNSET,
        investment_exclude_keywords: Any = _UNSET,
    ) -> dict[str, Any] | None:
        """Update pairing-center investment settings while keeping storage on ``users``."""
        normalized_user_id = int(user_id)
        if normalized_user_id <= 0:
            return None

        update_data: dict[str, Any] = {}
        if import_learning_enabled is not self._UNSET:
            update_data["import_learning_enabled"] = 1 if bool(import_learning_enabled) else 0
        if investment_platform_keywords is not self._UNSET:
            update_data["investment_platform_keywords"] = serialize_keyword_list(investment_platform_keywords)
        if investment_product_keywords is not self._UNSET:
            update_data["investment_product_keywords"] = serialize_keyword_list(investment_product_keywords)
        if investment_exclude_keywords is not self._UNSET:
            update_data["investment_exclude_keywords"] = serialize_keyword_list(investment_exclude_keywords)

        conn = await self._get_connection()
        try:
            await conn.execute("BEGIN IMMEDIATE")

            async with conn.execute(
                "SELECT id FROM users WHERE id = ? LIMIT 1",
                (normalized_user_id,),
            ) as cursor:
                user_row = await cursor.fetchone()
            if not user_row:
                await conn.rollback()
                return None

            if update_data:
                set_clause = ", ".join(f"{field_name} = ?" for field_name in update_data)
                cursor = await conn.execute(
                    f"UPDATE users SET {set_clause} WHERE id = ?",
                    [*update_data.values(), normalized_user_id],
                )
                if int(cursor.rowcount or 0) != 1:
                    await conn.rollback()
                    return None

            settings = await self.get_pairing_investment_settings(
                user_id=normalized_user_id,
                conn=conn,
            )
            await conn.commit()
            return settings
        except Exception:
            await conn.rollback()
            raise

    @classmethod
    def _resolve_pair_time_diff_seconds(
        cls,
        left_bill: dict[str, Any],
        right_bill: dict[str, Any],
        *,
        lookback_days: int,
    ) -> float | None:
        left_datetime = parse_bill_datetime(left_bill.get("date"))
        right_datetime = parse_bill_datetime(right_bill.get("date"))
        if left_datetime is None or right_datetime is None:
            return None

        time_diff_seconds = abs((left_datetime - right_datetime).total_seconds())
        max_window_seconds = float(max(lookback_days, 0) * 24 * 60 * 60)
        if max_window_seconds <= 0 or time_diff_seconds > max_window_seconds:
            return None
        return time_diff_seconds

    async def _get_bill_pair_link_for_bill(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
        pair_type: str | None = _TRANSFER_PAIR_TYPE,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        active_conn = conn or await self._get_connection()
        query = (
            "SELECT * FROM bill_pair_links "
            "WHERE user_id = ? AND (left_bill_id = ? OR right_bill_id = ?) "
        )
        params: list[Any] = [user_id, bill_id, bill_id]
        if pair_type is not None:
            query += "AND pair_type = ? "
            params.append(pair_type)
        query += "LIMIT 1"
        async with active_conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _get_bill_pair_link_by_id(
        self,
        pair_id: int,
        *,
        user_id: int = 1,
        pair_type: str | None = _TRANSFER_PAIR_TYPE,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        active_conn = conn or await self._get_connection()
        query = "SELECT * FROM bill_pair_links WHERE id = ? AND user_id = ? "
        params: list[Any] = [int(pair_id), user_id]
        if pair_type is not None:
            query += "AND pair_type = ? "
            params.append(pair_type)
        query += "LIMIT 1"
        async with active_conn.execute(query, params) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    @classmethod
    def _build_linked_pair_payload(
        cls,
        existing_pair: dict[str, Any],
        *,
        bill_id: int,
    ) -> dict[str, Any]:
        other_bill_id = (
            int(existing_pair["right_bill_id"])
            if int(existing_pair["left_bill_id"]) == int(bill_id)
            else int(existing_pair["left_bill_id"])
        )
        return {
            "id": int(existing_pair["id"]),
            "pair_type": str(existing_pair.get("pair_type") or cls._TRANSFER_PAIR_TYPE),
            "source": str(existing_pair.get("source") or cls._MANUAL_PAIR_SOURCE),
            "left_bill_id": int(existing_pair["left_bill_id"]),
            "right_bill_id": int(existing_pair["right_bill_id"]),
            "other_bill_id": other_bill_id,
        }

    @staticmethod
    def _build_bill_pair_feedback_payload(
        *,
        kind: str,
        bill_id: int,
        candidate_bill_id: int,
        pair: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "scope": "bill",
            "kind": str(kind or "").strip(),
            "bill_id": int(bill_id),
            "candidate_bill_id": int(candidate_bill_id),
        }
        if isinstance(pair, dict):
            payload["pair"] = {
                "id": int(pair.get("id") or 0),
                "pair_type": str(pair.get("pair_type") or ""),
                "source": str(pair.get("source") or ""),
                "left_bill_id": int(pair.get("left_bill_id") or 0),
                "right_bill_id": int(pair.get("right_bill_id") or 0),
            }
        return payload

    @staticmethod
    def _deserialize_bill_pair_feedback_payload(raw_payload_json: Any) -> dict[str, Any]:
        if isinstance(raw_payload_json, dict):
            return dict(raw_payload_json)
        if raw_payload_json in (None, ""):
            return {}
        try:
            payload = json.loads(str(raw_payload_json))
        except (TypeError, ValueError, json.JSONDecodeError):
            return {}
        return dict(payload) if isinstance(payload, dict) else {}

    @staticmethod
    def _is_bill_related_feedback_payload(payload: dict[str, Any], bill_id: int) -> bool:
        normalized_bill_id = int(bill_id)
        related_bill_ids: set[int] = set()

        for raw_bill_id in (payload.get("bill_id"), payload.get("candidate_bill_id")):
            if raw_bill_id in (None, ""):
                continue
            try:
                related_bill_ids.add(int(raw_bill_id))
            except (TypeError, ValueError):
                continue

        pair_payload = payload.get("pair")
        if isinstance(pair_payload, dict):
            for raw_bill_id in (pair_payload.get("left_bill_id"), pair_payload.get("right_bill_id")):
                if raw_bill_id in (None, ""):
                    continue
                try:
                    related_bill_ids.add(int(raw_bill_id))
                except (TypeError, ValueError):
                    continue

        return normalized_bill_id in related_bill_ids

    @log_method
    async def record_bill_pair_feedback(
        self,
        candidate_id: str,
        action: str,
        payload: dict[str, Any] | None = None,
        *,
        user_id: int,
        conn: Any | None = None,
    ) -> dict[str, Any]:
        """Persist an append-only feedback event for historical bill matching actions."""
        normalized_candidate_id = str(candidate_id or "").strip()
        normalized_action = str(action or "").strip().lower()
        normalized_user_id = int(user_id)
        if not normalized_candidate_id:
            raise ValueError("Invalid candidateId")
        if normalized_action not in {"accept", "reject", "manual_override"}:
            raise ValueError("Invalid action")
        if normalized_user_id <= 0:
            raise ValueError("Invalid userId")

        normalized_payload = payload if isinstance(payload, dict) else {}
        created_at = utc_now_iso()
        payload_json = json.dumps(normalized_payload, ensure_ascii=False)
        active_conn = conn or await self._get_connection()
        cursor = await active_conn.execute(
            """
            INSERT INTO bill_pair_feedback (
                user_id, candidate_id, action, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?)
            """,
            (normalized_user_id, normalized_candidate_id, normalized_action, payload_json, created_at),
        )
        if conn is None:
            await active_conn.commit()

        return {
            "id": int(cursor.lastrowid or 0),
            "candidate_id": normalized_candidate_id,
            "action": normalized_action,
            "payload": normalized_payload,
            "created_at": created_at,
        }

    @log_method
    async def list_bill_matching_feedback_for_bill(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """Return append-only formal-bill matching feedback events related to a bill."""
        normalized_bill_id = int(bill_id)
        if normalized_bill_id <= 0:
            return []

        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT *
            FROM bill_pair_feedback
            WHERE user_id = ?
            ORDER BY COALESCE(created_at, '') DESC, id DESC
            """,
            (user_id,),
        ) as cursor:
            rows = await cursor.fetchall()

        events: list[dict[str, Any]] = []
        for row in rows:
            row_dict = dict(row)
            payload = self._deserialize_bill_pair_feedback_payload(row_dict.get("payload_json"))
            if not self._is_bill_related_feedback_payload(payload, normalized_bill_id):
                continue
            events.append(
                {
                    "id": int(row_dict.get("id") or 0),
                    "user_id": int(row_dict.get("user_id") or 0),
                    "candidate_id": str(row_dict.get("candidate_id") or ""),
                    "action": str(row_dict.get("action") or ""),
                    "payload": payload,
                    "created_at": str(row_dict.get("created_at") or ""),
                }
            )

        return events

    async def _delete_bill_pair_links_for_bill_ids(
        self,
        conn: Any,
        bill_ids: list[int],
        *,
        user_id: int = 1,
    ) -> None:
        normalized_bill_ids = sorted({int(bill_id) for bill_id in bill_ids if int(bill_id) > 0})
        if not normalized_bill_ids:
            return

        placeholders = ",".join(["?" for _ in normalized_bill_ids])
        params = [user_id, *normalized_bill_ids, *normalized_bill_ids]
        await conn.execute(
            (
                "DELETE FROM bill_pair_links "
                f"WHERE user_id = ? AND (left_bill_id IN ({placeholders}) "
                f"OR right_bill_id IN ({placeholders}))"
            ),
            params,
        )

    async def _get_bill_transfer_pair_suppression(
        self,
        bill_id: int,
        candidate_bill_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            bill_id,
            candidate_bill_id,
        )
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT *
            FROM bill_transfer_pair_suppressions
            WHERE user_id = ? AND left_bill_id = ? AND right_bill_id = ?
            LIMIT 1
            """,
            (user_id, left_bill_id, right_bill_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _get_suppressed_bill_transfer_candidate_ids(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> set[int]:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT
                CASE
                    WHEN left_bill_id = ? THEN right_bill_id
                    ELSE left_bill_id
                END AS other_bill_id
            FROM bill_transfer_pair_suppressions
            WHERE user_id = ? AND (left_bill_id = ? OR right_bill_id = ?)
            """,
            (int(bill_id), user_id, int(bill_id), int(bill_id)),
        ) as cursor:
            rows = await cursor.fetchall()
        return {int(row["other_bill_id"]) for row in rows if row and row["other_bill_id"] is not None}

    async def _delete_bill_transfer_pair_suppressions_for_bill_ids(
        self,
        conn: Any,
        bill_ids: list[int],
        *,
        user_id: int = 1,
    ) -> None:
        normalized_bill_ids = sorted({int(bill_id) for bill_id in bill_ids if int(bill_id) > 0})
        if not normalized_bill_ids:
            return

        placeholders = ",".join(["?" for _ in normalized_bill_ids])
        params = [user_id, *normalized_bill_ids, *normalized_bill_ids]
        await conn.execute(
            (
                "DELETE FROM bill_transfer_pair_suppressions "
                f"WHERE user_id = ? AND (left_bill_id IN ({placeholders}) "
                f"OR right_bill_id IN ({placeholders}))"
            ),
            params,
        )

    async def _get_bill_investment_pair_suppression(
        self,
        bill_id: int,
        candidate_bill_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            bill_id,
            candidate_bill_id,
        )
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT *
            FROM bill_investment_pair_suppressions
            WHERE user_id = ? AND left_bill_id = ? AND right_bill_id = ?
            LIMIT 1
            """,
            (user_id, left_bill_id, right_bill_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _get_suppressed_bill_investment_candidate_ids(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> set[int]:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT
                CASE
                    WHEN left_bill_id = ? THEN right_bill_id
                    ELSE left_bill_id
                END AS other_bill_id
            FROM bill_investment_pair_suppressions
            WHERE user_id = ? AND (left_bill_id = ? OR right_bill_id = ?)
            """,
            (int(bill_id), user_id, int(bill_id), int(bill_id)),
        ) as cursor:
            rows = await cursor.fetchall()
        return {int(row["other_bill_id"]) for row in rows if row and row["other_bill_id"] is not None}

    async def _delete_bill_investment_pair_suppressions_for_bill_ids(
        self,
        conn: Any,
        bill_ids: list[int],
        *,
        user_id: int = 1,
    ) -> None:
        normalized_bill_ids = sorted({int(bill_id) for bill_id in bill_ids if int(bill_id) > 0})
        if not normalized_bill_ids:
            return

        placeholders = ",".join(["?" for _ in normalized_bill_ids])
        params = [user_id, *normalized_bill_ids, *normalized_bill_ids]
        await conn.execute(
            (
                "DELETE FROM bill_investment_pair_suppressions "
                f"WHERE user_id = ? AND (left_bill_id IN ({placeholders}) "
                f"OR right_bill_id IN ({placeholders}))"
            ),
            params,
        )

    async def _get_bill_learning_rule_suppression(
        self,
        bill_id: int,
        rule_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT *
            FROM bill_learning_rule_suppressions
            WHERE user_id = ? AND bill_id = ? AND rule_id = ?
            LIMIT 1
            """,
            (user_id, int(bill_id), int(rule_id)),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _get_suppressed_bill_learning_rule_ids(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> set[int]:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT rule_id
            FROM bill_learning_rule_suppressions
            WHERE user_id = ? AND bill_id = ?
            """,
            (user_id, int(bill_id)),
        ) as cursor:
            rows = await cursor.fetchall()
        return {int(row["rule_id"]) for row in rows if row and row["rule_id"] is not None}

    async def _get_bill_learning_rule_suppression_created_at_map(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
        conn: Any | None = None,
    ) -> dict[int, str]:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT rule_id, created_at
            FROM bill_learning_rule_suppressions
            WHERE user_id = ? AND bill_id = ?
            """,
            (user_id, int(bill_id)),
        ) as cursor:
            rows = await cursor.fetchall()
        return {
            int(row["rule_id"]): str(row["created_at"] or "")
            for row in rows
            if row and row["rule_id"] is not None
        }

    async def _delete_bill_learning_rule_suppressions_for_bill_ids(
        self,
        conn: Any,
        bill_ids: list[int],
        *,
        user_id: int = 1,
    ) -> None:
        normalized_bill_ids = sorted({int(bill_id) for bill_id in bill_ids if int(bill_id) > 0})
        if not normalized_bill_ids:
            return

        placeholders = ",".join(["?" for _ in normalized_bill_ids])
        await conn.execute(
            (f"DELETE FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id IN ({placeholders})"),
            [user_id, *normalized_bill_ids],
        )

    @log_method
    async def list_manual_transfer_pairs(
        self,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """List persisted manual bill pairs for the current user."""
        conn = await self._get_connection()
        async with conn.execute(
            """
            SELECT
                pairs.id,
                pairs.pair_type,
                pairs.source,
                pairs.left_bill_id,
                pairs.right_bill_id,
                pairs.created_at,
                pairs.updated_at,
                left_bill.date AS left_bill_date,
                left_bill.type AS left_bill_type,
                left_bill.amount AS left_bill_amount,
                left_bill.counterparty AS left_bill_counterparty,
                left_bill.description AS left_bill_description,
                left_bill.payment_method AS left_bill_payment_method,
                left_bill.main_category AS left_bill_main_category,
                left_bill.sub_category AS left_bill_sub_category,
                left_bill.source_account_id AS left_bill_source_account_id,
                left_bill.destination_account_id AS left_bill_destination_account_id,
                right_bill.date AS right_bill_date,
                right_bill.type AS right_bill_type,
                right_bill.amount AS right_bill_amount,
                right_bill.counterparty AS right_bill_counterparty,
                right_bill.description AS right_bill_description,
                right_bill.payment_method AS right_bill_payment_method,
                right_bill.main_category AS right_bill_main_category,
                right_bill.sub_category AS right_bill_sub_category,
                right_bill.source_account_id AS right_bill_source_account_id,
                right_bill.destination_account_id AS right_bill_destination_account_id
            FROM bill_pair_links AS pairs
            JOIN bills AS left_bill
              ON left_bill.id = pairs.left_bill_id
             AND left_bill.user_id = pairs.user_id
            JOIN bills AS right_bill
              ON right_bill.id = pairs.right_bill_id
             AND right_bill.user_id = pairs.user_id
            WHERE pairs.user_id = ?
              AND pairs.source = ?
            ORDER BY COALESCE(pairs.updated_at, pairs.created_at) DESC, pairs.id DESC
            """,
                        (user_id, self._MANUAL_PAIR_SOURCE),
        ) as cursor:
            rows = await cursor.fetchall()

        pairs: list[dict[str, Any]] = []
        for row in rows:
            row_dict = dict(row)
            pairs.append(
                {
                    "id": int(row_dict.get("id") or 0),
                    "pair_type": str(row_dict.get("pair_type") or self._TRANSFER_PAIR_TYPE),
                    "source": str(row_dict.get("source") or self._MANUAL_PAIR_SOURCE),
                    "left_bill_id": int(row_dict.get("left_bill_id") or 0),
                    "right_bill_id": int(row_dict.get("right_bill_id") or 0),
                    "created_at": str(row_dict.get("created_at") or ""),
                    "updated_at": str(row_dict.get("updated_at") or ""),
                    "left_bill": self._build_bill_snapshot_from_row(
                        row_dict,
                        "left_bill",
                    ),
                    "right_bill": self._build_bill_snapshot_from_row(
                        row_dict,
                        "right_bill",
                    ),
                }
            )

        return pairs

    @log_method
    async def get_bill_transfer_candidates(
        self,
        bill_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Return transfer-only historical matching candidates for a persisted bill."""
        anchor_bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not anchor_bill:
            return {"bill": None, "linked_pair": None, "candidates": []}

        existing_pair = await self._get_bill_pair_link_for_bill(
            bill_id,
            user_id=user_id,
            pair_type=None,
        )
        if existing_pair:
            return {
                "bill": anchor_bill,
                "linked_pair": self._build_linked_pair_payload(existing_pair, bill_id=bill_id),
                "candidates": [],
            }

        anchor_amount = float(anchor_bill.get("amount") or 0.0)
        anchor_source_account_id = int(anchor_bill.get("source_account_id") or 0)
        anchor_datetime = parse_bill_datetime(anchor_bill.get("date"))
        if abs(anchor_amount) <= 0 or anchor_source_account_id <= 0 or anchor_datetime is None:
            return {
                "bill": anchor_bill,
                "linked_pair": None,
                "candidates": [],
            }

        conn = await self._get_connection()
        suppressed_candidate_bill_ids = await self._get_suppressed_bill_transfer_candidate_ids(
            bill_id,
            user_id=user_id,
            conn=conn,
        )
        async with conn.execute(
            """
            SELECT *
            FROM bills AS b
            WHERE b.user_id = ?
              AND b.id != ?
              AND COALESCE(b.source_account_id, 0) > 0
              AND COALESCE(b.source_account_id, 0) != ?
              AND ABS(ABS(COALESCE(b.amount, 0)) - ?) <= 0.01
              AND COALESCE(b.amount, 0) * ? < 0
              AND COALESCE(b.type, '') NOT IN ('转账', 'transfer')
              AND NOT EXISTS (
                  SELECT 1
                  FROM bill_pair_links AS links
                  WHERE links.user_id = ?
                    AND (links.left_bill_id = b.id OR links.right_bill_id = b.id)
              )
                        ORDER BY b.id ASC
            """,
            (
                user_id,
                bill_id,
                anchor_source_account_id,
                abs(anchor_amount),
                anchor_amount,
                user_id,
            ),
        ) as cursor:
            candidate_rows = await cursor.fetchall()

        candidates = build_transfer_pair_candidates(
            anchor_bill,
            [dict(row) for row in candidate_rows if int(row["id"] or 0) not in suppressed_candidate_bill_ids],
        )
        return {
            "bill": anchor_bill,
            "linked_pair": None,
            "candidates": candidates,
        }

    @log_method
    async def get_bill_investment_candidate_bills(
        self,
        bill_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Return raw historical investment candidate bills for a persisted bill."""
        anchor_bill = await self.get_bill_by_id(bill_id, user_id=user_id)
        if not anchor_bill:
            return {"bill": None, "linked_pair": None, "candidates": []}

        existing_pair = await self._get_bill_pair_link_for_bill(
            bill_id,
            user_id=user_id,
            pair_type=None,
        )
        if existing_pair:
            return {
                "bill": anchor_bill,
                "linked_pair": self._build_linked_pair_payload(existing_pair, bill_id=bill_id),
                "candidates": [],
            }

        anchor_amount = float(anchor_bill.get("amount") or 0.0)
        anchor_source_account_id = int(anchor_bill.get("source_account_id") or 0)
        if (
            abs(anchor_amount) <= 0
            or anchor_source_account_id <= 0
            or self._is_explicit_transfer_type(anchor_bill.get("type"))
        ):
            return {
                "bill": anchor_bill,
                "linked_pair": None,
                "candidates": [],
            }

        conn = await self._get_connection()
        keyword_config = await self._get_user_investment_keyword_config(user_id=user_id, conn=conn)
        transfer_suppressed_candidate_bill_ids = await self._get_suppressed_bill_transfer_candidate_ids(
            bill_id,
            user_id=user_id,
            conn=conn,
        )
        suppressed_candidate_bill_ids = await self._get_suppressed_bill_investment_candidate_ids(
            bill_id,
            user_id=user_id,
            conn=conn,
        )
        async with conn.execute(
            """
            SELECT *
            FROM bills AS b
            WHERE b.user_id = ?
              AND b.id != ?
              AND COALESCE(b.source_account_id, 0) > 0
              AND COALESCE(b.source_account_id, 0) != ?
              AND ABS(ABS(COALESCE(b.amount, 0)) - ?) <= 0.01
              AND COALESCE(b.amount, 0) * ? < 0
              AND COALESCE(b.type, '') NOT IN ('转账', 'transfer')
              AND NOT EXISTS (
                  SELECT 1
                  FROM bill_pair_links AS links
                  WHERE links.user_id = ?
                    AND (links.left_bill_id = b.id OR links.right_bill_id = b.id)
              )
            ORDER BY b.id ASC
            """,
            (
                user_id,
                bill_id,
                anchor_source_account_id,
                abs(anchor_amount),
                anchor_amount,
                user_id,
            ),
        ) as cursor:
            candidate_rows = await cursor.fetchall()

        candidates: list[dict[str, Any]] = []
        for row in candidate_rows:
            candidate = dict(row)
            candidate_id = int(candidate.get("id") or 0)
            if (
                candidate_id in transfer_suppressed_candidate_bill_ids
                or candidate_id in suppressed_candidate_bill_ids
            ):
                continue
            if not self._is_investment_like_bill(anchor_bill, keyword_config):
                continue
            if not self._is_investment_like_bill(candidate, keyword_config):
                continue
            if not self._has_opposite_matching_amounts(anchor_bill, candidate):
                continue
            if not self._has_distinct_valid_source_account_ids(anchor_bill, candidate):
                continue
            if (
                self._resolve_pair_time_diff_seconds(
                    anchor_bill,
                    candidate,
                    lookback_days=self._INVESTMENT_PAIR_LOOKBACK_DAYS,
                )
                is None
            ):
                continue
            candidates.append(candidate)

        return {
            "bill": anchor_bill,
            "linked_pair": None,
            "candidates": candidates,
        }

    @log_method
    async def reject_bill_transfer_candidate(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
        *,
        feedback_candidate_id: str | None = None,
    ) -> dict[str, Any]:
        """Persist a suppression for a historical transfer candidate pair."""
        normalized_bill_id = int(bill_id)
        normalized_candidate_bill_id = int(candidate_bill_id)
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            normalized_bill_id,
            normalized_candidate_bill_id,
        )
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? AND id IN (?, ?)",
                (user_id, left_bill_id, right_bill_id),
            ) as cursor:
                bill_rows = await cursor.fetchall()

            bills_by_id = {int(row["id"]): dict(row) for row in bill_rows}
            if left_bill_id not in bills_by_id or right_bill_id not in bills_by_id:
                await conn.rollback()
                raise LookupError("Bill not found")

            left_pair = await self._get_bill_pair_link_for_bill(
                left_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            right_pair = await self._get_bill_pair_link_for_bill(
                right_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            if left_pair or right_pair:
                await conn.rollback()
                raise ValueError("Bills already belong to an existing transfer pair")

            candidate_payload = build_transfer_pair_candidate(
                bills_by_id[left_bill_id],
                bills_by_id[right_bill_id],
            )
            if candidate_payload is None:
                await conn.rollback()
                raise ValueError("Bills are not eligible for transfer pairing")

            existing_suppression = await self._get_bill_transfer_pair_suppression(
                left_bill_id,
                right_bill_id,
                user_id=user_id,
                conn=conn,
            )
            if existing_suppression:
                await conn.rollback()
                return {
                    "left_bill_id": left_bill_id,
                    "right_bill_id": right_bill_id,
                }

            await conn.execute(
                """
                INSERT INTO bill_transfer_pair_suppressions (
                    user_id, left_bill_id, right_bill_id, created_at
                ) VALUES (?, ?, ?, ?)
                """,
                (user_id, left_bill_id, right_bill_id, utc_now_iso()),
            )
            if feedback_candidate_id not in (None, ""):
                await self.record_bill_pair_feedback(
                    str(feedback_candidate_id),
                    "reject",
                    self._build_bill_pair_feedback_payload(
                        kind=self._TRANSFER_PAIR_TYPE,
                        bill_id=normalized_bill_id,
                        candidate_bill_id=normalized_candidate_bill_id,
                    ),
                    user_id=user_id,
                    conn=conn,
                )
            await conn.commit()
            return {
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
        except sqlite3.IntegrityError:
            await conn.rollback()
            return {
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def reject_bill_investment_candidate(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
        *,
        feedback_candidate_id: str | None = None,
    ) -> dict[str, Any]:
        """Persist a suppression for a historical investment candidate pair."""
        normalized_bill_id = int(bill_id)
        normalized_candidate_bill_id = int(candidate_bill_id)
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            normalized_bill_id,
            normalized_candidate_bill_id,
        )
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? AND id IN (?, ?)",
                (user_id, left_bill_id, right_bill_id),
            ) as cursor:
                bill_rows = await cursor.fetchall()

            bills_by_id = {int(row["id"]): dict(row) for row in bill_rows}
            if left_bill_id not in bills_by_id or right_bill_id not in bills_by_id:
                await conn.rollback()
                raise LookupError("Bill not found")

            left_pair = await self._get_bill_pair_link_for_bill(
                left_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            right_pair = await self._get_bill_pair_link_for_bill(
                right_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            if left_pair or right_pair:
                await conn.rollback()
                raise ValueError("Bills already belong to an existing transfer pair")

            left_bill = bills_by_id[left_bill_id]
            right_bill = bills_by_id[right_bill_id]
            keyword_config = await self._get_user_investment_keyword_config(
                user_id=user_id,
                conn=conn,
            )
            if self._is_explicit_transfer_type(
                left_bill.get("type"),
            ) or self._is_explicit_transfer_type(right_bill.get("type")):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if not self._is_investment_like_bill(
                left_bill,
                keyword_config,
            ) or not self._is_investment_like_bill(right_bill, keyword_config):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if not self._has_opposite_matching_amounts(left_bill, right_bill):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if not self._has_distinct_valid_source_account_ids(left_bill, right_bill):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if (
                self._resolve_pair_time_diff_seconds(
                    left_bill,
                    right_bill,
                    lookback_days=self._INVESTMENT_PAIR_LOOKBACK_DAYS,
                )
                is None
            ):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")

            existing_suppression = await self._get_bill_investment_pair_suppression(
                left_bill_id,
                right_bill_id,
                user_id=user_id,
                conn=conn,
            )
            if existing_suppression:
                await conn.rollback()
                return {
                    "left_bill_id": left_bill_id,
                    "right_bill_id": right_bill_id,
                }

            await conn.execute(
                """
                INSERT INTO bill_investment_pair_suppressions (
                    user_id, left_bill_id, right_bill_id, created_at
                ) VALUES (?, ?, ?, ?)
                """,
                (user_id, left_bill_id, right_bill_id, utc_now_iso()),
            )
            if feedback_candidate_id not in (None, ""):
                await self.record_bill_pair_feedback(
                    str(feedback_candidate_id),
                    "reject",
                    self._build_bill_pair_feedback_payload(
                        kind=self._INVESTMENT_PAIR_TYPE,
                        bill_id=normalized_bill_id,
                        candidate_bill_id=normalized_candidate_bill_id,
                    ),
                    user_id=user_id,
                    conn=conn,
                )
            await conn.commit()
            return {
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
        except sqlite3.IntegrityError:
            await conn.rollback()
            return {
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def reject_bill_learning_candidate(
        self,
        bill_id: int,
        rule_id: int,
        user_id: int = 1,
        expected_rule_revision: str | None = None,
    ) -> dict[str, Any]:
        """Persist a suppression for a historical learning candidate."""
        normalized_bill_id = int(bill_id)
        normalized_rule_id = int(rule_id)
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            async with conn.execute(
                "SELECT id FROM bills WHERE user_id = ? AND id = ? LIMIT 1",
                (user_id, normalized_bill_id),
            ) as cursor:
                bill_row = await cursor.fetchone()
            if not bill_row:
                await conn.rollback()
                raise LookupError("Bill not found")

            async with conn.execute(
                "SELECT * FROM import_learning_rules WHERE user_id = ? AND id = ? LIMIT 1",
                (user_id, normalized_rule_id),
            ) as cursor:
                rule_row = await cursor.fetchone()
            if not rule_row:
                await conn.rollback()
                raise LookupError("Learning rule not found")

            current_rule_revision = build_learning_rule_revision(dict(rule_row))
            normalized_expected_rule_revision = (
                normalize_learning_rule_revision(expected_rule_revision)
                if expected_rule_revision not in (None, "")
                else None
            )
            if normalized_expected_rule_revision and normalized_expected_rule_revision != current_rule_revision:
                await conn.rollback()
                raise ValueError("Learning candidate not available")

            existing_suppression = await self._get_bill_learning_rule_suppression(
                normalized_bill_id,
                normalized_rule_id,
                user_id=user_id,
                conn=conn,
            )
            if existing_suppression and str(existing_suppression.get("created_at") or "") == current_rule_revision:
                await conn.rollback()
                return {"bill_id": normalized_bill_id, "rule_id": normalized_rule_id}

            if existing_suppression:
                await conn.execute(
                    "UPDATE bill_learning_rule_suppressions SET created_at = ? WHERE id = ?",
                    (current_rule_revision, int(existing_suppression["id"])),
                )
                await conn.commit()
                return {"bill_id": normalized_bill_id, "rule_id": normalized_rule_id}

            await conn.execute(
                """
                INSERT INTO bill_learning_rule_suppressions (
                    user_id, bill_id, rule_id, created_at
                ) VALUES (?, ?, ?, ?)
                """,
                (user_id, normalized_bill_id, normalized_rule_id, current_rule_revision),
            )
            await conn.commit()
            return {"bill_id": normalized_bill_id, "rule_id": normalized_rule_id}
        except sqlite3.IntegrityError:
            await conn.rollback()
            return {"bill_id": normalized_bill_id, "rule_id": normalized_rule_id}
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def accept_bill_learning_candidate(  # pylint: disable=too-many-locals,too-many-statements
        self,
        bill_id: int,
        rule_id: int,
        user_id: int = 1,
        expected_rule_revision: str | None = None,
    ) -> dict[str, Any]:
        """Apply a historical learning rule onto a persisted bill."""
        normalized_bill_id = int(bill_id)
        normalized_rule_id = int(rule_id)
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? AND id = ? LIMIT 1",
                (user_id, normalized_bill_id),
            ) as cursor:
                bill_row = await cursor.fetchone()
            if not bill_row:
                await conn.rollback()
                raise LookupError("Bill not found")

            async with conn.execute(
                "SELECT * FROM import_learning_rules WHERE user_id = ? AND id = ? LIMIT 1",
                (user_id, normalized_rule_id),
            ) as cursor:
                rule_row = await cursor.fetchone()
            if not rule_row:
                await conn.rollback()
                raise LookupError("Learning rule not found")

            bill = dict(bill_row)
            rule = dict(rule_row)
            current_rule_revision = build_learning_rule_revision(rule)
            normalized_expected_rule_revision = (
                normalize_learning_rule_revision(expected_rule_revision)
                if expected_rule_revision not in (None, "")
                else None
            )
            if normalized_expected_rule_revision and normalized_expected_rule_revision != current_rule_revision:
                await conn.rollback()
                raise ValueError("Learning candidate not available")
            existing_suppression = await self._get_bill_learning_rule_suppression(
                normalized_bill_id,
                normalized_rule_id,
                user_id=user_id,
                conn=conn,
            )
            if existing_suppression and str(existing_suppression.get("created_at") or "") == current_rule_revision:
                await conn.rollback()
                raise ValueError("Learning candidate not applicable")

            updates: dict[str, Any] = {}

            learned_type = str(rule.get("learned_type") or "").strip()
            if learned_type and learned_type != str(bill.get("type") or "").strip():
                updates["type"] = learned_type

            learned_category_id = rule.get("learned_category_id")
            if learned_category_id not in (None, "", 0, "0"):
                category = await self.get_category_by_id(int(learned_category_id), user_id=user_id)
                if category and (
                    str(category.get("main_category") or "") != str(bill.get("main_category") or "")
                    or str(category.get("sub_category") or "") != str(bill.get("sub_category") or "")
                ):
                    updates["main_category"] = str(category.get("main_category") or "")
                    updates["sub_category"] = str(category.get("sub_category") or "")

            learned_source_account_id = rule.get("learned_source_account_id")
            if learned_source_account_id not in (None, "", 0, "0"):
                async with conn.execute(
                    "SELECT id FROM accounts WHERE user_id = ? AND id = ? LIMIT 1",
                    (user_id, int(learned_source_account_id)),
                ) as cursor:
                    source_account_row = await cursor.fetchone()
                if source_account_row:
                    normalized_source_account_id = int(source_account_row["id"] or 0)
                    if normalized_source_account_id != int(bill.get("source_account_id") or 0):
                        updates["source_account_id"] = normalized_source_account_id

            learned_destination_account_id = rule.get("learned_destination_account_id")
            if learned_destination_account_id not in (None, "", 0, "0"):
                async with conn.execute(
                    "SELECT id FROM accounts WHERE user_id = ? AND id = ? LIMIT 1",
                    (user_id, int(learned_destination_account_id)),
                ) as cursor:
                    destination_account_row = await cursor.fetchone()
                if destination_account_row:
                    normalized_destination_account_id = int(destination_account_row["id"] or 0)
                    if normalized_destination_account_id != int(bill.get("destination_account_id") or 0):
                        updates["destination_account_id"] = normalized_destination_account_id

            updated_at = utc_now_iso()
            if updates:
                update_payload = {**updates, "updated_at": updated_at}
                set_clause = ", ".join(f"{key} = ?" for key in update_payload)
                cursor = await conn.execute(
                    f"UPDATE bills SET {set_clause} WHERE id = ? AND user_id = ?",
                    [*update_payload.values(), normalized_bill_id, user_id],
                )
                if int(cursor.rowcount or 0) != 1:
                    await conn.rollback()
                    raise LookupError("Bill not found")

            await conn.execute(
                ("DELETE FROM bill_learning_rule_suppressions WHERE user_id = ? AND bill_id = ? AND rule_id = ?"),
                (user_id, normalized_bill_id, normalized_rule_id),
            )

            await self._record_import_learning_rule_log(
                conn,
                rule_id=normalized_rule_id,
                user_id=user_id,
                action="accepted",
                match_type=str(rule.get("match_type") or ""),
                match_value=str(rule.get("match_value") or ""),
                normalized_match_value=str(rule.get("normalized_match_value") or ""),
                session_id=rule.get("source_session_id"),
                preview_id=rule.get("source_preview_id"),
                payload={
                    "bill_id": normalized_bill_id,
                    "applied_updates": updates,
                    "previous_bill": {
                        "type": str(bill.get("type") or ""),
                        "main_category": str(bill.get("main_category") or ""),
                        "sub_category": str(bill.get("sub_category") or ""),
                        "source_account_id": int(bill.get("source_account_id") or 0),
                        "destination_account_id": int(bill.get("destination_account_id") or 0),
                    },
                },
            )
            await conn.execute(
                """
                UPDATE import_learning_rules
                SET applied_count = applied_count + 1,
                    last_applied_at = ?,
                    updated_at = ?
                WHERE user_id = ? AND id = ?
                """,
                (updated_at, updated_at, user_id, normalized_rule_id),
            )
            await conn.execute(
                """
                INSERT OR IGNORE INTO bill_learning_rule_suppressions (
                    user_id, bill_id, rule_id, created_at
                ) VALUES (?, ?, ?, ?)
                """,
                (user_id, normalized_bill_id, normalized_rule_id, current_rule_revision),
            )

            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? AND id = ? LIMIT 1",
                (user_id, normalized_bill_id),
            ) as cursor:
                updated_bill_row = await cursor.fetchone()

            await conn.commit()
            return {
                "bill_id": normalized_bill_id,
                "rule_id": normalized_rule_id,
                "bill": dict(updated_bill_row) if updated_bill_row else None,
            }
        except sqlite3.IntegrityError as exc:
            await conn.rollback()
            raise ValueError("Learning candidate not applicable") from exc
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def create_manual_transfer_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
        *,
        feedback_candidate_id: str | None = None,
    ) -> dict[str, Any]:
        """Persist a single manual transfer pair for two historical bills."""
        normalized_bill_id = int(bill_id)
        normalized_candidate_bill_id = int(candidate_bill_id)
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            normalized_bill_id,
            normalized_candidate_bill_id,
        )
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? AND id IN (?, ?)",
                (user_id, left_bill_id, right_bill_id),
            ) as cursor:
                bill_rows = await cursor.fetchall()

            bills_by_id = {int(row["id"]): dict(row) for row in bill_rows}
            if left_bill_id not in bills_by_id or right_bill_id not in bills_by_id:
                await conn.rollback()
                raise LookupError("Bill not found")

            left_pair = await self._get_bill_pair_link_for_bill(
                left_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            right_pair = await self._get_bill_pair_link_for_bill(
                right_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            if left_pair or right_pair:
                await conn.rollback()
                raise ValueError("Bills already belong to an existing transfer pair")

            if await self._get_bill_transfer_pair_suppression(
                left_bill_id,
                right_bill_id,
                user_id=user_id,
                conn=conn,
            ):
                await conn.rollback()
                raise ValueError("Bills already rejected for transfer pairing")

            candidate_payload = build_transfer_pair_candidate(
                bills_by_id[left_bill_id],
                bills_by_id[right_bill_id],
            )
            if candidate_payload is None:
                await conn.rollback()
                raise ValueError("Bills are not eligible for transfer pairing")

            now = utc_now_iso()
            cursor = await conn.execute(
                """
                INSERT INTO bill_pair_links (
                    user_id, pair_type, left_bill_id, right_bill_id, source, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    self._TRANSFER_PAIR_TYPE,
                    left_bill_id,
                    right_bill_id,
                    self._MANUAL_PAIR_SOURCE,
                    now,
                    now,
                ),
            )
            pair = {
                "id": int(cursor.lastrowid or 0),
                "pair_type": self._TRANSFER_PAIR_TYPE,
                "source": self._MANUAL_PAIR_SOURCE,
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
            if feedback_candidate_id not in (None, ""):
                await self.record_bill_pair_feedback(
                    str(feedback_candidate_id),
                    "accept",
                    self._build_bill_pair_feedback_payload(
                        kind=self._TRANSFER_PAIR_TYPE,
                        bill_id=normalized_bill_id,
                        candidate_bill_id=normalized_candidate_bill_id,
                        pair=pair,
                    ),
                    user_id=user_id,
                    conn=conn,
                )
            await conn.commit()
            return pair
        except sqlite3.IntegrityError as exc:
            await conn.rollback()
            raise ValueError("Bills already belong to an existing transfer pair") from exc
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def create_manual_investment_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
        *,
        feedback_candidate_id: str | None = None,
    ) -> dict[str, Any]:
        """Persist a single manual investment pair for two historical bills."""
        normalized_bill_id = int(bill_id)
        normalized_candidate_bill_id = int(candidate_bill_id)
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            normalized_bill_id,
            normalized_candidate_bill_id,
        )
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            async with conn.execute(
                "SELECT * FROM bills WHERE user_id = ? AND id IN (?, ?)",
                (user_id, left_bill_id, right_bill_id),
            ) as cursor:
                bill_rows = await cursor.fetchall()

            bills_by_id = {int(row["id"]): dict(row) for row in bill_rows}
            if left_bill_id not in bills_by_id or right_bill_id not in bills_by_id:
                await conn.rollback()
                raise LookupError("Bill not found")

            left_pair = await self._get_bill_pair_link_for_bill(
                left_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            right_pair = await self._get_bill_pair_link_for_bill(
                right_bill_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            if left_pair or right_pair:
                await conn.rollback()
                raise ValueError("Bills already belong to an existing transfer pair")

            if await self._get_bill_transfer_pair_suppression(
                left_bill_id,
                right_bill_id,
                user_id=user_id,
                conn=conn,
            ):
                await conn.rollback()
                raise ValueError("Bills already rejected for transfer pairing")

            if await self._get_bill_investment_pair_suppression(
                left_bill_id,
                right_bill_id,
                user_id=user_id,
                conn=conn,
            ):
                await conn.rollback()
                raise ValueError("Bills already rejected for investment pairing")

            left_bill = bills_by_id[left_bill_id]
            right_bill = bills_by_id[right_bill_id]
            keyword_config = await self._get_user_investment_keyword_config(
                user_id=user_id,
                conn=conn,
            )
            if self._is_explicit_transfer_type(
                left_bill.get("type"),
            ) or self._is_explicit_transfer_type(right_bill.get("type")):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if not self._is_investment_like_bill(
                left_bill,
                keyword_config,
            ) or not self._is_investment_like_bill(right_bill, keyword_config):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if not self._has_opposite_matching_amounts(left_bill, right_bill):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if not self._has_distinct_valid_source_account_ids(left_bill, right_bill):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")
            if (
                self._resolve_pair_time_diff_seconds(
                    left_bill,
                    right_bill,
                    lookback_days=self._INVESTMENT_PAIR_LOOKBACK_DAYS,
                )
                is None
            ):
                await conn.rollback()
                raise ValueError("Bills are not eligible for investment pairing")

            now = utc_now_iso()
            cursor = await conn.execute(
                """
                INSERT INTO bill_pair_links (
                    user_id, pair_type, left_bill_id, right_bill_id, source, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    user_id,
                    self._INVESTMENT_PAIR_TYPE,
                    left_bill_id,
                    right_bill_id,
                    self._MANUAL_PAIR_SOURCE,
                    now,
                    now,
                ),
            )
            pair = {
                "id": int(cursor.lastrowid or 0),
                "pair_type": self._INVESTMENT_PAIR_TYPE,
                "source": self._MANUAL_PAIR_SOURCE,
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
            if feedback_candidate_id not in (None, ""):
                await self.record_bill_pair_feedback(
                    str(feedback_candidate_id),
                    "accept",
                    self._build_bill_pair_feedback_payload(
                        kind=self._INVESTMENT_PAIR_TYPE,
                        bill_id=normalized_bill_id,
                        candidate_bill_id=normalized_candidate_bill_id,
                        pair=pair,
                    ),
                    user_id=user_id,
                    conn=conn,
                )
            await conn.commit()
            return pair
        except sqlite3.IntegrityError as exc:
            await conn.rollback()
            raise ValueError(
                "Bills already belong to an existing transfer pair",
            ) from exc
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def delete_manual_transfer_pair(
        self,
        pair_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Delete a persisted manual transfer pair for historical bills."""
        normalized_pair_id = int(pair_id)
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            pair = await self._get_bill_pair_link_by_id(
                normalized_pair_id,
                user_id=user_id,
                pair_type=self._TRANSFER_PAIR_TYPE,
                conn=conn,
            )
            if not pair:
                await conn.rollback()
                raise LookupError("Pair not found")

            if str(pair.get("source") or self._MANUAL_PAIR_SOURCE) != self._MANUAL_PAIR_SOURCE:
                await conn.rollback()
                raise ValueError("Only manual transfer pairs can be deleted")

            cursor = await conn.execute(
                "DELETE FROM bill_pair_links WHERE id = ? AND user_id = ?",
                (normalized_pair_id, user_id),
            )
            if int(cursor.rowcount or 0) != 1:
                await conn.rollback()
                raise LookupError("Pair not found")

            await conn.commit()
            return {
                "id": int(pair["id"]),
                "pair_type": str(pair.get("pair_type") or self._TRANSFER_PAIR_TYPE),
                "source": str(pair.get("source") or self._MANUAL_PAIR_SOURCE),
                "left_bill_id": int(pair["left_bill_id"]),
                "right_bill_id": int(pair["right_bill_id"]),
            }
        except Exception:
            await conn.rollback()
            raise

    @log_method
    async def delete_manual_pair(
        self,
        pair_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Delete a persisted manual pair for historical bills."""
        normalized_pair_id = int(pair_id)
        conn = await self._get_connection()

        try:
            await conn.execute("BEGIN IMMEDIATE")
            pair = await self._get_bill_pair_link_by_id(
                normalized_pair_id,
                user_id=user_id,
                pair_type=None,
                conn=conn,
            )
            if not pair:
                await conn.rollback()
                raise LookupError("Pair not found")

            if str(pair.get("source") or self._MANUAL_PAIR_SOURCE) != self._MANUAL_PAIR_SOURCE:
                await conn.rollback()
                raise ValueError("Only manual pairs can be deleted")

            cursor = await conn.execute(
                "DELETE FROM bill_pair_links WHERE id = ? AND user_id = ?",
                (normalized_pair_id, user_id),
            )
            if int(cursor.rowcount or 0) != 1:
                await conn.rollback()
                raise LookupError("Pair not found")

            await conn.commit()
            return {
                "id": int(pair["id"]),
                "pair_type": str(
                    pair.get("pair_type") or self._TRANSFER_PAIR_TYPE,
                ),
                "source": str(pair.get("source") or self._MANUAL_PAIR_SOURCE),
                "left_bill_id": int(pair["left_bill_id"]),
                "right_bill_id": int(pair["right_bill_id"]),
            }
        except Exception:
            await conn.rollback()
            raise
