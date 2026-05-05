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


class MatchingSuppressionsMixin(object):
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
