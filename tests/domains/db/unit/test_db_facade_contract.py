from __future__ import annotations

import inspect
from pathlib import Path

import pytest

from bill_analyser.core.db import Database


def _build_user_payload(username: str) -> dict[str, object]:
    return {
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


def _build_account_payload(name: str) -> dict[str, object]:
    return {
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
    }


def _build_budget_payload() -> dict[str, object]:
    return {
        "name": "门面契约预算",
        "category": "门面契约分类",
        "sub_category": "",
        "period_type": "monthly",
        "amount": 88.0,
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": 1,
        "created_at": "2026-03-01 00:00:00",
        "updated_at": "2026-03-01 00:00:00",
    }


def test_database_facade_preserves_selected_surface_and_keyword_parameters() -> None:
    """Database façade 应持续暴露跨域代表性方法与关键关键字参数。"""
    method_expectations = {
        "init_db": set(),
        "close": set(),
        "create_user": {"data"},
        "create_session": {"data"},
        "replace_two_factor_recovery_codes": {"user_id", "recovery_codes"},
        "create_account": {"data", "user_id"},
        "get_account_alias_mapping": {"user_id"},
        "create_budget": {"data", "user_id"},
        "get_budget_execution_details": {
            "budget_type",
            "period_type",
            "start_date",
            "end_date",
            "account_ids",
            "tag_ids",
            "user_id",
        },
        "get_period_forecast": {
            "budget_type",
            "period_type",
            "start_date",
            "end_date",
            "forecast_strategy",
            "history_periods",
            "user_id",
        },
        "batch_update_bills": {"bill_ids", "updates", "user_id"},
        "save_import_config": {"data", "user_id"},
        "get_import_session": {"session_id", "user_id"},
        "get_preview_by_session": {"session_id", "user_id", "selected_only"},
        "create_backup_record": {"payload"},
    }
    helper_names = {
        "_normalize_import_learning_text",
        "build_composite_match_hash",
        "_build_budget_history_filter_summary",
        "_normalize_budget_query_end_date",
    }

    constructor_signature = inspect.signature(Database)
    assert "db_path" in constructor_signature.parameters

    for method_name, expected_parameters in method_expectations.items():
        assert hasattr(Database, method_name), f"Database 缺少公共方法 {method_name}"
        method = getattr(Database, method_name)
        assert callable(method), f"Database.{method_name} 不是可调用对象"

        signature = inspect.signature(method)
        parameter_names = set(signature.parameters)
        assert expected_parameters.issubset(parameter_names), (
            f"Database.{method_name} 参数漂移: 缺少 {sorted(expected_parameters - parameter_names)}"
        )

    for helper_name in helper_names:
        assert hasattr(Database, helper_name), f"Database 缺少 helper {helper_name}"
        assert callable(getattr(Database, helper_name))


@pytest.mark.asyncio
async def test_database_async_context_manager_initializes_and_close_is_idempotent(tmp_path: Path) -> None:
    """Database 应继续支持公共导入入口、异步上下文和重复 close。"""
    db = Database(str(tmp_path / "test_db_facade_context.db"))

    async with db as opened_db:
        stats = await opened_db.get_statistics()
        assert stats["total_bills"] == 0

        user_id = await opened_db.create_user(_build_user_payload("facade_context_user"))
        created_user = await opened_db.get_user_by_id(user_id)
        assert created_user is not None
        assert created_user["username"] == "facade_context_user"

    await db.close()


@pytest.mark.asyncio
async def test_database_single_instance_public_methods_work_across_domains(tmp_path: Path) -> None:
    """同一个 Database 实例应能通过公共 API 跑通跨域最小烟雾链路。"""
    db = Database(str(tmp_path / "test_db_facade_smoke.db"))
    await db.init_db()

    try:
        user_id = await db.create_user(_build_user_payload("facade_smoke_user"))
        account_id = await db.create_account(_build_account_payload("门面契约账户"), user_id=user_id)
        budget_id = await db.create_budget(_build_budget_payload(), user_id=user_id)
        filter_id = await db.save_filter("门面契约筛选", {"keyword": "门面测试"}, description="门面契约")
        replaced_count = await db.replace_two_factor_recovery_codes(user_id, ["ABCD-1234"])
        backup_id = await db.create_backup_record(
            {
                "backup_name": "facade-contract.zip",
                "storage_type": "local",
                "file_path": str(tmp_path / "facade-contract.zip"),
                "checksum": "checksum-123",
                "encrypted": False,
                "status": "created",
                "metadata": {"scope": "facade-smoke"},
            }
        )

        created_user = await db.get_user_by_id(user_id)
        created_account = await db.get_account_by_id(account_id, user_id=user_id)
        created_budget = await db.get_budget_by_id(budget_id, user_id=user_id)
        saved_filter = await db.get_saved_filter(filter_id=filter_id)
        backup_records = await db.get_backup_records()

        assert created_user is not None
        assert created_user["username"] == "facade_smoke_user"
        assert created_account is not None
        assert created_account["name"] == "门面契约账户"
        assert created_budget is not None
        assert created_budget["name"] == "门面契约预算"
        assert saved_filter is not None
        assert saved_filter["name"] == "门面契约筛选"
        assert replaced_count == 1
        assert await db.count_active_two_factor_recovery_codes(user_id) == 1
        assert backup_id >= 0
        assert any(record["backup_name"] == "facade-contract.zip" for record in backup_records)
    finally:
        await db.close()
