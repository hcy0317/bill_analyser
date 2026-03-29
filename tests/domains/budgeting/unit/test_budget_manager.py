from __future__ import annotations

from typing import Any, cast

import pytest

from bill_analyser.core import budget as budget_module
from bill_analyser.core.budget import BudgetManager, BudgetStatus


class LoggerRecorder:
    """Lightweight logger spy for budget tests."""

    def __init__(self) -> None:
        self.info_messages: list[str] = []
        self.warning_messages: list[str] = []
        self.error_messages: list[str] = []

    def info(self, message: str, *args: object) -> None:
        self.info_messages.append(message % args if args else message)

    def warning(self, message: str, *args: object) -> None:
        self.warning_messages.append(message % args if args else message)

    def error(self, message: str, *args: object) -> None:
        self.error_messages.append(message % args if args else message)


def _create_manager(logger: LoggerRecorder | None = None) -> BudgetManager:
    """Create a budget manager without booting a real database connection."""
    manager = BudgetManager(db=cast(Any, object()))
    if logger is not None:
        manager.logger = cast(Any, logger)
    return manager


@pytest.mark.asyncio
async def test_load_budgets_reads_config_and_logs(monkeypatch: pytest.MonkeyPatch) -> None:
    """预算加载应使用配置模块返回值，并更新内部预算表。"""
    logger = LoggerRecorder()
    monkeypatch.setattr(budget_module, "get_config", lambda filename: {"餐饮": 1000.0, "交通": 500.0})

    manager = _create_manager(logger)

    await manager.load_budgets("budget.json")

    assert manager.budgets == {"餐饮": 1000.0, "交通": 500.0}
    assert any("已加载 2 个预算项" in message for message in logger.info_messages)



def test_check_budget_status_handles_missing_budgets_and_alert_thresholds(monkeypatch: pytest.MonkeyPatch) -> None:
    """状态检查应覆盖缺省预算、阈值分档和触发警报的条件。"""
    manager = _create_manager()
    manager.budgets = {"餐饮": 100.0}
    triggered_alerts: list[tuple[str, float, float, BudgetStatus]] = []
    monkeypatch.setattr(
        manager,
        "_trigger_alert",
        lambda category, budget, spent, status: triggered_alerts.append((category, budget, spent, status)),
    )

    assert manager.check_budget_status("不存在", 999.0) == BudgetStatus.NORMAL
    assert manager.check_budget_status("餐饮", 79.99) == BudgetStatus.NORMAL
    assert manager.check_budget_status("餐饮", 80.0) == BudgetStatus.WARNING
    assert manager.check_budget_status("餐饮", 90.0) == BudgetStatus.CRITICAL
    assert manager.check_budget_status("餐饮", 100.0) == BudgetStatus.EXCEEDED
    assert triggered_alerts == [
        ("餐饮", 100.0, 80.0, BudgetStatus.WARNING),
        ("餐饮", 100.0, 90.0, BudgetStatus.CRITICAL),
        ("餐饮", 100.0, 100.0, BudgetStatus.EXCEEDED),
    ]



def test_trigger_alert_invokes_callbacks_and_logs_callback_failures() -> None:
    """警报触发应调用全部回调，并吞掉单个回调异常。"""
    logger = LoggerRecorder()
    manager = _create_manager(logger)

    received_alerts: list[tuple[str, float, float, BudgetStatus]] = []

    def good_callback(category: str, budget: float, spent: float, status: BudgetStatus) -> None:
        received_alerts.append((category, budget, spent, status))

    def bad_callback(*_args: object) -> None:
        raise RuntimeError("callback failed")

    manager.alert_callbacks = [good_callback, bad_callback]

    manager._trigger_alert("餐饮", 100.0, 120.0, BudgetStatus.EXCEEDED)

    assert received_alerts == [("餐饮", 100.0, 120.0, BudgetStatus.EXCEEDED)]
    assert any("预算警报" in message for message in logger.warning_messages)
    assert any("警报回调执行失败" in message for message in logger.error_messages)



def test_register_alert_callback_appends_callback_and_logs() -> None:
    """注册回调应把函数保存下来并记录日志。"""
    logger = LoggerRecorder()
    manager = _create_manager(logger)

    def callback(category: str, budget: float, spent: float, status: BudgetStatus) -> None:
        _ = (category, budget, spent, status)

    manager.register_alert_callback(callback)

    assert manager.alert_callbacks == [callback]
    assert any("已注册警报回调函数" in message for message in logger.info_messages)


@pytest.mark.asyncio
async def test_get_budget_report_summarizes_status_counts_and_zero_budget(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """预算报告应计算剩余金额、占比和各状态计数。"""
    logger = LoggerRecorder()

    class FakeAnalyzer:
        def __init__(self, db: object) -> None:
            self.db = db

        async def generate_report(self, period: str) -> dict[str, Any]:
            assert period == "month"
            return {
                "by_category": {
                    "餐饮": {"total": 85.0},
                    "交通": {"total": 95.0},
                    "零预算": {"total": 10.0},
                    "娱乐": {"total": 210.0},
                }
            }

    from bill_analyser.core import analyzer as analyzer_module

    monkeypatch.setattr(analyzer_module, "Analyzer", FakeAnalyzer)

    manager = _create_manager(logger)
    manager.budgets = {
        "餐饮": 100.0,
        "交通": 100.0,
        "零预算": 0.0,
        "娱乐": 200.0,
    }

    report = await manager.get_budget_report("month")

    assert report["period"] == "month"
    assert report["summary"] == {
        "total_budget": 400.0,
        "total_spent": 400.0,
        "normal_count": 1,
        "warning_count": 1,
        "critical_count": 1,
        "exceeded_count": 1,
    }
    assert report["categories"]["餐饮"] == {
        "budget": 100.0,
        "spent": 85.0,
        "remaining": 15.0,
        "usage_ratio": 85.0,
        "status": BudgetStatus.WARNING.value,
    }
    assert report["categories"]["交通"]["status"] == BudgetStatus.CRITICAL.value
    assert report["categories"]["娱乐"]["status"] == BudgetStatus.EXCEEDED.value
    assert report["categories"]["零预算"]["usage_ratio"] == 0
    assert report["categories"]["零预算"]["status"] == BudgetStatus.NORMAL.value
    assert "generated_at" in report
    assert any("预算报告生成完成" in message for message in logger.info_messages)
