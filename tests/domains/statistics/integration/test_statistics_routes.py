from __future__ import annotations


RUST_OWNED_STATISTICS_REQUESTS = [
    ("get", "/api/statistics/category-statistics"),
    ("get", "/api/statistics/category-statistics/trends"),
    ("get", "/api/statistics/asset-trends"),
    ("get", "/api/statistics/category-pie"),
    ("get", "/api/statistics/top-merchants"),
    ("get", "/api/statistics/amounts"),
    ("get", "/api/statistics/exchange-rates"),
    ("put", "/api/statistics/exchange-rates/custom"),
    ("delete", "/api/statistics/exchange-rates/custom/USD"),
]


def test_statistics_read_and_exchange_flask_sidecar_routes_are_deleted(
    client,
    auth_headers,
) -> None:
    """Rust-owned statistics read/exchange routes are no longer registered in Flask."""
    for method, path in RUST_OWNED_STATISTICS_REQUESTS:
        request = getattr(client, method)
        if method == "put":
            response = request(
                path,
                json={"currency": "USD", "rate": 1.0},
                headers=auth_headers,
            )
        else:
            response = request(path, headers=auth_headers)

        assert response.status_code in (404, 405), path
        payload = response.get_json() or {}
        assert payload["success"] is False


def test_statistics_analyzer_flask_sidecar_routes_remain_registered(
    client,
    auth_headers,
) -> None:
    """Analyzer overview/trends/comparison/category/trend still run through Python sidecar."""
    for path in [
        "/api/statistics/overview",
        "/api/statistics/trends",
        "/api/statistics/comparison",
        "/api/statistics/category",
        "/api/statistics/trend",
    ]:
        response = client.get(path, headers=auth_headers)
        assert response.status_code == 200, response.get_data(as_text=True)
        payload = response.get_json() or {}
        assert payload["success"] is True


def test_flask_app_statistics_route_map_keeps_only_analyzer_routes(app) -> None:
    """The sidecar statistics blueprint should expose only Python-proxied Analyzer routes."""
    statistics_rules = sorted(
        str(rule)
        for rule in app.url_map.iter_rules()
        if str(rule).startswith("/api/statistics")
    )

    assert statistics_rules == [
        "/api/statistics/category",
        "/api/statistics/comparison",
        "/api/statistics/overview",
        "/api/statistics/trend",
        "/api/statistics/trends",
    ]
