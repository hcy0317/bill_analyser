from __future__ import annotations

from datetime import date
from typing import TYPE_CHECKING, Any, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import budgets as budgets_module

if TYPE_CHECKING:
    from collections.abc import Callable


@pytest.fixture(name="budgets_route_app")
def budgets_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct budgets route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeBudgetDB:
    """Synchronous DB stub when budgets route async runner is patched to identity."""

    def __init__(self) -> None:
        self.execution_calls: list[dict[str, Any]] = []
        self.forecast_calls: list[dict[str, Any]] = []
        self.snapshot_calls: list[dict[str, Any]] = []
        self.history_calls: list[dict[str, Any]] = []

    def get_budget_execution_details(self, **kwargs: Any) -> list[dict[str, Any]]:
        self.execution_calls.append(dict(kwargs))
        return [
            {
                "id": 9,
                "budget_amount": 100.0,
                "spent_amount": 40.0,
                "remaining_amount": 60.0,
                "execution_rate": 40.0,
            }
        ]

    def get_period_forecast(self, **kwargs: Any) -> list[dict[str, Any]]:
        self.forecast_calls.append(dict(kwargs))
        return [
            {
                "category": "餐饮",
                "forecast_amount": 30.0,
                "backtest_mape": None,
                "confidence": "low",
            },
            {
                "category": "交通",
                "forecast_amount": 20.0,
                "backtest_mape": 12.5,
                "confidence": "medium",
            },
        ]

    def create_budget_execution_snapshots(self, **kwargs: Any) -> dict[str, Any]:
        self.snapshot_calls.append(dict(kwargs))
        return {
            "created_count": 1,
            "period_start": kwargs["start_date"],
            "period_end": kwargs["end_date"],
            "filter_summary": "{}",
            "calculated_at": "2026-04-05 12:00:00",
        }

    def get_budget_execution_history(self, **kwargs: Any) -> list[dict[str, Any]]:
        self.history_calls.append(dict(kwargs))
        return [
            {
                "budget_id": kwargs.get("budget_id") or 3,
                "period_start": kwargs["start_date"],
                "period_end": kwargs["end_date"],
                "spent_amount": 88.0,
            }
        ]


class FakeToday(date):
    """Deterministic today() provider for forecast route assertions."""

    @classmethod
    def today(cls) -> FakeToday:
        return cls(2026, 4, 15)



def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)



def test_budget_forecast_route_resolves_scope_and_averages_only_non_null_mape(
    budgets_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """预算 forecast 路由应透传 resolved scope，并忽略 None 的回测误差均值。"""
    db = FakeBudgetDB()
    monkeypatch.setattr(budgets_module, "_run_async", lambda value: value)
    monkeypatch.setattr(budgets_module, "get_app_context", lambda: db)
    monkeypatch.setattr(budgets_module, "date", FakeToday)

    get_period_forecast = _unwrap(budgets_module.get_period_forecast)

    with budgets_route_app.test_request_context(
        "/api/budgets/forecast?budget_type=5&period_type=monthly&"
        "start_date=2026-04-01&end_date=2026-04-30&"
        "forecast_strategy=moving_average&months_history=3"
    ):
        setattr(cast("Any", budgets_module.request), "user_id", 42)
        payload = get_period_forecast().get_json() or {}

    assert payload["success"] is True
    assert payload["result"]["period_start"] == "2026-04-01"
    assert payload["result"]["period_end"] == "2026-04-30"
    assert payload["result"]["daysElapsed"] == 15
    assert payload["result"]["daysRemaining"] == 15
    assert payload["result"]["summary"] == {
        "total_forecast": 50.0,
        "count": 2,
        "forecast_strategy": "moving_average",
        "history_periods": 3,
        "avg_backtest_mape": 12.5,
        "days_elapsed": 15,
        "days_remaining": 15,
    }
    assert db.forecast_calls == [
        {
            "budget_type": 5,
            "period_type": "monthly",
            "start_date": "2026-04-01",
            "end_date": "2026-04-30",
            "forecast_strategy": "moving_average",
            "history_periods": 3,
            "user_id": 42,
        }
    ]



def test_budget_execution_route_parses_integer_filters_and_resolved_quarter_scope(
    budgets_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """预算 execution 路由应解析整数筛选参数，并把季度范围透传给 DB。"""
    db = FakeBudgetDB()
    monkeypatch.setattr(budgets_module, "_run_async", lambda value: value)
    monkeypatch.setattr(budgets_module, "get_app_context", lambda: db)

    get_budget_execution = _unwrap(budgets_module.get_budget_execution)

    with budgets_route_app.test_request_context(
        "/api/budgets/execution?budget_type=3&period_type=quarterly&year=2026&quarter=2&"
        "budget_id=9&category_id=12&account_ids=4,5&tag_ids=7,8"
    ):
        setattr(cast("Any", budgets_module.request), "user_id", 7)
        payload = get_budget_execution().get_json() or {}

    assert payload["success"] is True
    assert payload["result"]["summary"] == {
        "total_budget": 100.0,
        "total_spent": 40.0,
        "total_remaining": 60.0,
        "overall_execution_rate": 40.0,
        "count": 1,
    }
    assert payload["result"]["period_start"] == "2026-04-01"
    assert payload["result"]["period_end"] == "2026-06-30"
    assert db.execution_calls == [
        {
            "budget_type": 3,
            "period_type": "quarterly",
            "start_date": "2026-04-01",
            "end_date": "2026-06-30",
            "budget_id": 9,
            "category_id": 12,
            "account_ids": [4, 5],
            "tag_ids": [7, 8],
            "user_id": 7,
        }
    ]



def test_budget_history_snapshot_route_normalizes_empty_json_arrays_to_none(
    budgets_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """预算快照路由应接受空 JSON 数组，并向 DB 透传 None 过滤器。"""
    db = FakeBudgetDB()
    monkeypatch.setattr(budgets_module, "_run_async", lambda value: value)
    monkeypatch.setattr(budgets_module, "get_app_context", lambda: db)

    create_budget_history_snapshot = _unwrap(budgets_module.create_budget_history_snapshot)

    with budgets_route_app.test_request_context(
        "/api/budgets/history/snapshot",
        method="POST",
        json={
            "budget_type": 3,
            "period_type": "yearly",
            "year": 2026,
            "budget_id": 11,
            "category_id": 15,
            "account_ids": [],
            "tag_ids": [],
        },
    ):
        setattr(cast("Any", budgets_module.request), "user_id", 99)
        payload = create_budget_history_snapshot().get_json() or {}

    assert payload == {
        "success": True,
        "result": {
            "created_count": 1,
            "period_start": "2026-01-01",
            "period_end": "2026-12-31",
            "filter_summary": "{}",
            "calculated_at": "2026-04-05 12:00:00",
        },
    }
    assert db.snapshot_calls == [
        {
            "budget_type": 3,
            "period_type": "yearly",
            "start_date": "2026-01-01",
            "end_date": "2026-12-31",
            "budget_id": 11,
            "category_id": 15,
            "account_ids": None,
            "tag_ids": None,
            "user_id": 99,
        }
    ]
