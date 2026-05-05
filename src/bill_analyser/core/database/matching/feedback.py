"""Historical bill matching persistence helpers."""

from __future__ import annotations

import json
import sqlite3
from typing import Any

from bill_analyser.utils.logger import log_method
from bill_analyser.core.bill_date_utils import parse_bill_datetime
from bill_analyser.core.database.shared import DatabaseFacadeBase
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.investment.matching import score_investment_candidate
from bill_analyser.core.investment.settings import (
    build_user_investment_keyword_settings,
    serialize_keyword_list,
)
from bill_analyser.core.matching import build_transfer_pair_candidate, build_transfer_pair_candidates
from bill_analyser.core.matching.candidate_ids import build_learning_rule_revision, normalize_learning_rule_revision
from .base import MatchingBaseMixin

_TRANSFER_PAIR_TYPE = MatchingBaseMixin._TRANSFER_PAIR_TYPE


class MatchingFeedbackMixin(object):
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
