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


class MatchingManualPairsMixin(object):
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
