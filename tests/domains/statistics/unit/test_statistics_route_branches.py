from __future__ import annotations

from collections.abc import Callable
from typing import Any, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import statistics as statistics_module


@pytest.fixture(name="statistics_route_app")
def statistics_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct Analyzer route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeAnalyzer:
    """Synchronous analyzer stub when _run_async is patched to identity."""

    def __init__(self, db: object) -> None:
        self.db = db

    def generate_report(
        self,
        period: str,
        filters: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return {
            "summary": {"total_income": 120.125, "total_expense": -35.567},
            "total_records": 3,
            "by_category": {"餐饮": 3557},
            "by_type": {"支出": 3557},
            "top_income": [{"amount": 120.125}],
            "top_expenses": [{"amount": 35.567}],
            "period": period,
            "start_date": (filters or {}).get("start_date", ""),
            "end_date": (filters or {}).get("end_date", ""),
        }

    def get_trends(self, period: str, category: str | None = None) -> dict[str, Any]:
        return {
            "period": period,
            "category": category,
            "trends": [{"period": "2026-03", "income": 100, "expense": 50, "net": 50}],
        }

    def get_comparison(self, period: str, compare_type: str = "category") -> dict[str, Any]:
        return {"period": period, "compare_type": compare_type, "items": [1, 2]}

    def analyze_category(self, period: str, main_category: str | None = None) -> dict[str, Any]:
        return {"period": period, "main_category": main_category, "items": ["餐饮"]}


class ExplodingAnalyzer(FakeAnalyzer):
    """Analyzer stub that raises per route for error-handler coverage."""

    def generate_report(
        self,
        period: str,
        filters: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        _ = (period, filters)
        raise RuntimeError("overview boom")

    def get_trends(self, period: str, category: str | None = None) -> dict[str, Any]:
        _ = (period, category)
        raise RuntimeError("trends boom")

    def get_comparison(self, period: str, compare_type: str = "category") -> dict[str, Any]:
        _ = (period, compare_type)
        raise RuntimeError("comparison boom")

    def analyze_category(self, period: str, main_category: str | None = None) -> dict[str, Any]:
        _ = (period, main_category)
        raise RuntimeError("category boom")


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)


def test_basic_statistics_routes_transform_analyzer_outputs(
    statistics_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Analyzer overview/trends/comparison/category/trend routes stay Python-proxied."""
    analyzer_db = object()

    monkeypatch.setattr(statistics_module, "Analyzer", FakeAnalyzer)
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: analyzer_db)

    get_overview = _unwrap(statistics_module.get_overview)
    get_trends = _unwrap(statistics_module.get_trends)
    get_comparison = _unwrap(statistics_module.get_comparison)
    get_category_analysis = _unwrap(statistics_module.get_category_analysis)
    get_trend = _unwrap(statistics_module.get_trend)

    with statistics_route_app.test_request_context(
        "/api/statistics/overview?period=month&start_date=2026-03-01&end_date=2026-03-31"
    ):
        payload = get_overview().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["total_income"] == 120.12
        assert payload["result"]["total_expense"] == 35.57
        assert payload["result"]["net_income"] == 84.55
        assert payload["result"]["bill_count"] == 3

    with statistics_route_app.test_request_context(
        "/api/statistics/trends?period=week&category=餐饮"
    ):
        payload = get_trends().get_json() or {}
        assert payload["result"]["period"] == "week"
        assert payload["result"]["category"] == "餐饮"

    with statistics_route_app.test_request_context(
        "/api/statistics/comparison?period=year&type=month"
    ):
        payload = get_comparison().get_json() or {}
        assert payload["result"]["compare_type"] == "month"

    with statistics_route_app.test_request_context(
        "/api/statistics/category?period=month&main_category=餐饮"
    ):
        payload = get_category_analysis().get_json() or {}
        assert payload == {
            "success": True,
            "data": {"period": "month", "main_category": "餐饮", "items": ["餐饮"]},
        }

    with statistics_route_app.test_request_context(
        "/api/statistics/trend?granularity=month&category=餐饮"
    ):
        payload = get_trend().get_json() or {}
        assert payload == {
            "success": True,
            "data": [{"date": "2026-03", "income": 100, "expense": 50, "net": 50}],
        }


def test_analyzer_routes_cover_app_context_helper_and_error_handlers(
    statistics_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The remaining Python statistics package only owns Analyzer proxy error handling."""
    db = object()
    with statistics_route_app.app_context():
        statistics_route_app.config["DB_INSTANCE"] = db
        assert statistics_module.get_app_context() is db

    monkeypatch.setattr(statistics_module, "Analyzer", ExplodingAnalyzer)
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)

    route_expectations = [
        (statistics_module.get_overview, "/api/statistics/overview", "overview boom"),
        (statistics_module.get_trends, "/api/statistics/trends", "trends boom"),
        (statistics_module.get_comparison, "/api/statistics/comparison", "comparison boom"),
        (statistics_module.get_category_analysis, "/api/statistics/category", "category boom"),
        (statistics_module.get_trend, "/api/statistics/trend", "trends boom"),
    ]

    for route, path, expected_error in route_expectations:
        with statistics_route_app.test_request_context(path):
            response, status = _unwrap(route)()
            assert status == 500
            assert response.get_json()["error"] == expected_error


def test_rust_owned_statistics_handlers_are_not_exported_from_flask_package() -> None:
    """Rust-owned read/exchange statistics handlers should not remain monkeypatchable."""
    deleted_handler_names = [
        "get_categorical_analysis",
        "get_trend_analysis",
        "get_asset_trends",
        "get_category_pie",
        "get_top_merchants",
        "get_transaction_amounts",
        "get_exchange_rates",
        "update_user_custom_exchange_rate",
        "delete_user_custom_exchange_rate",
        "_fetch_exchange_rates_from_providers",
        "_get_fallback_exchange_rates",
    ]

    for name in deleted_handler_names:
        assert not hasattr(statistics_module, name), name
