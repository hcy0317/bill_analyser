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


class MatchingLearningMixin(object):
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
