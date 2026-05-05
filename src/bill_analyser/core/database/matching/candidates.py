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


class MatchingCandidatesMixin(object):
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
