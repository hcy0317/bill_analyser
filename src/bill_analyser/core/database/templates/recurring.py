"""Recurring-bill template matching and binding persistence helpers."""

# pylint: disable=line-too-long,protected-access,too-many-locals

from __future__ import annotations

from typing import Any

from bill_analyser.core import template_rust_bridge
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.utils.logger import log_method


@log_method
async def get_recurring_candidates_for_bill(
    self,
    bill_id: int,
    user_id: int = 1,
    tolerance_days: int = 3,
) -> dict[str, Any]:
    """获取账单可匹配的定时交易候选。"""
    conn = await self._get_connection()
    bill = await self.get_bill_by_id(bill_id, user_id=user_id)
    if not bill:
        return {"bill": None, "linked_recurring_id": None, "linked_recurring_name": "", "candidates": []}

    linked_recurring_id = bill.get("created_from_recurring")
    linked_recurring_name = ""
    if linked_recurring_id:
        async with conn.execute(
            "SELECT name FROM recurring_bills WHERE id = ? AND user_id = ?",
            (linked_recurring_id, user_id),
        ) as cursor:
            row = await cursor.fetchone()
            if row:
                linked_recurring_name = str(row["name"] or "")

    recurring_rows = await self.get_enabled_recurring_templates(user_id=user_id)
    candidates = self.build_recurring_candidates_for_bill_data(
        bill,
        recurring_rows,
        linked_recurring_id=linked_recurring_id,
        tolerance_days=tolerance_days,
    )
    return {
        "bill": bill,
        "linked_recurring_id": linked_recurring_id,
        "linked_recurring_name": linked_recurring_name,
        "candidates": candidates,
    }


@log_method
async def get_enabled_recurring_templates(self, user_id: int = 1) -> list[dict[str, Any]]:
    """获取当前用户启用中的定时交易模板。"""
    if self._should_use_rust_template_bridge():
        return template_rust_bridge.list_enabled_recurring_templates(self.db_path, user_id=user_id)

    conn = await self._get_connection()
    async with conn.execute(
        """
        SELECT * FROM recurring_bills
        WHERE user_id = ? AND enabled = 1
        ORDER BY COALESCE(display_order, 0), name
        """,
        (user_id,),
    ) as cursor:
        rows = await cursor.fetchall()
    return [dict(row) for row in rows]


def build_recurring_candidates_for_bill_data(
    self,
    bill: dict[str, Any],
    recurring_rows: list[dict[str, Any]],
    linked_recurring_id: Any = None,
    tolerance_days: int = 3,
) -> list[dict[str, Any]]:
    """基于账单数据构建定时账单候选列表。"""
    bill_date = self._parse_date_value(bill.get("date"))
    if not bill_date:
        return []

    bill_type = self._normalize_template_transaction_type(bill.get("type"))
    bill_amount_cents = round(abs(float(bill.get("amount") or 0)) * 100)
    bill_source_account = str(bill.get("source_account_id") or "0")
    bill_destination_account = str(bill.get("destination_account_id") or "0")

    candidates: list[dict[str, Any]] = []
    for recurring in recurring_rows:
        recurring_type = self._normalize_template_transaction_type(recurring.get("type"))
        if recurring_type != bill_type:
            continue

        recurring_amount_cents = round(abs(float(recurring.get("amount") or 0)))
        if recurring_amount_cents != bill_amount_cents:
            continue

        matched_occurrence = self._find_recurring_occurrence_near_date(
            recurring,
            bill_date,
            tolerance_days=tolerance_days,
        )
        if not matched_occurrence:
            continue

        score = 80
        reasons: list[str] = ["type", "amount", "schedule"]
        recurring_source_account = str(recurring.get("account") or "0")
        recurring_destination_account = str(recurring.get("counterparty") or "0")

        if recurring_source_account == bill_source_account:
            reasons.append("source_account")
            score += 10
        if bill_destination_account not in ("", "0") and recurring_destination_account == bill_destination_account:
            reasons.append("destination_account")
            score += 10

        days_offset = abs((matched_occurrence - bill_date).days)
        score += max(0, 10 - days_offset * 2)

        candidate = self._serialize_template_row(recurring, template_type=2)
        candidate.update(
            {
                "matchScore": score,
                "matchReasons": reasons,
                "matchedOccurrenceDate": matched_occurrence.isoformat(),
                "matchedDayOffset": days_offset,
                "linked": int(linked_recurring_id or 0) == int(recurring.get("id") or 0),
            }
        )
        candidates.append(candidate)

    candidates.sort(
        key=lambda item: (
            -int(item.get("matchScore") or 0),
            int(item.get("matchedDayOffset") or 999),
            str(item.get("name") or ""),
        )
    )
    return candidates


@log_method
async def bind_bill_to_recurring(self, bill_id: int, recurring_id: int, user_id: int = 1) -> dict[str, Any] | None:
    """将账单绑定到定时交易，并推进 next_date。"""
    conn = await self._get_connection()
    bill = await self.get_bill_by_id(bill_id, user_id=user_id)
    if not bill:
        return None

    previous_recurring_id = bill.get("created_from_recurring")
    async with conn.execute(
        "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?",
        (recurring_id, user_id),
    ) as cursor:
        row = await cursor.fetchone()
        recurring = dict(row) if row else None

    if not recurring:
        return None

    bill_date = self._parse_date_value(bill.get("date"))
    next_occurrence = None
    if bill_date:
        next_date = self._get_next_recurring_occurrence_after(recurring, bill_date)
        next_occurrence = next_date.isoformat() if next_date else None

    now = utc_now_iso()
    await conn.execute(
        "UPDATE bills SET created_from_recurring = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        (recurring_id, now, bill_id, user_id),
    )
    await conn.execute(
        "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        (next_occurrence or recurring.get("next_date"), now, recurring_id, user_id),
    )

    if previous_recurring_id and str(previous_recurring_id) != str(recurring_id):
        await self._recalculate_recurring_next_date(conn, int(previous_recurring_id), user_id, now)

    await conn.commit()
    return {
        "billId": bill_id,
        "recurringId": recurring_id,
        "nextScheduledDate": next_occurrence or recurring.get("next_date"),
    }


async def _recalculate_recurring_next_date(
    self,
    conn,
    recurring_id: int,
    user_id: int,
    now: str | None = None,
) -> None:
    """根据当前已绑定账单重算定时模板的下一次计划日期。"""
    async with conn.execute(
        "SELECT * FROM recurring_bills WHERE id = ? AND user_id = ?",
        (recurring_id, user_id),
    ) as recurring_cursor:
        recurring_row = await recurring_cursor.fetchone()

    recurring = dict(recurring_row) if recurring_row else None
    if not recurring:
        return

    async with conn.execute(
        """
        SELECT date FROM bills
        WHERE user_id = ? AND created_from_recurring = ?
        ORDER BY date DESC
        LIMIT 1
        """,
        (user_id, recurring_id),
    ) as linked_cursor:
        latest_linked_row = await linked_cursor.fetchone()

    latest_linked_date = self._parse_date_value(latest_linked_row["date"] if latest_linked_row else None)
    if latest_linked_date:
        next_date = self._get_next_recurring_occurrence_after(recurring, latest_linked_date)
    else:
        next_date = self._get_first_recurring_occurrence(recurring)

    next_occurrence = next_date.isoformat() if next_date else recurring.get("next_date")
    await conn.execute(
        "UPDATE recurring_bills SET next_date = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        (next_occurrence, now or utc_now_iso(), recurring_id, user_id),
    )


@log_method
async def unbind_bill_from_recurring(self, bill_id: int, user_id: int = 1) -> bool:
    """取消账单与定时交易的绑定。"""
    conn = await self._get_connection()
    bill = await self.get_bill_by_id(bill_id, user_id=user_id)
    if not bill:
        return False

    recurring_id = bill.get("created_from_recurring")
    now = utc_now_iso()
    cursor = await conn.execute(
        "UPDATE bills SET created_from_recurring = NULL, updated_at = ? WHERE id = ? AND user_id = ?",
        (now, bill_id, user_id),
    )

    if recurring_id:
        await self._recalculate_recurring_next_date(conn, int(recurring_id), user_id, now)

    await conn.commit()
    return cursor.rowcount > 0
