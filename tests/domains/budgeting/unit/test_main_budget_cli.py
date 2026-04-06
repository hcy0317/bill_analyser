from __future__ import annotations

from datetime import datetime
from typing import Any, cast

import main as main_module


def test_resolve_budget_period_window_for_month_quarter_and_year(monkeypatch) -> None:
    """CLI 预算窗口解析应返回当前月、季度和年度的闭区间。"""

    class FixedDatetime(datetime):
        @classmethod
        def now(cls, tz=None):
            return cls(2026, 4, 15, 12, 0, 0)

    monkeypatch.setattr(main_module, "datetime", FixedDatetime)

    assert main_module._resolve_budget_period_window("month") == (
        "monthly",
        "2026-04-01",
        "2026-04-30",
    )
    assert main_module._resolve_budget_period_window("quarter") == (
        "quarterly",
        "2026-04-01",
        "2026-06-30",
    )
    assert main_module._resolve_budget_period_window("year") == (
        "yearly",
        "2026-01-01",
        "2026-12-31",
    )


def test_build_budget_cli_report_aggregates_summary_and_status_counts() -> None:
    """CLI 预算报告组装应计算汇总金额、分类条目和状态计数。"""
    report = cast(
        "dict[str, Any]",
        main_module._build_budget_cli_report(
            "month",
            [
                {
                    "id": 1,
                    "name": "早餐预算",
                    "category": "餐饮",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 100.0,
                    "spent_amount": 50.0,
                    "remaining_amount": 50.0,
                    "execution_rate": 50.0,
                },
                {
                    "id": 2,
                    "name": "午餐预算",
                    "category": "午餐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 100.0,
                    "spent_amount": 85.0,
                    "remaining_amount": 15.0,
                    "execution_rate": 85.0,
                },
                {
                    "id": 3,
                    "name": "交通预算",
                    "category": "交通",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 100.0,
                    "spent_amount": 95.0,
                    "remaining_amount": 5.0,
                    "execution_rate": 95.0,
                },
                {
                    "id": 4,
                    "name": "娱乐预算",
                    "category": "娱乐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 100.0,
                    "spent_amount": 120.0,
                    "remaining_amount": -20.0,
                    "execution_rate": 120.0,
                },
            ],
        ),
    )

    assert report["period"] == "month"
    assert report["summary"] == {
        "total_budget": 400.0,
        "total_spent": 350.0,
        "total_remaining": 50.0,
        "overall_execution_rate": 87.5,
        "count": 4,
        "normal_count": 1,
        "warning_count": 1,
        "critical_count": 1,
        "exceeded_count": 1,
    }
    assert report["categories"]["早餐预算"]["status"] == "normal"
    assert report["categories"]["午餐预算"]["status"] == "warning"
    assert report["categories"]["交通预算"]["status"] == "critical"
    assert report["categories"]["娱乐预算"]["status"] == "exceeded"
    assert "generated_at" in report


def test_build_budget_cli_report_prefers_sub_budgets_over_synchronized_primary_budget() -> None:
    """同组存在子预算时，CLI 汇总不应把同步一级预算重复计入。"""
    report = cast(
        "dict[str, Any]",
        main_module._build_budget_cli_report(
            "month",
            [
                {
                    "id": 1,
                    "name": "餐饮总预算",
                    "category": "餐饮",
                    "sub_category": "",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "end_date": "2026-04-30",
                    "budget_amount": 200.0,
                    "spent_amount": 130.0,
                    "remaining_amount": 70.0,
                    "execution_rate": 65.0,
                },
                {
                    "id": 2,
                    "name": "早餐预算",
                    "category": "餐饮",
                    "sub_category": "早餐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "end_date": "2026-04-30",
                    "budget_amount": 120.0,
                    "spent_amount": 80.0,
                    "remaining_amount": 40.0,
                    "execution_rate": 66.7,
                },
                {
                    "id": 3,
                    "name": "午餐预算",
                    "category": "餐饮",
                    "sub_category": "午餐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "end_date": "2026-04-30",
                    "budget_amount": 80.0,
                    "spent_amount": 50.0,
                    "remaining_amount": 30.0,
                    "execution_rate": 62.5,
                },
            ],
        ),
    )

    assert report["summary"]["total_budget"] == 200.0
    assert report["summary"]["total_spent"] == 130.0
    assert report["summary"]["count"] == 2
    assert set(report["categories"]) == {"早餐预算", "午餐预算"}


def test_build_budget_cli_report_makes_duplicate_labels_unique() -> None:
    """同名预算在 CLI 明细中应保留为不同条目，而不是静默覆盖。"""
    report = cast(
        "dict[str, Any]",
        main_module._build_budget_cli_report(
            "month",
            [
                {
                    "id": 11,
                    "name": "重复预算",
                    "category": "餐饮",
                    "sub_category": "早餐",
                    "budget_amount": 50.0,
                    "spent_amount": 20.0,
                    "remaining_amount": 30.0,
                    "execution_rate": 40.0,
                },
                {
                    "id": 12,
                    "name": "重复预算",
                    "category": "餐饮",
                    "sub_category": "午餐",
                    "budget_amount": 80.0,
                    "spent_amount": 30.0,
                    "remaining_amount": 50.0,
                    "execution_rate": 37.5,
                },
            ],
        ),
    )

    assert len(report["categories"]) == 2
    assert "重复预算" in report["categories"]
    assert "重复预算 [餐饮/午餐]" in report["categories"]


def test_build_budget_cli_report_keeps_primary_budget_when_it_has_manual_headroom() -> None:
    """当一级预算高于子预算合计时，CLI summary/detail 都应保留一级预算口径。"""
    report = cast(
        "dict[str, Any]",
        main_module._build_budget_cli_report(
            "month",
            [
                {
                    "id": 21,
                    "name": "餐饮总预算",
                    "category": "餐饮",
                    "sub_category": "",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 180.0,
                    "spent_amount": 130.0,
                    "remaining_amount": 50.0,
                    "execution_rate": 72.22,
                },
                {
                    "id": 22,
                    "name": "早餐预算",
                    "category": "餐饮",
                    "sub_category": "早餐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 100.0,
                    "spent_amount": 80.0,
                    "remaining_amount": 20.0,
                    "execution_rate": 80.0,
                },
                {
                    "id": 23,
                    "name": "午餐预算",
                    "category": "餐饮",
                    "sub_category": "午餐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 30.0,
                    "spent_amount": 50.0,
                    "remaining_amount": -20.0,
                    "execution_rate": 166.67,
                },
            ],
        ),
    )

    assert report["summary"]["total_budget"] == 180.0
    assert report["summary"]["total_spent"] == 130.0
    assert report["summary"]["count"] == 1
    assert set(report["categories"]) == {"餐饮总预算"}


def test_build_budget_cli_report_does_not_drop_ambiguous_multiple_primary_budgets() -> None:
    """同组存在多个一级预算时，CLI 不应静默丢掉其中任何一条。"""
    report = cast(
        "dict[str, Any]",
        main_module._build_budget_cli_report(
            "month",
            [
                {
                    "id": 31,
                    "name": "一级预算-A",
                    "category": "餐饮",
                    "sub_category": "",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 100.0,
                    "spent_amount": 40.0,
                    "remaining_amount": 60.0,
                    "execution_rate": 40.0,
                },
                {
                    "id": 32,
                    "name": "一级预算-B",
                    "category": "餐饮",
                    "sub_category": "",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 80.0,
                    "spent_amount": 20.0,
                    "remaining_amount": 60.0,
                    "execution_rate": 25.0,
                },
                {
                    "id": 33,
                    "name": "早餐预算",
                    "category": "餐饮",
                    "sub_category": "早餐",
                    "period_type": "monthly",
                    "start_date": "2026-04-01",
                    "budget_amount": 30.0,
                    "spent_amount": 10.0,
                    "remaining_amount": 20.0,
                    "execution_rate": 33.33,
                },
            ],
        ),
    )

    assert report["summary"]["total_budget"] == 210.0
    assert report["summary"]["total_spent"] == 70.0
    assert report["summary"]["count"] == 3
    assert set(report["categories"]) == {"一级预算-A", "一级预算-B", "早餐预算"}
