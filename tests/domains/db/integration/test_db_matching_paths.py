from __future__ import annotations

from typing import TYPE_CHECKING

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
async def test_db_delete_and_batch_delete_remove_related_bill_pair_links(tmp_path: Path) -> None:
    """删除单条/批量账单时，应同步清理关联的 bill_pair_links。"""
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

        await db.create_manual_transfer_pair(first_expense_bill_id, first_income_bill_id, user_id=user_id)
        await db.create_manual_transfer_pair(second_expense_bill_id, second_income_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 2

        assert await db.delete_bill(first_expense_bill_id, user_id=user_id) is True
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        deleted_count = await db.batch_delete_bills([second_expense_bill_id, second_income_bill_id], user_id=user_id)
        assert deleted_count == 2
        assert await _count_bill_pair_links(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_clear_user_transactions_and_data_remove_bill_pair_links(tmp_path: Path) -> None:
    """清空用户交易或全量业务数据时，应同步移除关联的 bill_pair_links。"""
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
        await db.create_manual_transfer_pair(first_expense_bill_id, first_income_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        clear_transactions_result = await db.clear_user_transactions(user_id=user_id)
        assert clear_transactions_result["success"] is True
        assert clear_transactions_result["deleted_count"] == 2
        assert await _count_bill_pair_links(db, user_id=user_id) == 0

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
        await db.create_manual_transfer_pair(second_expense_bill_id, second_income_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        clear_data_result = await db.clear_user_data(user_id=user_id)
        assert clear_data_result["success"] is True
        assert clear_data_result["counts"]["bills"] == 2
        assert await _count_bill_pair_links(db, user_id=user_id) == 0
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_account_transaction_clear_removes_pair_links(
    tmp_path: Path,
) -> None:
    """账户级清交易后不应残留 bill_pair_links。"""
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
        await db.create_manual_transfer_pair(expense_bill_id, income_bill_id, user_id=user_id)
        assert await _count_bill_pair_links(db, user_id=user_id) == 1

        result = await db.delete_all_transactions_by_account(source_account_id, user_id=user_id)
        assert result["success"] is True
        assert result["deleted_count"] == 1
        assert await _count_bill_pair_links(db, user_id=user_id) == 0

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
