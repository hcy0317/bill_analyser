from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_import_preview_paths.db"))
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


async def _create_category(db: Database, *, user_id: int, main_category: str, sub_category: str = "") -> int:
    category_id = await db.create_category(
        {
            "type": 3,
            "main_category": main_category,
            "sub_category": sub_category,
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
    return int(category_id)


@pytest.mark.asyncio
async def test_preview_edit_confirm_and_cleanup_paths_cover_alias_fields_and_counts(tmp_path: Path) -> None:
    """导入预览链路应覆盖插入、批量编辑、确认入账和清理计数。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "preview_path_user")
        session_id = "preview-path-session"
        await db.create_import_session(session_id, user_id=user_id, file_count=1)

        breakfast_category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="餐饮",
            sub_category="早餐",
        )
        transit_category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="交通",
            sub_category="地铁",
        )

        first_preview_id = await db.insert_preview_bill(
            session_id,
            {
                "preview_date": "2026-04-01 08:00:00",
                "preview_type": "支出",
                "preview_amount": 18.5,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_counterparty": "测试早餐铺",
                "preview_payment_method": "支付宝",
                "preview_description": "豆浆油条",
            },
            user_id=user_id,
            dedup_type="remaining",
            dedup_source_ids=[11, 22],
        )
        assert first_preview_id > 0

        inserted_count = await db.insert_preview_bills_batch(
            session_id,
            [
                {
                    "preview_data": {
                        "preview_date": "2026-04-01 09:00:00",
                        "preview_type": "支出",
                        "preview_amount": 4.0,
                        "preview_main_category": "交通",
                        "preview_sub_category": "地铁",
                        "preview_counterparty": "地铁站",
                        "preview_payment_method": "微信支付",
                        "preview_description": "早高峰通勤",
                        "preview_parser_id": "wechat",
                    },
                    "dedup_type": "platform_bank",
                    "dedup_source_ids": [33, 44],
                },
                {
                    "preview_data": {
                        "preview_date": "2026-04-01 10:30:00",
                        "preview_type": "收入",
                        "preview_amount": 100.0,
                        "preview_counterparty": "报销入账",
                        "preview_payment_method": "银行卡",
                        "preview_description": "交通报销",
                    },
                    "dedup_type": "remaining",
                    "dedup_source_ids": [],
                },
            ],
            user_id=user_id,
        )
        assert inserted_count == 2

        previews = await db.get_preview_by_session(session_id, user_id=user_id)
        assert len(previews) == 3
        previews_by_id = {int(item["id"]): item for item in previews}
        assert previews_by_id[first_preview_id]["category_id"] == breakfast_category_id
        second_preview_id = next(
            preview_id
            for preview_id, preview in previews_by_id.items()
            if preview_id != first_preview_id and preview.get("preview_parser_id") == "wechat"
        )
        assert previews_by_id[second_preview_id]["category_id"] == transit_category_id

        updated = await db.update_preview_bill(
            first_preview_id,
            {
                "category_id": transit_category_id,
                "is_selected": 0,
                "preview_description": "早餐改成通勤餐",
            },
            user_id=user_id,
        )
        assert updated is True

        updated_count = await db.update_preview_bills_batch(
            session_id,
            [
                {
                    "id": first_preview_id,
                    "date": "2026-04-01 08:05:00",
                    "amount": 20.0,
                    "category_id": breakfast_category_id,
                    "isSelected": True,
                },
                {
                    "id": second_preview_id,
                    "mainCategory": "交通",
                    "subCategory": "地铁",
                    "selected": False,
                    "description": "已取消选择的预览",
                },
            ],
            user_id=user_id,
        )
        assert updated_count == 2

        selected_preview_ids = {
            int(item["id"])
            for item in await db.get_preview_by_session(session_id, user_id=user_id, selected_only=True)
        }
        assert first_preview_id in selected_preview_ids
        assert second_preview_id not in selected_preview_ids
        assert len(selected_preview_ids) == 2

        reset_count = await db.reset_session_preview_selection(session_id, user_id=user_id)
        assert reset_count == 3
        assert await db.get_preview_by_session(session_id, user_id=user_id, selected_only=True) == []

        assert await db.update_preview_selection([first_preview_id, second_preview_id], True, user_id=user_id) == 2

        conn = await db._get_connection()
        await conn.execute(
            """
            INSERT INTO bills_parser_template (
                session_id, user_id, parser_date, parser_amount, parser_type,
                parser_description, parser_id, parser_counterparty, parser_payment_method,
                parser_original_type, parser_original_category, parser_account_id,
                parser_is_processed, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))
            """,
            (
                session_id,
                user_id,
                "2026-04-01 07:59:59",
                20.0,
                "支出",
                "原始解析记录",
                "wechat",
                "原始对手方",
                "微信支付",
                "消费",
                "",
                1,
            ),
        )
        await conn.commit()

        saved_annotations = await db.save_import_annotation_samples(
            session_id,
            [
                {
                    "id": first_preview_id,
                    "preview_type": "支出",
                    "category_id": breakfast_category_id,
                    "preview_source_account_id": None,
                    "preview_destination_account_id": None,
                }
            ],
            user_id=user_id,
        )
        assert saved_annotations == 1

        confirm_result = await db.confirm_preview_to_bills(session_id, user_id=user_id)
        assert confirm_result["confirmed_count"] == 2
        assert confirm_result["duplicate_count"] == 0
        assert confirm_result["errors"] == []

        confirmed_bills = await db.get_bills(user_id=user_id)
        bill_descriptions = {bill["description"] for bill in confirmed_bills}
        assert "早餐改成通勤餐" in bill_descriptions
        assert "已取消选择的预览" in bill_descriptions

        cleanup_result = await db.clear_session_data(session_id, user_id=user_id)
        assert cleanup_result == {
            "parser_count": 1,
            "preview_count": 3,
            "annotation_count": 1,
        }
        assert await db.get_preview_by_session(session_id, user_id=user_id) == []
    finally:
        await db.close()
