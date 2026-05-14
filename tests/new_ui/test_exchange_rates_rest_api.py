"""汇率域 Rust 接管后的 Flask sidecar 回归测试。"""

from __future__ import annotations

import asyncio

import pytest


def _current_user_id(db, username: str) -> int:
    user = asyncio.run(db.get_user_by_username(username))
    assert user is not None
    return int(user["id"])


def _custom_rate_count(db, user_id: int) -> int:
    rates = asyncio.run(db.get_user_custom_exchange_rates("CNY", user_id=user_id))
    return len(rates)


def test_exchange_rates_rest_routes_removed_from_flask_sidecar(
    client,
    auth_context,
    auth_headers,
    db,
) -> None:
    """汇率 REST 主链已由 Rust runtime 接管，Python sidecar 不再写入 custom rate。"""
    user_id = _current_user_id(db, auth_context["username"])
    before_count = _custom_rate_count(db, user_id)

    requests = [
        client.get("/api/statistics/exchange-rates", headers=auth_headers),
        client.put(
            "/api/statistics/exchange-rates/custom",
            json={"currency": "USD", "rate": "0.5"},
            headers=auth_headers,
        ),
        client.delete("/api/statistics/exchange-rates/custom/USD", headers=auth_headers),
    ]

    for response in requests:
        assert response.status_code in (404, 405), response.get_data(as_text=True)
        payload = response.get_json() or {}
        assert payload["success"] is False

    assert _custom_rate_count(db, user_id) == before_count


@pytest.mark.parametrize(
    "legacy_path,method",
    [
        ("/api/v1/exchange_rates/user_custom/update.json", "post"),
        ("/api/v1/exchange_rates/user_custom/delete.json", "post"),
    ],
)
def test_exchange_rates_legacy_routes_removed(client, auth_headers, legacy_path, method) -> None:
    """旧自定义汇率路径应已移除。"""
    response = getattr(client, method)(
        legacy_path,
        json={"currency": "USD", "rate": "0.5"},
        headers=auth_headers,
    )
    assert response.status_code == 404
