"""Focused branch coverage for statistics-adjacent Flask sidecar routes."""

from __future__ import annotations

# pylint: disable=missing-function-docstring,missing-class-docstring,too-few-public-methods

from collections.abc import Callable
from typing import Any, cast

import pytest
from flask import Flask, request

from bill_analyser.api.routes import calendar as calendar_module
from bill_analyser.api.routes import insights as insights_module
from bill_analyser.api.routes import networth as networth_module


@pytest.fixture(name="route_app")
def route_app_fixture() -> Flask:
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    first = getattr(current, "__wrapped__", None)
    if first is None:
        return cast("Callable[..., Any]", current)

    second = getattr(first, "__wrapped__", None)
    return cast("Callable[..., Any]", second or first)


class NetWorthDB:
    def get_all_accounts(self, *, user_id: int) -> list[dict[str, Any]]:
        assert user_id == 7
        return [
            {
                "id": 1,
                "name": "Cash",
                "type": "cash",
                "icon": "wallet",
                "balance": 1200.125,
                "currency": "CNY",
            },
            {
                "id": 2,
                "name": "Card",
                "type": "credit_card",
                "icon": "card",
                "balance": -300.4,
                "currency": "USD",
            },
            {"id": 3, "name": "Hidden", "type": "cash", "balance": 999, "hidden": True},
        ]


class BrokenNetWorthDB:
    def get_all_accounts(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        raise RuntimeError("networth db down")


def test_networth_snapshot_groups_assets_liabilities_and_errors(
    route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(networth_module, "_run_async", lambda value: value)
    monkeypatch.setattr(networth_module, "_get_db", NetWorthDB)

    with route_app.test_request_context("/api/networth/snapshot"):
        request.user_id = 7
        payload = _unwrap(networth_module.get_net_worth_snapshot)().get_json()

    assert payload["success"] is True
    assert payload["data"]["totalAssets"] == 1200.12
    assert payload["data"]["totalLiabilities"] == 300.4
    assert payload["data"]["netWorth"] == 899.73
    assert payload["data"]["accountCount"] == 2

    monkeypatch.setattr(networth_module, "_get_db", BrokenNetWorthDB)
    with route_app.test_request_context("/api/networth/snapshot"):
        request.user_id = 7
        response, status = _unwrap(networth_module.get_net_worth_snapshot)()

    assert status == 500
    assert response.get_json()["message"] == "networth db down"


class CalendarDB:
    def get_bills_by_date_range(
        self,
        start_date: str,
        end_date: str,
        *,
        user_id: int,
    ) -> list[dict[str, Any]]:
        assert (start_date, end_date, user_id) == ("2026-03-01", "2026-03-15", 7)
        return [
            {
                "id": 1,
                "date": "2026-03-01T08:30:00",
                "amount": -25.5,
                "type": "expense",
                "counterparty": "Cafe",
                "description": "Breakfast",
                "main_category": "Food",
                "sub_category": "Coffee",
            },
            {
                "id": 2,
                "date": "2026-03-02",
                "amount": 100.0,
                "type": "income",
                "counterparty": "Client",
                "description": "Invoice",
                "main_category": "Work",
                "sub_category": "Consulting",
            },
            {
                "id": 3,
                "date": "2026-03-03",
                "amount": 10.0,
                "type": "transfer",
                "counterparty": "Savings",
                "description": "Move",
                "main_category": "Transfer",
                "sub_category": "Internal",
            },
            {"id": 4, "date": "", "amount": 999, "type": "expense"},
        ]

    def get_enabled_recurring_templates(self, *, user_id: int) -> list[dict[str, Any]]:
        assert user_id == 7
        return [
            {
                "id": "rent",
                "next_date": "2026-03-01",
                "frequency": "weekly",
                "name": "Rent",
                "amount": 9.99,
                "type": "expense",
            },
            {"id": "bad-date", "next_date": "not-a-date"},
            {"id": "missing-date"},
        ]


class CalendarRecurringBrokenDB(CalendarDB):
    def get_enabled_recurring_templates(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        raise RuntimeError("recurring down")


class CalendarBrokenDB(CalendarDB):
    def get_bills_by_date_range(
        self,
        start_date: str,
        end_date: str,
        *,
        user_id: int,
    ) -> list[dict[str, Any]]:
        _ = (start_date, end_date, user_id)
        raise RuntimeError("calendar db down")


def test_calendar_events_cover_daily_rollup_recurring_and_errors(
    route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(calendar_module, "_run_async", lambda value: value)
    monkeypatch.setattr(calendar_module, "_get_db", CalendarDB)

    with route_app.test_request_context(
        "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-15"
    ):
        request.user_id = 7
        payload = _unwrap(calendar_module.get_calendar_events)().get_json()

    assert payload["success"] is True
    assert payload["data"]["events"][0]["expense"] == 25.5
    assert payload["data"]["events"][1]["income"] == 100.0
    assert payload["data"]["events"][2]["transferOut"] == 10.0
    assert len(payload["data"]["recurringProjections"]) == 3

    with route_app.test_request_context("/api/calendar/events?start_date=2026-03-01"):
        request.user_id = 7
        response, status = _unwrap(calendar_module.get_calendar_events)()
    assert status == 400
    assert response.get_json()["message"] == "start_date and end_date are required"

    monkeypatch.setattr(calendar_module, "_get_db", CalendarRecurringBrokenDB)
    with route_app.test_request_context(
        "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-15"
    ):
        request.user_id = 7
        payload = _unwrap(calendar_module.get_calendar_events)().get_json()
    assert payload["success"] is True
    assert payload["data"]["recurringProjections"] == []

    monkeypatch.setattr(calendar_module, "_get_db", CalendarBrokenDB)
    with route_app.test_request_context(
        "/api/calendar/events?start_date=2026-03-01&end_date=2026-03-15"
    ):
        request.user_id = 7
        response, status = _unwrap(calendar_module.get_calendar_events)()
    assert status == 500
    assert response.get_json()["message"] == "calendar db down"


class InsightsDB:
    def get_bills(
        self,
        *,
        filters: dict[str, str],
        limit: int,
        offset: int,
        user_id: int,
    ) -> list[dict[str, Any]]:
        assert set(filters) == {"start_date", "end_date"}
        assert (limit, offset, user_id) == (50000, 0, 7)
        return [
            {
                "id": 1,
                "date": "2025-11-05",
                "amount": 60,
                "type": "expense",
                "main_category": "Food",
                "counterparty": "Market",
            },
            {
                "id": 2,
                "date": "2025-12-05",
                "amount": 60,
                "type": "expense",
                "main_category": "Food",
                "counterparty": "Market",
            },
            {
                "id": 3,
                "date": "2026-01-05",
                "amount": 60,
                "type": "expense",
                "main_category": "Food",
                "counterparty": "Market",
            },
            {
                "id": 4,
                "date": "2026-02-05",
                "amount": 60,
                "type": "expense",
                "main_category": "Food",
                "counterparty": "Market",
            },
            {
                "id": 5,
                "date": "2026-03-05",
                "amount": 500,
                "type": "expense",
                "main_category": "Food",
                "counterparty": "Restaurant",
                "description": "Banquet",
            },
            {
                "id": 6,
                "date": "2026-03-06",
                "amount": 12,
                "type": "expense",
                "main_category": "Fitness",
                "counterparty": "Gym",
            },
            {
                "id": 7,
                "date": "2026-03-08",
                "amount": 12,
                "type": "expense",
                "main_category": "Fitness",
                "counterparty": "Gym",
            },
            {
                "id": 8,
                "date": "2026-03-09",
                "amount": 999,
                "type": "income",
                "main_category": "Salary",
                "counterparty": "Client",
            },
            {
                "id": 9,
                "date": "bad-date",
                "amount": 12,
                "type": "income",
                "main_category": "Fitness",
                "counterparty": "Gym",
            },
        ]


def test_insights_anomalies_cover_large_duplicate_spike_and_error(
    route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(insights_module, "_run_async", lambda value: value)
    monkeypatch.setattr(insights_module, "_get_db", InsightsDB)

    with route_app.test_request_context("/api/insights/anomalies?months=99"):
        request.user_id = 7
        payload = _unwrap(insights_module.get_anomalies)().get_json()

    anomaly_types = {item["type"] for item in payload["data"]["anomalies"]}
    assert payload["success"] is True
    assert payload["data"]["analyzedMonths"] == 24
    assert {"large_transaction", "duplicate_charge", "category_spike"} <= anomaly_types

    with route_app.test_request_context("/api/insights/anomalies?months=bad"):
        request.user_id = 7
        response, status = _unwrap(insights_module.get_anomalies)()

    assert status == 500
    assert "invalid literal" in response.get_json()["message"]
