from __future__ import annotations

from typing import Any, Callable, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import statistics as statistics_module


@pytest.fixture(name="statistics_route_app")
def statistics_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct statistics route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeAnalyzer:
    """Synchronous analyzer stub when _run_async is patched to identity."""

    def __init__(self, db: object) -> None:
        self.db = db

    def generate_report(self, period: str, filters: dict[str, Any] | None = None) -> dict[str, Any]:
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


class FakeStatisticsDB:
    """Synchronous DB stub for statistics route tests when _run_async is identity."""

    def __init__(self) -> None:
        self.custom_rates: list[dict[str, Any]] = []
        self.upsert_result: dict[str, Any] = {"success": True, "update_time": 1234567890}
        self.delete_result: bool = True

    def query_bills(self, *_, **kwargs) -> tuple[list[dict[str, Any]], int]:
        filters = kwargs.get("filters") or {}
        if filters.get("type") == "支出":
            return (
                [
                    {"main_category": "餐饮", "amount": -18.5},
                    {"main_category": "餐饮", "amount": -6.5},
                    {"main_category": "交通", "amount": -12.0},
                ],
                3,
            )
        if "start_date" in filters and "end_date" in filters and "type" not in filters:
            return (
                [
                    {"type": "收入", "amount": 10.01},
                    {"type": "支出", "amount": -2.55},
                    {"type": "支出", "amount": -1.45},
                ],
                3,
            )
        return (
            [
                {"counterparty": "早餐店", "amount": -18.5},
                {"counterparty": "早餐店", "amount": -6.5},
                {"counterparty": "地铁", "amount": -12.0},
            ],
            3,
        )

    def get_user_by_id(self, _user_id: int) -> dict[str, Any]:
        return {"id": 1, "default_currency": "CNY", "username": "alice", "email": "alice@example.com"}

    def get_user_custom_exchange_rates(self, _base_currency: str, _user_id: int) -> list[dict[str, Any]]:
        return self.custom_rates

    def upsert_user_custom_exchange_rate(self, _base_currency: str, _currency: str, _rate: float, _user_id: int) -> dict[str, Any]:
        return self.upsert_result

    def delete_user_custom_exchange_rate(self, _base_currency: str, _currency: str, _user_id: int) -> bool:
        return self.delete_result



def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast(Any, func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast(Callable[..., Any], current)

    second = getattr(first, "__wrapped__", None)
    return cast(Callable[..., Any], second or first)


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)



def test_basic_statistics_routes_transform_analyzer_outputs(
    statistics_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """概览/趋势/对比/分类/趋势图路由应正确转换 analyzer 输出。"""
    analyzer_db = object()

    def _get_analyzer_db() -> object:
        return analyzer_db

    monkeypatch.setattr(statistics_module, "Analyzer", FakeAnalyzer)
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", _get_analyzer_db)

    get_overview = _unwrap(statistics_module.get_overview)
    get_trends = _unwrap(statistics_module.get_trends)
    get_comparison = _unwrap(statistics_module.get_comparison)
    get_category_analysis = _unwrap(statistics_module.get_category_analysis)
    get_trend = _unwrap(statistics_module.get_trend)

    with statistics_route_app.test_request_context("/api/statistics/overview?period=month&start_date=2026-03-01&end_date=2026-03-31"):
        payload = get_overview().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["total_income"] == 120.12
        assert payload["result"]["total_expense"] == 35.57
        assert payload["result"]["net_income"] == 84.55
        assert payload["result"]["bill_count"] == 3

    with statistics_route_app.test_request_context("/api/statistics/trends?period=week&category=餐饮"):
        payload = get_trends().get_json() or {}
        assert payload == {"success": True, "result": {"period": "week", "category": "餐饮", "trends": [{"period": "2026-03", "income": 100, "expense": 50, "net": 50}]}}

    with statistics_route_app.test_request_context("/api/statistics/comparison?period=year&type=month"):
        payload = get_comparison().get_json() or {}
        assert payload["result"]["compare_type"] == "month"

    with statistics_route_app.test_request_context("/api/statistics/category?period=month&main_category=餐饮"):
        payload = get_category_analysis().get_json() or {}
        assert payload == {"success": True, "data": {"period": "month", "main_category": "餐饮", "items": ["餐饮"]}}

    with statistics_route_app.test_request_context("/api/statistics/trend?granularity=month&category=餐饮"):
        payload = get_trend().get_json() or {}
        assert payload == {
            "success": True,
            "data": [{"date": "2026-03", "income": 100, "expense": 50, "net": 50}],
        }



def test_collection_statistics_routes_cover_category_pie_top_merchants_and_amounts(
    statistics_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """集合型统计路由应覆盖饼图、商家排行和区间金额统计。"""
    db = FakeStatisticsDB()
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    get_category_pie = _unwrap(statistics_module.get_category_pie)
    get_top_merchants = _unwrap(statistics_module.get_top_merchants)
    get_transaction_amounts = _unwrap(statistics_module.get_transaction_amounts)

    with statistics_route_app.test_request_context("/api/statistics/category-pie?type=支出"):
        payload = get_category_pie().get_json() or {}
        assert payload == {
            "success": True,
            "data": [
                {"name": "餐饮", "value": 25.0},
                {"name": "交通", "value": 12.0},
            ],
        }

    with statistics_route_app.test_request_context("/api/statistics/top-merchants?limit=1"):
        payload = get_top_merchants().get_json() or {}
        assert payload == {
            "success": True,
            "data": [{"name": "早餐店", "amount": 25.0, "count": 2}],
        }

    with statistics_route_app.test_request_context("/api/statistics/amounts"):
        response, status = get_transaction_amounts()
        assert status == 400
        assert response.get_json()["error"] == "Missing query parameter"

    with statistics_route_app.test_request_context(
        "/api/statistics/amounts?periods=range_1740787200_1740873599"
    ):
        payload = get_transaction_amounts().get_json() or {}
        assert payload == {
            "success": True,
            "result": {
                "range": {
                    "startTime": 1740787200,
                    "endTime": 1740873599,
                    "amounts": [{"currency": "CNY", "incomeAmount": 1001, "expenseAmount": 400}],
                }
            },
        }



def test_exchange_rate_routes_cover_short_circuit_success_fallback_and_mutations(
    statistics_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """汇率读写路由应覆盖非法 provider、自定义短路、provider 成功、fallback 和增删改分支。"""
    db = FakeStatisticsDB()
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: db)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)

    get_exchange_rates = _unwrap(statistics_module.get_exchange_rates)
    update_custom_rate = _unwrap(statistics_module.update_user_custom_exchange_rate)
    delete_custom_rate = _unwrap(statistics_module.delete_user_custom_exchange_rate)

    with statistics_route_app.test_request_context("/api/statistics/exchange-rates?provider=bad-provider"):
        response, status = get_exchange_rates()
        assert status == 400
        assert response.get_json()["error"] == "Unsupported exchange rate provider: bad-provider"

    db.custom_rates = [{"to_currency": "USD", "rate": 7.12, "effective_date": "2026-03-01T12:00:00"}]
    with statistics_route_app.test_request_context("/api/statistics/exchange-rates"):
        payload = get_exchange_rates().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["providerKey"] == "user_custom"
        assert payload["result"]["exchangeRates"][1] == {"currency": "USD", "rate": "7.12"}

    db.custom_rates = []
    monkeypatch.setattr(
        statistics_module,
        "_fetch_exchange_rates_from_providers",
        lambda *_args, **_kwargs: {
            "rates": {"USD": 0.139},
            "source": "ECB",
            "url": "https://example.test/ecb",
            "provider_key": "ecb",
            "fallback_used": False,
        },
    )
    with statistics_route_app.test_request_context("/api/statistics/exchange-rates?provider=ecb"):
        payload = get_exchange_rates().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["providerKey"] == "ecb"
        assert payload["result"]["exchangeRates"][0] == {"currency": "CNY", "rate": "1.0"}
        assert payload["result"]["exchangeRates"][1] == {"currency": "USD", "rate": "0.139"}

    monkeypatch.setattr(
        statistics_module,
        "_fetch_exchange_rates_from_providers",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("network down")),
    )
    with statistics_route_app.test_request_context("/api/statistics/exchange-rates"):
        payload = get_exchange_rates().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["providerKey"] == "fallback"

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom",
        method="PUT",
        json={"currency": "", "rate": 1},
    ):
        response, status = update_custom_rate()
        assert status == 400
        assert response.get_json()["message"] == "currency and rate are required"

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom",
        method="PUT",
        json={"currency": "usd", "rate": "not-a-number"},
    ):
        response, status = update_custom_rate()
        assert status == 400
        assert response.get_json()["message"] == "rate must be numeric"

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom",
        method="PUT",
        json={"currency": "usd", "rate": 0},
    ):
        response, status = update_custom_rate()
        assert status == 400
        assert response.get_json()["message"] == "rate must be greater than 0"

    db.upsert_result = {"success": False, "message": "db write failed"}
    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom",
        method="PUT",
        json={"currency": "usd", "rate": 7.2},
    ):
        response, status = update_custom_rate()
        assert status == 500
        assert response.get_json()["message"] == "db write failed"

    db.upsert_result = {"success": True, "update_time": 24680}
    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom",
        method="PUT",
        json={"currency": "usd", "rate": 7.2},
    ):
        payload = update_custom_rate().get_json() or {}
        assert payload == {
            "success": True,
            "result": {"currency": "USD", "rate": "7.2", "updateTime": 24680},
        }

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom/ ",
        method="DELETE",
    ):
        response, status = delete_custom_rate("")
        assert status == 400
        assert response.get_json()["message"] == "currency is required"

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom/usd",
        method="DELETE",
    ):
        payload = delete_custom_rate("usd").get_json() or {}
        assert payload == {"success": True, "result": True}


def test_statistics_routes_cover_helpers_and_error_handlers(
    statistics_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """统计路由应覆盖 helper 与主要异常兜底分支。"""
    get_request_user_id_helper = getattr(statistics_module, "_get_request_user_id")
    db = FakeStatisticsDB()
    analyzer_db = object()

    def _get_analyzer_db() -> object:
        return analyzer_db

    with statistics_route_app.app_context():
        statistics_route_app.config["DB_INSTANCE"] = db
        assert statistics_module.get_app_context() is db

    monkeypatch.setattr(statistics_module, "get_required_request_int", lambda _name: 12)
    with statistics_route_app.test_request_context("/api/statistics/overview"):
        assert get_request_user_id_helper() == 12

    class ExplodingAnalyzer(FakeAnalyzer):
        def generate_report(self, period: str, filters: dict[str, Any] | None = None) -> dict[str, Any]:
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

    monkeypatch.setattr(statistics_module, "Analyzer", ExplodingAnalyzer)
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", _get_analyzer_db)

    get_overview = _unwrap(statistics_module.get_overview)
    get_trends = _unwrap(statistics_module.get_trends)
    get_comparison = _unwrap(statistics_module.get_comparison)
    get_category_analysis = _unwrap(statistics_module.get_category_analysis)
    get_trend = _unwrap(statistics_module.get_trend)

    with statistics_route_app.test_request_context("/api/statistics/overview"):
        response, status = get_overview()
        assert status == 500
        assert response.get_json()["error"] == "overview boom"

    with statistics_route_app.test_request_context("/api/statistics/trends"):
        response, status = get_trends()
        assert status == 500
        assert response.get_json()["error"] == "trends boom"

    with statistics_route_app.test_request_context("/api/statistics/comparison"):
        response, status = get_comparison()
        assert status == 500
        assert response.get_json()["error"] == "comparison boom"

    with statistics_route_app.test_request_context("/api/statistics/category"):
        response, status = get_category_analysis()
        assert status == 500
        assert response.get_json()["error"] == "category boom"

    with statistics_route_app.test_request_context("/api/statistics/trend"):
        response, status = get_trend()
        assert status == 500
        assert response.get_json()["error"] == "trends boom"

    exploding_db = FakeStatisticsDB()
    monkeypatch.setattr(statistics_module, "_run_async", lambda value: value)
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: exploding_db)
    monkeypatch.setattr(statistics_module, "_get_request_user_id", lambda: 1)
    monkeypatch.setattr(
        exploding_db,
        "query_bills",
        lambda *args, **kwargs: _raise_runtime_error("query boom"),
    )

    get_category_pie = _unwrap(statistics_module.get_category_pie)
    get_top_merchants = _unwrap(statistics_module.get_top_merchants)
    get_transaction_amounts = _unwrap(statistics_module.get_transaction_amounts)

    with statistics_route_app.test_request_context("/api/statistics/category-pie?type=支出"):
        response, status = get_category_pie()
        assert status == 500
        assert response.get_json()["error"] == "query boom"

    with statistics_route_app.test_request_context("/api/statistics/top-merchants"):
        response, status = get_top_merchants()
        assert status == 500
        assert response.get_json()["error"] == "query boom"

    with statistics_route_app.test_request_context(
        "/api/statistics/amounts?periods=range_1740787200_1740873599"
    ):
        response, status = get_transaction_amounts()
        assert status == 500
        assert response.get_json()["error"] == "query boom"

    mutation_db = FakeStatisticsDB()
    monkeypatch.setattr(statistics_module, "get_app_context", lambda: mutation_db)
    monkeypatch.setattr(
        mutation_db,
        "upsert_user_custom_exchange_rate",
        lambda *args, **kwargs: _raise_runtime_error("upsert boom"),
    )
    monkeypatch.setattr(
        mutation_db,
        "delete_user_custom_exchange_rate",
        lambda *args, **kwargs: _raise_runtime_error("delete boom"),
    )

    update_custom_rate = _unwrap(statistics_module.update_user_custom_exchange_rate)
    delete_custom_rate = _unwrap(statistics_module.delete_user_custom_exchange_rate)

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom",
        method="PUT",
        json={"currency": "usd", "rate": 7.2},
    ):
        response, status = update_custom_rate()
        assert status == 500
        assert response.get_json()["message"] == "upsert boom"

    with statistics_route_app.test_request_context(
        "/api/statistics/exchange-rates/custom/usd",
        method="DELETE",
    ):
        response, status = delete_custom_rate("usd")
        assert status == 500
        assert response.get_json()["message"] == "delete boom"
