from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_bills_edge_paths.db"))
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


async def _create_bill(db: Database, *, user_id: int, **overrides: Any) -> int:
    payload = {
        "date": "2026-04-07 12:00:00",
        "type": "支出",
        "amount": -18.5,
        "counterparty": "边缘路径商户",
        "description": "边缘路径描述",
        "payment_method": "支付宝",
        "main_category": "餐饮",
        "sub_category": "午餐",
    }
    payload.update(overrides)
    bill_id = await db.create_bill(payload, user_id=user_id)
    assert bill_id is not None
    return int(bill_id)


@pytest.mark.asyncio
async def test_insert_bill_alias_mapping_duplicate_check_and_date_range_paths(tmp_path: Path) -> None:
    """账单包装 helper 应覆盖 alias 映射、重复检查与日期范围查询。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_bills_edge_insert")

        assert await db.insert_bills([], user_id=user_id) == 0

        aliased_bill_id = await db.insert_bill(
            {
                "date": "2026-04-07 08:00:00",
                "type": "支出",
                "amount": -20.0,
                "counterparty": "早餐店",
                "description": "豆浆油条",
                "channel": "云闪付",
                "category": "早餐",
                "sub_category": "工作日",
            },
            user_id=user_id,
        )
        assert aliased_bill_id > 0

        stored_bill = await db.get_bill_by_id(aliased_bill_id, user_id=user_id)
        assert stored_bill is not None
        assert stored_bill["payment_method"] == "云闪付"
        assert stored_bill["main_category"] == "早餐"

        duplicate_payload = {
            "date": "2026-04-07 09:00:00",
            "type": "支出",
            "amount": -12.5,
            "counterparty": "奶茶店",
            "description": "午后奶茶",
        }
        assert await db.check_duplicate(duplicate_payload, user_id=user_id) is False
        inserted_count = await db.insert_bills([duplicate_payload], batch_id="dup-batch", user_id=user_id)
        assert inserted_count == 1
        assert await db.insert_bills([duplicate_payload], batch_id="dup-batch", user_id=user_id) == 0
        assert await db.check_duplicate(duplicate_payload, user_id=user_id) is True

        range_results = await db.get_bills_by_date_range("2026-04-07", "2026-04-07", user_id=user_id)
        assert [bill["description"] for bill in range_results] == ["豆浆油条", "午后奶茶"]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_create_update_and_batch_delete_guard_paths_cover_missing_required_and_empty_inputs(
    tmp_path: Path,
) -> None:
    """create/update/delete 的 guard 路径应返回安全默认值并保持用户隔离。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_bills_edge_guards")
        other_user_id = await _create_user(db, "db_bills_edge_other")

        assert await db.create_bill(
            {
                "date": "2026-04-07 12:00:00",
                "type": "支出",
                "amount": -8.0,
                "counterparty": "缺字段商户",
            },
            user_id=user_id,
        ) is None
        assert await db.update_bill(999999, {}, user_id=user_id) is False
        assert await db.batch_delete_bills([], user_id=user_id) == 0

        channel_bill_id = await db.create_bill(
            {
                "date": "2026-04-07 13:00:00",
                "type": "支出",
                "amount": -16.0,
                "counterparty": "银行卡商户",
                "description": "create_bill channel alias",
                "channel": "储蓄卡",
            },
            user_id=user_id,
        )
        assert channel_bill_id is not None
        channel_bill = await db.get_bill_by_id(int(channel_bill_id), user_id=user_id)
        assert channel_bill is not None
        assert channel_bill["payment_method"] == "储蓄卡"

        first_bill_id = await _create_bill(db, user_id=user_id, description="待删除账单一")
        second_bill_id = await _create_bill(db, user_id=user_id, description="待删除账单二")
        other_user_bill_id = await _create_bill(db, user_id=other_user_id, description="其他用户账单")

        deleted_count = await db.batch_delete_bills(
            [first_bill_id, second_bill_id, other_user_bill_id, 999999],
            user_id=user_id,
        )
        assert deleted_count == 2
        assert await db.get_bill_by_id(first_bill_id, user_id=user_id) is None
        assert await db.get_bill_by_id(second_bill_id, user_id=user_id) is None
        assert await db.get_bill_by_id(other_user_bill_id, user_id=other_user_id) is not None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_deduplicate_removes_duplicate_null_hash_rows(tmp_path: Path) -> None:
    """deduplicate() 应能清理多条 legacy NULL hash 重复账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_bills_edge_dedup")
        conn = await db._get_connection()
        await conn.executemany(
            """
            INSERT INTO bills (
                user_id, date, type, amount, counterparty, description,
                payment_method, main_category, sub_category, batch_id, hash,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            [
                (
                    user_id,
                    "2026-04-07 09:00:00",
                    "支出",
                    -18.0,
                    "legacy duplicate",
                    "legacy duplicate",
                    "支付宝",
                    "餐饮",
                    "早餐",
                    "legacy-batch",
                    None,
                    "2026-04-07T09:00:00",
                    "2026-04-07T09:00:00",
                ),
                (
                    user_id,
                    "2026-04-07 09:00:00",
                    "支出",
                    -18.0,
                    "legacy duplicate",
                    "legacy duplicate",
                    "支付宝",
                    "餐饮",
                    "早餐",
                    "legacy-batch",
                    None,
                    "2026-04-07T09:01:00",
                    "2026-04-07T09:01:00",
                ),
            ],
        )
        await conn.commit()

        deleted_count = await db.deduplicate()
        assert deleted_count == 1

        duplicate_rows = await db.get_bills(filters={"counterparty": "legacy duplicate"}, user_id=user_id)
        assert len(duplicate_rows) == 1
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_get_ml_training_data_returns_only_classified_bills_for_requested_user(tmp_path: Path) -> None:
    """ML 训练数据应只返回当前用户已分类且主分类非空的账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_bills_edge_ml")
        other_user_id = await _create_user(db, "db_bills_edge_ml_other")

        await _create_bill(
            db,
            user_id=user_id,
            date="2026-04-07 08:00:00",
            counterparty="训练商户A",
            description="训练描述A",
            main_category="餐饮",
            sub_category="早餐",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-04-07 09:00:00",
            counterparty="未分类商户",
            description="未分类描述",
            main_category="",
            sub_category="",
        )
        await _create_bill(
            db,
            user_id=other_user_id,
            date="2026-04-07 10:00:00",
            counterparty="其他用户商户",
            description="其他用户描述",
            main_category="交通",
            sub_category="地铁",
        )

        training_rows = await db.get_ml_training_data(user_id=user_id)
        assert training_rows == [
            {
                "counterparty": "训练商户A",
                "description": "训练描述A",
                "main_category": "餐饮",
                "sub_category": "早餐",
            }
        ]

        stats = await db.get_statistics()
        assert stats["total_bills"] == 3
        assert stats["by_type"]["支出"]["count"] == 3
        assert stats["by_type"]["支出"]["total"] == pytest.approx(-55.5)
        assert stats["by_category"]["餐饮"]["count"] == 1
        assert stats["by_category"]["交通"]["count"] == 1
        assert stats["by_category"][""]["count"] == 1
    finally:
        await db.close()
