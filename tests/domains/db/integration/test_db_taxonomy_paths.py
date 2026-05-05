from __future__ import annotations

import json
from types import SimpleNamespace
from typing import TYPE_CHECKING

import pytest

from bill_analyser.core import account_rust_bridge, category_rust_bridge
from bill_analyser.core.db import Database
from bill_analyser.core.default_category_seed import ensure_default_categories

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


def _fail_account_bridge(*_args: object, **_kwargs: object) -> None:
    pytest.fail("account Rust bridge should not be used for this database mode")


def _fail_category_bridge(*_args: object, **_kwargs: object) -> None:
    pytest.fail("category Rust bridge should not be used for this database mode")


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
async def test_category_default_seed_uses_file_backed_master_data_path(tmp_path: Path) -> None:
    """默认分类种子的主数据补全应通过同一个分类 façade 在文件库闭环。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "taxonomy_category_seed_user")
        first_result = await ensure_default_categories(db, user_id=user_id)
        second_result = await ensure_default_categories(db, user_id=user_id)

        assert first_result["created"] > 0
        assert second_result["created"] == 0
        assert second_result["skipped"] >= first_result["created"]
        seeded = await db.get_category_by_name("餐饮", "早餐", user_id=user_id)
        assert seeded is not None
        assert seeded["icon"]
        assert seeded["color"]
        assert await db.get_category_by_name("", "", user_id=user_id) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_category_master_data_in_memory_fallback_round_trip(monkeypatch: pytest.MonkeyPatch) -> None:
    """:memory: 分类主数据应继续使用 Python fallback 并保持闭环。"""
    monkeypatch.setattr(category_rust_bridge, "list_categories", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "get_category", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "get_category_by_name", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "create_category", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "update_category", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "delete_category", _fail_category_bridge)
    monkeypatch.setattr(
        category_rust_bridge,
        "delete_categories_by_main_category",
        _fail_category_bridge,
    )
    monkeypatch.setattr(
        category_rust_bridge,
        "update_main_category_name",
        _fail_category_bridge,
    )

    db = Database(":memory:")
    await db.init_db()
    try:
        user_id = await _create_user(db, "taxonomy_category_memory_user")
        parent_payload = {
            "type": 3,
            "main_category": "内存分类",
            "sub_category": "",
            "description": "父级",
            "priority": 2,
            "keywords": "",
            "hidden": False,
            "icon": "mdi-folder",
            "color": "#336699",
        }
        child_payload = {
            "type": 3,
            "main_category": "内存分类",
            "sub_category": "子类",
            "description": "",
            "priority": 1,
            "keywords": "fallback",
            "hidden": False,
            "icon": "",
            "color": "",
        }
        assert await db.ensure_categories([parent_payload, child_payload], user_id=user_id) == {
            "created": 2,
            "skipped": 0,
        }
        assert await db.ensure_categories([parent_payload, child_payload], user_id=user_id) == {
            "created": 0,
            "skipped": 2,
        }
        parent = await db.get_category_by_name("内存分类", "", user_id=user_id)
        child = await db.get_category_by_name("内存分类", "子类", user_id=user_id)
        assert parent is not None
        assert child is not None
        parent_id = parent["id"]
        child_id = child["id"]

        assert parent_id is not None
        assert child_id is not None
        assert await db.update_category(child_id, {"hidden": True}, user_id=user_id) is True
        categories = await db.get_all_categories(user_id=user_id)
        assert [category["sub_category"] for category in categories] == ["子类", ""]

        assert await db.update_main_category_name("内存分类", "内存分类改名", user_id=user_id) is True
        renamed = await db.get_category_by_name("内存分类改名", "子类", user_id=user_id)
        assert renamed is not None
        assert renamed["hidden"] in (1, True)

        assert await db.delete_category(parent_id, user_id=user_id) is True
        assert await db.get_category_by_id(child_id, user_id=user_id) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_category_master_data_sqlcipher_fallback_round_trip(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """启用 SQLCipher 配置时分类主数据应继续走 Python fallback。"""
    monkeypatch.setattr(category_rust_bridge, "list_categories", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "get_category", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "get_category_by_name", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "create_category", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "update_category", _fail_category_bridge)
    monkeypatch.setattr(category_rust_bridge, "delete_category", _fail_category_bridge)
    monkeypatch.setattr(
        category_rust_bridge,
        "delete_categories_by_main_category",
        _fail_category_bridge,
    )
    monkeypatch.setattr(
        category_rust_bridge,
        "update_main_category_name",
        _fail_category_bridge,
    )

    db = await _create_database(tmp_path)
    db._encryption_config = SimpleNamespace(enabled=True)
    try:
        user_id = await _create_user(db, "taxonomy_category_sqlcipher_user")
        category_id = await db.create_category(
            {
                "type": 3,
                "main_category": "SQLCipher分类",
                "sub_category": "",
                "priority": 2,
                "hidden": False,
            },
            user_id=user_id,
        )
        assert category_id is not None

        assert await db.update_category(category_id, {"priority": 1}, user_id=user_id) is True
        categories = await db.get_all_categories(user_id=user_id)
        assert [category["main_category"] for category in categories] == ["SQLCipher分类"]
        assert int(categories[0]["priority"]) == 1

        assert await db.delete_categories_by_main_category("SQLCipher分类", user_id=user_id) is True
        assert await db.get_category_by_id(category_id, user_id=user_id) is None
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
        assert [tag["name"] for tag in await db.get_tags_for_bill(bill_id, user_id=user_id)] == [
            "通勤"
        ]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_account_crud_subaccounts_display_order_and_user_scope_round_trip(tmp_path: Path) -> None:
    """账户主数据链路应覆盖 CRUD、子账户、显示顺序和用户隔离。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "taxonomy_account_user")
        other_user_id = await _create_user(db, "taxonomy_account_other_user")

        parent_id = await db.create_account(
            {
                "name": "Rust父账户",
                "type": 2,
                "category": 1,
                "currency": "CNY",
                "icon": "1",
                "color": "ffcc00",
                "balance": 0.0,
                "initial_balance": 0.0,
                "hidden": False,
                "display_order": 20,
                "comment": "父账户",
                "aliases": ["主账户", "家庭账户"],
                "subAccounts": [
                    {
                        "name": "Rust子账户",
                        "type": 1,
                        "category": 1,
                        "currency": "USD",
                        "balance": 10.5,
                        "initial_balance": 10.5,
                        "display_order": 30,
                        "aliases": [],
                    }
                ],
            },
            user_id=user_id,
        )
        second_id = await db.create_account(
            {
                "name": "Rust排序账户",
                "type": 1,
                "category": "asset",
                "currency": "CNY",
                "display_order": 10,
            },
            user_id=user_id,
        )
        other_account_id = await db.create_account(
            {"name": "其他用户账户", "type": 1},
            user_id=other_user_id,
        )

        parent = await db.get_account_by_id(parent_id, user_id=user_id)
        assert parent is not None
        assert parent["name"] == "Rust父账户"
        assert json.loads(parent["aliases"]) == ["主账户", "家庭账户"]

        sub_accounts = await db.get_sub_accounts(parent_id, user_id=user_id)
        assert len(sub_accounts) == 1
        child_id = int(sub_accounts[0]["id"])
        assert sub_accounts[0]["parent_id"] == parent_id
        assert sub_accounts[0]["currency"] == "USD"

        assert await db.update_account(
            parent_id,
            {
                "id": str(parent_id),
                "name": "Rust父账户-已更新",
                "hidden": True,
                "displayOrder": 40,
                "parent_id": second_id,
                "clientSessionId": "frontend-session",
            },
            user_id=user_id,
        ) is True
        assert await db.update_account(other_account_id, {"name": "越权更新"}, user_id=user_id) is False
        assert await db.update_account_display_orders(
            [(parent_id, 2), (second_id, 1), (other_account_id, 0)],
            user_id=user_id,
        ) is True

        all_accounts = await db.get_all_accounts(user_id=user_id)
        assert [account["name"] for account in all_accounts] == [
            "Rust排序账户",
            "Rust父账户-已更新",
            "Rust子账户",
        ]
        assert all_accounts[1]["hidden"] in (1, True)
        assert all_accounts[0]["category"] == "asset"

        other_account = await db.get_account_by_id(other_account_id, user_id=other_user_id)
        assert other_account is not None
        assert other_account["display_order"] == 0
        assert other_account["name"] == "其他用户账户"
        moved_parent = await db.get_account_by_id(parent_id, user_id=user_id)
        assert moved_parent is not None
        assert moved_parent["parent_id"] == second_id

        assert await db.delete_account(child_id, user_id=user_id) is True
        assert await db.get_account_by_id(child_id, user_id=user_id) is None
        assert await db.delete_account(other_account_id, user_id=user_id) is False
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_account_file_bridge_preserves_legacy_userless_account_round_trip(tmp_path: Path) -> None:
    """文件库账户 bridge 不应比旧 Python 连接更严格要求 users 行先存在。"""
    db = await _create_database(tmp_path)
    try:
        account_id = await db.create_account(
            {
                "name": "历史无用户账户",
                "type": 1,
                "aliases": ["旧数据", 12, True, None],
            },
            user_id=1,
        )

        account = await db.get_account_by_id(account_id, user_id=1)
        assert account is not None
        assert account["name"] == "历史无用户账户"
        assert json.loads(account["aliases"]) == ["旧数据", "12", "True", "None"]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_account_master_data_in_memory_fallback_round_trip(monkeypatch: pytest.MonkeyPatch) -> None:
    """:memory: 账户主数据应继续使用 Python fallback 并保持闭环。"""
    monkeypatch.setattr(account_rust_bridge, "create_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "update_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "delete_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "update_display_orders", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "list_accounts", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "get_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "get_sub_accounts", _fail_account_bridge)

    db = Database(":memory:")
    await db.init_db()
    try:
        user_id = await _create_user(db, "taxonomy_account_memory_user")
        parent_id = await db.create_account(
            {
                "name": "内存父账户",
                "type": 2,
                "category": 1,
                "currency": "CNY",
                "balance": 0.0,
                "initial_balance": 0.0,
                "hidden": False,
                "display_order": 3,
                "aliases": ["内存账户"],
                "subAccounts": [{"name": "内存子账户", "type": 1, "display_order": 4}],
            },
            user_id=user_id,
        )
        second_id = await db.create_account({"name": "内存排序账户", "type": 1}, user_id=user_id)

        assert await db.update_account_display_orders(
            [(parent_id, 2), (second_id, 1)],
            user_id=user_id,
        ) is True
        assert await db.update_account_display_orders([], user_id=user_id) is True
        accounts = await db.get_all_accounts(user_id=user_id)
        assert [account["name"] for account in accounts[:2]] == ["内存排序账户", "内存父账户"]

        parent = await db.get_account_by_id(parent_id, user_id=user_id)
        assert parent is not None
        assert json.loads(parent["aliases"]) == ["内存账户"]

        sub_accounts = await db.get_sub_accounts(parent_id, user_id=user_id)
        assert len(sub_accounts) == 1
        child_id = int(sub_accounts[0]["id"])

        assert await db.update_account(parent_id, {"hidden": True}, user_id=user_id) is True
        updated_parent = await db.get_account_by_id(parent_id, user_id=user_id)
        assert updated_parent is not None
        assert updated_parent["hidden"] in (1, True)

        assert await db.delete_account(child_id, user_id=user_id) is True
        assert await db.get_account_by_id(child_id, user_id=user_id) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_account_master_data_sqlcipher_fallback_round_trip(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """启用 SQLCipher 配置时账户主数据应继续走 Python fallback。"""
    monkeypatch.setattr(account_rust_bridge, "create_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "update_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "delete_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "update_display_orders", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "list_accounts", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "get_account", _fail_account_bridge)
    monkeypatch.setattr(account_rust_bridge, "get_sub_accounts", _fail_account_bridge)

    db = await _create_database(tmp_path)
    db._encryption_config = SimpleNamespace(enabled=True)
    try:
        user_id = await _create_user(db, "taxonomy_account_sqlcipher_user")
        account_id = await db.create_account(
            {
                "name": "SQLCipher账户",
                "type": 1,
                "display_order": 2,
                "aliases": ["加密"],
            },
            user_id=user_id,
        )

        assert await db.update_account(account_id, {"display_order": 1, "hidden": True}, user_id=user_id) is True
        accounts = await db.get_all_accounts(user_id=user_id)
        assert [account["name"] for account in accounts] == ["SQLCipher账户"]
        assert accounts[0]["hidden"] in (1, True)

        account = await db.get_account_by_id(account_id, user_id=user_id)
        assert account is not None
        assert json.loads(account["aliases"]) == ["加密"]

        assert await db.delete_account(account_id, user_id=user_id) is True
        assert await db.get_account_by_id(account_id, user_id=user_id) is None
    finally:
        await db.close()
