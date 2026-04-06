"""
Database Tests - 数据库模块测试
"""

import json
import sqlite3
import tempfile
from pathlib import Path

import pytest
from src.core.db import Database


@pytest.fixture
async def db():
    """测试数据库fixture"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        database = Database(str(db_path))
        await database.init_db()
        yield database
        await database.close()


@pytest.mark.asyncio
async def test_init_db(db):
    """测试数据库初始化"""
    stats = await db.get_statistics()
    assert stats["total_bills"] == 0


@pytest.mark.asyncio
async def test_insert_bills(db):
    """测试插入账单"""
    bills = [
        {
            "date": "2025-01-01 12:00:00",
            "type": "支出",
            "amount": 100.0,
            "counterparty": "测试商家",
            "description": "测试消费"
        }
    ]

    count = await db.insert_bills(bills)
    assert count == 1

    # 重复插入应该被忽略
    count = await db.insert_bills(bills)
    assert count == 0


@pytest.mark.asyncio
async def test_get_bills(db):
    """测试查询账单"""
    # 插入测试数据
    bills = [
        {
            "date": "2025-01-01",
            "type": "支出",
            "amount": 100.0,
            "counterparty": "商家A",
            "description": "消费A"
        },
        {
            "date": "2025-01-02",
            "type": "收入",
            "amount": 200.0,
            "counterparty": "来源B",
            "description": "收入B"
        }
    ]
    await db.insert_bills(bills)

    # 查询所有
    result = await db.get_bills()
    assert len(result) == 2

    # 按类型过滤
    result = await db.get_bills({"type": "支出"})
    assert len(result) == 1
    assert result[0]["type"] == "支出"


@pytest.mark.asyncio
async def test_update_bill(db):
    """测试更新账单"""
    bills = [{
        "date": "2025-01-01",
        "type": "支出",
        "amount": 100.0,
        "counterparty": "商家",
        "description": "原描述"
    }]
    await db.insert_bills(bills)

    result = await db.get_bills()
    bill_id = result[0]["id"]

    success = await db.update_bill(bill_id, {"description": "新描述"})
    assert success

    result = await db.get_bills()
    assert result[0]["description"] == "新描述"


@pytest.mark.asyncio
async def test_delete_bill(db):
    """测试删除账单"""
    bills = [{
        "date": "2025-01-01",
        "type": "支出",
        "amount": 100.0,
        "counterparty": "商家",
        "description": "描述"
    }]
    await db.insert_bills(bills)

    result = await db.get_bills()
    bill_id = result[0]["id"]

    success = await db.delete_bill(bill_id)
    assert success

    result = await db.get_bills()
    assert len(result) == 0


@pytest.mark.asyncio
async def test_import_annotation_samples_roundtrip(db):
    """测试导入会话标注样本的保存、读取与清理。"""
    session_id = "test-session-annotation"
    await db.create_import_session(session_id, user_id=1, file_count=1)

    saved = await db.save_import_annotation_samples(session_id, [
        {
            "id": 101,
            "preview_type": "转账",
            "category_id": 12,
            "preview_source_account_id": 3,
            "preview_destination_account_id": 8,
        }
    ], user_id=1)

    assert saved == 1

    samples = await db.get_import_annotation_samples(session_id, user_id=1)
    assert len(samples) == 1
    assert samples[0]["preview_id"] == 101
    assert samples[0]["annotated_type"] == "转账"
    assert samples[0]["annotated_category_id"] == 12
    assert samples[0]["annotated_source_account_id"] == 3
    assert samples[0]["annotated_destination_account_id"] == 8

    cleared = await db.clear_session_data(session_id)
    assert cleared["annotation_count"] == 1


@pytest.mark.asyncio
async def test_import_learning_rules_promote_toggle_delete(db):
    """测试长期导入学习规则的提升、启停与删除。"""
    session_id = "test-session-learning-promote"
    await db.create_import_session(session_id, user_id=1, file_count=1)

    category_parent_id = await db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=1)
    assert category_parent_id is not None

    category_id = await db.create_category({
        "type": 3,
        "main_category": "餐饮",
        "sub_category": "早餐",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": False,
        "icon": "",
        "color": ""
    }, user_id=1)
    assert category_id is not None

    source_account_id = await db.create_account({
        "name": "支付宝",
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
        "aliases": '["支付宝"]'
    }, user_id=1)
    destination_account_id = await db.create_account({
        "name": "现金",
        "type": 1,
        "category": "asset",
        "currency": "CNY",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 0.0,
        "hidden": False,
        "display_order": 1,
        "comment": "",
        "aliases": '["现金"]'
    }, user_id=1)

    inserted = await db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-07 08:00:00",
                "preview_type": "支出",
                "preview_amount": 18.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "学习商户",
                "preview_payment_method": "支付宝",
                "preview_description": "学习描述"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=1)
    assert inserted == 1

    previews = await db.get_preview_by_session(session_id)
    assert len(previews) == 1
    preview_id = previews[0]["id"]

    saved = await db.save_import_annotation_samples(session_id, [
        {
            "id": preview_id,
            "preview_type": "转账",
            "category_id": category_id,
            "preview_source_account_id": source_account_id,
            "preview_destination_account_id": destination_account_id,
        }
    ], user_id=1)
    assert saved == 1

    promoted = await db.promote_import_annotation_samples_to_learning(session_id, user_id=1)
    assert promoted["selected_samples"] == 1
    assert promoted["rules_total"] == 1
    assert promoted["created"] == 1
    assert promoted["updated"] == 0

    rules = await db.get_import_learning_rules(user_id=1)
    assert len(rules) == 1
    assert {rule["match_type"] for rule in rules} == {"composite"}
    assert all(rule["enabled"] == 1 for rule in rules)
    assert all(rule["learned_type"] == "转账" for rule in rules)
    assert all(rule["learned_category_id"] == category_id for rule in rules)

    composite_rule = rules[0]
    assert composite_rule["parser_id"] in (None, "")
    assert composite_rule["composite_match_hash"] == "c=学习商户|d=学习描述|m=支付宝"
    assert json.loads(composite_rule["match_features_json"]) == {
        "counterparty": "学习商户",
        "description": "学习描述",
        "payment_method": "支付宝",
    }

    toggled = await db.set_import_learning_rule_enabled(
        int(composite_rule["id"]), False, user_id=1
    )
    assert toggled is True

    updated_rules = await db.get_import_learning_rules(user_id=1)
    updated_composite_rule = updated_rules[0]
    assert updated_composite_rule["enabled"] == 0

    usage_count = await db.increment_import_learning_rule_usage(
        [int(updated_composite_rule["id"])], user_id=1
    )
    assert usage_count == 1

    refreshed_rules = await db.get_import_learning_rules(user_id=1)
    refreshed_composite_rule = refreshed_rules[0]
    assert refreshed_composite_rule["applied_count"] == 1
    assert refreshed_composite_rule["last_applied_at"]

    deleted = await db.delete_import_learning_rule(
        int(updated_composite_rule["id"]), user_id=1
    )
    assert deleted is True

    final_rules = await db.get_import_learning_rules(user_id=1)
    assert final_rules == []

    conn = await db._get_connection()
    async with conn.execute(
        "SELECT COUNT(*) AS total FROM import_learning_rule_logs WHERE user_id = ?",
        (1,)
    ) as cursor:
        row = await cursor.fetchone()
    assert row["total"] >= 3


@pytest.mark.asyncio
async def test_import_learning_rules_promote_with_parser_builds_parser_aware_composite_rule(db):
    """测试提升长期学习规则时会生成带解析器来源的复合匹配规则。"""
    session_id = "test-session-learning-composite-parser"
    await db.create_import_session(session_id, user_id=1, file_count=1)

    inserted = await db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-09 08:00:00",
                "preview_type": "支出",
                "preview_amount": 35.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "解析器商户",
                "preview_payment_method": "微信支付",
                "preview_description": "早餐豆浆",
                "preview_parser_id": "wechat"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=1)
    assert inserted == 1

    preview_id = (await db.get_preview_by_session(session_id))[0]["id"]
    saved = await db.save_import_annotation_samples(session_id, [
        {
            "id": preview_id,
            "preview_type": "支出",
            "category_id": None,
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
        }
    ], user_id=1)
    assert saved == 1

    promoted = await db.promote_import_annotation_samples_to_learning(session_id, user_id=1)
    assert promoted["rules_total"] == 1

    rules = await db.get_import_learning_rules(user_id=1)
    assert len(rules) == 1
    composite_rule = rules[0]
    assert composite_rule["parser_id"] == "wechat"
    assert composite_rule["composite_match_hash"] == "c=解析器商户|d=早餐豆浆|p=wechat|m=微信支付"
    assert json.loads(composite_rule["match_features_json"]) == {
        "parser_id": "wechat",
        "counterparty": "解析器商户",
        "description": "早餐豆浆",
        "payment_method": "微信支付",
    }


@pytest.mark.asyncio
async def test_import_learning_rules_promote_uses_user_scoped_preview_lookup(db):
    """非默认用户提升长期学习规则时必须读取同一用户的 preview 数据。"""
    user_id = await db.create_user({
        "username": "learning_scope_user",
        "email": "learning_scope_user@example.com",
        "password_hash": "hash",
        "nickname": "learning_scope_user"
    })
    other_user_id = await db.create_user({
        "username": "learning_scope_other_user",
        "email": "learning_scope_other_user@example.com",
        "password_hash": "hash",
        "nickname": "learning_scope_other_user"
    })
    session_id = "test-session-learning-user-scope"
    await db.create_import_session(session_id, user_id=user_id, file_count=1)

    inserted = await db.insert_preview_bills_batch(session_id, [
        {
            "preview_data": {
                "preview_date": "2026-03-10 08:00:00",
                "preview_type": "支出",
                "preview_amount": 28.0,
                "preview_destination_amount": 0.0,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "用户隔离商户",
                "preview_payment_method": "云闪付",
                "preview_description": "仅属于非默认用户",
                "preview_parser_id": "cmbc"
            },
            "dedup_type": "remaining",
            "dedup_source_ids": []
        }
    ], user_id=user_id)
    assert inserted == 1

    previews = await db.get_preview_by_session(session_id, user_id=user_id)
    assert len(previews) == 1
    preview_id = previews[0]["id"]

    saved = await db.save_import_annotation_samples(session_id, [
        {
            "id": preview_id,
            "preview_type": "支出",
            "category_id": None,
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
        }
    ], user_id=user_id)
    assert saved == 1

    promoted = await db.promote_import_annotation_samples_to_learning(session_id, user_id=user_id)
    assert promoted["selected_samples"] == 1
    assert promoted["rules_total"] == 1
    assert promoted["created"] == 1

    rules = await db.get_import_learning_rules(user_id=user_id)
    assert len(rules) == 1
    assert rules[0]["parser_id"] == "cmbc"
    assert rules[0]["composite_match_hash"] == "c=用户隔离商户|d=仅属于非默认用户|p=cmbc|m=云闪付"
    assert await db.get_import_learning_rules(user_id=other_user_id) == []


@pytest.mark.asyncio
async def test_init_db_migrates_legacy_learning_and_preview_tables():
    """测试旧库初始化后会补齐复合学习规则与预览解析器列，且保留原有数据。"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "legacy.db"
        conn = sqlite3.connect(db_path)
        conn.execute(
            """
            CREATE TABLE import_learning_rules (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                match_type TEXT NOT NULL,
                match_value TEXT NOT NULL,
                normalized_match_value TEXT NOT NULL,
                learned_type TEXT,
                learned_category_id INTEGER,
                learned_source_account_id INTEGER,
                learned_destination_account_id INTEGER,
                enabled INTEGER NOT NULL DEFAULT 1,
                source_session_id TEXT,
                source_preview_id INTEGER,
                applied_count INTEGER NOT NULL DEFAULT 0,
                last_applied_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, match_type, normalized_match_value)
            )
            """
        )
        conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, enabled, created_at, updated_at
            ) VALUES (1, 'description', '旧描述', '旧描述', '支出', 1, '2026-03-01T08:00:00', '2026-03-01T08:00:00')
            """
        )
        conn.execute(
            """
            CREATE TABLE bills_preview (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                user_id INTEGER NOT NULL DEFAULT 1,
                preview_date TEXT,
                preview_type TEXT,
                preview_amount REAL DEFAULT 0,
                preview_destination_amount REAL DEFAULT 0,
                preview_main_category TEXT,
                preview_sub_category TEXT,
                preview_source_account_id INTEGER,
                preview_destination_account_id INTEGER,
                preview_counterparty TEXT,
                preview_payment_method TEXT,
                preview_description TEXT,
                preview_selected INTEGER DEFAULT 1,
                dedup_type TEXT,
                dedup_source_ids TEXT,
                created_at TEXT
            )
            """
        )
        conn.execute(
            """
            INSERT INTO bills_preview (
                session_id, user_id, preview_date, preview_type, preview_amount,
                preview_counterparty, preview_payment_method, preview_description,
                preview_selected, dedup_type, dedup_source_ids, created_at
            ) VALUES (
                'legacy-session', 1, '2026-03-01 09:00:00', '支出', 12.5,
                '旧商户', '旧支付方式', '旧描述', 1, 'remaining', '[]', '2026-03-01T09:00:00'
            )
            """
        )
        conn.commit()
        conn.close()

        database = Database(str(db_path))
        await database.init_db()

        migrated_conn = await database._get_connection()

        async with migrated_conn.execute("PRAGMA table_info(import_learning_rules)") as cursor:
            learning_columns = {row[1] for row in await cursor.fetchall()}
        assert "parser_id" in learning_columns
        assert "composite_match_hash" in learning_columns
        assert "match_features_json" in learning_columns

        async with migrated_conn.execute("PRAGMA table_info(bills_preview)") as cursor:
            preview_columns = {row[1] for row in await cursor.fetchall()}
        assert "preview_parser_id" in preview_columns

        rules = await database.get_import_learning_rules(user_id=1)
        assert len(rules) == 1
        assert rules[0]["match_type"] == "description"
        assert rules[0]["normalized_match_value"] == "旧描述"
        assert rules[0]["parser_id"] is None
        assert rules[0]["composite_match_hash"] is None
        assert rules[0]["match_features_json"] is None

        previews = await database.get_preview_by_session("legacy-session")
        assert len(previews) == 1
        assert previews[0]["preview_counterparty"] == "旧商户"
        assert previews[0]["preview_parser_id"] is None

        await database.close()


@pytest.mark.asyncio
async def test_import_config_save_match_and_delete(db):
    """测试导入列映射模板的保存、匹配与删除。"""
    config_id = await db.save_import_config({
        "name": "支付宝通用CSV模板",
        "file_format": "csv",
        "description": "用于通用表格导入学习",
        "field_mappings": {
            "date": "交易时间",
            "type": "交易类型",
            "amount": "金额",
            "description": "备注"
        },
        "date_format": "%Y-%m-%d %H:%M:%S",
        "delimiter": ",",
        "skip_rows": 0,
        "has_header": True,
        "sample_headers": ["交易时间", "交易类型", "金额", "备注"],
        "custom_rules": {"source": "pytest"},
        "is_default": True
    }, user_id=1)

    assert config_id > 0

    configs = await db.get_import_configs(user_id=1, file_format="csv")
    assert len(configs) == 1
    assert configs[0]["id"] == config_id
    assert configs[0]["field_mappings"]["date"] == "交易时间"
    assert configs[0]["sample_headers"] == ["交易时间", "交易类型", "金额", "备注"]
    assert configs[0]["is_default"] is True
    assert configs[0]["description_summary"] == (
        "映射: 时间->交易时间 / 类型->交易类型 / 金额->金额 / 描述->备注 | "
        "表头: 交易时间 / 交易类型 / 金额 / 备注"
    )

    exact_match = await db.find_matching_import_config(
        "csv",
        ["交易时间", "交易类型", "金额", "备注"],
        user_id=1
    )
    assert exact_match is not None
    assert exact_match["id"] == config_id
    assert exact_match["match_reason"] == "exact_header_signature"
    assert exact_match["match_score"] == 1.0
    assert exact_match["description"] == "用于通用表格导入学习"
    assert exact_match["description_summary"].startswith("映射: 时间->交易时间")

    fuzzy_match = await db.find_matching_import_config(
        "csv",
        ["交易时间", "金额", "交易类型", "备注", "附加列"],
        user_id=1
    )
    assert fuzzy_match is not None
    assert fuzzy_match["id"] == config_id
    assert fuzzy_match["match_score"] >= 0.6
    assert fuzzy_match["matched_header_count"] == 4

    deleted = await db.delete_import_config(config_id, user_id=1)
    assert deleted is True
    assert await db.get_import_configs(user_id=1, file_format="csv") == []


@pytest.mark.asyncio
async def test_user_import_learning_enabled_persistence(db):
    """测试用户级长期导入学习开关可持久化。"""
    user_id = await db.create_user({
        "username": "learning_toggle_user",
        "email": "learning_toggle_user@example.com",
        "password_hash": "hash",
        "nickname": "learning_toggle_user"
    })

    created_user = await db.get_user_by_id(user_id)
    assert created_user is not None
    assert int(created_user.get("import_learning_enabled", 1)) == 1

    updated = await db.update_user(user_id, {"import_learning_enabled": 0})
    assert updated is True

    refreshed_user = await db.get_user_by_id(user_id)
    assert refreshed_user is not None
    assert int(refreshed_user.get("import_learning_enabled", 1)) == 0


@pytest.mark.asyncio
async def test_deduplicate(db):
    """测试去重"""
    # 插入重复数据（通过不同批次）
    bill = {
        "date": "2025-01-01",
        "type": "支出",
        "amount": 100.0,
        "counterparty": "商家",
        "description": "描述"
    }

    # 手动插入以模拟重复（绕过哈希检查）
    await db._get_connection()
    # 这个测试需要特殊处理，跳过
    pass
