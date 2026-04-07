"""DB integration coverage for category duplicate, no-op, and bulk rename/delete paths."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

# pylint: disable=duplicate-code,too-many-locals

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_category_edge_paths.db"))
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


async def _create_category(
    db: Database,
    *,
    user_id: int,
    main_category: str,
    sub_category: str = "",
    priority: int = 0,
) -> int | None:
    return await db.create_category(
        {
            "type": 3,
            "main_category": main_category,
            "sub_category": sub_category,
            "description": "",
            "priority": priority,
            "keywords": "",
            "hidden": False,
            "icon": "",
            "color": "",
        },
        user_id=user_id,
    )


@pytest.mark.asyncio
async def test_category_edge_paths_cover_duplicate_noop_bulk_rename_and_bulk_delete(
    tmp_path: Path,
) -> None:
    """分类域应覆盖唯一约束冲突、空更新和按主分类批量改名/删除分支。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "category_edge_user")
        other_user_id = await _create_user(db, "category_edge_other_user")
        parent_id = await _create_category(
            db,
            user_id=user_id,
            main_category="待批量处理分类",
            sub_category="",
        )
        child_id = await _create_category(
            db,
            user_id=user_id,
            main_category="待批量处理分类",
            sub_category="子分类",
            priority=1,
        )
        assert parent_id is not None
        assert child_id is not None

        other_user_parent_id = await _create_category(
            db,
            user_id=other_user_id,
            main_category="待批量处理分类",
            sub_category="",
        )
        other_user_child_id = await _create_category(
            db,
            user_id=other_user_id,
            main_category="待批量处理分类",
            sub_category="其他用户子分类",
            priority=2,
        )
        assert other_user_parent_id is not None
        assert other_user_child_id is not None

        duplicate_parent_id = await _create_category(
            db,
            user_id=user_id,
            main_category="待批量处理分类",
            sub_category="",
        )
        assert duplicate_parent_id is None

        assert await db.update_category(int(parent_id), {}, user_id=user_id) is False
        assert (
            await db.update_category(999999, {"description": "missing"}, user_id=user_id)
            is False
        )

        category_mappings_before_update = await db.get_category_mappings(user_id=user_id)
        assert category_mappings_before_update["id_to_category"][int(child_id)]["description"] == ""
        assert (
            await db.update_category(
                int(child_id),
                {"description": "已更新描述"},
                user_id=user_id,
            )
            is True
        )
        category_mappings_after_update = await db.get_category_mappings(user_id=user_id)
        assert category_mappings_after_update["id_to_category"][int(child_id)]["description"] == "已更新描述"

        extra_child_id = await _create_category(
            db,
            user_id=user_id,
            main_category="待批量处理分类",
            sub_category="新创建子分类",
            priority=3,
        )
        assert extra_child_id is not None
        category_mappings_after_create = await db.get_category_mappings(user_id=user_id)
        assert (
            category_mappings_after_create["name_to_id"][("待批量处理分类", "新创建子分类")]
            == int(extra_child_id)
        )

        category_mappings_before_rename = await db.get_category_mappings(user_id=user_id)
        other_user_category_mappings = await db.get_category_mappings(user_id=other_user_id)
        assert (
            category_mappings_before_rename["name_to_id"][("待批量处理分类", "")]
            == int(parent_id)
        )
        assert int(parent_id) not in other_user_category_mappings["id_to_category"]
        assert (
            other_user_category_mappings["id_to_category"][int(other_user_parent_id)][
                "main_category"
            ]
            == "待批量处理分类"
        )

        assert await db.update_main_category_name(
            "待批量处理分类",
            "已批量改名分类",
            user_id=user_id,
        ) is True
        renamed_parent = await db.get_category_by_id(int(parent_id), user_id=user_id)
        renamed_child = await db.get_category_by_id(int(child_id), user_id=user_id)
        untouched_other_parent = await db.get_category_by_id(
            int(other_user_parent_id),
            user_id=other_user_id,
        )
        untouched_other_child = await db.get_category_by_id(
            int(other_user_child_id),
            user_id=other_user_id,
        )
        assert renamed_parent is not None
        assert renamed_child is not None
        assert untouched_other_parent is not None
        assert untouched_other_child is not None
        assert renamed_parent["main_category"] == "已批量改名分类"
        assert renamed_child["main_category"] == "已批量改名分类"
        assert untouched_other_parent["main_category"] == "待批量处理分类"
        assert untouched_other_child["main_category"] == "待批量处理分类"

        category_mappings_after_rename = await db.get_category_mappings(user_id=user_id)
        assert (
            category_mappings_after_rename["name_to_id"][("已批量改名分类", "")]
            == int(parent_id)
        )
        assert ("待批量处理分类", "") not in category_mappings_after_rename["name_to_id"]

        assert await db.delete_categories_by_main_category(
            "已批量改名分类",
            user_id=user_id,
        ) is True
        assert await db.delete_categories_by_main_category(
            "已批量改名分类",
            user_id=user_id,
        ) is False
        assert await db.get_category_by_id(int(parent_id), user_id=user_id) is None
        assert await db.get_category_by_id(int(child_id), user_id=user_id) is None
        assert (
            await db.get_category_by_id(int(other_user_parent_id), user_id=other_user_id)
            is not None
        )
        assert (
            await db.get_category_by_id(int(other_user_child_id), user_id=other_user_id)
            is not None
        )

        category_mappings_after_delete = await db.get_category_mappings(user_id=user_id)
        assert ("已批量改名分类", "") not in category_mappings_after_delete["name_to_id"]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_update_main_category_name_returns_false_on_unique_conflict(tmp_path: Path) -> None:
    """批量主分类改名遇到唯一约束冲突时应返回 False，且不产生部分写入。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "category_rename_conflict_user")

        old_parent_id = await _create_category(
            db,
            user_id=user_id,
            main_category="旧主分类",
            sub_category="",
        )
        old_child_id = await _create_category(
            db,
            user_id=user_id,
            main_category="旧主分类",
            sub_category="重复子类",
        )
        target_parent_id = await _create_category(
            db,
            user_id=user_id,
            main_category="新主分类",
            sub_category="",
        )
        target_child_id = await _create_category(
            db,
            user_id=user_id,
            main_category="新主分类",
            sub_category="重复子类",
        )
        assert old_parent_id is not None
        assert old_child_id is not None
        assert target_parent_id is not None
        assert target_child_id is not None

        assert await db.update_main_category_name("旧主分类", "新主分类", user_id=user_id) is False

        old_parent = await db.get_category_by_id(int(old_parent_id), user_id=user_id)
        old_child = await db.get_category_by_id(int(old_child_id), user_id=user_id)
        target_parent = await db.get_category_by_id(int(target_parent_id), user_id=user_id)
        target_child = await db.get_category_by_id(int(target_child_id), user_id=user_id)
        assert old_parent is not None
        assert old_parent["main_category"] == "旧主分类"
        assert old_child is not None
        assert old_child["main_category"] == "旧主分类"
        assert target_parent is not None
        assert target_parent["main_category"] == "新主分类"
        assert target_child is not None
        assert target_child["main_category"] == "新主分类"
    finally:
        await db.close()
