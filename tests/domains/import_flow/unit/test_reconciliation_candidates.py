from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.bills import BillService
from bill_analyser.core.db import Database
from bill_analyser.core.database.time import utc_now_iso
from bill_analyser.core.matching import build_transfer_pair_candidates
from bill_analyser.core.matching.candidate_ids import parse_matching_candidate_id
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


def test_reconciliation_candidate_id_parser_keeps_existing_families() -> None:
    """Generic matching action dispatch can recognize reconciliation without breaking old ids."""
    reconciliation_id = "reconcile:import:duplicate:bill:42:abcdef1234567890"

    assert parse_matching_candidate_id(reconciliation_id) == {
        "scope": "reconciliation",
        "kind": "duplicate",
        "existing_bill_id": 42,
        "import_key_hash": "abcdef1234567890",
    }
    assert parse_matching_candidate_id("preview:7:learning") == {
        "scope": "preview",
        "preview_id": 7,
        "kind": "learning",
    }
    assert parse_matching_candidate_id("bill:11:transfer:12") == {
        "scope": "bill",
        "bill_id": 11,
        "kind": "transfer",
        "candidate_bill_id": 12,
    }


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


@pytest.mark.asyncio
async def test_reconciliation_accept_merges_three_members_and_clear_rolls_back_projection(
    tmp_path: Path,
) -> None:
    """Two accepted import duplicates against one formal bill share one reversible merge projection."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-merge")
        manual_tag_id = await db.create_tag({"name": "人工"}, user_id=user_id)
        alipay_tag_id = await db.create_tag({"name": "支付宝"}, user_id=user_id)
        wechat_tag_id = await db.create_tag({"name": "微信"}, user_id=user_id)
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-01 10:00:05",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "merge merchant",
                "description": "manual base|same item",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        assert await db.add_tags_to_bill(existing_bill_id, [manual_tag_id], user_id=user_id) is True

        session_id = "session-a2-merge"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        first_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-01 10:00:00",
                "preview_type": "支出",
                "preview_amount": 42.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "午餐",
                "preview_counterparty": "merge merchant",
                "preview_payment_method": "alipay",
                "preview_description": "same item|alipay detail",
                "preview_parser_id": "alipay",
                "preview_parser_tags": ["parser:alipay"],
            },
            user_id=user_id,
        )
        second_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-01 10:00:02",
                "preview_type": "支出",
                "preview_amount": 42.0,
                "preview_main_category": "餐饮",
                "preview_sub_category": "午餐",
                "preview_counterparty": "merge merchant",
                "preview_payment_method": "wechat",
                "preview_description": "wechat detail",
                "preview_parser_id": "wechat",
                "preview_parser_tags": ["parser:wechat"],
            },
            user_id=user_id,
        )

        group_key = (
            f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
            "amount:42.00:2026-05-01"
        )
        first_candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:a2first"
        second_candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:a2second"
        candidates = [
            {
                "candidate_id": first_candidate_id,
                "candidate_type": "duplicate",
                "session_id": session_id,
                "preview_id": first_preview_id,
                "import_bill_key": f"preview:{first_preview_id}",
                "existing_bill_id": int(existing_bill_id),
                "group_key": group_key,
                "amount_abs": 42.0,
                "time_diff_seconds": 5,
                "score": 0.98,
                "level": "high",
                "reason": "same_amount|same_day|time_close",
                "import_bill_snapshot": {
                    "date": "2026-05-01 10:00:00",
                    "type": "支出",
                    "amount": -42.0,
                    "description": "same item|alipay detail",
                    "parser_id": "alipay",
                    "tag_ids": [alipay_tag_id],
                },
                "existing_bill_snapshot": {
                    "id": int(existing_bill_id),
                    "date": "2026-05-01 10:00:05",
                    "type": "支出",
                    "amount": -42.0,
                    "description": "manual base|same item",
                },
                "source_payload": {"family": "import_reconciliation"},
            },
            {
                "candidate_id": second_candidate_id,
                "candidate_type": "duplicate",
                "session_id": session_id,
                "preview_id": second_preview_id,
                "import_bill_key": f"preview:{second_preview_id}",
                "existing_bill_id": int(existing_bill_id),
                "group_key": group_key,
                "amount_abs": 42.0,
                "time_diff_seconds": 3,
                "score": 0.99,
                "level": "high",
                "reason": "same_amount|same_day|time_close",
                "import_bill_snapshot": {
                    "date": "2026-05-01 10:00:02",
                    "type": "支出",
                    "amount": -42.0,
                    "description": "wechat detail",
                    "parser_id": "wechat",
                    "tag_ids": [wechat_tag_id],
                },
                "existing_bill_snapshot": {
                    "id": int(existing_bill_id),
                    "date": "2026-05-01 10:00:05",
                    "type": "支出",
                    "amount": -42.0,
                    "description": "manual base|same item",
                },
                "source_payload": {"family": "import_reconciliation"},
            },
        ]
        await db.persist_import_reconciliation_candidates(candidates, user_id=user_id)

        preview_read_model = await service.get_import_preview(
            session_id,
            selected_only=False,
            user_id=user_id,
        )
        first_preview_signal = next(
            item for item in preview_read_model if int(item["id"]) == first_preview_id
        )["matching"]["reconciliation"]
        assert first_preview_signal["candidate_id"] == first_candidate_id
        assert first_preview_signal["signal_label"] == "人工|支付宝"
        session_candidates = await service.get_matching_session_candidates(session_id, user_id=user_id)
        assert first_candidate_id in {
            candidate["candidate_id"] for candidate in session_candidates["candidates"]
        }

        first_accept = await service._accept_matching_candidate(  # pylint: disable=protected-access
            first_candidate_id,
            {},
            user_id=user_id,
        )
        assert first_accept["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual base|same item|alipay detail"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id,
            alipay_tag_id,
        ]

        second_accept = await service._accept_matching_candidate(  # pylint: disable=protected-access
            second_candidate_id,
            {},
            user_id=user_id,
        )
        assert second_accept["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual base|same item|alipay detail|wechat detail"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id,
            alipay_tag_id,
            wechat_tag_id,
        ]

        formal_read_model = await service.get_matching_bill_candidates(existing_bill_id, user_id=user_id)
        assert formal_read_model["reconciliation"]["signal_label"] == "人工|支付宝|微信"
        assert set(formal_read_model["reconciliation"]["candidate_ids"]) == {
            first_candidate_id,
            second_candidate_id,
        }

        first_clear = await service._clear_matching_candidate(  # pylint: disable=protected-access
            first_candidate_id,
            {},
            user_id=user_id,
        )
        assert first_clear["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual base|same item|wechat detail"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id,
            wechat_tag_id,
        ]
        formal_read_model = await service.get_matching_bill_candidates(existing_bill_id, user_id=user_id)
        assert formal_read_model["reconciliation"]["signal_label"] == "人工|微信"

        second_clear = await service._clear_matching_candidate(  # pylint: disable=protected-access
            second_candidate_id,
            {},
            user_id=user_id,
        )
        assert second_clear["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual base|same item"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id
        ]
        formal_read_model = await service.get_matching_bill_candidates(existing_bill_id, user_id=user_id)
        assert formal_read_model["reconciliation"] is None
    finally:
        await service.close()


@pytest.mark.asyncio
async def test_reconciliation_reject_clear_keeps_manual_confirm_default(
    tmp_path: Path,
) -> None:
    """Reject should suppress the candidate without applying projection; clear should restore pending review."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-reject")
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-02 09:00:00",
                "type": "支出",
                "amount": -16.0,
                "counterparty": "reject merchant",
                "description": "manual only",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "早餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        session_id = "session-a2-reject"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-02 09:00:02",
                "preview_type": "支出",
                "preview_amount": 16.0,
                "preview_counterparty": "reject merchant",
                "preview_payment_method": "alipay",
                "preview_description": "should not merge",
                "preview_parser_id": "alipay",
                "preview_parser_tags": ["parser:alipay"],
            },
            user_id=user_id,
        )
        candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:rejectme"
        await db.persist_import_reconciliation_candidates(
            [
                {
                    "candidate_id": candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "preview_id": preview_id,
                    "import_bill_key": f"preview:{preview_id}",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": (
                        f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
                        "amount:16.00:2026-05-02"
                    ),
                    "amount_abs": 16.0,
                    "time_diff_seconds": 2,
                    "score": 0.98,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-02 09:00:02",
                        "type": "支出",
                        "amount": -16.0,
                        "description": "should not merge",
                        "parser_id": "alipay",
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-02 09:00:00",
                        "type": "支出",
                        "amount": -16.0,
                        "description": "manual only",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                }
            ],
            user_id=user_id,
        )

        reject_result = await service._reject_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert reject_result["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual only"
        assert await service.get_matching_bill_candidates(existing_bill_id, user_id=user_id) == {
            "success": True,
            "bill_id": int(existing_bill_id),
            "linked_pair": None,
            "candidates": [],
            "reconciliation": None,
        }

        clear_result = await service._clear_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert clear_result["success"] is True
        bill_candidates = await service.get_matching_bill_candidates(existing_bill_id, user_id=user_id)
        assert [candidate["candidate_id"] for candidate in bill_candidates["candidates"]] == [candidate_id]
    finally:
        await service.close()


async def _get_preview_selected(db: Database, preview_id: int, *, user_id: int) -> bool:
    conn = await db._get_connection()  # pylint: disable=protected-access
    async with conn.execute(
        "SELECT preview_selected FROM bills_preview WHERE id = ? AND user_id = ?",
        (preview_id, user_id),
    ) as cursor:
        row = await cursor.fetchone()
    assert row is not None
    return bool(row["preview_selected"])


async def _get_merge_group_metadata(db: Database, group_key: str, *, user_id: int) -> dict[str, Any]:
    conn = await db._get_connection()  # pylint: disable=protected-access
    async with conn.execute(
        "SELECT metadata_json FROM bill_merge_groups WHERE group_key = ? AND user_id = ?",
        (group_key, user_id),
    ) as cursor:
        row = await cursor.fetchone()
    assert row is not None
    return dict(json.loads(row["metadata_json"] or "{}"))


@pytest.mark.asyncio
async def test_reconciliation_recompute_conflicts_after_manual_bill_edit(
    tmp_path: Path,
) -> None:
    """Later accept/clear must not overwrite a manually edited projected bill."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-dirty")
        manual_tag_id = await db.create_tag({"name": "人工"}, user_id=user_id)
        alipay_tag_id = await db.create_tag({"name": "支付宝"}, user_id=user_id)
        override_tag_id = await db.create_tag({"name": "人工改动"}, user_id=user_id)
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-03 10:00:00",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "dirty merchant",
                "description": "manual base",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        assert await db.add_tags_to_bill(existing_bill_id, [manual_tag_id], user_id=user_id) is True

        session_id = "session-a2-dirty"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        first_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-03 10:00:01",
                "preview_type": "支出",
                "preview_amount": 42.0,
                "preview_counterparty": "dirty merchant",
                "preview_payment_method": "alipay",
                "preview_description": "alipay detail",
                "preview_parser_id": "alipay",
            },
            user_id=user_id,
        )
        second_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-03 10:00:02",
                "preview_type": "支出",
                "preview_amount": 42.0,
                "preview_counterparty": "dirty merchant",
                "preview_payment_method": "wechat",
                "preview_description": "wechat detail",
                "preview_parser_id": "wechat",
            },
            user_id=user_id,
        )
        group_key = (
            f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
            "amount:42.00:2026-05-03"
        )
        first_candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:dirty-first"
        second_candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:dirty-second"
        await db.persist_import_reconciliation_candidates(
            [
                {
                    "candidate_id": first_candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "preview_id": first_preview_id,
                    "import_bill_key": f"preview:{first_preview_id}",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": group_key,
                    "amount_abs": 42.0,
                    "time_diff_seconds": 1,
                    "score": 0.98,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-03 10:00:01",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "alipay detail",
                        "parser_id": "alipay",
                        "tag_ids": [alipay_tag_id],
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-03 10:00:00",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "manual base",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                },
                {
                    "candidate_id": second_candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "preview_id": second_preview_id,
                    "import_bill_key": f"preview:{second_preview_id}",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": group_key,
                    "amount_abs": 42.0,
                    "time_diff_seconds": 2,
                    "score": 0.98,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-03 10:00:02",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "wechat detail",
                        "parser_id": "wechat",
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-03 10:00:00",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "manual base",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                },
            ],
            user_id=user_id,
        )

        first_accept = await service._accept_matching_candidate(  # pylint: disable=protected-access
            first_candidate_id,
            {},
            user_id=user_id,
        )
        assert first_accept["success"] is True
        assert await db.update_bill(
            existing_bill_id,
            {"description": "manual override after merge"},
            user_id=user_id,
        ) is True
        assert await db.update_bill_tags(
            existing_bill_id,
            [manual_tag_id, override_tag_id],
            user_id=user_id,
        ) is True

        second_accept = await service._accept_matching_candidate(  # pylint: disable=protected-access
            second_candidate_id,
            {},
            user_id=user_id,
        )
        first_clear = await service._clear_matching_candidate(  # pylint: disable=protected-access
            first_candidate_id,
            {},
            user_id=user_id,
        )

        assert second_accept == {
            "success": False,
            "error": "Bill changed since reconciliation projection, please refresh",
            "status_code": 409,
        }
        assert first_clear == {
            "success": False,
            "error": "Bill changed since reconciliation projection, please refresh",
            "status_code": 409,
        }
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual override after merge"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id,
            override_tag_id,
        ]
    finally:
        await service.close()


@pytest.mark.asyncio
async def test_reconciliation_persist_same_group_preserves_active_projection(
    tmp_path: Path,
) -> None:
    """Refreshing an existing group must not erase rollback/projection metadata."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-preserve-projection")
        manual_tag_id = await db.create_tag({"name": "人工"}, user_id=user_id)
        import_tag_id = await db.create_tag({"name": "支付宝"}, user_id=user_id)
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-06 10:00:00",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "preserve merchant",
                "description": "manual base",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        assert await db.add_tags_to_bill(existing_bill_id, [manual_tag_id], user_id=user_id) is True
        session_id = "session-a2-preserve-projection"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        group_key = (
            f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
            "amount:42.00:2026-05-06"
        )
        candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:preserve"
        candidate = {
            "candidate_id": candidate_id,
            "candidate_type": "duplicate",
            "session_id": session_id,
            "import_bill_key": "session:preserve:template:1",
            "existing_bill_id": int(existing_bill_id),
            "group_key": group_key,
            "amount_abs": 42.0,
            "time_diff_seconds": 1,
            "score": 0.98,
            "level": "high",
            "reason": "same_amount|same_day|time_close",
            "import_bill_snapshot": {
                "date": "2026-05-06 10:00:01",
                "type": "支出",
                "amount": -42.0,
                "description": "import detail",
                "parser_id": "alipay",
                "tag_ids": [import_tag_id],
            },
            "existing_bill_snapshot": {
                "id": int(existing_bill_id),
                "date": "2026-05-06 10:00:00",
                "type": "支出",
                "amount": -42.0,
                "description": "manual base",
            },
            "source_payload": {"family": "import_reconciliation"},
        }
        await db.persist_import_reconciliation_candidates([candidate], user_id=user_id)

        accept_result = await service._accept_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert accept_result["success"] is True
        await db.persist_import_reconciliation_candidates(
            [{**candidate, "reason": "refreshed after accept"}],
            user_id=user_id,
        )

        projection = await db.get_bill_reconciliation_projection(existing_bill_id, user_id=user_id)
        assert projection is not None
        assert projection["candidate_ids"] == [candidate_id]
        assert projection["description"] == "manual base|import detail"

        reject_result = await service._reject_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert reject_result["success"] is True
        clear_result = await service._clear_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert clear_result["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual base"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id
        ]
    finally:
        await service.close()


@pytest.mark.asyncio
async def test_reconciliation_clear_noop_rebases_future_decisions_after_manual_edit(
    tmp_path: Path,
) -> None:
    """After clear leaves no applied candidate, later decisions must use the current bill as base."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-clear-rebase")
        manual_tag_id = await db.create_tag({"name": "人工"}, user_id=user_id)
        import_tag_id = await db.create_tag({"name": "支付宝"}, user_id=user_id)
        override_tag_id = await db.create_tag({"name": "人工改动"}, user_id=user_id)
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-07 10:00:00",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "rebase merchant",
                "description": "manual base",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        assert await db.add_tags_to_bill(existing_bill_id, [manual_tag_id], user_id=user_id) is True
        session_id = "session-a2-clear-rebase"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        group_key = (
            f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
            "amount:42.00:2026-05-07"
        )
        candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:rebase"
        await db.persist_import_reconciliation_candidates(
            [
                {
                    "candidate_id": candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "import_bill_key": "session:rebase:template:1",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": group_key,
                    "amount_abs": 42.0,
                    "time_diff_seconds": 1,
                    "score": 0.98,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-07 10:00:01",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "import detail",
                        "parser_id": "alipay",
                        "tag_ids": [import_tag_id],
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-07 10:00:00",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "manual base",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                }
            ],
            user_id=user_id,
        )
        accept_result = await service._accept_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert accept_result["success"] is True
        clear_result = await service._clear_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert clear_result["success"] is True
        cleared_metadata = await _get_merge_group_metadata(db, group_key, user_id=user_id)
        assert "base_bill_snapshot" not in cleared_metadata
        assert "projection" not in cleared_metadata

        assert await db.update_bill(
            existing_bill_id,
            {"description": "manual edit after clear"},
            user_id=user_id,
        ) is True
        assert await db.update_bill_tags(
            existing_bill_id,
            [manual_tag_id, override_tag_id],
            user_id=user_id,
        ) is True

        second_accept = await service._accept_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert second_accept["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual edit after clear|import detail"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id,
            import_tag_id,
            override_tag_id,
        ]

        reject_result = await service._reject_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert reject_result["success"] is True
        clear_again_result = await service._clear_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert clear_again_result["success"] is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual edit after clear"
        assert sorted(tag["id"] for tag in await db.get_tags_for_bill(existing_bill_id, user_id=user_id)) == [
            manual_tag_id,
            override_tag_id,
        ]
    finally:
        await service.close()


@pytest.mark.asyncio
async def test_bill_reconciliation_projection_skips_newer_pending_group(
    tmp_path: Path,
) -> None:
    """A newer pending reconciliation group must not hide an older active projection."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-active-projection")
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-04 10:00:00",
                "type": "支出",
                "amount": -42.0,
                "counterparty": "projection merchant",
                "description": "manual base",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "午餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        session_id = "session-a2-projection"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        accepted_candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:active"
        pending_candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:pending"
        await db.persist_import_reconciliation_candidates(
            [
                {
                    "candidate_id": accepted_candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "import_bill_key": "session:projection:accepted",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": (
                        f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
                        "amount:42.00:2026-05-04:accepted"
                    ),
                    "amount_abs": 42.0,
                    "time_diff_seconds": 1,
                    "score": 0.98,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-04 10:00:01",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "accepted detail",
                        "parser_id": "alipay",
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-04 10:00:00",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "manual base",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                }
            ],
            user_id=user_id,
        )
        accepted = await service._accept_matching_candidate(  # pylint: disable=protected-access
            accepted_candidate_id,
            {},
            user_id=user_id,
        )
        assert accepted["success"] is True

        await db.persist_import_reconciliation_candidates(
            [
                {
                    "candidate_id": pending_candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "import_bill_key": "session:projection:pending",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": (
                        f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
                        "amount:42.00:2026-05-04:pending"
                    ),
                    "amount_abs": 42.0,
                    "time_diff_seconds": 2,
                    "score": 0.97,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-04 10:00:02",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "pending detail",
                        "parser_id": "wechat",
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-04 10:00:00",
                        "type": "支出",
                        "amount": -42.0,
                        "description": "manual base",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                }
            ],
            user_id=user_id,
        )

        projection = await db.get_bill_reconciliation_projection(existing_bill_id, user_id=user_id)

        assert projection is not None
        assert projection["status"] == "merged"
        assert projection["candidate_ids"] == [accepted_candidate_id]
        assert projection["description"] == "manual base|accepted detail"
    finally:
        await service.close()


@pytest.mark.asyncio
async def test_reconciliation_reject_merged_candidate_restores_preview_selection(
    tmp_path: Path,
) -> None:
    """Rejecting a merged reconciliation candidate should reselect its preview row."""
    db = await _create_database(tmp_path)
    service = BillService(db)
    try:
        await service.initialize()
        user_id = await _create_user(db, "-a2-reject-merged")
        existing_bill_id = await db.create_bill(
            {
                "date": "2026-05-05 09:00:00",
                "type": "支出",
                "amount": -16.0,
                "counterparty": "reject merged merchant",
                "description": "manual only",
                "payment_method": "manual",
                "main_category": "餐饮",
                "sub_category": "早餐",
                "source_account_id": 3001,
            },
            user_id=user_id,
        )
        assert existing_bill_id is not None
        session_id = "session-a2-reject-merged"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-05-05 09:00:02",
                "preview_type": "支出",
                "preview_amount": 16.0,
                "preview_counterparty": "reject merged merchant",
                "preview_payment_method": "alipay",
                "preview_description": "merged detail",
                "preview_parser_id": "alipay",
            },
            user_id=user_id,
        )
        candidate_id = f"reconcile:import:duplicate:bill:{existing_bill_id}:reject-merged"
        await db.persist_import_reconciliation_candidates(
            [
                {
                    "candidate_id": candidate_id,
                    "candidate_type": "duplicate",
                    "session_id": session_id,
                    "preview_id": preview_id,
                    "import_bill_key": f"preview:{preview_id}",
                    "existing_bill_id": int(existing_bill_id),
                    "group_key": (
                        f"import_reconciliation:duplicate:bill:{existing_bill_id}:"
                        "amount:16.00:2026-05-05"
                    ),
                    "amount_abs": 16.0,
                    "time_diff_seconds": 2,
                    "score": 0.98,
                    "level": "high",
                    "reason": "same_amount|same_day|time_close",
                    "import_bill_snapshot": {
                        "date": "2026-05-05 09:00:02",
                        "type": "支出",
                        "amount": -16.0,
                        "description": "merged detail",
                        "parser_id": "alipay",
                    },
                    "existing_bill_snapshot": {
                        "id": int(existing_bill_id),
                        "date": "2026-05-05 09:00:00",
                        "type": "支出",
                        "amount": -16.0,
                        "description": "manual only",
                    },
                    "source_payload": {"family": "import_reconciliation"},
                }
            ],
            user_id=user_id,
        )

        assert await _get_preview_selected(db, preview_id, user_id=user_id) is True
        accept_result = await service._accept_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )
        assert accept_result["success"] is True
        assert await _get_preview_selected(db, preview_id, user_id=user_id) is False

        reject_result = await service._reject_matching_candidate(  # pylint: disable=protected-access
            candidate_id,
            {},
            user_id=user_id,
        )

        assert reject_result["success"] is True
        assert await _get_preview_selected(db, preview_id, user_id=user_id) is True
        bill = await db.get_bill_by_id(existing_bill_id, user_id=user_id)
        assert bill is not None
        assert bill["description"] == "manual only"
    finally:
        await service.close()
