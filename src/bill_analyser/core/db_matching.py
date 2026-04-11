"""Historical bill matching persistence helpers."""

from __future__ import annotations

import sqlite3
from typing import Any

from ..utils.logger import log_method
from .bill_date_utils import parse_bill_datetime
from .db_shared import DatabaseFacadeBase
from .db_time import utc_now_iso
from .matching import build_transfer_pair_candidate, build_transfer_pair_candidates


class DatabaseMatchingMixin(DatabaseFacadeBase):
    """Persistence helpers for historical bill transfer pairing."""

    _TRANSFER_PAIR_TYPE = "transfer"
    _TRANSFER_PAIR_LOOKBACK_DAYS = 3
    _MANUAL_PAIR_SOURCE = "manual"

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
            "destination_account_id": int(
                row.get(f"{prefix}_destination_account_id") or 0
            ),
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

    async def _get_bill_pair_link_for_bill(
        self,
        bill_id: int,
        *,
        user_id: int = 1,
        pair_type: str = _TRANSFER_PAIR_TYPE,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT *
            FROM bill_pair_links
            WHERE user_id = ? AND pair_type = ? AND (left_bill_id = ? OR right_bill_id = ?)
            LIMIT 1
            """,
            (user_id, pair_type, bill_id, bill_id),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _get_bill_pair_link_by_id(
        self,
        pair_id: int,
        *,
        user_id: int = 1,
        pair_type: str = _TRANSFER_PAIR_TYPE,
        conn: Any | None = None,
    ) -> dict[str, Any] | None:
        active_conn = conn or await self._get_connection()
        async with active_conn.execute(
            """
            SELECT *
            FROM bill_pair_links
            WHERE id = ? AND user_id = ? AND pair_type = ?
            LIMIT 1
            """,
            (int(pair_id), user_id, pair_type),
        ) as cursor:
            row = await cursor.fetchone()
        return dict(row) if row else None

    async def _delete_bill_pair_links_for_bill_ids(
        self,
        conn: Any,
        bill_ids: list[int],
        *,
        user_id: int = 1,
    ) -> None:
        normalized_bill_ids = sorted(
            {int(bill_id) for bill_id in bill_ids if int(bill_id) > 0}
        )
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

    @log_method
    async def list_manual_transfer_pairs(
        self,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """List persisted manual transfer pairs for the current user."""
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
              AND pairs.pair_type = ?
              AND pairs.source = ?
            ORDER BY COALESCE(pairs.updated_at, pairs.created_at) DESC, pairs.id DESC
            """,
            (user_id, self._TRANSFER_PAIR_TYPE, self._MANUAL_PAIR_SOURCE),
        ) as cursor:
            rows = await cursor.fetchall()

        pairs: list[dict[str, Any]] = []
        for row in rows:
            row_dict = dict(row)
            pairs.append(
                {
                    "id": int(row_dict.get("id") or 0),
                    "pair_type": str(
                        row_dict.get("pair_type") or self._TRANSFER_PAIR_TYPE
                    ),
                    "source": str(
                        row_dict.get("source") or self._MANUAL_PAIR_SOURCE
                    ),
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
        )
        if existing_pair:
            other_bill_id = (
                int(existing_pair["right_bill_id"])
                if int(existing_pair["left_bill_id"]) == int(bill_id)
                else int(existing_pair["left_bill_id"])
            )
            return {
                "bill": anchor_bill,
                "linked_pair": {
                    "id": int(existing_pair["id"]),
                    "pair_type": str(
                        existing_pair.get("pair_type") or self._TRANSFER_PAIR_TYPE
                    ),
                    "source": str(existing_pair.get("source") or "manual"),
                    "left_bill_id": int(existing_pair["left_bill_id"]),
                    "right_bill_id": int(existing_pair["right_bill_id"]),
                    "other_bill_id": other_bill_id,
                },
                "candidates": [],
            }

        anchor_amount = float(anchor_bill.get("amount") or 0.0)
        anchor_source_account_id = int(anchor_bill.get("source_account_id") or 0)
        anchor_datetime = parse_bill_datetime(anchor_bill.get("date"))
        if (
            abs(anchor_amount) <= 0
            or anchor_source_account_id <= 0
            or anchor_datetime is None
        ):
            return {
                "bill": anchor_bill,
                "linked_pair": None,
                "candidates": [],
            }

        conn = await self._get_connection()
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
                    AND links.pair_type = ?
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
                self._TRANSFER_PAIR_TYPE,
            ),
        ) as cursor:
            candidate_rows = await cursor.fetchall()

        candidates = build_transfer_pair_candidates(
            anchor_bill,
            [dict(row) for row in candidate_rows],
        )
        return {
            "bill": anchor_bill,
            "linked_pair": None,
            "candidates": candidates,
        }

    @log_method
    async def create_manual_transfer_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Persist a single manual transfer pair for two historical bills."""
        left_bill_id, right_bill_id = self._normalize_transfer_pair_bill_ids(
            bill_id,
            candidate_bill_id,
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
                pair_type=self._TRANSFER_PAIR_TYPE,
                conn=conn,
            )
            right_pair = await self._get_bill_pair_link_for_bill(
                right_bill_id,
                user_id=user_id,
                pair_type=self._TRANSFER_PAIR_TYPE,
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
            await conn.commit()
            return {
                "id": int(cursor.lastrowid or 0),
                "pair_type": self._TRANSFER_PAIR_TYPE,
                "source": self._MANUAL_PAIR_SOURCE,
                "left_bill_id": left_bill_id,
                "right_bill_id": right_bill_id,
            }
        except sqlite3.IntegrityError as exc:
            await conn.rollback()
            raise ValueError(
                "Bills already belong to an existing transfer pair"
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

            if (
                str(pair.get("source") or self._MANUAL_PAIR_SOURCE)
                != self._MANUAL_PAIR_SOURCE
            ):
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
