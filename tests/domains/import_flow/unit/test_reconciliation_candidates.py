from __future__ import annotations

from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.bill_service import BillService
from bill_analyser.core.db import Database
from bill_analyser.core.db_time import utc_now_iso
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


async def _create_user(db: Database, suffix: str = "") -> int:
    username = f"reconciliation_candidates{suffix}"
    return await db.create_user(
        {
            "username": username,
            "email": f"{username}@example.com",
            "password_hash": "pytest-hash",
            "nickname": username,
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


@pytest.mark.asyncio
async def test_stage2_dedup_persists_session_queryable_template_reconciliation_candidate(
    tmp_path: Path,
) -> None:
    """The normal v2 stage2 path keeps session/template anchors before preview ids exist."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-stage2")
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-01 10:00:05",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "stage2 merchant",
                "description": "stage2 existing duplicate",
                "payment_method": "bank",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None

        session_id = "session-a1-stage2-anchor"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        inserted = await db.insert_parser_templates(
            session_id,
            [
                {
                    "date": "2026-05-01 10:00:00",
                    "amount": -42.0,
                    "type": "支出",
                    "description": "stage2 existing duplicate",
                    "counterparty": "stage2 merchant",
                    "payment_method": "wechat",
                    "parser_tags": ["parser:wechat"],
                }
            ],
            parser_id="wechat",
            user_id=user_id,
        )
        assert inserted == 1
        templates = await db.get_unprocessed_templates_for_dedup(session_id)
        template_id = int(templates[0]["id"])

        result = await service.import_stage2_dedup(session_id, user_id=user_id)

        assert result["success"] is True
        candidates = await db.list_import_reconciliation_candidates(
            user_id=user_id,
            session_id=session_id,
        )
        assert len(candidates) == 1
        assert candidates[0]["existing_bill_id"] == int(existing_bill_id)
        assert candidates[0]["session_id"] == session_id
        assert candidates[0]["import_bill_key"] == f"session:{session_id}:template:{template_id}"
        assert candidates[0]["import_bill_snapshot"]["session_id"] == session_id
        assert candidates[0]["import_bill_snapshot"]["template_id"] == template_id
    finally:
        await service.close()


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


@pytest.mark.asyncio
async def test_record_bill_merge_event_requires_group_ownership(tmp_path: Path) -> None:
    """Merge ledger writes must not let one user append events to another user's group."""
    db = await _create_database(tmp_path)
    try:
        owner_user_id = await _create_user(db, "-owner")
        other_user_id = await _create_user(db, "-other")
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
            user_id=owner_user_id,
        )
        assert existing_bill_id is not None
        candidate = {
            "candidate_id": f"reconcile:import:duplicate:bill:{existing_bill_id}:owner",
            "candidate_type": "duplicate",
            "session_id": "session-a1-owner",
            "preview_id": 8001,
            "import_bill_key": "preview:8001",
            "existing_bill_id": int(existing_bill_id),
            "group_key": (
                f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
                "amount:42.00:2026-05-01"
            ),
            "amount_abs": 42.0,
            "time_diff_seconds": 5,
            "score": 0.98,
            "level": "high",
            "reason": "same_amount|same_day|time_close",
            "import_bill_snapshot": {"date": "2026-05-01 10:00:00", "amount": -42.0},
            "existing_bill_snapshot": {"id": int(existing_bill_id), "amount": -42.0},
            "source_payload": {"family": "import_reconciliation"},
        }
        persisted = await db.persist_import_reconciliation_candidates(
            [candidate],
            user_id=owner_user_id,
        )
        group_id = int(persisted[0]["group_id"])

        owner_event = await db.record_bill_merge_event(
            group_id=group_id,
            candidate_id=candidate["candidate_id"],
            event_type="accepted",
            payload={"source": "pytest"},
            user_id=owner_user_id,
        )
        assert owner_event["group_id"] == group_id

        with pytest.raises(ValueError, match="Merge group not found"):
            await db.record_bill_merge_event(
                group_id=group_id,
                candidate_id=candidate["candidate_id"],
                event_type="accepted",
                payload={"source": "pytest-other-user"},
                user_id=other_user_id,
            )
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_bill_merge_member_update_backfills_bill_and_import_keys(tmp_path: Path) -> None:
    """Existing merge members can be enriched when A2 later learns stronger anchors."""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "-member")
        imported_bill_id = await db.create_bill(
            {
                "date": "2026-05-01 10:00:00",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "member merchant",
                "description": "member import",
                "payment_method": "wechat",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert imported_bill_id is not None
        candidate = {
            "family": "import_reconciliation",
            "candidate_id": "reconcile:import:duplicate:bill:42:member",
            "candidate_type": "duplicate",
            "session_id": "session-a1-member",
            "preview_id": None,
            "import_bill_key": "session:session-a1-member:template:9001",
            "existing_bill_id": 42,
            "group_key": "import_reconciliation:duplicate:bill:42:amount:42.00:2026-05-01",
            "amount_abs": 42.0,
        }
        conn = await db._get_connection()  # pylint: disable=protected-access
        now = utc_now_iso()
        group_id = await db._ensure_bill_merge_group(  # pylint: disable=protected-access
            conn,
            candidate,
            user_id=user_id,
            now=now,
        )
        await db._ensure_bill_merge_member(  # pylint: disable=protected-access
            conn,
            group_id=group_id,
            user_id=user_id,
            member_key="import:session:session-a1-member:template:9001",
            member_type="import_bill",
            role="candidate",
            snapshot={"description": "initial"},
            candidate_id="candidate-initial",
            now=now,
        )
        await db._ensure_bill_merge_member(  # pylint: disable=protected-access
            conn,
            group_id=group_id,
            user_id=user_id,
            member_key="import:session:session-a1-member:template:9001",
            member_type="import_bill",
            role="candidate",
            snapshot={"description": "backfilled"},
            bill_id=int(imported_bill_id),
            import_bill_key="session:session-a1-member:template:9001",
            candidate_id="candidate-backfilled",
            now=utc_now_iso(),
        )
        await conn.commit()

        async with conn.execute(
            """
            SELECT bill_id, import_bill_key, candidate_id, snapshot_json
            FROM bill_merge_members
            WHERE group_id = ? AND member_key = ?
            """,
            (group_id, "import:session:session-a1-member:template:9001"),
        ) as cursor:
            member = await cursor.fetchone()

        assert member is not None
        assert int(member["bill_id"]) == int(imported_bill_id)
        assert member["import_bill_key"] == "session:session-a1-member:template:9001"
        assert member["candidate_id"] == "candidate-backfilled"
        assert "backfilled" in member["snapshot_json"]
    finally:
        await db.close()
