"""Budget-focused integration coverage tests for db.py."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core.db import Database

# pylint: disable=line-too-long,too-many-arguments,too-many-locals,too-many-statements


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_budget_paths.db"))
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


async def _create_category(
    db: Database,
    *,
    user_id: int,
    main_category: str,
    sub_category: str,
    category_type: int = 3,
    icon: str = "",
    color: str = "",
) -> int:
    category_id = await db.create_category(
        {
            "type": category_type,
            "main_category": main_category,
            "sub_category": sub_category,
            "description": f"{main_category}/{sub_category}",
            "priority": 0,
            "keywords": "",
            "hidden": False,
            "icon": icon,
            "color": color,
        },
        user_id=user_id,
    )
    assert category_id is not None
    return int(category_id)


async def _create_tag(db: Database, *, user_id: int, name: str) -> int:
    return int(
        await db.create_tag(
            {
                "name": name,
                "color": "#ff8800",
                "icon": "tag",
                "hidden": False,
            },
            user_id=user_id,
        )
    )


async def _add_tags_to_bill(db: Database, *, bill_id: int, user_id: int, tag_ids: list[int]) -> None:
    assert await db.add_tags_to_bill(bill_id, tag_ids, user_id=user_id) is True


async def _create_bill(db: Database, *, user_id: int, **overrides: Any) -> int:
    payload = {
        "date": "2026-03-05 12:00:00",
        "type": "支出",
        "amount": -28.5,
        "counterparty": "预算路径商户",
        "description": "预算路径账单",
        "payment_method": "测试银行卡",
        "main_category": "预算测试分类",
        "sub_category": "午餐",
        "source_account_id": 0,
        "destination_account_id": 0,
        "destination_amount": 0.0,
    }
    payload.update(overrides)
    bill_id = await db.create_bill(payload, user_id=user_id)
    assert bill_id is not None
    return int(bill_id)


async def _create_budget(db: Database, *, user_id: int, **overrides: Any) -> int:
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    payload = {
        "name": "预算路径预算",
        "category": "预算测试分类",
        "sub_category": "午餐",
        "period_type": "monthly",
        "amount": 100.0,
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": 1,
        "created_at": now,
        "updated_at": now,
    }
    payload.update(overrides)
    return int(await db.create_budget(payload, user_id=user_id))


@pytest.mark.asyncio
async def test_budget_execution_snapshots_and_history_cover_filters_replace_and_fallback_query(
    tmp_path: Path,
) -> None:
    """预算执行详情/快照/历史应覆盖账户/标签过滤与快照优先路径。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_execution")
        included_account_id = await _create_account(db, user_id=user_id, name="命中预算账户")
        excluded_account_id = await _create_account(db, user_id=user_id, name="排除预算账户")
        matching_tag_id = await _create_tag(db, user_id=user_id, name="预算命中标签")
        other_tag_id = await _create_tag(db, user_id=user_id, name="预算排除标签")

        await _create_category(
            db,
            user_id=user_id,
            main_category="预算测试分类",
            sub_category="",
            icon="fork-spoon",
            color="#ffaa00",
        )
        sub_category_id = await _create_category(
            db,
            user_id=user_id,
            main_category="预算测试分类",
            sub_category="午餐",
            icon="",
            color="#ffaa00",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category="预算测试交通",
            sub_category="地铁",
            icon="train",
            color="#00aaff",
        )

        budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="午餐预算",
            category="预算测试分类",
            sub_category="午餐",
            amount=100.0,
        )

        matching_bill_id = await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-05 12:00:00",
            amount=-80.0,
            main_category="预算测试分类",
            sub_category="午餐",
            source_account_id=included_account_id,
            description="命中预算账单",
        )
        other_tag_bill_id = await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-06 12:00:00",
            amount=-30.0,
            main_category="预算测试分类",
            sub_category="午餐",
            source_account_id=included_account_id,
            description="标签过滤账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-06 18:00:00",
            amount=-25.0,
            main_category="预算测试分类",
            sub_category="午餐",
            source_account_id=excluded_account_id,
            description="账户过滤账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-07 12:00:00",
            amount=-15.0,
            main_category="预算测试交通",
            sub_category="地铁",
            source_account_id=included_account_id,
            description="分类过滤账单",
        )
        await _add_tags_to_bill(db, bill_id=matching_bill_id, user_id=user_id, tag_ids=[matching_tag_id])
        await _add_tags_to_bill(db, bill_id=other_tag_bill_id, user_id=user_id, tag_ids=[other_tag_id])

        execution_details = await db.get_budget_execution_details(
            budget_type=3,
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            category_id=sub_category_id,
            account_ids=[included_account_id],
            tag_ids=[matching_tag_id],
            user_id=user_id,
        )

        assert len(execution_details) == 1
        detail = execution_details[0]
        assert detail["id"] == budget_id
        assert detail["category_id"] == str(sub_category_id)
        assert detail["category_info"] is not None
        assert detail["spent_amount"] == pytest.approx(80.0)
        assert detail["remaining_amount"] == pytest.approx(20.0)
        assert detail["execution_rate"] == pytest.approx(80.0)

        on_demand_history = await db.get_budget_execution_history(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            category_id=sub_category_id,
            account_ids=[included_account_id],
            tag_ids=[matching_tag_id],
            user_id=user_id,
        )
        assert len(on_demand_history) == 1
        assert on_demand_history[0]["spent_amount"] == pytest.approx(80.0)
        assert on_demand_history[0]["status"] == "within_budget"
        assert on_demand_history[0]["filter_summary"]

        first_snapshot = await db.create_budget_execution_snapshots(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            category_id=sub_category_id,
            account_ids=[included_account_id],
            tag_ids=[matching_tag_id],
            user_id=user_id,
        )
        assert first_snapshot["created_count"] == 1
        assert first_snapshot["period_start"] == "2026-03-01"
        assert first_snapshot["period_end"] == "2026-03-31"

        late_matching_bill_id = await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-08 12:00:00",
            amount=-35.0,
            main_category="预算测试分类",
            sub_category="午餐",
            source_account_id=included_account_id,
            description="超预算账单",
        )
        await _add_tags_to_bill(db, bill_id=late_matching_bill_id, user_id=user_id, tag_ids=[matching_tag_id])

        history_from_snapshot = await db.get_budget_execution_history(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            category_id=sub_category_id,
            account_ids=[included_account_id],
            tag_ids=[matching_tag_id],
            user_id=user_id,
        )
        assert len(history_from_snapshot) == 1
        assert history_from_snapshot[0]["spent_amount"] == pytest.approx(80.0)
        assert history_from_snapshot[0]["status"] == "within_budget"

        replaced_snapshot = await db.create_budget_execution_snapshots(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            category_id=sub_category_id,
            account_ids=[included_account_id],
            tag_ids=[matching_tag_id],
            user_id=user_id,
        )
        assert replaced_snapshot["created_count"] == 1
        assert replaced_snapshot["filter_summary"] == first_snapshot["filter_summary"]

        stored_history = await db.get_budget_execution_history(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            category_id=sub_category_id,
            account_ids=[included_account_id],
            tag_ids=[matching_tag_id],
            user_id=user_id,
        )

        assert len(stored_history) == 1
        assert stored_history[0]["budget_id"] == budget_id
        assert stored_history[0]["status"] == "over_budget"
        assert stored_history[0]["spent_amount"] == pytest.approx(115.0)
        assert stored_history[0]["execution_rate"] == pytest.approx(115.0)
    finally:
        await db.close()



@pytest.mark.asyncio
async def test_budget_forecast_import_and_export_cover_strategy_trend_and_invalid_rows(
    tmp_path: Path,
) -> None:
    """预算预测与导入导出应覆盖策略/趋势/创建更新/错误分支。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_forecast")
        await _create_category(
            db,
            user_id=user_id,
            main_category="预测预算分类",
            sub_category="",
            icon="chart-line",
            color="#44aa44",
        )
        await _create_budget(
            db,
            user_id=user_id,
            name="预测预算",
            category="预测预算分类",
            sub_category="",
            amount=100.0,
            start_date="2026-01-01",
            end_date="2026-12-31",
        )

        for date_text, amount in [
            ("2026-01-05 12:00:00", -100.0),
            ("2026-02-05 12:00:00", -110.0),
            ("2026-03-05 12:00:00", -120.0),
        ]:
            await _create_bill(
                db,
                user_id=user_id,
                date=date_text,
                amount=amount,
                main_category="预测预算分类",
                sub_category="",
                description=f"预测账单 {date_text}",
            )

        moving_forecast = await db.get_period_forecast(
            budget_type=3,
            period_type="monthly",
            start_date="2026-01-01",
            end_date="2026-03-31",
            forecast_strategy="moving_average",
            history_periods=3,
            user_id=user_id,
        )
        assert moving_forecast

        forecast_item = next(item for item in moving_forecast if item["category"] == "预测预算分类")
        assert forecast_item["forecast_strategy"] == "moving_average"
        assert forecast_item["strategy_explanation"].startswith("基于最近")
        assert forecast_item["trend"] == "up"
        assert forecast_item["backtest_mape"] is not None
        assert forecast_item["confidence"] in {"medium", "high"}
        assert forecast_item["budget_amount"] == pytest.approx(100.0)
        assert forecast_item["projected_over_budget"] is True
        assert len(forecast_item["periods"]) == 3

        daily_forecast = await db.get_period_forecast(
            budget_type=3,
            period_type="daily",
            start_date="2026-01-01",
            end_date="2026-03-31",
            forecast_strategy="historical_average",
            history_periods=2,
            user_id=user_id,
        )
        yearly_forecast = await db.get_period_forecast(
            budget_type=3,
            period_type="yearly",
            start_date="2026-01-01",
            end_date="2026-12-31",
            forecast_strategy="historical_average",
            history_periods=2,
            user_id=user_id,
        )
        quarterly_forecast = await db.get_period_forecast(
            budget_type=3,
            period_type="quarterly",
            start_date="2026-01-01",
            end_date="2026-03-31",
            forecast_strategy="historical_average",
            history_periods=2,
            user_id=user_id,
        )

        assert any(item["category"] == "预测预算分类" for item in daily_forecast)
        assert any(item["category"] == "预测预算分类" for item in quarterly_forecast)
        assert any(item["category"] == "预测预算分类" for item in yearly_forecast)

        import_result = await db.import_budgets(
            [
                {
                    "name": "导入预算A",
                    "category": "导入预算分类",
                    "sub_category": "",
                    "period_type": "monthly",
                    "amount": 40.0,
                    "start_date": "2026-04-01",
                    "end_date": "2026-04-30",
                },
                {
                    "name": "导入预算A",
                    "category": "导入预算分类",
                    "sub_category": "",
                    "period_type": "monthly",
                    "amount": 55.0,
                    "start_date": "2026-04-01",
                    "end_date": "2026-04-30",
                },
                {
                    "name": "",
                    "category": "缺失名称分类",
                    "period_type": "monthly",
                    "amount": 10.0,
                    "start_date": "2026-04-01",
                },
            ],
            user_id=user_id,
        )

        assert import_result == {
            "created": 1,
            "updated": 1,
            "errors": 1,
            "error_details": ["第3条: 缺少必填字段(name或amount)"],
        }

        exported_budgets = await db.export_budgets(user_id=user_id)
        exported_by_name = {budget["name"]: budget for budget in exported_budgets}

        assert "导入预算A" in exported_by_name
        assert exported_by_name["导入预算A"]["amount"] == pytest.approx(55.0)
        assert all("id" not in budget for budget in exported_budgets)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_queries_include_bills_on_end_date_with_time_component(tmp_path: Path) -> None:
    """预算执行与预测应包含 end_date 当天带时间的账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_end_date")
        await _create_category(
            db,
            user_id=user_id,
            main_category="结束日预算分类",
            sub_category="",
            icon="calendar",
            color="#3377ff",
        )
        budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="结束日预算",
            category="结束日预算分类",
            sub_category="",
            amount=100.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-31 23:59:59",
            amount=-45.0,
            main_category="结束日预算分类",
            sub_category="",
            description="结束日账单",
        )

        execution_details = await db.get_budget_execution_details(
            budget_type=3,
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            user_id=user_id,
        )
        assert len(execution_details) == 1
        assert execution_details[0]["spent_amount"] == pytest.approx(45.0)

        forecast_items = await db.get_period_forecast(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            user_id=user_id,
        )
        assert len(forecast_items) == 1
        assert forecast_items[0]["category"] == "结束日预算分类"
        assert forecast_items[0]["total_amount"] == pytest.approx(45.0)
        assert forecast_items[0]["current_spent"] == pytest.approx(45.0)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_history_merges_snapshots_with_missing_periods(tmp_path: Path) -> None:
    """预算历史跨周期查询应保留已有快照，并补算未落库周期。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_history_merge")
        await _create_category(
            db,
            user_id=user_id,
            main_category="混合快照分类",
            sub_category="",
            icon="calendar-sync",
            color="#8844ff",
        )
        budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="混合快照预算",
            category="混合快照分类",
            sub_category="",
            amount=100.0,
            start_date="2026-01-01",
            end_date="2026-02-28",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-01-10 12:00:00",
            amount=-10.0,
            main_category="混合快照分类",
            sub_category="",
            description="一月快照账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-02-15 12:00:00",
            amount=-20.0,
            main_category="混合快照分类",
            sub_category="",
            description="二月实时账单",
        )

        january_snapshot = await db.create_budget_execution_snapshots(
            budget_type=3,
            period_type="monthly",
            start_date="2026-01-01",
            end_date="2026-01-31",
            budget_id=budget_id,
            user_id=user_id,
        )
        assert january_snapshot["created_count"] == 1

        await _create_bill(
            db,
            user_id=user_id,
            date="2026-01-20 12:00:00",
            amount=-90.0,
            main_category="混合快照分类",
            sub_category="",
            description="一月快照后新增账单",
        )

        history_items = await db.get_budget_execution_history(
            budget_type=3,
            period_type="monthly",
            start_date="2026-01-01",
            end_date="2026-02-28",
            budget_id=budget_id,
            user_id=user_id,
        )

        assert len(history_items) == 2
        january_item = next(item for item in history_items if item["period_start"] == "2026-01-01")
        february_item = next(item for item in history_items if item["period_start"] == "2026-02-01")
        assert january_item["spent_amount"] == pytest.approx(10.0)
        assert january_item["status"] == "within_budget"
        assert february_item["spent_amount"] == pytest.approx(20.0)
        assert february_item["status"] == "within_budget"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_execution_uses_destination_account_filter_and_budget_window_intersection(
    tmp_path: Path,
) -> None:
    """预算执行应命中 destination_account_id，并只统计请求区间与预算定义交集内的账单。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_destination_overlap")
        included_account_id = await _create_account(db, user_id=user_id, name="目标命中账户")
        excluded_account_id = await _create_account(db, user_id=user_id, name="目标排除账户")
        await _create_category(
            db,
            user_id=user_id,
            main_category="交集预算分类",
            sub_category="",
            icon="wallet",
            color="#4455ff",
        )
        budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="交集预算",
            category="交集预算分类",
            sub_category="",
            amount=100.0,
            start_date="2026-03-10",
            end_date="2026-03-20",
        )

        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-09 08:00:00",
            amount=-7.0,
            main_category="交集预算分类",
            sub_category="",
            destination_account_id=included_account_id,
            description="预算前账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-10 08:00:00",
            amount=-11.0,
            main_category="交集预算分类",
            sub_category="",
            destination_account_id=included_account_id,
            description="预算起始日账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-20 21:00:00",
            amount=-13.0,
            main_category="交集预算分类",
            sub_category="",
            destination_account_id=included_account_id,
            description="预算结束日账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-21 08:00:00",
            amount=-17.0,
            main_category="交集预算分类",
            sub_category="",
            destination_account_id=included_account_id,
            description="预算后账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-15 08:00:00",
            amount=-19.0,
            main_category="交集预算分类",
            sub_category="",
            destination_account_id=excluded_account_id,
            description="其他目标账户账单",
        )

        execution_details = await db.get_budget_execution_details(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            account_ids=[included_account_id],
            user_id=user_id,
        )
        history_items = await db.get_budget_execution_history(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            account_ids=[included_account_id],
            user_id=user_id,
        )

        assert len(execution_details) == 1
        assert execution_details[0]["spent_amount"] == pytest.approx(24.0)
        assert execution_details[0]["remaining_amount"] == pytest.approx(76.0)
        assert execution_details[0]["execution_rate"] == pytest.approx(24.0)

        assert len(history_items) == 1
        assert history_items[0]["spent_amount"] == pytest.approx(24.0)
        assert history_items[0]["period_start"] == "2026-03-01"
        assert history_items[0]["period_end"] == "2026-03-31"
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_execution_filters_same_category_budgets_by_period_type(tmp_path: Path) -> None:
    """同分类多周期预算查询时，应只返回与查询周期一致的预算。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_period_type")
        await _create_category(
            db,
            user_id=user_id,
            main_category="同分类多周期预算",
            sub_category="",
            icon="layers",
            color="#00aa88",
        )
        monthly_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="月预算",
            category="同分类多周期预算",
            sub_category="",
            period_type="monthly",
            amount=100.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )
        yearly_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="年预算",
            category="同分类多周期预算",
            sub_category="",
            period_type="yearly",
            amount=1200.0,
            start_date="2026-01-01",
            end_date="2026-12-31",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-18 12:00:00",
            amount=-30.0,
            main_category="同分类多周期预算",
            sub_category="",
            description="同分类多周期账单",
        )

        monthly_details = await db.get_budget_execution_details(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            user_id=user_id,
        )
        yearly_details = await db.get_budget_execution_details(
            budget_type=3,
            period_type="yearly",
            start_date="2026-01-01",
            end_date="2026-12-31",
            user_id=user_id,
        )

        assert [item["id"] for item in monthly_details] == [monthly_budget_id]
        assert [item["id"] for item in yearly_details] == [yearly_budget_id]
        assert monthly_details[0]["spent_amount"] == pytest.approx(30.0)
        assert yearly_details[0]["spent_amount"] == pytest.approx(30.0)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_history_supports_daily_and_weekly_period_ranges(tmp_path: Path) -> None:
    """预算历史动态补算应支持 daily / weekly 分桶。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_daily_weekly_history")
        await _create_category(
            db,
            user_id=user_id,
            main_category="日周预算分类",
            sub_category="",
            icon="calendar-range",
            color="#2266cc",
        )
        daily_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="日预算",
            category="日周预算分类",
            sub_category="",
            period_type="daily",
            amount=20.0,
            start_date="2026-03-10",
            end_date="2026-03-11",
        )
        weekly_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="周预算",
            category="日周预算分类",
            sub_category="",
            period_type="weekly",
            amount=100.0,
            start_date="2026-03-09",
            end_date="2026-03-22",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-10 08:00:00",
            amount=-8.0,
            main_category="日周预算分类",
            sub_category="",
            description="日预算-第一天",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-11 08:00:00",
            amount=-6.0,
            main_category="日周预算分类",
            sub_category="",
            description="日预算-第二天",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-18 08:00:00",
            amount=-20.0,
            main_category="日周预算分类",
            sub_category="",
            description="周预算-第二周",
        )

        daily_history = await db.get_budget_execution_history(
            budget_type=3,
            period_type="daily",
            start_date="2026-03-10",
            end_date="2026-03-11",
            budget_id=daily_budget_id,
            user_id=user_id,
        )
        weekly_history = await db.get_budget_execution_history(
            budget_type=3,
            period_type="weekly",
            start_date="2026-03-10",
            end_date="2026-03-18",
            budget_id=weekly_budget_id,
            user_id=user_id,
        )

        assert [item["period_start"] for item in daily_history] == ["2026-03-11", "2026-03-10"]
        assert [item["spent_amount"] for item in daily_history] == [pytest.approx(6.0), pytest.approx(8.0)]
        assert [item["period_start"] for item in weekly_history] == ["2026-03-16", "2026-03-09"]
        assert [item["spent_amount"] for item in weekly_history] == [pytest.approx(20.0), pytest.approx(14.0)]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_history_reuses_weekly_snapshots_for_non_boundary_ranges(tmp_path: Path) -> None:
    """非周边界查询也应命中已落库的 weekly 快照，而不是回退实时重算。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_weekly_snapshot_overlap")
        await _create_category(
            db,
            user_id=user_id,
            main_category="周快照重用分类",
            sub_category="",
            icon="calendar-week",
            color="#8844cc",
        )
        budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="周快照预算",
            category="周快照重用分类",
            sub_category="",
            period_type="weekly",
            amount=100.0,
            start_date="2026-03-09",
            end_date="2026-03-22",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-10 08:00:00",
            amount=-14.0,
            main_category="周快照重用分类",
            sub_category="",
            description="快照前第一周账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-18 08:00:00",
            amount=-20.0,
            main_category="周快照重用分类",
            sub_category="",
            description="快照前第二周账单",
        )

        snapshot_result = await db.create_budget_execution_snapshots(
            budget_type=3,
            period_type="weekly",
            start_date="2026-03-10",
            end_date="2026-03-18",
            budget_id=budget_id,
            user_id=user_id,
        )
        assert snapshot_result["created_count"] == 1

        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-12 08:00:00",
            amount=-30.0,
            main_category="周快照重用分类",
            sub_category="",
            description="快照后补录第一周账单",
        )

        history_items = await db.get_budget_execution_history(
            budget_type=3,
            period_type="weekly",
            start_date="2026-03-10",
            end_date="2026-03-18",
            budget_id=budget_id,
            user_id=user_id,
        )

        assert len(history_items) == 1
        assert history_items[0]["period_start"] == "2026-03-10"
        assert history_items[0]["period_end"] == "2026-03-18"
        assert history_items[0]["spent_amount"] == pytest.approx(34.0)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_history_exact_match_filters_out_only_overlapping_snapshots(tmp_path: Path) -> None:
    """命中 exact 区间快照时，不应把仅重叠的其他快照一并返回。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_history_exact_snapshot")
        await _create_category(
            db,
            user_id=user_id,
            main_category="精确快照分类",
            sub_category="",
            icon="calendar-check",
            color="#2288aa",
        )
        budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="精确快照预算",
            category="精确快照分类",
            sub_category="",
            amount=100.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-12 12:00:00",
            amount=-10.0,
            main_category="精确快照分类",
            sub_category="",
            description="exact 区间账单",
        )
        await _create_bill(
            db,
            user_id=user_id,
            date="2026-03-25 12:00:00",
            amount=-20.0,
            main_category="精确快照分类",
            sub_category="",
            description="仅月度重叠账单",
        )

        exact_snapshot = await db.create_budget_execution_snapshots(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-10",
            end_date="2026-03-18",
            budget_id=budget_id,
            user_id=user_id,
        )
        overlapping_snapshot = await db.create_budget_execution_snapshots(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-01",
            end_date="2026-03-31",
            budget_id=budget_id,
            user_id=user_id,
        )

        assert exact_snapshot["created_count"] == 1
        assert overlapping_snapshot["created_count"] == 1

        history_items = await db.get_budget_execution_history(
            budget_type=3,
            period_type="monthly",
            start_date="2026-03-10",
            end_date="2026-03-18",
            budget_id=budget_id,
            user_id=user_id,
        )

        assert len(history_items) == 1
        assert history_items[0]["period_start"] == "2026-03-10"
        assert history_items[0]["period_end"] == "2026-03-18"
        assert history_items[0]["spent_amount"] == pytest.approx(10.0)
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_lookup_normalizes_blank_sub_category_and_syncs_groups_when_secondary_moves(
    tmp_path: Path,
) -> None:
    """预算查询应归一化空白子分类，并在二级预算迁组后同步新旧父预算。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_group_move")
        await _create_category(
            db,
            user_id=user_id,
            main_category="旧预算分类",
            sub_category="",
            icon="",
            color="",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category="旧预算分类",
            sub_category="午餐",
            icon="fork-spoon",
            color="#ffaa00",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category="新预算分类",
            sub_category="",
            icon="",
            color="",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category="新预算分类",
            sub_category="通勤",
            icon="train",
            color="#00aaff",
        )

        secondary_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="待迁移二级预算",
            category="旧预算分类",
            sub_category="午餐",
            amount=50.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )

        primary_before_move = await db.get_primary_category_budget(
            "旧预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        assert primary_before_move is not None
        assert primary_before_move["amount"] == pytest.approx(50.0)

        blank_sub_category_lookup = await db.get_budget_by_category(
            "旧预算分类",
            "   ",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        assert blank_sub_category_lookup is not None
        assert int(blank_sub_category_lookup["id"]) == int(primary_before_move["id"])

        assert (
            await db.update_budget(
                secondary_budget_id,
                {
                    "category": "新预算分类",
                    "sub_category": "通勤",
                    "amount": 70.0,
                    "updated_at": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
                },
                user_id=user_id,
            )
            is True
        )

        old_primary_after_move = await db.get_primary_category_budget(
            "旧预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        new_primary_after_move = await db.get_primary_category_budget(
            "新预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        moved_secondary_budget = await db.get_budget_by_category(
            "新预算分类",
            "通勤",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )

        assert old_primary_after_move is not None
        assert old_primary_after_move["amount"] == pytest.approx(50.0)
        assert await db.get_sub_category_budgets_total(
            "旧预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        ) == pytest.approx(0.0)
        assert new_primary_after_move is not None
        assert new_primary_after_move["amount"] == pytest.approx(70.0)
        assert moved_secondary_budget is not None
        assert int(moved_secondary_budget["id"]) == secondary_budget_id
        assert (
            await db.get_budget_by_category(
                "旧预算分类",
                "午餐",
                "monthly",
                "2026-03-01",
                user_id=user_id,
            )
            is None
        )
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_delete_keeps_primary_after_secondary_delete_and_cascades_on_primary_delete(
    tmp_path: Path,
) -> None:
    """删除二级预算不应误删父预算，删除父预算时应级联删除同组预算。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_delete_group")
        await _create_category(
            db,
            user_id=user_id,
            main_category="删除预算分类",
            sub_category="",
            icon="",
            color="",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category="删除预算分类",
            sub_category="午餐",
            icon="fork-spoon",
            color="#ffaa00",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category="删除预算分类",
            sub_category="晚餐",
            icon="food",
            color="#ff6699",
        )

        lunch_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="午餐预算",
            category="删除预算分类",
            sub_category="午餐",
            amount=20.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )
        dinner_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="晚餐预算",
            category="删除预算分类",
            sub_category="晚餐",
            amount=30.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )

        primary_budget = await db.get_primary_category_budget(
            "删除预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        assert primary_budget is not None
        assert primary_budget["amount"] == pytest.approx(50.0)

        assert await db.delete_budget(lunch_budget_id, user_id=user_id) is True

        primary_after_secondary_delete = await db.get_primary_category_budget(
            "删除预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        remaining_secondary_budget = await db.get_budget_by_category(
            "删除预算分类",
            "晚餐",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )

        assert primary_after_secondary_delete is not None
        assert primary_after_secondary_delete["amount"] == pytest.approx(50.0)
        assert remaining_secondary_budget is not None
        assert int(remaining_secondary_budget["id"]) == dinner_budget_id
        assert await db.get_sub_category_budgets_total(
            "删除预算分类",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        ) == pytest.approx(30.0)

        assert await db.delete_budget(int(primary_budget["id"]), user_id=user_id) is True
        assert (
            await db.get_primary_category_budget(
                "删除预算分类",
                "monthly",
                "2026-03-01",
                user_id=user_id,
            )
            is None
        )
        assert (
            await db.get_budget_by_category(
                "删除预算分类",
                "晚餐",
                "monthly",
                "2026-03-01",
                user_id=user_id,
            )
            is None
        )
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_budget_group_sync_preserves_raw_category_values_across_create_and_lookup(
    tmp_path: Path,
) -> None:
    """预算分组联动应对齐写库值，不能在 helper 里偷偷改写分类键语义。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_budget_raw_group_key")
        raw_category = "  原样预算分类  "

        await _create_category(
            db,
            user_id=user_id,
            main_category=raw_category,
            sub_category="",
            icon="",
            color="",
        )
        await _create_category(
            db,
            user_id=user_id,
            main_category=raw_category,
            sub_category="午餐",
            icon="fork-spoon",
            color="#ffaa00",
        )

        secondary_budget_id = await _create_budget(
            db,
            user_id=user_id,
            name="原样键二级预算",
            category=raw_category,
            sub_category="午餐",
            amount=20.0,
            start_date="2026-03-01",
            end_date="2026-03-31",
        )

        primary_budget = await db.get_primary_category_budget(
            raw_category,
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )
        secondary_budget = await db.get_budget_by_category(
            raw_category,
            "午餐",
            "monthly",
            "2026-03-01",
            user_id=user_id,
        )

        assert primary_budget is not None
        assert primary_budget["amount"] == pytest.approx(20.0)
        assert secondary_budget is not None
        assert int(secondary_budget["id"]) == secondary_budget_id
        assert await db.get_sub_category_budgets_total(
            raw_category,
            "monthly",
            "2026-03-01",
            user_id=user_id,
        ) == pytest.approx(20.0)
    finally:
        await db.close()
