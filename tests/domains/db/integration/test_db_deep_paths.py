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


@pytest.mark.asyncio
async def test_saved_filter_round_trip_covers_create_update_lookup_and_delete(tmp_path: Path) -> None:
    """保存的筛选条件应覆盖创建、更新、查询与删除路径。"""
    db = await _create_database(tmp_path)
    try:
        created_user_id = await _create_user(db, "db_saved_filters")
        assert created_user_id == 1

        filter_id = await db.save_filter(
            "域测试筛选",
            {"keyword": "奶茶", "amount_filter": "lt:0"},
            description="初始描述",
        )
        assert filter_id > 0

        by_id = await db.get_saved_filter(filter_id=filter_id)
        assert by_id is not None
        assert by_id["name"] == "域测试筛选"
        assert by_id["description"] == "初始描述"
        assert by_id["filter_data"] == {"keyword": "奶茶", "amount_filter": "lt:0"}

        updated_filter_id = await db.save_filter(
            "域测试筛选",
            {"keyword": "咖啡", "categories": [{"main": "餐饮", "sub": "饮品"}]},
            description="已更新描述",
        )
        assert updated_filter_id == filter_id

        by_name = await db.get_saved_filter(name="域测试筛选")
        assert by_name is not None
        assert by_name["id"] == filter_id
        assert by_name["description"] == "已更新描述"
        assert by_name["filter_data"] == {
            "keyword": "咖啡",
            "categories": [{"main": "餐饮", "sub": "饮品"}],
        }

        second_filter_id = await db.save_filter(
            "域测试第二筛选",
            {"type": "支出", "account_ids": [1, 2]},
            description=None,
        )
        assert second_filter_id > filter_id

        saved_filters = await db.get_saved_filters()
        assert {saved_filter["name"] for saved_filter in saved_filters} == {"域测试筛选", "域测试第二筛选"}
        saved_filter_by_name = {saved_filter["name"]: saved_filter for saved_filter in saved_filters}
        assert saved_filter_by_name["域测试筛选"]["filter_data"]["keyword"] == "咖啡"
        assert saved_filter_by_name["域测试第二筛选"]["filter_data"] == {"type": "支出", "account_ids": [1, 2]}

        assert await db.delete_saved_filter(filter_id=filter_id) is True
        assert await db.get_saved_filter(filter_id=filter_id) is None
        assert await db.delete_saved_filter(name="域测试第二筛选") is True
        assert await db.get_saved_filter(name="域测试第二筛选") is None
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_saved_filter_missing_arguments_and_missing_rows_return_safe_defaults(tmp_path: Path) -> None:
    """筛选条件查询/删除在缺少参数或记录不存在时应返回安全默认值。"""
    db = await _create_database(tmp_path)
    try:
        created_user_id = await _create_user(db, "db_saved_filters_defaults")
        assert created_user_id == 1

        assert await db.get_saved_filter() is None
        assert await db.delete_saved_filter() is False
        assert await db.get_saved_filter(filter_id=999999) is None
        assert await db.get_saved_filter(name="不存在的筛选") is None
        assert await db.delete_saved_filter(filter_id=999999) is False
        assert await db.delete_saved_filter(name="不存在的筛选") is False
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_batch_update_bills_and_categories_cover_empty_success_and_missing_ids(tmp_path: Path) -> None:
    """批量更新账单应覆盖空输入、成功更新、缺失 ID 与分类包装路径。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_batch_update")
        other_user_id = await _create_user(db, "db_batch_update_other")
        first_bill_id = await _create_bill(db, user_id=user_id, description="待更新账单一")
        second_bill_id = await _create_bill(db, user_id=user_id, description="待更新账单二")
        other_user_bill_id = await _create_bill(db, user_id=other_user_id, description="其他用户账单")

        assert await db.batch_update_bills([], {"description": "不会执行"}, user_id=user_id) == {
            "success_count": 0,
            "failed_count": 0,
            "failed_ids": [],
        }
        assert await db.batch_update_bills([first_bill_id], {}, user_id=user_id) == {
            "success_count": 0,
            "failed_count": 0,
            "failed_ids": [],
        }
        with pytest.raises(ValueError, match="unsupported batch update fields: user_id"):
            await db.batch_update_bills([first_bill_id], {"user_id": other_user_id}, user_id=user_id)

        update_result = await db.batch_update_bills(
            [first_bill_id, second_bill_id, other_user_bill_id, 999999],
            {"description": "批量更新后的描述", "payment_method": "云闪付"},
            user_id=user_id,
        )
        assert update_result["success_count"] == 2
        assert update_result["failed_count"] == 2
        assert update_result["failed_ids"] == [other_user_bill_id, 999999]

        updated_first_bill = await db.get_bill_by_id(first_bill_id, user_id=user_id)
        updated_second_bill = await db.get_bill_by_id(second_bill_id, user_id=user_id)
        untouched_other_user_bill = await db.get_bill_by_id(other_user_bill_id, user_id=other_user_id)
        assert updated_first_bill is not None and updated_second_bill is not None
        assert untouched_other_user_bill is not None
        assert updated_first_bill["description"] == "批量更新后的描述"
        assert updated_second_bill["payment_method"] == "云闪付"
        assert untouched_other_user_bill["description"] == "其他用户账单"

        category_result = await db.batch_update_categories(
            [first_bill_id, 888888],
            "批量分类",
            "晚餐",
            user_id=user_id,
        )
        assert category_result["success_count"] == 1
        assert category_result["failed_count"] == 1
        assert category_result["failed_ids"] == [888888]

        recategorized_bill = await db.get_bill_by_id(first_bill_id, user_id=user_id)
        assert recategorized_bill is not None
        assert recategorized_bill["main_category"] == "批量分类"
        assert recategorized_bill["sub_category"] == "晚餐"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_update_account_balance_covers_add_subtract_invalid_operation_and_missing_account(
    tmp_path: Path,
) -> None:
    """账户余额更新应覆盖加减、非法操作和账户不存在分支。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_account_balance")
        account_id = await _create_account(db, user_id=user_id, name="余额账户")

        assert await db.update_account_balance(account_id, 25.5, operation="add") is True
        account_after_add = await db.get_account_by_id(account_id, user_id=user_id)
        assert account_after_add is not None
        assert account_after_add["balance"] == pytest.approx(25.5)

        assert await db.update_account_balance(account_id, 5.5, operation="subtract") is True
        account_after_subtract = await db.get_account_by_id(account_id, user_id=user_id)
        assert account_after_subtract is not None
        assert account_after_subtract["balance"] == pytest.approx(20.0)

        assert await db.update_account_balance(account_id, 1.0, operation="multiply") is False
        assert await db.update_account_balance(999999, 10.0, operation="add") is False
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_audit_logs_and_backup_records_cover_filters_metadata_merge_and_invalid_json(
    tmp_path: Path,
) -> None:
    """审计日志与备份记录应覆盖筛选、JSON 容错和 metadata 合并更新。"""
    db = await _create_database(tmp_path)
    try:
        move_log_id = await db.create_audit_log(
            operation_type="move_transactions",
            operation_target="account",
            target_id=10,
            details={"from_account": 1, "to_account": 2},
            affected_count=2,
            status="success",
        )
        assert move_log_id > 0

        await db.create_audit_log(
            operation_type="delete_transactions",
            operation_target="bill",
            target_id=11,
            details={"deleted_ids": [1, 2]},
            affected_count=2,
            status="failed",
            error_message="permission denied",
        )

        conn = await db._get_connection()
        await conn.execute(
            """
            INSERT INTO audit_logs (
                operation_type, operation_target, target_id, details,
                affected_count, ip_address, user_agent, session_id,
                status, error_message, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                "backup_record",
                "backup",
                12,
                "{broken-json",
                0,
                None,
                None,
                None,
                "failed",
                "broken payload",
                datetime.now().isoformat(),
            ),
        )
        await conn.commit()

        filtered_logs = await db.get_audit_logs(
            operation_type="move_transactions",
            operation_target="account",
            target_id=10,
            status="success",
        )
        assert len(filtered_logs) == 1
        assert filtered_logs[0]["details"] == {"from_account": 1, "to_account": 2}
        assert filtered_logs[0]["affected_count"] == 2

        failed_backup_logs = await db.get_audit_logs(operation_type="backup_record", status="failed")
        assert len(failed_backup_logs) == 1
        assert failed_backup_logs[0]["details"] == "{broken-json"

        backup_record_id = await db.create_backup_record(
            {
                "backup_name": "backup-1.zip",
                "storage_type": "local",
                "file_path": "C:/tmp/backup-1.zip",
                "checksum": "abc123",
                "encrypted": False,
                "status": "created",
                "metadata": {"size": 123, "note": "initial"},
            }
        )
        assert backup_record_id > 0

        broken_record_id = await db.create_backup_record(
            {
                "backup_name": "broken.zip",
                "storage_type": "local",
                "file_path": "C:/tmp/broken.zip",
                "checksum": "broken",
                "encrypted": False,
                "status": "created",
                "metadata": {},
            }
        )
        assert broken_record_id >= 0

        await conn.execute(
            "UPDATE backup_records SET metadata_json = ? WHERE backup_name = ?",
            ("{broken-json", "broken.zip"),
        )
        await conn.commit()

        assert await db.update_backup_record_by_filename(
            "backup-1.zip",
            {"status": "deleted", "encrypted": True, "metadata": {"deleted_by": "pytest"}},
        ) is True
        assert await db.update_backup_record_by_filename("backup-1.zip", {}) is False
        assert await db.update_backup_record_by_filename("", {"status": "deleted"}) is False
        assert await db.update_backup_record_by_filename("missing.zip", {"status": "deleted"}) is False

        backup_records = await db.get_backup_records()
        records_by_name = {record["backup_name"]: record for record in backup_records}
        assert records_by_name["backup-1.zip"]["status"] == "deleted"
        assert records_by_name["backup-1.zip"]["encrypted"] is True
        assert records_by_name["backup-1.zip"]["metadata"] == {
            "size": 123,
            "note": "initial",
            "deleted_by": "pytest",
        }
        assert records_by_name["broken.zip"]["metadata"] == {}
    finally:
        await db.close()
