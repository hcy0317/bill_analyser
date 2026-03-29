from __future__ import annotations
# pyright: reportPrivateUsage=false

from datetime import datetime, timedelta
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.db import Database


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_deep_paths.db"))
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
    return await db.create_account(
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


async def _create_bill(db: Database, *, user_id: int, **overrides: Any) -> int:
    payload = {
        "date": "2026-03-05 12:00:00",
        "type": "支出",
        "amount": -28.5,
        "counterparty": "域测试商户",
        "description": "域测试描述",
        "payment_method": "支付宝",
        "main_category": "域测试餐饮",
        "sub_category": "午餐",
        "source_account_id": 0,
        "destination_account_id": 0,
        "destination_amount": 0.0,
    }
    payload.update(overrides)
    bill_id = await db.create_bill(payload, user_id=user_id)
    assert bill_id is not None
    return int(bill_id)


async def _insert_import_learning_rule(db: Database, *, user_id: int, match_value: str, enabled: bool = True) -> int:
    conn = await db._get_connection()
    now = datetime.now().isoformat()
    normalized_value = db._normalize_import_learning_text(match_value)
    cursor = await conn.execute(
        """
        INSERT INTO import_learning_rules (
            user_id, match_type, match_value, normalized_match_value,
            learned_type, learned_category_id,
            learned_source_account_id, learned_destination_account_id,
            enabled, source_session_id, source_preview_id,
            created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        (
            user_id,
            "description",
            match_value,
            normalized_value,
            "支出",
            None,
            None,
            None,
            1 if enabled else 0,
            "pytest-session",
            None,
            now,
            now,
        ),
    )
    await conn.commit()
    return int(cursor.lastrowid or 0)


@pytest.mark.asyncio
async def test_bill_crud_and_query_filters_cover_account_category_keyword_and_amount_paths(tmp_path: Path) -> None:
    """账单 CRUD 与筛选应覆盖 account/category/keyword/amount 主链。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_deep_crud")
        matched_account_id = await _create_account(db, user_id=user_id, name="命中账户")
        other_account_id = await _create_account(db, user_id=user_id, name="排除账户")

        matched_bill_id = await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-05 12:00:00",
            description="域测试命中账单",
            main_category="域测试餐饮",
            sub_category="午餐",
            source_account_id=matched_account_id,
        )
        other_bill_id = await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-06 12:00:00",
            description="域测试错误账户",
            main_category="域测试餐饮",
            sub_category="午餐",
            source_account_id=other_account_id,
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-04-01 12:00:00",
            description="域测试错误分类",
            main_category="域测试交通",
            sub_category="地铁",
            source_account_id=matched_account_id,
            amount=-12.0,
        )

        filtered_bills = await db.get_bills(
            filters={
                "start_date": "2026-03-01",
                "end_date": "2026-03-31",
                "keyword": "域测试命中",
                "account_ids": [matched_account_id],
                "categories": [{"main": "域测试餐饮", "sub": "午餐"}],
                "amount_filter": "lt:0",
            },
            user_id=user_id,
        )
        assert [int(bill["id"]) for bill in filtered_bills] == [matched_bill_id]

        paged_bills, total_count = await db.query_bills(
            page=1,
            page_size=1,
            filters={"keyword": "域测试"},
            user_id=user_id,
        )
        assert len(paged_bills) == 1
        assert total_count == 3

        assert await db.update_bill(matched_bill_id, {"description": "已更新描述"}, user_id=user_id) is True
        refreshed_bill = await db.get_bill_by_id(matched_bill_id, user_id=user_id)
        assert refreshed_bill is not None
        assert refreshed_bill["description"] == "已更新描述"

        assert await db.delete_bill(other_bill_id, user_id=user_id) is True
        assert await db.get_bill_by_id(other_bill_id, user_id=user_id) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_session_lifecycle_and_cleanup_keep_active_sessions(tmp_path: Path) -> None:
    """会话创建、失效与清理应只移除过期会话。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_deep_sessions")
        now = datetime.now()

        active_session_id = await db.create_session(
            {
                "user_id": user_id,
                "token_hash": "active-token",
                "refresh_token_hash": "active-refresh",
                "expires_at": (now + timedelta(days=1)).isoformat(),
                "refresh_expires_at": (now + timedelta(days=7)).isoformat(),
                "user_agent": "pytest-agent",
                "ip_address": "127.0.0.1",
            }
        )
        expired_session_id = await db.create_session(
            {
                "user_id": user_id,
                "token_hash": "expired-token",
                "refresh_token_hash": "expired-refresh",
                "expires_at": (now - timedelta(days=2)).isoformat(),
                "refresh_expires_at": (now - timedelta(days=1)).isoformat(),
                "user_agent": "pytest-agent",
                "ip_address": "127.0.0.1",
            }
        )
        other_active_session_id = await db.create_session(
            {
                "user_id": user_id,
                "token_hash": "other-token",
                "refresh_token_hash": "other-refresh",
                "expires_at": (now + timedelta(days=2)).isoformat(),
                "refresh_expires_at": (now + timedelta(days=8)).isoformat(),
                "user_agent": "pytest-agent",
                "ip_address": "127.0.0.1",
            }
        )

        active_session = await db.get_session_by_token_hash("active-token")
        assert active_session is not None
        assert int(active_session["id"]) == active_session_id
        assert active_session["username"] == "db_deep_sessions"
        assert active_session["email"] == "db_deep_sessions@example.com"
        assert int(active_session["user_is_active"]) == 1

        active_sessions = await db.get_user_sessions(user_id)
        assert {int(session["id"]) for session in active_sessions} == {
            active_session_id,
            expired_session_id,
            other_active_session_id,
        }

        assert await db.invalidate_session("expired-token") is True
        assert await db.get_session_by_token_hash("expired-token") is None

        cleaned_count = await db.cleanup_expired_sessions()
        assert cleaned_count == 1

        remaining_sessions = await db.get_user_sessions(user_id)
        assert {int(session["id"]) for session in remaining_sessions} == {
            active_session_id,
            other_active_session_id,
        }

        invalidated_other_count = await db.invalidate_other_user_sessions(user_id, active_session_id)
        assert invalidated_other_count == 1
        final_sessions = await db.get_user_sessions(user_id)
        assert [int(session["id"]) for session in final_sessions] == [active_session_id]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_group_invariants_auto_sync_primary_and_cascade_delete(tmp_path: Path) -> None:
    """二级预算应自动同步一级预算，删除一级预算时应级联删除整组。"""
    db = await _create_database(tmp_path)
    try:
        now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
        secondary_breakfast_id = await db.create_budget(
            {
                "name": "早餐预算",
                "category": "域测试餐饮",
                "sub_category": "早餐",
                "period_type": "monthly",
                "amount": 120.0,
                "start_date": "2026-03-01",
                "end_date": None,
                "alert_threshold": 80,
                "enabled": 1,
                "created_at": now,
                "updated_at": now,
            },
            user_id=1,
        )
        secondary_lunch_id = await db.create_budget(
            {
                "name": "午餐预算",
                "category": "域测试餐饮",
                "sub_category": "午餐",
                "period_type": "monthly",
                "amount": 80.0,
                "start_date": "2026-03-01",
                "end_date": None,
                "alert_threshold": 80,
                "enabled": 1,
                "created_at": now,
                "updated_at": now,
            },
            user_id=1,
        )

        primary_budget = await db.get_primary_category_budget("域测试餐饮", "monthly", "2026-03-01", user_id=1)
        assert primary_budget is not None
        assert primary_budget["amount"] == pytest.approx(200.0)

        assert await db.update_budget(primary_budget["id"], {"amount": 50.0, "updated_at": now}, user_id=1) is True
        refreshed_primary = await db.get_primary_category_budget("域测试餐饮", "monthly", "2026-03-01", user_id=1)
        assert refreshed_primary is not None
        assert refreshed_primary["amount"] == pytest.approx(200.0)

        assert await db.delete_budget(primary_budget["id"], user_id=1) is True
        assert await db.get_primary_category_budget("域测试餐饮", "monthly", "2026-03-01", user_id=1) is None
        assert await db.get_budget_by_id(secondary_breakfast_id, user_id=1) is None
        assert await db.get_budget_by_id(secondary_lunch_id, user_id=1) is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_import_learning_rule_enable_usage_and_delete_paths(tmp_path: Path) -> None:
    """导入学习规则应支持统计、启停、命中计数与删除。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_deep_learning")
        rule_id = await _insert_import_learning_rule(db, user_id=user_id, match_value="域测试学习规则")

        enabled_rules = await db.get_import_learning_rules(user_id=user_id, enabled_only=True, limit=20)
        assert [int(rule["id"]) for rule in enabled_rules] == [rule_id]
        assert await db.count_import_learning_rules(user_id=user_id, enabled_only=True) == 1

        assert await db.increment_import_learning_rule_usage([rule_id, rule_id], user_id=user_id) == 1
        updated_rule = (await db.get_import_learning_rules(user_id=user_id, enabled_only=False, limit=20))[0]
        assert int(updated_rule["applied_count"]) == 1
        assert updated_rule["last_applied_at"]

        assert await db.set_import_learning_rule_enabled(rule_id, False, user_id=user_id) is True
        assert await db.count_import_learning_rules(user_id=user_id, enabled_only=True) == 0

        assert await db.delete_import_learning_rule(rule_id, user_id=user_id) is True
        assert await db.count_import_learning_rules(user_id=user_id, enabled_only=False) == 0
    finally:
        await db.close()
