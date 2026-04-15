from __future__ import annotations

import json
import sqlite3
from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_matching_paths.db"))
    await db.init_db()
    return db


async def _create_user(db: Database, username: str) -> int:
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


async def _create_account(db: Database, *, user_id: int, name: str) -> int:
    account_id = await db.create_account(
        {
            "name": name,
            "type": 1,
            "category": "asset",
            "currency": "CNY",
            "icon": "",
            "color": "",
            "balance": 0.0,
            "initial_balance": 0.0,
            "hidden": False,
            "display_order": 0,
            "comment": "",
            "aliases": [],
        },
        user_id=user_id,
    )
    assert account_id is not None
    return int(account_id)


async def _create_bill(
    db: Database,
    *,
    user_id: int,
    source_account_id: int,
    amount: float,
    bill_type: str,
    date: str,
    description: str,
    destination_account_id: int = 0,
    counterparty: str = "pytest-pair",
) -> int:
    bill_id = await db.create_bill(
        {
            "date": date,
            "type": bill_type,
            "amount": amount,
            "counterparty": counterparty,
            "description": description,
            "payment_method": "银行卡",
            "main_category": "转账",
            "sub_category": "历史后配对",
            "source_account_id": source_account_id,
            "destination_account_id": destination_account_id,
            "destination_amount": 0.0,
        },
        user_id=user_id,
    )
    assert bill_id is not None
    return int(bill_id)


async def _count_bill_pair_links(db: Database, *, user_id: int) -> int:
    conn = await db._get_connection()
    async with conn.execute("SELECT COUNT(*) FROM bill_pair_links WHERE user_id = ?", (user_id,)) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


async def _count_bill_transfer_pair_suppressions(db: Database, *, user_id: int) -> int:
    conn = await db._get_connection()
    async with conn.execute(
        "SELECT COUNT(*) FROM bill_transfer_pair_suppressions WHERE user_id = ?",
        (user_id,),
    ) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


async def _count_bill_investment_pair_suppressions(db: Database, *, user_id: int) -> int:
    conn = await db._get_connection()
    async with conn.execute(
        "SELECT COUNT(*) FROM bill_investment_pair_suppressions WHERE user_id = ?",
        (user_id,),
    ) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


async def _count_bill_learning_rule_suppressions(db: Database, *, user_id: int) -> int:
    conn = await db._get_connection()
    async with conn.execute(
        "SELECT COUNT(*) FROM bill_learning_rule_suppressions WHERE user_id = ?",
        (user_id,),
    ) as cursor:
        row = await cursor.fetchone()
    return int(row[0] if row else 0)


async def _list_bill_pair_feedback(
    db: Database,
    *,
    user_id: int,
    candidate_id: str | None = None,
) -> list[dict[str, Any]]:
    conn = await db._get_connection()
    if candidate_id is None:
        query = "SELECT * FROM bill_pair_feedback WHERE user_id = ? ORDER BY id ASC"
        params: tuple[object, ...] = (user_id,)
    else:
        query = "SELECT * FROM bill_pair_feedback WHERE user_id = ? AND candidate_id = ? ORDER BY id ASC"
        params = (user_id, candidate_id)

    async with conn.execute(query, params) as cursor:
        rows = await cursor.fetchall()

    feedback_rows: list[dict[str, Any]] = []
    for row in rows:
        row_dict = dict(row)
        feedback_rows.append(
            {
                **row_dict,
                "id": int(row_dict.get("id") or 0),
                "user_id": int(row_dict.get("user_id") or 0),
                "candidate_id": str(row_dict.get("candidate_id") or ""),
                "action": str(row_dict.get("action") or ""),
                "payload": json.loads(str(row_dict.get("payload_json") or "{}")),
                "created_at": str(row_dict.get("created_at") or ""),
            }
        )
    return feedback_rows


@pytest.mark.asyncio
async def test_db_record_bill_pair_feedback_persists_append_only_rows(tmp_path: Path) -> None:
    """bill_pair_feedback 应作为 append-only 事件流持久化 historical transfer/investment accept/reject。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_feedback_event_user")

        first_event = await db.record_bill_pair_feedback(
            "bill:11:transfer:12",
            "accept",
            {
                "scope": "bill",
                "kind": "transfer",
                "bill_id": 11,
                "candidate_bill_id": 12,
                "pair": {
                    "id": 91,
                    "pair_type": "transfer",
                    "source": "manual",
                    "left_bill_id": 11,
                    "right_bill_id": 12,
                },
            },
            user_id=user_id,
        )
        second_event = await db.record_bill_pair_feedback(
            "bill:11:investment:12",
            "reject",
            {
                "scope": "bill",
                "kind": "investment",
                "bill_id": 11,
                "candidate_bill_id": 12,
            },
            user_id=user_id,
        )

        feedback_rows = await _list_bill_pair_feedback(db, user_id=user_id)

        assert first_event["id"] > 0
        assert first_event["candidate_id"] == "bill:11:transfer:12"
        assert first_event["action"] == "accept"
        assert first_event["payload"]["pair"]["pair_type"] == "transfer"
        assert second_event["id"] > first_event["id"]
        assert second_event["candidate_id"] == "bill:11:investment:12"
        assert second_event["action"] == "reject"
        assert second_event["payload"]["kind"] == "investment"
        assert [row["candidate_id"] for row in feedback_rows] == [
            "bill:11:transfer:12",
            "bill:11:investment:12",
        ]
        assert [row["action"] for row in feedback_rows] == ["accept", "reject"]
        assert feedback_rows[0]["payload"]["pair"]["pair_type"] == "transfer"
        assert feedback_rows[1]["payload"] == {
            "scope": "bill",
            "kind": "investment",
            "bill_id": 11,
            "candidate_bill_id": 12,
        }
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_create_manual_transfer_pair_rolls_back_when_feedback_write_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """当 feedback 事件写入失败时，manual transfer pair 应整体回滚，避免主写与事件流失配。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_feedback_txn_user")
        source_account_id = await _create_account(db, user_id=user_id, name="feedback txn 源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="feedback txn 目标账户")
        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-51.0,
            bill_type="支出",
            date="2026-07-29 09:00:00",
            description="feedback txn anchor bill",
        )
        candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=51.0,
            bill_type="收入",
            date="2026-07-29 09:03:00",
            description="feedback txn candidate bill",
        )

        async def _raise_feedback_write_failure(*args: object, **kwargs: object) -> dict[str, object]:
            _ = (args, kwargs)
            raise sqlite3.OperationalError("feedback write failed")

        monkeypatch.setattr(db, "record_bill_pair_feedback", _raise_feedback_write_failure)

        with pytest.raises(sqlite3.OperationalError, match="feedback write failed"):
            await db.create_manual_transfer_pair(
                anchor_bill_id,
                candidate_bill_id,
                user_id=user_id,
                feedback_candidate_id=f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}",
            )

        assert await _count_bill_pair_links(db, user_id=user_id) == 0
        assert await _list_bill_pair_feedback(
            db,
            user_id=user_id,
            candidate_id=f"bill:{anchor_bill_id}:transfer:{candidate_bill_id}",
        ) == []

        candidate_result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)
        assert candidate_result["linked_pair"] is None
        assert [candidate["bill_id"] for candidate in candidate_result["candidates"]] == [candidate_bill_id]
    finally:
        await db.close()


async def _create_composite_learning_rule(
    db: Database,
    *,
    user_id: int,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
    learned_type: str,
) -> int:
    conn = await db._get_connection()
    now = "2026-07-24T00:00:00"
    rule_hash = db.build_composite_match_hash(
        parser_id=parser_id,
        counterparty=counterparty,
        description=description,
        payment_method=payment_method,
    )
    assert rule_hash is not None
    cursor = await conn.execute(
        """
        INSERT INTO import_learning_rules (
            user_id, match_type, match_value, normalized_match_value,
            learned_type, enabled, parser_id, composite_match_hash,
            match_features_json, applied_count, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            "composite",
            rule_hash,
            rule_hash,
            learned_type,
            parser_id,
            rule_hash,
            '{"counterparty": "%s", "description": "%s", "parser_id": "%s", "payment_method": "%s"}'
            % (counterparty, description, parser_id, payment_method),
            0,
            now,
            now,
        ),
    )
    await conn.commit()
    return int(cursor.lastrowid or 0)


async def _insert_bill_pair_link(
    db: Database,
    *,
    user_id: int,
    left_bill_id: int,
    right_bill_id: int,
    pair_type: str = "transfer",
    source: str = "manual",
    created_at: str = "2026-07-01T00:00:00",
    updated_at: str = "2026-07-01T00:00:00",
) -> int:
    conn = await db._get_connection()
    cursor = await conn.execute(
        """
        INSERT INTO bill_pair_links (
            user_id, pair_type, left_bill_id, right_bill_id, source, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            pair_type,
            left_bill_id,
            right_bill_id,
            source,
            created_at,
            updated_at,
        ),
    )
    await conn.commit()
    return int(cursor.lastrowid or 0)


@pytest.mark.asyncio
async def test_db_get_bill_transfer_candidates_is_user_scoped_and_excludes_invalid_or_linked_bills(
    tmp_path: Path,
) -> None:
    """历史账单 transfer 候选应只返回同用户、未配对且满足金额方向约束的账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_candidate_user")
        other_user_id = await _create_user(db, "matching_candidate_other")
        source_account_id = await _create_account(db, user_id=user_id, name="源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="目标账户")
        third_account_id = await _create_account(db, user_id=user_id, name="第三账户")
        other_account_id = await _create_account(db, user_id=other_user_id, name="其他用户账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-88.0,
            bill_type="支出",
            date="2026-07-01 10:00:00",
            description="anchor transfer bill",
        )
        near_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=88.0,
            bill_type="收入",
            date="2026-07-01 10:05:00",
            description="near candidate bill",
        )
        far_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=88.0,
            bill_type="收入",
            date="2026-07-02 10:05:00",
            description="far candidate bill",
        )
        invalid_same_sign_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=-88.0,
            bill_type="支出",
            date="2026-07-01 10:03:00",
            description="same sign bill",
        )
        invalid_other_user_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_account_id,
            amount=88.0,
            bill_type="收入",
            date="2026-07-01 10:04:00",
            description="other user candidate bill",
        )

        result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)

        assert result["bill"] is not None
        assert int(result["bill"]["id"]) == anchor_bill_id
        assert result["linked_pair"] is None
        assert [candidate["bill_id"] for candidate in result["candidates"]] == [
            near_candidate_bill_id,
            far_candidate_bill_id,
        ]
        returned_candidate_ids = {candidate["bill_id"] for candidate in result["candidates"]}
        assert invalid_same_sign_bill_id not in returned_candidate_ids
        assert invalid_other_user_bill_id not in returned_candidate_ids
        assert result["candidates"][0]["score"] >= result["candidates"][1]["score"]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_list_manual_transfer_pairs_returns_only_current_user_pairs_with_bill_summaries(
    tmp_path: Path,
) -> None:
    """pair 列表应只返回当前用户的 transfer/manual pairs，并附左右账单最小摘要。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_list_user")
        other_user_id = await _create_user(db, "matching_pair_list_other")

        source_account_id = await _create_account(db, user_id=user_id, name="列表源账户 A")
        target_account_id = await _create_account(db, user_id=user_id, name="列表目标账户 B")
        third_account_id = await _create_account(db, user_id=user_id, name="列表目标账户 C")
        fourth_account_id = await _create_account(db, user_id=user_id, name="列表目标账户 D")
        other_source_account_id = await _create_account(db, user_id=other_user_id, name="其他用户源账户")
        other_target_account_id = await _create_account(db, user_id=other_user_id, name="其他用户目标账户")

        first_left_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-31.0,
            bill_type="支出",
            date="2026-07-18 09:00:00",
            description="pair list first expense",
        )
        first_right_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=31.0,
            bill_type="收入",
            date="2026-07-18 09:02:00",
            description="pair list first income",
        )
        second_left_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-41.0,
            bill_type="支出",
            date="2026-07-18 10:00:00",
            description="pair list second expense",
        )
        second_right_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=41.0,
            bill_type="收入",
            date="2026-07-18 10:03:00",
            description="pair list second income",
        )
        auto_left_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-51.0,
            bill_type="支出",
            date="2026-07-18 11:00:00",
            description="pair list auto expense",
        )
        auto_right_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=fourth_account_id,
            amount=51.0,
            bill_type="收入",
            date="2026-07-18 11:03:00",
            description="pair list auto income",
        )
        investment_left_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-61.0,
            bill_type="支出",
            date="2026-07-18 12:00:00",
            description="pair list investment expense",
        )
        investment_right_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=61.0,
            bill_type="收入",
            date="2026-07-18 12:03:00",
            description="pair list investment income",
        )
        other_left_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_source_account_id,
            amount=-71.0,
            bill_type="支出",
            date="2026-07-18 13:00:00",
            description="pair list other expense",
        )
        other_right_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_target_account_id,
            amount=71.0,
            bill_type="收入",
            date="2026-07-18 13:03:00",
            description="pair list other income",
        )

        first_pair = await db.create_manual_transfer_pair(first_left_bill_id, first_right_bill_id, user_id=user_id)
        second_pair = await db.create_manual_transfer_pair(second_left_bill_id, second_right_bill_id, user_id=user_id)
        await _insert_bill_pair_link(
            db,
            user_id=user_id,
            left_bill_id=auto_left_bill_id,
            right_bill_id=auto_right_bill_id,
            source="auto",
            created_at="2026-07-18T11:05:00",
            updated_at="2026-07-18T11:05:00",
        )
        await _insert_bill_pair_link(
            db,
            user_id=user_id,
            left_bill_id=investment_left_bill_id,
            right_bill_id=investment_right_bill_id,
            pair_type="investment",
            source="manual",
            created_at="2026-07-18T12:05:00",
            updated_at="2026-07-18T12:05:00",
        )
        await db.create_manual_transfer_pair(other_left_bill_id, other_right_bill_id, user_id=other_user_id)

        conn = await db._get_connection()
        await conn.execute(
            "UPDATE bill_pair_links SET updated_at = ? WHERE id = ?",
            ("2026-07-18T09:05:00", int(first_pair["id"])),
        )
        await conn.execute(
            "UPDATE bill_pair_links SET updated_at = ? WHERE id = ?",
            ("2026-07-18T10:05:00", int(second_pair["id"])),
        )
        await conn.commit()

        pairs = await db.list_manual_transfer_pairs(user_id=user_id)

        assert [pair["id"] for pair in pairs] == [int(second_pair["id"]), int(first_pair["id"])]
        assert all(pair["pair_type"] == "transfer" for pair in pairs)
        assert all(pair["source"] == "manual" for pair in pairs)
        assert pairs[0]["left_bill"]["id"] == pairs[0]["left_bill_id"]
        assert pairs[0]["right_bill"]["id"] == pairs[0]["right_bill_id"]
        assert pairs[0]["left_bill"]["payment_method"] == "银行卡"
        assert pairs[0]["right_bill"]["description"] == "pair list second income"
        assert pairs[1]["left_bill"]["description"] == "pair list first expense"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_list_manual_transfer_pairs_returns_empty_list_for_user_without_pairs(
    tmp_path: Path,
) -> None:
    """没有 pair 的用户读取列表时应得到空数组。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_list_empty_user")

        pairs = await db.list_manual_transfer_pairs(user_id=user_id)

        assert pairs == []
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_manual_pair_persists_single_logical_pair_and_followup_read_hides_target(
    tmp_path: Path,
) -> None:
    """手工后配对成功后应生成单条逻辑 pair，并让后续候选读取收敛到 linked_pair。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_user")
        source_account_id = await _create_account(db, user_id=user_id, name="转出账户")
        target_account_id = await _create_account(db, user_id=user_id, name="转入账户")

        expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-66.0,
            bill_type="支出",
            date="2026-07-03 09:00:00",
            description="manual pair expense bill",
        )
        income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=66.0,
            bill_type="收入",
            date="2026-07-03 09:02:00",
            description="manual pair income bill",
        )

        pair = await db.create_manual_transfer_pair(income_bill_id, expense_bill_id, user_id=user_id)

        assert pair["pair_type"] == "transfer"
        assert pair["source"] == "manual"
        assert pair["left_bill_id"] == min(expense_bill_id, income_bill_id)
        assert pair["right_bill_id"] == max(expense_bill_id, income_bill_id)

        expense_result = await db.get_bill_transfer_candidates(expense_bill_id, user_id=user_id)
        assert expense_result["linked_pair"] is not None
        assert expense_result["linked_pair"]["other_bill_id"] == income_bill_id
        assert expense_result["candidates"] == []

        income_result = await db.get_bill_transfer_candidates(income_bill_id, user_id=user_id)
        assert income_result["linked_pair"] is not None
        assert income_result["linked_pair"]["other_bill_id"] == expense_bill_id
        assert income_result["candidates"] == []
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_delete_manual_pair_removes_link_and_restores_candidates(tmp_path: Path) -> None:
    """删除历史手工配对后，应恢复 linked_pair 为空且候选重新可见。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_delete_restore_user")
        source_account_id = await _create_account(db, user_id=user_id, name="解链源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="解链目标账户")

        expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-86.0,
            bill_type="支出",
            date="2026-07-03 11:00:00",
            description="manual pair delete expense bill",
        )
        income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=86.0,
            bill_type="收入",
            date="2026-07-03 11:03:00",
            description="manual pair delete income bill",
        )

        pair = await db.create_manual_transfer_pair(expense_bill_id, income_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        deleted_pair = await db.delete_manual_transfer_pair(int(pair["id"]), user_id=user_id)
        assert deleted_pair["id"] == int(pair["id"])
        assert await _count_bill_pair_links(db, user_id=user_id) == 0

        expense_result = await db.get_bill_transfer_candidates(expense_bill_id, user_id=user_id)
        assert expense_result["linked_pair"] is None
        assert [candidate["bill_id"] for candidate in expense_result["candidates"]] == [income_bill_id]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_manual_investment_pair_persists_single_logical_pair_and_blocks_other_pairing(
    tmp_path: Path,
) -> None:
    """historical investment accept 应写入 investment/manual pair，并让两侧账单退出后续配对池。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_investment_pair_user")
        source_account_id = await _create_account(db, user_id=user_id, name="投资转出账户")
        target_account_id = await _create_account(db, user_id=user_id, name="投资转入账户")
        third_account_id = await _create_account(db, user_id=user_id, name="投资第三账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-66.0,
            bill_type="投资",
            date="2026-07-23 09:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
            counterparty="蚂蚁财富",
        )
        candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=66.0,
            bill_type="投资",
            date="2026-07-23 09:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
            counterparty="蚂蚁财富",
        )
        extra_transfer_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=66.0,
            bill_type="收入",
            date="2026-07-23 09:05:00",
            description="额外 transfer 候选",
            counterparty="pytest transfer",
        )

        pair = await db.create_manual_investment_pair(anchor_bill_id, candidate_bill_id, user_id=user_id)

        assert pair["pair_type"] == "investment"
        assert pair["source"] == "manual"
        assert pair["left_bill_id"] == min(anchor_bill_id, candidate_bill_id)
        assert pair["right_bill_id"] == max(anchor_bill_id, candidate_bill_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        transfer_result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)
        assert transfer_result["linked_pair"] is not None
        assert transfer_result["linked_pair"]["pair_type"] == "investment"
        assert transfer_result["candidates"] == []

        investment_result = await db.get_bill_investment_candidate_bills(anchor_bill_id, user_id=user_id)
        assert investment_result["linked_pair"] is not None
        assert investment_result["linked_pair"]["pair_type"] == "investment"
        assert investment_result["candidates"] == []

        extra_transfer_result = await db.get_bill_transfer_candidates(extra_transfer_candidate_bill_id, user_id=user_id)
        assert anchor_bill_id not in {candidate["bill_id"] for candidate in extra_transfer_result["candidates"]}
        assert candidate_bill_id not in {candidate["bill_id"] for candidate in extra_transfer_result["candidates"]}

        with pytest.raises(ValueError, match="existing transfer pair"):
            await db.create_manual_transfer_pair(anchor_bill_id, extra_transfer_candidate_bill_id, user_id=user_id)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_delete_manual_pair_removes_investment_link_and_restores_candidates(tmp_path: Path) -> None:
    """删除 manual investment pair 后，应恢复 linked_pair 为空且 investment candidates 重新可见。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_investment_pair_delete_user")
        source_account_id = await _create_account(db, user_id=user_id, name="投资解链源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="投资解链目标账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-86.0,
            bill_type="投资",
            date="2026-07-23 11:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
            counterparty="蚂蚁财富",
        )
        candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=86.0,
            bill_type="投资",
            date="2026-07-23 11:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
            counterparty="蚂蚁财富",
        )

        pair = await db.create_manual_investment_pair(anchor_bill_id, candidate_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        deleted_pair = await db.delete_manual_pair(int(pair["id"]), user_id=user_id)
        assert deleted_pair["id"] == int(pair["id"])
        assert deleted_pair["pair_type"] == "investment"
        assert await _count_bill_pair_links(db, user_id=user_id) == 0

        investment_result = await db.get_bill_investment_candidate_bills(anchor_bill_id, user_id=user_id)
        assert investment_result["linked_pair"] is None
        assert [int(candidate["id"] or 0) for candidate in investment_result["candidates"]] == [candidate_bill_id]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_delete_manual_pair_is_user_scoped_and_rejects_missing_pair(tmp_path: Path) -> None:
    """删除历史手工配对应保持 user scope，并拒绝不存在的 pair。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_delete_scope_user")
        other_user_id = await _create_user(db, "matching_pair_delete_scope_other")
        source_account_id = await _create_account(db, user_id=user_id, name="删除隔离源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="删除隔离目标账户")

        expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-96.0,
            bill_type="支出",
            date="2026-07-03 12:00:00",
            description="manual pair delete scope expense bill",
        )
        income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=96.0,
            bill_type="收入",
            date="2026-07-03 12:02:00",
            description="manual pair delete scope income bill",
        )

        pair = await db.create_manual_transfer_pair(expense_bill_id, income_bill_id, user_id=user_id)

        with pytest.raises(LookupError, match="Pair not found"):
            await db.delete_manual_transfer_pair(int(pair["id"]), user_id=other_user_id)

        with pytest.raises(LookupError, match="Pair not found"):
            await db.delete_manual_transfer_pair(999999, user_id=user_id)

        assert await _count_bill_pair_links(db, user_id=user_id) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_manual_pair_rejects_reverse_duplicate_and_second_pair_for_either_bill(
    tmp_path: Path,
) -> None:
    """手工后配对应拒绝反向重复，以及任一账单再次参与第二条 transfer pair。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_conflict_user")
        source_account_id = await _create_account(db, user_id=user_id, name="账户 A")
        target_account_id = await _create_account(db, user_id=user_id, name="账户 B")
        third_account_id = await _create_account(db, user_id=user_id, name="账户 C")
        fourth_account_id = await _create_account(db, user_id=user_id, name="账户 D")

        expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-77.0,
            bill_type="支出",
            date="2026-07-04 08:00:00",
            description="pair conflict expense bill",
        )
        income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=77.0,
            bill_type="收入",
            date="2026-07-04 08:03:00",
            description="pair conflict income bill",
        )
        second_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=77.0,
            bill_type="收入",
            date="2026-07-04 08:05:00",
            description="second income candidate bill",
        )
        second_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=fourth_account_id,
            amount=-77.0,
            bill_type="支出",
            date="2026-07-04 08:06:00",
            description="second expense candidate bill",
        )

        await db.create_manual_transfer_pair(expense_bill_id, income_bill_id, user_id=user_id)

        with pytest.raises(ValueError, match="existing transfer pair"):
            await db.create_manual_transfer_pair(income_bill_id, expense_bill_id, user_id=user_id)

        with pytest.raises(ValueError, match="existing transfer pair"):
            await db.create_manual_transfer_pair(expense_bill_id, second_income_bill_id, user_id=user_id)

        with pytest.raises(ValueError, match="existing transfer pair"):
            await db.create_manual_transfer_pair(second_expense_bill_id, income_bill_id, user_id=user_id)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_reject_bill_transfer_candidate_persists_suppression_and_filters_only_rejected_logical_pair(
    tmp_path: Path,
) -> None:
    """historical transfer reject 应持久化 suppression，并只过滤被拒绝的 logical pair。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_reject_user")
        source_account_id = await _create_account(db, user_id=user_id, name="reject 源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="reject 目标账户")
        third_account_id = await _create_account(db, user_id=user_id, name="reject 第三账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-73.0,
            bill_type="支出",
            date="2026-07-19 09:00:00",
            description="reject logical pair anchor bill",
        )
        rejected_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=73.0,
            bill_type="收入",
            date="2026-07-19 09:02:00",
            description="reject logical pair candidate bill",
        )
        retained_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=73.0,
            bill_type="收入",
            date="2026-07-19 09:05:00",
            description="reject logical pair retained bill",
        )

        initial_result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)
        assert [candidate["bill_id"] for candidate in initial_result["candidates"]] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]

        suppression = await db.reject_bill_transfer_candidate(
            anchor_bill_id,
            rejected_candidate_bill_id,
            user_id=user_id,
        )

        assert suppression == {
            "left_bill_id": min(anchor_bill_id, rejected_candidate_bill_id),
            "right_bill_id": max(anchor_bill_id, rejected_candidate_bill_id),
        }
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1

        anchor_result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)
        assert anchor_result["linked_pair"] is None
        assert [candidate["bill_id"] for candidate in anchor_result["candidates"]] == [retained_candidate_bill_id]

        rejected_candidate_result = await db.get_bill_transfer_candidates(rejected_candidate_bill_id, user_id=user_id)
        assert anchor_bill_id not in {candidate["bill_id"] for candidate in rejected_candidate_result["candidates"]}

        retained_candidate_result = await db.get_bill_transfer_candidates(retained_candidate_bill_id, user_id=user_id)
        assert [candidate["bill_id"] for candidate in retained_candidate_result["candidates"]] == [anchor_bill_id]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_reject_bill_transfer_candidate_is_user_scoped_idempotent_and_blocks_manual_pair(
    tmp_path: Path,
) -> None:
    """historical transfer reject 应保持 user scope、重复 reject 幂等，并阻止后续 accept/manual-pair 绕过。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_reject_scope_user")
        other_user_id = await _create_user(db, "matching_pair_reject_scope_other")
        source_account_id = await _create_account(db, user_id=user_id, name="reject scope 源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="reject scope 目标账户")
        other_source_account_id = await _create_account(db, user_id=other_user_id, name="other reject 源账户")
        other_target_account_id = await _create_account(db, user_id=other_user_id, name="other reject 目标账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-83.0,
            bill_type="支出",
            date="2026-07-19 11:00:00",
            description="reject scope anchor bill",
        )
        candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=83.0,
            bill_type="收入",
            date="2026-07-19 11:03:00",
            description="reject scope candidate bill",
        )
        other_anchor_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_source_account_id,
            amount=-93.0,
            bill_type="支出",
            date="2026-07-19 12:00:00",
            description="other reject scope anchor bill",
        )
        other_candidate_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_target_account_id,
            amount=93.0,
            bill_type="收入",
            date="2026-07-19 12:03:00",
            description="other reject scope candidate bill",
        )

        first_suppression = await db.reject_bill_transfer_candidate(
            anchor_bill_id,
            candidate_bill_id,
            user_id=user_id,
        )
        second_suppression = await db.reject_bill_transfer_candidate(
            candidate_bill_id,
            anchor_bill_id,
            user_id=user_id,
        )

        assert first_suppression == second_suppression
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1
        assert await _count_bill_transfer_pair_suppressions(db, user_id=other_user_id) == 0

        with pytest.raises(ValueError, match="rejected for transfer pairing"):
            await db.create_manual_transfer_pair(anchor_bill_id, candidate_bill_id, user_id=user_id)

        other_result = await db.get_bill_transfer_candidates(other_anchor_bill_id, user_id=other_user_id)
        assert [candidate["bill_id"] for candidate in other_result["candidates"]] == [other_candidate_bill_id]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_reject_bill_investment_candidate_persists_suppression_and_filters_only_rejected_logical_pair(
    tmp_path: Path,
) -> None:
    """historical investment reject 应持久化 suppression，并只过滤被拒绝的 logical pair。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_investment_reject_user")
        source_account_id = await _create_account(db, user_id=user_id, name="investment reject 源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="investment reject 目标账户")
        third_account_id = await _create_account(db, user_id=user_id, name="investment reject 第三账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-73.0,
            bill_type="投资",
            date="2026-07-19 09:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
            counterparty="蚂蚁财富",
        )
        rejected_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=73.0,
            bill_type="投资",
            date="2026-07-19 09:02:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
            counterparty="蚂蚁财富",
        )
        retained_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=73.0,
            bill_type="投资",
            date="2026-07-19 09:05:00",
            description="蚂蚁财富 黄金ETF 自动定投 赎回",
            counterparty="蚂蚁财富",
        )

        initial_result = await db.get_bill_investment_candidate_bills(anchor_bill_id, user_id=user_id)
        assert [candidate["id"] for candidate in initial_result["candidates"]] == [
            rejected_candidate_bill_id,
            retained_candidate_bill_id,
        ]

        suppression = await db.reject_bill_investment_candidate(
            anchor_bill_id,
            rejected_candidate_bill_id,
            user_id=user_id,
        )

        assert suppression == {
            "left_bill_id": min(anchor_bill_id, rejected_candidate_bill_id),
            "right_bill_id": max(anchor_bill_id, rejected_candidate_bill_id),
        }
        assert await _count_bill_investment_pair_suppressions(db, user_id=user_id) == 1

        anchor_result = await db.get_bill_investment_candidate_bills(anchor_bill_id, user_id=user_id)
        assert [candidate["id"] for candidate in anchor_result["candidates"]] == [retained_candidate_bill_id]

        rejected_candidate_result = await db.get_bill_investment_candidate_bills(
            rejected_candidate_bill_id,
            user_id=user_id,
        )
        assert anchor_bill_id not in {int(candidate["id"] or 0) for candidate in rejected_candidate_result["candidates"]}

        retained_candidate_result = await db.get_bill_investment_candidate_bills(
            retained_candidate_bill_id,
            user_id=user_id,
        )
        assert [candidate["id"] for candidate in retained_candidate_result["candidates"]] == [anchor_bill_id]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_reject_bill_investment_candidate_is_user_scoped_idempotent_and_cleanup_removes_suppression(
    tmp_path: Path,
) -> None:
    """historical investment reject 应保持 user scope、重复 reject 幂等，并在清理路径中被移除。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_investment_reject_scope_user")
        other_user_id = await _create_user(db, "matching_investment_reject_scope_other")
        source_account_id = await _create_account(db, user_id=user_id, name="investment scope 源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="investment scope 目标账户")
        other_source_account_id = await _create_account(db, user_id=other_user_id, name="other investment 源账户")
        other_target_account_id = await _create_account(db, user_id=other_user_id, name="other investment 目标账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-83.0,
            bill_type="投资",
            date="2026-07-19 11:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
            counterparty="蚂蚁财富",
        )
        candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=83.0,
            bill_type="投资",
            date="2026-07-19 11:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
            counterparty="蚂蚁财富",
        )
        other_anchor_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_source_account_id,
            amount=-93.0,
            bill_type="投资",
            date="2026-07-19 12:00:00",
            description="蚂蚁财富 黄金ETF 自动定投 买入",
            counterparty="蚂蚁财富",
        )
        other_candidate_bill_id = await _create_bill(
            db,
            user_id=other_user_id,
            source_account_id=other_target_account_id,
            amount=93.0,
            bill_type="投资",
            date="2026-07-19 12:03:00",
            description="蚂蚁财富 黄金ETF 自动定投 卖出",
            counterparty="蚂蚁财富",
        )

        first_suppression = await db.reject_bill_investment_candidate(
            anchor_bill_id,
            candidate_bill_id,
            user_id=user_id,
        )
        second_suppression = await db.reject_bill_investment_candidate(
            candidate_bill_id,
            anchor_bill_id,
            user_id=user_id,
        )

        assert first_suppression == second_suppression
        assert await _count_bill_investment_pair_suppressions(db, user_id=user_id) == 1
        assert await _count_bill_investment_pair_suppressions(db, user_id=other_user_id) == 0

        with pytest.raises(LookupError, match="Bill not found"):
            await db.reject_bill_investment_candidate(anchor_bill_id, candidate_bill_id, user_id=other_user_id)

        other_result = await db.get_bill_investment_candidate_bills(other_anchor_bill_id, user_id=other_user_id)
        assert [candidate["id"] for candidate in other_result["candidates"]] == [other_candidate_bill_id]

        assert await db.delete_bill(anchor_bill_id, user_id=user_id) is True
        assert await _count_bill_investment_pair_suppressions(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_reject_bill_learning_candidate_is_user_scoped_idempotent_and_accept_clears_suppression(
    tmp_path: Path,
) -> None:
    """historical learning reject 应保持 user scope、重复 reject 幂等，accept 后应应用 rule 并保持 resolved suppression。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_learning_reject_scope_user")
        other_user_id = await _create_user(db, "matching_learning_reject_scope_other")
        source_account_id = await _create_account(db, user_id=user_id, name="learning scope 源账户")
        destination_account_id = await _create_account(db, user_id=user_id, name="learning scope 目标账户")

        category_id = await db.create_category(
            {
                "type": 1,
                "main_category": "餐饮",
                "sub_category": "午餐",
                "description": "",
                "priority": 0,
                "keywords": "",
                "hidden": False,
                "icon": "",
                "color": "",
            },
            user_id=user_id,
        )
        assert category_id is not None

        rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="pytest learning vendor",
            description="pytest learning note",
            payment_method="银行卡",
            learned_type="收入",
        )

        conn = await db._get_connection()
        await conn.execute(
            """
            UPDATE import_learning_rules
            SET learned_category_id = ?, learned_destination_account_id = ?
            WHERE id = ? AND user_id = ?
            """,
            (int(category_id), destination_account_id, rule_id, user_id),
        )
        await conn.commit()

        bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-72.5,
            bill_type="支出",
            date="2026-07-24 16:00:00",
            description="pytest learning note",
            counterparty="pytest learning vendor",
        )

        first_suppression = await db.reject_bill_learning_candidate(bill_id, rule_id, user_id=user_id)
        second_suppression = await db.reject_bill_learning_candidate(bill_id, rule_id, user_id=user_id)

        assert first_suppression == second_suppression == {"bill_id": bill_id, "rule_id": rule_id}
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 1

        with pytest.raises(LookupError):
            await db.reject_bill_learning_candidate(bill_id, rule_id, user_id=other_user_id)

        with pytest.raises(ValueError, match="Learning candidate not applicable"):
            await db.accept_bill_learning_candidate(bill_id, rule_id, user_id=user_id)

        accepted_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-73.5,
            bill_type="支出",
            date="2026-07-24 16:10:00",
            description="pytest learning note",
            counterparty="pytest learning vendor",
        )

        accepted = await db.accept_bill_learning_candidate(accepted_bill_id, rule_id, user_id=user_id)
        assert accepted["bill_id"] == accepted_bill_id
        assert accepted["rule_id"] == rule_id
        assert accepted["bill"]["type"] == "收入"
        assert accepted["bill"]["main_category"] == "餐饮"
        assert accepted["bill"]["sub_category"] == "午餐"
        assert int(accepted["bill"]["destination_account_id"] or 0) == destination_account_id
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 2

        with pytest.raises(ValueError, match="Learning candidate not applicable"):
            await db.accept_bill_learning_candidate(accepted_bill_id, rule_id, user_id=user_id)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_accept_bill_learning_candidate_can_resolve_noop_candidate_once(tmp_path: Path) -> None:
    """当 historical learning rule 不产生字段变化时，首次 accept 仍应记为 resolved，重复 accept 应失败。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_learning_accept_noop_user")
        source_account_id = await _create_account(db, user_id=user_id, name="learning noop 源账户")
        rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="pytest learning noop vendor",
            description="pytest learning noop note",
            payment_method="银行卡",
            learned_type="支出",
        )
        bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-21.5,
            bill_type="支出",
            date="2026-07-26 09:00:00",
            description="pytest learning noop note",
            counterparty="pytest learning noop vendor",
        )

        accepted = await db.accept_bill_learning_candidate(bill_id, rule_id, user_id=user_id)

        assert accepted["bill_id"] == bill_id
        assert accepted["rule_id"] == rule_id
        assert accepted["bill"]["type"] == "支出"
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 1

        with pytest.raises(ValueError, match="Learning candidate not applicable"):
            await db.accept_bill_learning_candidate(bill_id, rule_id, user_id=user_id)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_accept_bill_learning_candidate_does_not_clear_accounts_for_zero_rule_values(tmp_path: Path) -> None:
    """当 learning rule 的账户字段为 0/空值语义时，historical accept 不应把正式账单账户清空。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_learning_zero_account_user")
        source_account_id = await _create_account(db, user_id=user_id, name="learning zero 源账户")
        destination_account_id = await _create_account(db, user_id=user_id, name="learning zero 目标账户")
        rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="pytest learning zero vendor",
            description="pytest learning zero note",
            payment_method="银行卡",
            learned_type="收入",
        )

        conn = await db._get_connection()
        await conn.execute(
            """
            UPDATE import_learning_rules
            SET learned_destination_account_id = 0
            WHERE id = ? AND user_id = ?
            """,
            (rule_id, user_id),
        )
        await conn.commit()

        bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            destination_account_id=destination_account_id,
            amount=-26.5,
            bill_type="支出",
            date="2026-07-26 11:00:00",
            description="pytest learning zero note",
            counterparty="pytest learning zero vendor",
        )

        accepted = await db.accept_bill_learning_candidate(bill_id, rule_id, user_id=user_id)

        assert accepted["bill_id"] == bill_id
        assert accepted["bill"]["type"] == "收入"
        assert int(accepted["bill"]["destination_account_id"] or 0) == destination_account_id
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_accept_bill_learning_candidate_ignores_stale_account_ids(tmp_path: Path) -> None:
    """historical learning accept 遇到已失效账户 ID 时，不应把悬空账户写回正式账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_learning_stale_account_user")
        source_account_id = await _create_account(db, user_id=user_id, name="learning stale 源账户")
        destination_account_id = await _create_account(db, user_id=user_id, name="learning stale 目标账户")
        rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="pytest learning stale vendor",
            description="pytest learning stale note",
            payment_method="银行卡",
            learned_type="收入",
        )

        conn = await db._get_connection()
        await conn.execute(
            """
            UPDATE import_learning_rules
            SET learned_source_account_id = ?,
                learned_destination_account_id = ?
            WHERE id = ? AND user_id = ?
            """,
            (999999, 999998, rule_id, user_id),
        )
        await conn.commit()

        bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            destination_account_id=destination_account_id,
            amount=-18.5,
            bill_type="支出",
            date="2026-07-26 12:00:00",
            description="pytest learning stale note",
            counterparty="pytest learning stale vendor",
        )

        accepted = await db.accept_bill_learning_candidate(bill_id, rule_id, user_id=user_id)

        assert accepted["bill_id"] == bill_id
        assert accepted["bill"]["type"] == "收入"
        assert int(accepted["bill"]["source_account_id"] or 0) == source_account_id
        assert int(accepted["bill"]["destination_account_id"] or 0) == destination_account_id
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_accept_and_reject_bill_learning_candidate_reject_stale_expected_rule_revision(tmp_path: Path) -> None:
    """historical learning accept/reject 应在事务内拒绝 stale expected_rule_revision。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_learning_stale_revision_user")
        source_account_id = await _create_account(db, user_id=user_id, name="learning stale revision 源账户")
        rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="pytest learning stale revision vendor",
            description="pytest learning stale revision note",
            payment_method="银行卡",
            learned_type="收入",
        )
        bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-28.5,
            bill_type="支出",
            date="2026-07-26 12:30:00",
            description="pytest learning stale revision note",
            counterparty="pytest learning stale revision vendor",
        )

        with pytest.raises(ValueError, match="Learning candidate not available"):
            await db.accept_bill_learning_candidate(
                bill_id,
                rule_id,
                user_id=user_id,
                expected_rule_revision="stale-revision",
            )

        with pytest.raises(ValueError, match="Learning candidate not available"):
            await db.reject_bill_learning_candidate(
                bill_id,
                rule_id,
                user_id=user_id,
                expected_rule_revision="stale-revision",
            )
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_delete_and_clear_paths_remove_learning_rule_suppressions(tmp_path: Path) -> None:
    """删除账单、批量删除、账户清交易与清空用户交易时，应同步移除 learning suppression。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_learning_cleanup_user")
        account_id = await _create_account(db, user_id=user_id, name="learning cleanup 账户")
        second_account_id = await _create_account(db, user_id=user_id, name="learning cleanup 第二账户")

        first_rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="cleanup vendor one",
            description="cleanup note one",
            payment_method="银行卡",
            learned_type="支出",
        )
        second_rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="cleanup vendor two",
            description="cleanup note two",
            payment_method="银行卡",
            learned_type="支出",
        )

        delete_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=account_id,
            amount=-11.0,
            bill_type="支出",
            date="2026-07-25 09:00:00",
            description="cleanup note one",
            counterparty="cleanup vendor one",
        )
        batch_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=account_id,
            amount=-12.0,
            bill_type="支出",
            date="2026-07-25 09:05:00",
            description="cleanup note two",
            counterparty="cleanup vendor two",
        )

        await db.reject_bill_learning_candidate(delete_bill_id, first_rule_id, user_id=user_id)
        await db.reject_bill_learning_candidate(batch_bill_id, second_rule_id, user_id=user_id)
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 2

        assert await db.delete_bill(delete_bill_id, user_id=user_id) is True
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 1

        deleted_count = await db.batch_delete_bills([batch_bill_id], user_id=user_id)
        assert deleted_count == 1
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 0

        account_clear_rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="cleanup vendor three",
            description="cleanup note three",
            payment_method="银行卡",
            learned_type="支出",
        )
        account_clear_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=second_account_id,
            amount=-13.0,
            bill_type="支出",
            date="2026-07-25 09:10:00",
            description="cleanup note three",
            counterparty="cleanup vendor three",
        )
        await db.reject_bill_learning_candidate(account_clear_bill_id, account_clear_rule_id, user_id=user_id)
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 1

        account_clear_result = await db.delete_all_transactions_by_account(second_account_id, user_id=user_id)
        assert account_clear_result["success"] is True
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 0

        clear_rule_id = await _create_composite_learning_rule(
            db,
            user_id=user_id,
            parser_id="wechat",
            counterparty="cleanup vendor four",
            description="cleanup note four",
            payment_method="银行卡",
            learned_type="支出",
        )
        clear_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=account_id,
            amount=-14.0,
            bill_type="支出",
            date="2026-07-25 09:15:00",
            description="cleanup note four",
            counterparty="cleanup vendor four",
        )
        await db.reject_bill_learning_candidate(clear_bill_id, clear_rule_id, user_id=user_id)
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 1

        clear_transactions_result = await db.clear_user_transactions(user_id=user_id)
        assert clear_transactions_result["success"] is True
        assert await _count_bill_learning_rule_suppressions(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_delete_and_batch_delete_remove_related_bill_pair_links(tmp_path: Path) -> None:
    """删除单条/批量账单时，应同步清理关联的 bill_pair_links 与 suppression。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_delete_user")
        source_account_id = await _create_account(db, user_id=user_id, name="删除账户 A")
        target_account_id = await _create_account(db, user_id=user_id, name="删除账户 B")
        third_account_id = await _create_account(db, user_id=user_id, name="删除账户 C")

        first_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-18.0,
            bill_type="支出",
            date="2026-07-05 09:00:00",
            description="delete pair expense bill",
        )
        first_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=18.0,
            bill_type="收入",
            date="2026-07-05 09:02:00",
            description="delete pair income bill",
        )
        second_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-28.0,
            bill_type="支出",
            date="2026-07-05 10:00:00",
            description="batch delete pair expense bill",
        )
        second_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=28.0,
            bill_type="收入",
            date="2026-07-05 10:03:00",
            description="batch delete pair income bill",
        )
        suppressed_delete_anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-19.0,
            bill_type="支出",
            date="2026-07-05 10:10:00",
            description="delete suppression anchor bill",
        )
        suppressed_delete_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=19.0,
            bill_type="收入",
            date="2026-07-05 10:12:00",
            description="delete suppression candidate bill",
        )
        suppressed_batch_anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-29.0,
            bill_type="支出",
            date="2026-07-05 10:20:00",
            description="batch delete suppression anchor bill",
        )
        suppressed_batch_candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=third_account_id,
            amount=29.0,
            bill_type="收入",
            date="2026-07-05 10:23:00",
            description="batch delete suppression candidate bill",
        )

        await db.create_manual_transfer_pair(first_expense_bill_id, first_income_bill_id, user_id=user_id)
        await db.create_manual_transfer_pair(second_expense_bill_id, second_income_bill_id, user_id=user_id)
        await db.reject_bill_transfer_candidate(
            suppressed_delete_anchor_bill_id,
            suppressed_delete_candidate_bill_id,
            user_id=user_id,
        )
        await db.reject_bill_transfer_candidate(
            suppressed_batch_anchor_bill_id,
            suppressed_batch_candidate_bill_id,
            user_id=user_id,
        )
        assert await _count_bill_pair_links(db, user_id=user_id) == 2
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 2

        assert await db.delete_bill(first_expense_bill_id, user_id=user_id) is True
        assert await _count_bill_pair_links(db, user_id=user_id) == 1
        assert await db.delete_bill(suppressed_delete_anchor_bill_id, user_id=user_id) is True
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1

        deleted_count = await db.batch_delete_bills(
            [
                second_expense_bill_id,
                second_income_bill_id,
                suppressed_batch_anchor_bill_id,
                suppressed_batch_candidate_bill_id,
            ],
            user_id=user_id,
        )
        assert deleted_count == 4
        assert await _count_bill_pair_links(db, user_id=user_id) == 0
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_clear_user_transactions_and_data_remove_bill_pair_links(tmp_path: Path) -> None:
    """清空用户交易或全量业务数据时，应同步移除关联的 bill_pair_links 与 suppression。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_clear_user")
        source_account_id = await _create_account(db, user_id=user_id, name="清理账户 A")
        target_account_id = await _create_account(db, user_id=user_id, name="清理账户 B")

        first_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-38.0,
            bill_type="支出",
            date="2026-07-06 08:00:00",
            description="clear transactions expense bill",
        )
        first_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=38.0,
            bill_type="收入",
            date="2026-07-06 08:03:00",
            description="clear transactions income bill",
        )
        suppressed_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-39.0,
            bill_type="支出",
            date="2026-07-06 08:10:00",
            description="clear transactions suppression expense bill",
        )
        suppressed_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=39.0,
            bill_type="收入",
            date="2026-07-06 08:13:00",
            description="clear transactions suppression income bill",
        )
        await db.create_manual_transfer_pair(first_expense_bill_id, first_income_bill_id, user_id=user_id)
        await db.reject_bill_transfer_candidate(
            suppressed_expense_bill_id,
            suppressed_income_bill_id,
            user_id=user_id,
        )
        assert await _count_bill_pair_links(db, user_id=user_id) == 1
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1

        clear_transactions_result = await db.clear_user_transactions(user_id=user_id)
        assert clear_transactions_result["success"] is True
        assert clear_transactions_result["deleted_count"] == 4
        assert await _count_bill_pair_links(db, user_id=user_id) == 0
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 0

        second_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-48.0,
            bill_type="支出",
            date="2026-07-06 09:00:00",
            description="clear data expense bill",
        )
        second_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=48.0,
            bill_type="收入",
            date="2026-07-06 09:02:00",
            description="clear data income bill",
        )
        second_suppressed_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-49.0,
            bill_type="支出",
            date="2026-07-06 09:10:00",
            description="clear data suppression expense bill",
        )
        second_suppressed_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=49.0,
            bill_type="收入",
            date="2026-07-06 09:13:00",
            description="clear data suppression income bill",
        )
        await db.create_manual_transfer_pair(second_expense_bill_id, second_income_bill_id, user_id=user_id)
        await db.reject_bill_transfer_candidate(
            second_suppressed_expense_bill_id,
            second_suppressed_income_bill_id,
            user_id=user_id,
        )
        assert await _count_bill_pair_links(db, user_id=user_id) == 1
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1

        clear_data_result = await db.clear_user_data(user_id=user_id)
        assert clear_data_result["success"] is True
        assert clear_data_result["counts"]["bills"] == 4
        assert await _count_bill_pair_links(db, user_id=user_id) == 0
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_account_transaction_clear_removes_pair_links(
    tmp_path: Path,
) -> None:
    """账户级清交易后不应残留 bill_pair_links 与 suppression。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_account_clear_user")
        source_account_id = await _create_account(db, user_id=user_id, name="账户清理源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="账户清理目标账户")

        expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-58.0,
            bill_type="支出",
            date="2026-07-07 08:00:00",
            description="account clear expense bill",
        )
        income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=58.0,
            bill_type="收入",
            date="2026-07-07 08:03:00",
            description="account clear income bill",
        )
        suppressed_expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-59.0,
            bill_type="支出",
            date="2026-07-07 08:10:00",
            description="account clear suppression expense bill",
        )
        suppressed_income_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=59.0,
            bill_type="收入",
            date="2026-07-07 08:13:00",
            description="account clear suppression income bill",
        )
        await db.create_manual_transfer_pair(expense_bill_id, income_bill_id, user_id=user_id)
        await db.reject_bill_transfer_candidate(
            suppressed_expense_bill_id,
            suppressed_income_bill_id,
            user_id=user_id,
        )
        assert await _count_bill_pair_links(db, user_id=user_id) == 1
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1

        result = await db.delete_all_transactions_by_account(source_account_id, user_id=user_id)
        assert result["success"] is True
        assert result["deleted_count"] == 2
        assert await _count_bill_pair_links(db, user_id=user_id) == 0
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 0

        income_result = await db.get_bill_transfer_candidates(income_bill_id, user_id=user_id)
        assert income_result["linked_pair"] is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_deduplicate_removes_pair_links_for_deleted_duplicate_bill(tmp_path: Path) -> None:
    """去重删除重复账单时，应同步移除引用被删账单的 bill_pair_links。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_deduplicate_user")
        source_account_id = await _create_account(db, user_id=user_id, name="去重源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="去重目标账户")

        expense_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-68.0,
            bill_type="支出",
            date="2026-07-08 08:00:00",
            description="deduplicate pair expense bill",
        )

        conn = await db._get_connection()
        await conn.execute(
            "UPDATE bills SET hash = ? WHERE id = ?",
            ("deduplicate-pair-expense-hash", expense_bill_id),
        )
        await conn.executemany(
            """
            INSERT INTO bills (
                user_id, date, type, amount, counterparty, description,
                payment_method, main_category, sub_category, source_account_id,
                destination_account_id, destination_amount, hash, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            [
                (
                    user_id,
                    "2026-07-08 08:02:00",
                    "收入",
                    68.0,
                    "deduplicate income bill",
                    "deduplicate income bill",
                    "银行卡",
                    "转账",
                    "历史后配对",
                    target_account_id,
                    0,
                    0.0,
                    None,
                    "2026-07-08T08:02:00",
                    "2026-07-08T08:02:00",
                ),
                (
                    user_id,
                    "2026-07-08 08:02:00",
                    "收入",
                    68.0,
                    "deduplicate income bill",
                    "deduplicate income bill",
                    "银行卡",
                    "转账",
                    "历史后配对",
                    target_account_id,
                    0,
                    0.0,
                    None,
                    "2026-07-08T08:03:00",
                    "2026-07-08T08:03:00",
                ),
            ],
        )
        await conn.commit()

        duplicate_bills = await db.get_bills(filters={"counterparty": "deduplicate income bill"}, user_id=user_id)
        assert len(duplicate_bills) == 2
        kept_bill_id = min(int(bill["id"]) for bill in duplicate_bills)
        deleted_bill_id = max(int(bill["id"]) for bill in duplicate_bills)

        await db.create_manual_transfer_pair(expense_bill_id, deleted_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        deleted_count = await db.deduplicate()
        assert deleted_count == 1
        assert await _count_bill_pair_links(db, user_id=user_id) == 0

        remaining_duplicates = await db.get_bills(
            filters={"counterparty": "deduplicate income bill"},
            user_id=user_id,
        )
        assert [int(bill["id"]) for bill in remaining_duplicates] == [kept_bill_id]
        expense_result = await db.get_bill_transfer_candidates(expense_bill_id, user_id=user_id)
        assert expense_result["linked_pair"] is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_deduplicate_removes_transfer_pair_suppressions_for_deleted_duplicate_bill(
    tmp_path: Path,
) -> None:
    """去重删除重复账单时，应同步移除引用被删账单的 suppression。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_pair_suppression_deduplicate_user")
        source_account_id = await _create_account(db, user_id=user_id, name="去重 suppression 源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="去重 suppression 目标账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-69.0,
            bill_type="支出",
            date="2026-07-08 09:00:00",
            description="deduplicate suppression anchor bill",
        )

        conn = await db._get_connection()
        await conn.executemany(
            """
            INSERT INTO bills (
                user_id, date, type, amount, counterparty, description,
                payment_method, main_category, sub_category, source_account_id,
                destination_account_id, destination_amount, hash, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            [
                (
                    user_id,
                    "2026-07-08 09:02:00",
                    "收入",
                    69.0,
                    "deduplicate suppression income bill",
                    "deduplicate suppression income bill",
                    "银行卡",
                    "转账",
                    "历史后配对",
                    target_account_id,
                    0,
                    0.0,
                    None,
                    "2026-07-08T09:02:00",
                    "2026-07-08T09:02:00",
                ),
                (
                    user_id,
                    "2026-07-08 09:02:00",
                    "收入",
                    69.0,
                    "deduplicate suppression income bill",
                    "deduplicate suppression income bill",
                    "银行卡",
                    "转账",
                    "历史后配对",
                    target_account_id,
                    0,
                    0.0,
                    None,
                    "2026-07-08T09:03:00",
                    "2026-07-08T09:03:00",
                ),
            ],
        )
        await conn.commit()

        duplicate_bills = await db.get_bills(
            filters={"counterparty": "deduplicate suppression income bill"},
            user_id=user_id,
        )
        assert len(duplicate_bills) == 2
        deleted_bill_id = max(int(bill["id"]) for bill in duplicate_bills)

        await db.reject_bill_transfer_candidate(anchor_bill_id, deleted_bill_id, user_id=user_id)
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 1

        deleted_count = await db.deduplicate()
        assert deleted_count == 1
        assert await _count_bill_transfer_pair_suppressions(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_matching_candidates_support_slash_and_cn_date_formats(tmp_path: Path) -> None:
    """历史 matching 应兼容验证器已接受的 `/` 与中文日期格式。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_date_formats_user")
        source_account_id = await _create_account(db, user_id=user_id, name="日期格式源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="日期格式目标账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-98.0,
            bill_type="支出",
            date="2026/07/09 08:00:00",
            description="slash format anchor",
        )
        candidate_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=98.0,
            bill_type="收入",
            date="2026年07月09日 08:04:00",
            description="cn format candidate",
        )

        result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)

        assert result["linked_pair"] is None
        assert [candidate["bill_id"] for candidate in result["candidates"]] == [candidate_bill_id]
        pair = await db.create_manual_transfer_pair(anchor_bill_id, candidate_bill_id, user_id=user_id)
        assert pair["left_bill_id"] == min(anchor_bill_id, candidate_bill_id)
        assert pair["right_bill_id"] == max(anchor_bill_id, candidate_bill_id)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_manual_pair_rejects_bills_already_typed_as_transfer(tmp_path: Path) -> None:
    """历史后配对读写都应拒绝显式 type=转账 的正式账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "matching_explicit_transfer_type_user")
        source_account_id = await _create_account(db, user_id=user_id, name="显式转账源账户")
        target_account_id = await _create_account(db, user_id=user_id, name="显式转账目标账户")

        anchor_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=source_account_id,
            amount=-128.0,
            bill_type="支出",
            date="2026-07-17 10:00:00",
            description="explicit transfer type anchor",
        )
        transfer_typed_bill_id = await _create_bill(
            db,
            user_id=user_id,
            source_account_id=target_account_id,
            amount=128.0,
            bill_type="转账",
            date="2026-07-17 10:02:00",
            description="explicit transfer type candidate",
        )

        result = await db.get_bill_transfer_candidates(anchor_bill_id, user_id=user_id)
        assert result["candidates"] == []

        with pytest.raises(ValueError, match="not eligible"):
            await db.create_manual_transfer_pair(anchor_bill_id, transfer_typed_bill_id, user_id=user_id)
    finally:
        await db.close()
