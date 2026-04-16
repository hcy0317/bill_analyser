from __future__ import annotations

from pathlib import Path

import pytest

from bill_analyser.core.db import Database


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_import_learning_suggestions.db"))
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


async def _create_category(db: Database, *, user_id: int, main_category: str, sub_category: str) -> int:
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


async def _insert_preview(
    db: Database,
    *,
    session_id: str,
    user_id: int,
    preview_date: str,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
) -> int:
    preview_id = await db.insert_preview_bill(
        session_id,
        {
            "preview_date": preview_date,
            "preview_type": "支出",
            "preview_amount": 18.8,
            "preview_counterparty": counterparty,
            "preview_payment_method": payment_method,
            "preview_description": description,
            "preview_parser_id": parser_id,
        },
        user_id=user_id,
    )
    return int(preview_id)


async def _create_composite_rule(
    db: Database,
    *,
    user_id: int,
    parser_id: str,
    counterparty: str,
    description: str,
    payment_method: str,
    learned_type: str,
) -> int:
    conn = await db._get_connection()  # pylint: disable=protected-access
    now = "2026-08-01T09:00:00"
    composite_hash = db.build_composite_match_hash(
        parser_id=parser_id,
        counterparty=counterparty,
        description=description,
        payment_method=payment_method,
    )
    match_features = db.build_composite_match_features(
        parser_id=parser_id,
        counterparty=counterparty,
        description=description,
        payment_method=payment_method,
    )
    assert composite_hash is not None
    assert match_features is not None
    cursor = await conn.execute(
        """
        INSERT INTO import_learning_rules (
            user_id, match_type, match_value, normalized_match_value,
            learned_type, enabled, source_session_id, source_preview_id,
            parser_id, composite_match_hash, match_features_json,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            "composite",
            composite_hash,
            composite_hash,
            learned_type,
            session_id := "pytest-existing-rule-session",
            None,
            parser_id,
            composite_hash,
            __import__("json").dumps(match_features, ensure_ascii=False, sort_keys=True),
            now,
            now,
        ),
    )
    await conn.commit()
    return int(cursor.lastrowid or 0)


@pytest.mark.asyncio
async def test_db_list_import_learning_suggestions_groups_same_composite_and_aggregates_samples(tmp_path: Path) -> None:
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "learning_suggestions_group_user")
        category_id = await _create_category(db, user_id=user_id, main_category="餐饮", sub_category="咖啡")
        account_id = await _create_account(db, user_id=user_id, name="测试账户")
        session_id = "pytest-learning-suggestions-group"

        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        first_preview_id = await _insert_preview(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_date="2026-08-02 09:00:00",
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
        )
        second_preview_id = await _insert_preview(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_date="2026-08-02 18:00:00",
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
        )
        await db.save_import_annotation_samples(
            session_id,
            [
                {
                    "id": first_preview_id,
                    "preview_type": "支出",
                    "category_id": category_id,
                    "preview_source_account_id": account_id,
                    "preview_destination_account_id": None,
                },
                {
                    "id": second_preview_id,
                    "preview_type": "支出",
                    "category_id": category_id,
                    "preview_source_account_id": account_id,
                    "preview_destination_account_id": None,
                },
            ],
            user_id=user_id,
        )

        suggestions = await db.list_import_learning_suggestions_for_session(session_id, user_id=user_id)

        assert len(suggestions) == 1
        suggestion = suggestions[0]
        assert suggestion["match_type"] == "composite"
        assert suggestion["sample_count"] == 2
        assert suggestion["source_preview_ids"] == [first_preview_id, second_preview_id]
        assert suggestion["match_features"] == {
            "parser_id": "alipay",
            "counterparty": "星巴克咖啡",
            "description": "门店消费",
            "payment_method": "支付宝",
        }
        assert suggestion["learned_type"] == "支出"
        assert suggestion["learned_category_id"] == category_id
        assert suggestion["learned_source_account_id"] == account_id
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_db_list_import_learning_suggestions_skips_conflicts_and_existing_rules(tmp_path: Path) -> None:
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "learning_suggestions_conflict_user")
        coffee_category_id = await _create_category(db, user_id=user_id, main_category="餐饮", sub_category="咖啡")
        tea_category_id = await _create_category(db, user_id=user_id, main_category="餐饮", sub_category="茶饮")
        session_id = "pytest-learning-suggestions-conflict"

        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        first_preview_id = await _insert_preview(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_date="2026-08-03 09:00:00",
            parser_id="alipay",
            counterparty="瑞幸咖啡",
            description="门店消费",
            payment_method="支付宝",
        )
        second_preview_id = await _insert_preview(
            db,
            session_id=session_id,
            user_id=user_id,
            preview_date="2026-08-03 18:00:00",
            parser_id="alipay",
            counterparty="瑞幸咖啡",
            description="门店消费",
            payment_method="支付宝",
        )
        await db.save_import_annotation_samples(
            session_id,
            [
                {"id": first_preview_id, "preview_type": "支出", "category_id": coffee_category_id},
                {"id": second_preview_id, "preview_type": "收入", "category_id": tea_category_id},
            ],
            user_id=user_id,
        )

        conflicted_suggestions = await db.list_import_learning_suggestions_for_session(session_id, user_id=user_id)
        assert conflicted_suggestions == []

        filtered_session_id = "pytest-learning-suggestions-existing"
        await db.create_import_session(filtered_session_id, user_id=user_id, file_count=1)
        preview_id = await _insert_preview(
            db,
            session_id=filtered_session_id,
            user_id=user_id,
            preview_date="2026-08-04 09:00:00",
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
        )
        await db.save_import_annotation_samples(
            filtered_session_id,
            [{"id": preview_id, "preview_type": "支出"}],
            user_id=user_id,
        )
        await _create_composite_rule(
            db,
            user_id=user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="支出",
        )

        filtered_suggestions = await db.list_import_learning_suggestions_for_session(
            filtered_session_id,
            user_id=user_id,
        )
        assert filtered_suggestions == []
    finally:
        await db.close()
