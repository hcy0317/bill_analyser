from __future__ import annotations

from datetime import date

from bill_analyser.core.db import Database


def _build_recurring_row(
    *,
    recurring_id: int,
    name: str,
    amount_cents: int,
    frequency_type: int,
    frequency: str,
    start_date: str,
    source_account: str,
    destination_account: str,
    transaction_type: str = "支出",
) -> dict[str, object]:
    return {
        "id": recurring_id,
        "name": name,
        "type": transaction_type,
        "amount": amount_cents,
        "account": source_account,
        "counterparty": destination_account,
        "category": "餐饮",
        "destination_amount": 0,
        "hide_amount": 0,
        "tag": "",
        "comment": "",
        "display_order": 0,
        "hidden": 0,
        "scheduled_frequency_type": frequency_type,
        "frequency": frequency,
        "start_date": start_date,
        "end_date": "2026-12-31",
        "next_date": start_date,
        "utc_offset": 0,
    }


def test_build_recurring_candidates_for_bill_data_filters_and_sorts_by_score() -> None:
    """Recurring 候选应按 type/amount/schedule 筛选，并按匹配分数排序。"""
    db = Database(":memory:")
    bill = {
        "date": "2026-04-07 09:30:00",
        "type": "支出",
        "amount": -12.0,
        "source_account_id": "101",
        "destination_account_id": "202",
    }
    recurring_rows = [
        _build_recurring_row(
            recurring_id=11,
            name="完全命中",
            amount_cents=1200,
            frequency_type=1,
            frequency="2",
            start_date="2026-04-07",
            source_account="101",
            destination_account="202",
        ),
        _build_recurring_row(
            recurring_id=12,
            name="近一天命中",
            amount_cents=1200,
            frequency_type=1,
            frequency="1",
            start_date="2026-04-06",
            source_account="101",
            destination_account="999",
        ),
        _build_recurring_row(
            recurring_id=13,
            name="无账户加分",
            amount_cents=1200,
            frequency_type=1,
            frequency="2",
            start_date="2026-04-07",
            source_account="999",
            destination_account="999",
        ),
        _build_recurring_row(
            recurring_id=14,
            name="错误类型",
            amount_cents=1200,
            frequency_type=1,
            frequency="2",
            start_date="2026-04-07",
            source_account="101",
            destination_account="202",
            transaction_type="收入",
        ),
        _build_recurring_row(
            recurring_id=15,
            name="错误金额",
            amount_cents=1500,
            frequency_type=1,
            frequency="2",
            start_date="2026-04-07",
            source_account="101",
            destination_account="202",
        ),
    ]

    candidates = db.build_recurring_candidates_for_bill_data(
        bill,
        recurring_rows,
        linked_recurring_id=11,
        tolerance_days=3,
    )

    assert [int(candidate["id"]) for candidate in candidates] == [11, 12, 13]
    assert candidates[0]["linked"] is True
    assert candidates[0]["matchScore"] > candidates[1]["matchScore"] > candidates[2]["matchScore"]
    assert candidates[0]["matchedOccurrenceDate"] == "2026-04-07"
    assert candidates[1]["matchedDayOffset"] == 1
    assert {"type", "amount", "schedule", "source_account", "destination_account"}.issubset(
        set(candidates[0]["matchReasons"])
    )


def test_recurring_schedule_helpers_cover_first_nearest_and_next_occurrence_paths() -> None:
    """Recurring 时间 helper 应覆盖首次、邻近和下一次发生日期。"""
    db = Database(":memory:")
    recurring = _build_recurring_row(
        recurring_id=21,
        name="每周一扣费",
        amount_cents=1200,
        frequency_type=1,
        frequency="",
        start_date="2026-04-06",
        source_account="101",
        destination_account="202",
    )

    assert db._get_first_recurring_occurrence(recurring) == date(2026, 4, 6)
    assert db._find_recurring_occurrence_near_date(recurring, date(2026, 4, 8), tolerance_days=3) == date(2026, 4, 6)
    assert db._get_next_recurring_occurrence_after(recurring, date(2026, 4, 6)) == date(2026, 4, 13)
    assert db.build_recurring_candidates_for_bill_data({"date": "bad-date"}, [recurring]) == []
