from __future__ import annotations

import asyncio
from datetime import datetime
from typing import Any, Callable, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import statistics as statistics_module


@pytest.fixture(name="statistics_deep_app")
def statistics_deep_app_fixture() -> Flask:
    """Create a tiny Flask app for deep statistics route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeLoop:
    """Minimal event-loop adapter for statistics routes that create their own loop."""

    def __init__(self) -> None:
        self.closed = False

    def run_until_complete(self, coroutine):
        return asyncio.run(coroutine)

    def close(self) -> None:
        self.closed = True


class FakeDeepStatisticsDB:
    """Async DB stub for deep categorical/trend/asset statistics routes."""

    def __init__(
        self,
        *,
        bills: list[dict[str, Any]] | None = None,
        categories: list[dict[str, Any]] | None = None,
        accounts: list[dict[str, Any]] | None = None,
        balances_before_date: dict[int, float] | None = None,
    ) -> None:
        self.bills = bills or []
        self.categories = categories or []
        self.accounts = accounts or []
        self.balances_before_date = balances_before_date or {}

    async def query_bills(self, *_, **__) -> tuple[list[dict[str, Any]], int]:
        return self.bills, len(self.bills)

    async def get_all_categories(self, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return self.categories

    async def get_all_accounts(self, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return self.accounts

    async def get_balances_before_date(self, date: str, user_id: int) -> dict[int, float]:
        _ = (date, user_id)
        return self.balances_before_date



def _install_fake_loop(monkeypatch: pytest.MonkeyPatch, loop: FakeLoop) -> None:
    monkeypatch.setattr(statistics_module.asyncio, "new_event_loop", lambda: loop)
    monkeypatch.setattr(statistics_module.asyncio, "set_event_loop", lambda _loop: None)



def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast(Any, func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast(Callable[..., Any], current)

    second = getattr(first, "__wrapped__", None)
    return cast(Callable[..., Any], second or first)



def test_get_categorical_analysis_covers_invalid_timestamp_and_successful_aggregation(
    statistics_deep_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """分类统计应覆盖坏时间戳与成功聚合路径。"""
    route = _unwrap(statistics_module.get_categorical_analysis)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics?startTime=bad&endTime=1"
    ):
        response, status = route()
        assert status == 400
        assert response.get_json()["error"].startswith("Invalid timestamp format")

    db = FakeDeepStatisticsDB(
        bills=[
            {
                "id": 1,
                "date": "2026-03-01T08:00:00",
                "type": "支出",
                "amount": 18.5,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
            {
                "id": 2,
                "date": "2026-03-01T09:00:00",
                "type": "收入",
                "amount": 5,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
        ],
        categories=[{"id": 1, "main_category": "餐饮", "sub_category": "早餐"}],
        accounts=[{"id": 10, "name": "现金"}],
    )
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics?startTime=1740787200&endTime=1740873599"
    ):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": {
            "startTime": 1740787200,
            "endTime": 1740873599,
            "items": [{"categoryId": "1", "accountId": "10", "amount": -1350}],
        },
    }


def test_get_categorical_analysis_covers_reverse_range_and_all_mode(
    statistics_deep_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """分类统计应覆盖反向区间校验和全量模式聚合。"""
    route = _unwrap(statistics_module.get_categorical_analysis)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics?startTime=1740873599&endTime=1740787200"
    ):
        response, status = route()
        assert status == 400
        assert response.get_json()["error"] == "Invalid time range"

    db = FakeDeepStatisticsDB(
        bills=[
            {
                "id": 1,
                "date": "2026-03-01T08:00:00",
                "type": "支出",
                "amount": 10,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            }
        ],
        categories=[{"id": 1, "main_category": "餐饮", "sub_category": "早餐"}],
        accounts=[{"id": 10, "name": "现金"}],
    )
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics?startTime=0&endTime=0"
    ):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": {
            "startTime": 0,
            "endTime": 0,
            "items": [{"categoryId": "1", "accountId": "10", "amount": -1000}],
        },
    }



def test_get_trend_analysis_covers_invalid_format_range_all_mode_and_monthly_output(
    statistics_deep_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """趋势分析应覆盖坏年月、反向范围、全量空结果和正常月份聚合。"""
    route = _unwrap(statistics_module.get_trend_analysis)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics/trends?startYearMonth=bad&endYearMonth=202603"
    ):
        response, status = route()
        assert status == 400
        assert response.get_json()["error"].startswith("Invalid year-month format")

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics/trends?startYearMonth=202604&endYearMonth=202603"
    ):
        response, status = route()
        assert status == 400
        assert response.get_json()["error"] == "Invalid year-month range"

    empty_db = FakeDeepStatisticsDB(bills=[])
    empty_loop = FakeLoop()
    _install_fake_loop(monkeypatch, empty_loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: empty_db)
    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics/trends?startYearMonth=0&endYearMonth=0"
    ):
        payload = route().get_json() or {}
        assert payload == {"success": True, "result": []}

    db = FakeDeepStatisticsDB(
        bills=[
            {
                "date": "2026-03-01T08:00:00",
                "type": "支出",
                "amount": 18.5,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
            {
                "date": "2026-04-02T08:00:00",
                "type": "收入",
                "amount": 6,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
        ],
        categories=[{"id": 1, "main_category": "餐饮", "sub_category": "早餐"}],
        accounts=[{"id": 10, "name": "现金"}],
    )
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)
    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics/trends?startYearMonth=202603&endYearMonth=202604"
    ):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": [
            {"year": 2026, "month": 3, "items": [{"categoryId": "1", "accountId": "10", "amount": -1850}]},
            {"year": 2026, "month": 4, "items": [{"categoryId": "1", "accountId": "10", "amount": 600}]},
        ],
    }


def test_get_trend_analysis_covers_all_mode_dynamic_range_and_bad_dates(
    statistics_deep_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """趋势分析应在全量模式下动态计算范围，并忽略坏日期账单。"""
    route = _unwrap(statistics_module.get_trend_analysis)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    db = FakeDeepStatisticsDB(
        bills=[
            {
                "date": "bad-date",
                "type": "支出",
                "amount": 1,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
            {
                "date": "2026-03-01T08:00:00",
                "type": "支出",
                "amount": 18.5,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
            {
                "date": "2026-05-02T08:00:00",
                "type": "收入",
                "amount": 6,
                "channel": "现金",
                "source_account_id": 10,
                "main_category": "餐饮",
                "sub_category": "早餐",
            },
        ],
        categories=[{"id": 1, "main_category": "餐饮", "sub_category": "早餐"}],
        accounts=[{"id": 10, "name": "现金"}],
    )
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)

    with statistics_deep_app.test_request_context(
        "/api/statistics/category-statistics/trends?startYearMonth=0&endYearMonth=0"
    ):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": [
            {"year": 2026, "month": 3, "items": [{"categoryId": "1", "accountId": "10", "amount": -1850}]},
            {"year": 2026, "month": 4, "items": []},
            {"year": 2026, "month": 5, "items": [{"categoryId": "1", "accountId": "10", "amount": 600}]},
        ],
    }



def test_get_asset_trends_covers_all_mode_empty_range_limit_and_success(
    statistics_deep_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """资产趋势应覆盖全量空结果、超长范围拒绝和单日余额计算。"""
    route = _unwrap(statistics_module.get_asset_trends)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    empty_db = FakeDeepStatisticsDB(bills=[])
    empty_loop = FakeLoop()
    _install_fake_loop(monkeypatch, empty_loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: empty_db)
    with statistics_deep_app.test_request_context(
        "/api/statistics/asset-trends?startTime=0&endTime=0"
    ):
        payload = route().get_json() or {}
        assert payload == {"success": True, "result": []}

    with statistics_deep_app.test_request_context(
        "/api/statistics/asset-trends?startTime=1704067200&endTime=1735776001"
    ):
        response, status = route()
        assert status == 400
        error_payload = response.get_json() or {}
        assert error_payload["errorCode"] == 400
        assert "365天范围" in error_payload["error"]

    db = FakeDeepStatisticsDB(
        bills=[
            {
                "date": "2025-03-01T12:00:00",
                "type": "支出",
                "amount": 20.0,
                "source_account_id": 10,
                "destination_account_id": 0,
            }
        ],
        accounts=[{"id": 10, "name": "现金", "initial_balance": 100.0}],
        balances_before_date={10: 5.0},
    )
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)
    with statistics_deep_app.test_request_context(
        "/api/statistics/asset-trends?startTime=1740787200&endTime=1740787200"
    ):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": [
            {
                "year": 2025,
                "month": 3,
                "day": 1,
                "items": [{"accountId": "10", "accountOpeningBalance": 10500, "accountClosingBalance": 8500}],
            }
        ],
    }


def test_get_asset_trends_covers_invalid_timestamp_reverse_range_default_month_and_all_mode(
    statistics_deep_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """资产趋势应覆盖时间戳错误、反向区间、默认月范围和全量动态范围。"""
    route = _unwrap(statistics_module.get_asset_trends)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    with statistics_deep_app.test_request_context(
        "/api/statistics/asset-trends?startTime=bad&endTime=1"
    ):
        response, status = route()
        assert status == 400
        assert response.get_json()["error"].startswith("Invalid timestamp format")

    with statistics_deep_app.test_request_context(
        "/api/statistics/asset-trends?startTime=2&endTime=1"
    ):
        response, status = route()
        assert status == 400
        assert response.get_json()["error"] == "Invalid time range"

    class FixedDateTime(datetime):
        @classmethod
        def now(cls, tz=None):
            _ = tz
            return cls(2025, 3, 15, 9, 0, 0)

    default_db = FakeDeepStatisticsDB(accounts=[])
    default_loop = FakeLoop()
    _install_fake_loop(monkeypatch, default_loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: default_db)
    monkeypatch.setattr(statistics_module, "datetime", FixedDateTime)

    with statistics_deep_app.test_request_context("/api/statistics/asset-trends"):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": [
            {"year": 2025, "month": 3, "day": day, "items": []}
            for day in range(1, 32)
        ],
    }

    all_mode_db = FakeDeepStatisticsDB(
        bills=[
            {
                "date": "2025-03-02T12:00:00",
                "type": "收入",
                "amount": 20.0,
                "source_account_id": 10,
                "destination_account_id": 0,
            },
            {
                "date": "2025-03-03T12:00:00",
                "type": "转账",
                "amount": 5.0,
                "source_account_id": 10,
                "destination_account_id": 11,
                "destination_amount": 4.5,
            },
        ],
        accounts=[
            {"id": 10, "name": "现金", "initial_balance": 100.0},
            {"id": 11, "name": "储蓄", "initial_balance": 20.0},
        ],
        balances_before_date={10: 0.0, 11: 0.0},
    )
    all_mode_loop = FakeLoop()
    _install_fake_loop(monkeypatch, all_mode_loop)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: all_mode_db)

    with statistics_deep_app.test_request_context(
        "/api/statistics/asset-trends?startTime=0&endTime=0"
    ):
        payload = route().get_json() or {}

    assert payload == {
        "success": True,
        "result": [
            {
                "year": 2025,
                "month": 3,
                "day": 2,
                "items": [
                    {"accountId": "10", "accountOpeningBalance": 10000, "accountClosingBalance": 12000},
                    {"accountId": "11", "accountOpeningBalance": 2000, "accountClosingBalance": 2000},
                ],
            },
            {
                "year": 2025,
                "month": 3,
                "day": 3,
                "items": [
                    {"accountId": "10", "accountOpeningBalance": 12000, "accountClosingBalance": 11500},
                    {"accountId": "11", "accountOpeningBalance": 2000, "accountClosingBalance": 2450},
                ],
            },
        ],
    }
