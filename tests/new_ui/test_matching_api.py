"""Matching API sidecar cutover regression tests."""

from __future__ import annotations


def test_matching_flask_sidecar_routes_are_deleted(client) -> None:
    """Matching routes are Rust-owned; the Python sidecar no longer registers them."""
    requests = [
        client.get("/api/matching/candidates"),
        client.get("/api/matching/candidates?sessionId=session-1"),
        client.get("/api/matching/candidates?billId=1"),
        client.get("/api/matching/sessions/session-1/candidates"),
        client.get("/api/matching/bills/1/candidates"),
        client.get("/api/matching/bills/1/feedback"),
        client.get("/api/matching/reconciliation-candidates"),
        client.get("/api/matching/pairs"),
        client.post("/api/matching/manual-pair", json={"billId": 1, "candidateBillId": 2}),
        client.delete("/api/matching/pairs/1"),
        client.post("/api/matching/reconcile-history", json={"billIds": [1, 2]}),
        client.post("/api/matching/candidates/bill:1:transfer:2/accept", json={}),
        client.post("/api/matching/candidates/bill:1:transfer:2/reject", json={}),
        client.post("/api/matching/candidates/preview:1:learning/clear", json={}),
        client.get("/api/matching/investment-settings"),
        client.put("/api/matching/investment-settings", json={}),
    ]

    for response in requests:
        assert response.status_code in (404, 405), response.get_data(as_text=True)
        payload = response.get_json() or {}
        assert payload["success"] is False


def test_flask_app_no_longer_registers_matching_blueprint(app) -> None:
    """The Python route map should not expose matching endpoints after P8b deletion."""
    matching_rules = [
        str(rule)
        for rule in app.url_map.iter_rules()
        if str(rule).startswith("/api/matching")
    ]

    assert matching_rules == []
