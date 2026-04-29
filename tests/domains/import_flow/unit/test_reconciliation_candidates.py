from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.db import Database
from bill_analyser.core.matching import build_transfer_pair_candidates
from bill_analyser.core.smart_dedup import SmartDeduplicationEngine


class FakeReconciliationDB:
    def __init__(self, existing_bills: list[dict[str, Any]]) -> None:
        self.existing_bills = existing_bills
        self.date_range_calls: list[tuple[str, str, int]] = []
        self.persisted_candidates: list[dict[str, Any]] = []

    async def get_bills_by_date_range(
        self,
        start_date: str,
        end_date: str,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        self.date_range_calls.append((start_date, end_date, user_id))
        return list(self.existing_bills)

    async def persist_import_reconciliation_candidates(
        self,
        candidates: list[dict[str, Any]],
        *,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        self.persisted_candidates = [dict(candidate) for candidate in candidates]
        return [{**candidate, "user_id": user_id, "status": "pending"} for candidate in candidates]


@pytest.mark.asyncio
async def test_import_reconciliation_candidates_are_same_day_and_type_split() -> None:
    """New import reconciliation candidates use same-day scope and split transfer/duplicate truth."""
    engine = SmartDeduplicationEngine()
    db = FakeReconciliationDB(
        [
            {
                "id": 11,
                "date": "2026-05-01 10:00:05",
                "type": "支出",
                "amount": -50.0,
                "counterparty": "Coffee Shop",
                "description": "morning coffee",
                "payment_method": "bank",
                "source_account_id": 101,
            },
            {
                "id": 12,
                "date": "2026-05-02 10:00:05",
                "type": "支出",
                "amount": -50.0,
                "counterparty": "Coffee Shop",
                "description": "next day must not match",
                "payment_method": "bank",
                "source_account_id": 101,
            },
            {
                "id": 21,
                "date": "2026-05-01 11:00:10",
                "type": "支出",
                "amount": -100.0,
                "counterparty": "Savings",
                "description": "转出",
                "payment_method": "bank",
                "source_account_id": 201,
            },
            {
                "id": 31,
                "date": "2026-05-01 12:15:00",
                "type": "支出",
                "amount": -30.0,
                "counterparty": "too far",
                "description": "too far",
                "payment_method": "bank",
                "source_account_id": 301,
            },
        ]
    )
    imported_bills = [
        {
            "date": "2026-05-01 10:00:00",
            "type": "支出",
            "amount": -50.0,
            "counterparty": "Coffee Shop",
            "description": "morning coffee",
            "payment_method": "alipay",
            "source_account_id": 501,
            "_parser_id": "alipay",
            "_dedup_id": "dup-1",
            "session_id": "session-a1",
            "preview_id": 7001,
        },
        {
            "date": "2026-05-01 11:00:00",
            "type": "收入",
            "amount": 100.0,
            "counterparty": "Savings",
            "description": "转入",
            "payment_method": "wechat",
            "source_account_id": 502,
            "_parser_id": "wechat",
            "_dedup_id": "transfer-1",
            "session_id": "session-a1",
            "preview_id": 7002,
        },
        {
            "date": "2026-05-01 12:00:00",
            "type": "收入",
            "amount": 30.0,
            "counterparty": "too far",
            "description": "too far",
            "payment_method": "wechat",
            "source_account_id": 503,
            "_parser_id": "wechat",
            "_dedup_id": "far-1",
        },
    ]

    candidates = await engine.find_import_reconciliation_candidates(imported_bills, db, user_id=7)

    assert db.date_range_calls == [("2026-05-01", "2026-05-01", 7)]
    assert [(candidate["existing_bill_id"], candidate["candidate_type"]) for candidate in candidates] == [
        (11, "duplicate"),
        (21, "transfer"),
    ]
    assert {candidate["source_payload"]["same_day"] for candidate in candidates} == {"2026-05-01"}
    assert all(candidate["time_diff_seconds"] <= engine.TIME_TOLERANCE for candidate in candidates)
    assert [candidate["candidate_type"] for candidate in db.persisted_candidates] == ["duplicate", "transfer"]


def test_formal_transfer_candidates_keep_legacy_three_day_window() -> None:
    """Old formal-bill transfer discovery still keeps its 3-day family window."""
    anchor_bill = {
        "id": 101,
        "date": "2026-05-01 09:00:00",
        "type": "支出",
        "amount": -88.0,
        "source_account_id": 1,
    }
    within_three_days = {
        "id": 102,
        "date": "2026-05-03 08:59:00",
        "type": "收入",
        "amount": 88.0,
        "counterparty": "pytest legacy window",
        "description": "pytest legacy window",
        "payment_method": "银行卡",
        "main_category": "转账",
        "sub_category": "历史后配对",
        "source_account_id": 2,
        "destination_account_id": 0,
    }
    outside_three_days = {
        **within_three_days,
        "id": 103,
        "date": "2026-05-04 09:00:01",
        "source_account_id": 3,
    }

    result = build_transfer_pair_candidates(anchor_bill, [outside_three_days, within_three_days])

    assert [candidate["candidate_id"] for candidate in result] == ["bill:101:transfer:102"]


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_reconciliation_candidates.db"))
    await db.init_db()
    return db


async def _create_user(db: Database) -> int:
    return await db.create_user(
        {
            "username": "reconciliation_candidates",
            "email": "reconciliation_candidates@example.com",
            "password_hash": "pytest-hash",
            "nickname": "reconciliation_candidates",
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


@pytest.mark.asyncio
async def test_reconciliation_candidate_persistence_is_update_safe_and_append_audited(
    tmp_path: Path,
) -> None:
    """Candidate rows update in place while merge-ledger events stay append-only."""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db)
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-01 10:00:05",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "ledger merchant",
                "description": "ledger existing",
                "payment_method": "bank",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None

        candidate = {
            "candidate_id": f"reconcile:import:duplicate:bill:{existing_bill_id}:abc",
            "candidate_type": "duplicate",
            "session_id": "session-a1",
            "preview_id": 8001,
            "import_bill_key": "preview:8001",
            "existing_bill_id": int(existing_bill_id),
            "group_key": f"import_reconciliation:duplicate:bill:{existing_bill_id}:amount:42.00:2026-05-01",
            "amount_abs": 42.0,
            "time_diff_seconds": 5,
            "score": 0.98,
            "level": "high",
            "reason": "same_amount|same_day|time_close",
            "import_bill_snapshot": {
                "date": "2026-05-01 10:00:00",
                "amount": -42.0,
                "description": "ledger import",
            },
            "existing_bill_snapshot": {
                "id": int(existing_bill_id),
                "date": "2026-05-01 10:00:05",
                "amount": -42.0,
                "description": "ledger existing",
            },
            "source_payload": {"family": "import_reconciliation"},
        }

        first = await db.persist_import_reconciliation_candidates([candidate], user_id=user_id)
        second = await db.persist_import_reconciliation_candidates([{**candidate, "reason": "refreshed"}], user_id=user_id)
        candidates = await db.list_import_reconciliation_candidates(user_id=user_id, session_id="session-a1")

        assert first[0]["id"] == second[0]["id"]
        assert len(candidates) == 1
        assert candidates[0]["reason"] == "refreshed"
        assert candidates[0]["seen_count"] == 2

        conn = await db._get_connection()  # pylint: disable=protected-access
        async with conn.execute("SELECT COUNT(*) AS count FROM bill_merge_groups") as cursor:
            group_count = (await cursor.fetchone())["count"]
        async with conn.execute("SELECT COUNT(*) AS count FROM bill_merge_members") as cursor:
            member_count = (await cursor.fetchone())["count"]
        async with conn.execute("SELECT event_type FROM bill_merge_events ORDER BY id ASC") as cursor:
            events = [str(row["event_type"]) for row in await cursor.fetchall()]

        assert group_count == 1
        assert member_count == 2
        assert events == ["candidate_discovered", "candidate_seen"]
    finally:
        await db.close()
