"""DB integration coverage for tag-domain no-op and false-return branches."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

# pylint: disable=duplicate-code

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_tag_edge_paths.db"))
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


async def _create_bill(db: Database, *, user_id: int) -> int:
    bill_id = await db.create_bill(
        {
            "date": "2026-04-03 12:00:00",
            "type": "支出",
            "amount": -25.0,
            "counterparty": "标签分支测试商户",
            "description": "标签边角账单",
            "payment_method": "支付宝",
            "main_category": "标签测试",
            "sub_category": "默认",
            "source_account_id": 0,
            "destination_account_id": 0,
            "destination_amount": 0.0,
        },
        user_id=user_id,
    )
    assert bill_id is not None
    return int(bill_id)


@pytest.mark.asyncio
async def test_tag_edge_paths_cover_false_and_noop_branches(tmp_path: Path) -> None:
    """标签域应覆盖空输入 no-op 和缺失记录的 False 返回分支。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "tag_edge_user")
        bill_id = await _create_bill(db, user_id=user_id)
        tag_id = await db.create_tag({"name": "边角标签", "color": "#123456"}, user_id=user_id)
        assert tag_id > 0

        assert await db.update_tag(tag_id, {}, user_id=user_id) is False
        assert await db.update_tag(999999, {"name": "missing"}, user_id=user_id) is False
        assert await db.update_tag_display_orders([], user_id=user_id) is True
        assert await db.add_tags_to_bill(bill_id, [], user_id=user_id) is True
        assert await db.get_tags_for_bills([], user_id=user_id) == {}

        assert await db.add_tags_to_bill(bill_id, [tag_id], user_id=user_id) is True
        assert await db.update_bill_tags(bill_id, [], user_id=user_id) is True
        assert await db.get_tags_for_bill(bill_id, user_id=user_id) == []
    finally:
        await db.close()
