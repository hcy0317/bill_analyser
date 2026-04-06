from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_taxonomy_paths.db"))
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


async def _create_bill(
    db: Database,
    *,
    user_id: int,
    main_category: str,
    sub_category: str,
    description: str,
) -> int:
    bill_id = await db.create_bill(
        {
            "date": "2026-04-02 12:00:00",
            "type": "支出",
            "amount": -15.5,
            "counterparty": "分类标签测试商户",
            "description": description,
            "payment_method": "支付宝",
            "main_category": main_category,
            "sub_category": sub_category,
            "source_account_id": 0,
            "destination_account_id": 0,
            "destination_amount": 0.0,
        },
        user_id=user_id,
    )
    assert bill_id is not None
    return int(bill_id)


@pytest.mark.asyncio
async def test_get_all_categories_fallback_respects_user_scope_and_category_statistics(tmp_path: Path) -> None:
    """分类 fallback 只应读取当前用户账单，且统计/CRUD 路径应保持可用。"""
    db = await _create_database(tmp_path)
    try:
        first_user_id = await _create_user(db, "taxonomy_user_one")
        second_user_id = await _create_user(db, "taxonomy_user_two")

        await _create_bill(
            db,
            user_id=first_user_id,
            main_category="用户一分类",
            sub_category="早餐",
            description="用户一账单",
        )
        await _create_bill(
            db,
            user_id=second_user_id,
            main_category="用户二分类",
            sub_category="通勤",
            description="用户二账单",
        )

        extracted_categories = await db.get_all_categories(user_id=first_user_id)
        assert extracted_categories == [
            {
                "id": 0,
                "main_category": "用户一分类",
                "sub_category": "早餐",
                "description": "",
                "priority": 0,
                "keywords": "",
            }
        ]

        parent_category_id = await db.create_category(
            {
                "type": 3,
                "main_category": "餐饮",
                "sub_category": "",
                "description": "父级分类",
                "priority": 10,
                "keywords": "",
                "hidden": False,
                "icon": "",
                "color": "",
            },
            user_id=first_user_id,
        )
        child_category_id = await db.create_category(
            {
                "type": 3,
                "main_category": "餐饮",
                "sub_category": "午餐",
                "description": "子级分类",
                "priority": 11,
                "keywords": "快餐",
                "hidden": False,
                "icon": "🍜",
                "color": "#FFAA00",
            },
            user_id=first_user_id,
        )
        assert parent_category_id is not None
        assert child_category_id is not None

        by_name = await db.get_category_by_name("餐饮", "午餐", user_id=first_user_id)
        assert by_name is not None
        assert int(by_name["id"]) == int(child_category_id)

        by_id = await db.get_category_by_id(int(child_category_id), user_id=first_user_id)
        assert by_id is not None
        assert by_id["keywords"] == "快餐"

        assert await db.update_category(
            int(child_category_id),
            {"description": "已更新的午餐分类", "priority": 99},
            user_id=first_user_id,
        ) is True
        updated_category = await db.get_category_by_id(int(child_category_id), user_id=first_user_id)
        assert updated_category is not None
        assert updated_category["description"] == "已更新的午餐分类"
        assert int(updated_category["priority"]) == 99

        await _create_bill(
            db,
            user_id=first_user_id,
            main_category="餐饮",
            sub_category="午餐",
            description="分类统计账单",
        )
        statistics = await db.get_category_statistics(
            start_date="2026-04-01",
            end_date="2026-04-30",
            user_id=first_user_id,
        )
        assert any(
            item["main_category"] == "餐饮" and item["sub_category"] == "午餐"
            for item in statistics
        )

        assert await db.delete_category(int(parent_category_id), user_id=first_user_id) is True
        assert await db.get_category_by_id(int(parent_category_id), user_id=first_user_id) is None
        assert await db.get_category_by_id(int(child_category_id), user_id=first_user_id) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_tag_crud_display_order_and_bill_relations_round_trip(tmp_path: Path) -> None:
    """标签链路应覆盖 CRUD、显示顺序和账单关联的整条回路。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "taxonomy_tag_user")
        bill_id = await _create_bill(
            db,
            user_id=user_id,
            main_category="标签测试",
            sub_category="默认",
            description="需要标签的账单",
        )

        first_tag_id = await db.create_tag({"name": "早餐", "color": "#FF0000"}, user_id=user_id)
        second_tag_id = await db.create_tag({"name": "通勤", "color": "#00FF00"}, user_id=user_id)
        assert first_tag_id > 0
        assert second_tag_id > 0

        assert await db.update_tag(first_tag_id, {"hidden": True, "icon": "🍳"}, user_id=user_id) is True
        assert await db.update_tag_display_orders(
            [(first_tag_id, 2), (second_tag_id, 1)],
            user_id=user_id,
        ) is True

        all_tags = await db.get_all_tags(user_id=user_id)
        assert [tag["name"] for tag in all_tags] == ["通勤", "早餐"]
        first_tag = await db.get_tag_by_id(first_tag_id, user_id=user_id)
        assert first_tag is not None
        assert bool(first_tag["hidden"]) is True
        assert first_tag["icon"] == "🍳"

        assert await db.add_tags_to_bill(bill_id, [first_tag_id, second_tag_id], user_id=user_id) is True
        bill_tags = await db.get_tags_for_bill(bill_id, user_id=user_id)
        assert [tag["name"] for tag in bill_tags] == ["早餐", "通勤"]

        grouped_tags = await db.get_tags_for_bills([bill_id], user_id=user_id)
        assert list(grouped_tags) == [bill_id]
        assert [tag["name"] for tag in grouped_tags[bill_id]] == ["早餐", "通勤"]

        assert await db.update_bill_tags(bill_id, [second_tag_id], user_id=user_id) is True
        updated_bill_tags = await db.get_tags_for_bill(bill_id, user_id=user_id)
        assert [tag["name"] for tag in updated_bill_tags] == ["通勤"]

        assert await db.delete_tag(first_tag_id, user_id=user_id) is True
        assert await db.get_tag_by_id(first_tag_id, user_id=user_id) is None
    finally:
        await db.close()
