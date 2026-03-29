from __future__ import annotations

from datetime import datetime



def _unix_seconds(date_text: str) -> int:
    return int(datetime.strptime(date_text, "%Y-%m-%d").timestamp())



def test_category_statistics_rejects_inverted_time_range(client, auth_headers) -> None:
    """分类统计应拒绝开始时间晚于结束时间的请求。"""
    response = client.get(
        "/api/statistics/category-statistics",
        query_string={
            "startTime": _unix_seconds("2026-03-10"),
            "endTime": _unix_seconds("2026-03-01"),
        },
        headers=auth_headers,
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload["success"] is False
    assert payload["error"] == "Invalid time range"
    assert "startTime" in payload["message"]



def test_category_statistics_trends_rejects_inverted_year_month_range(client, auth_headers) -> None:
    """分类趋势统计应拒绝开始年月晚于结束年月的请求。"""
    response = client.get(
        "/api/statistics/category-statistics/trends",
        query_string={"startYearMonth": "202603", "endYearMonth": "202602"},
        headers=auth_headers,
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload["success"] is False
    assert payload["error"] == "Invalid year-month range"
    assert "startYearMonth" in payload["message"]



def test_asset_trends_rejects_inverted_time_range(client, auth_headers) -> None:
    """资产趋势应拒绝反向时间范围，而不是静默返回空结果。"""
    response = client.get(
        "/api/statistics/asset-trends",
        query_string={
            "startTime": _unix_seconds("2026-03-10"),
            "endTime": _unix_seconds("2026-03-01"),
        },
        headers=auth_headers,
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload["success"] is False
    assert payload["error"] == "Invalid time range"
    assert "startTime" in payload["message"]



def test_exchange_rates_reject_unsupported_provider(client, auth_headers) -> None:
    """汇率接口应对未知 provider 返回显式 400。"""
    response = client.get(
        "/api/statistics/exchange-rates",
        query_string={"provider": "unknown_provider"},
        headers=auth_headers,
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload["success"] is False
    assert payload["error"] == "Unsupported exchange rate provider: unknown_provider"



def test_custom_exchange_rate_requires_positive_numeric_rate(client, auth_headers) -> None:
    """自定义汇率必须是正数。"""
    response = client.put(
        "/api/statistics/exchange-rates/custom",
        json={"currency": "USD", "rate": 0},
        headers=auth_headers,
    )

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload == {
        "success": False,
        "error": "Invalid request",
        "message": "rate must be greater than 0",
    }
