"""Budget REST sidecar cutover regression tests."""

from __future__ import annotations

import asyncio


def _run(coro):
    return asyncio.run(coro)


def _count_budget_rows(db_instance) -> int:
    async def _count() -> int:
        conn = await db_instance._get_connection()  # pylint: disable=protected-access
        async with conn.execute("SELECT COUNT(*) FROM budgets") as cursor:
            row = await cursor.fetchone()
        return int(row[0])

    return _run(_count())


def test_budget_flask_sidecar_routes_are_deleted_without_writes(client, db) -> None:
    """Budget routes are Rust-owned; the Python sidecar no longer registers them."""
    before_count = _count_budget_rows(db)

    requests = [
        client.get("/api/budgets/"),
        client.post(
            "/api/budgets/",
            json={
                "category": "sidecar-deleted",
                "period_type": "monthly",
                "amount": 10.0,
                "start_date": "2026-01-01",
            },
        ),
        client.get("/api/budgets/export"),
        client.get("/api/budgets/execution?period_type=monthly"),
        client.get("/api/budgets/forecast?period_type=monthly"),
        client.get("/api/budgets/history?period_type=monthly"),
        client.post("/api/budgets/history/snapshot", json={"period_type": "monthly"}),
        client.post(
            "/api/budgets/import",
            json=[
                {
                    "name": "sidecar-deleted-import",
                    "category": "sidecar-deleted",
                    "period_type": "monthly",
                    "amount": 10.0,
                    "start_date": "2026-01-01",
                }
            ],
        ),
        client.put("/api/budgets/999999", json={"amount": 20.0}),
        client.delete("/api/budgets/999999"),
    ]

    for response in requests:
        assert response.status_code in (404, 405), response.get_data(as_text=True)
        payload = response.get_json() or {}
        assert payload["success"] is False

    assert _count_budget_rows(db) == before_count


def test_budget_legacy_v1_routes_remain_removed(client) -> None:
    """Budget v1 compatibility endpoints stay physically removed."""
    legacy_paths = [
        ("GET", "/api/v1/budgets/list.json"),
        ("GET", "/api/v1/budgets/execution.json"),
        ("GET", "/api/v1/budgets/forecast.json"),
        ("GET", "/api/v1/budgets/export.json"),
        ("POST", "/api/v1/budgets/add.json"),
        ("POST", "/api/v1/budgets/modify.json"),
        ("POST", "/api/v1/budgets/delete.json"),
        ("POST", "/api/v1/budgets/import.json"),
    ]

    for method, path in legacy_paths:
        response = client.get(path) if method == "GET" else client.post(path, json={})
        assert response.status_code == 404, path


def test_flask_app_no_longer_registers_budget_blueprint(app) -> None:
    """The Python route map should not expose any budget endpoint after P9 deletion."""
    budget_rules = [
        str(rule)
        for rule in app.url_map.iter_rules()
        if str(rule).startswith("/api/budgets")
    ]

    assert budget_rules == []
