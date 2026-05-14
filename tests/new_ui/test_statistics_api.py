"""统计 API sidecar 边界回归测试。"""

from __future__ import annotations

import asyncio
import json
import time
from datetime import datetime, timedelta

import pytest

from tests.runtime_paths import get_test_db_path
from tests.user_cleanup_support import register_test_user_for_cleanup


@pytest.fixture(scope="module")
def client():
    """为统计测试创建独立应用与数据库，避免全量运行中的全局状态串扰。"""
    from bill_analyser.api.app import app, initialize

    test_db_path = get_test_db_path(f"test_statistics_api_{int(time.time() * 1000)}.db")
    if test_db_path.exists():
        test_db_path.unlink()

    asyncio.run(initialize(db_path=str(test_db_path)))
    app.config["TESTING"] = True
    app.config["DEBUG"] = False
    return app.test_client()


@pytest.fixture
def auth_headers(client):
    """为统计测试创建隔离用户，避免被前序模块写入的数据污染。"""
    username = f"test_statistics_{int(time.time() * 1000)}"
    password = "Test123456!"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]

    register_response = client.post(
        "/api/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": password,
            "nickname": username,
        },
    )
    assert register_response.status_code in (200, 201), register_response.get_data(as_text=True)
    register_test_user_for_cleanup(db, username)

    login_response = client.post(
        "/api/auth/login",
        json={"loginName": username, "password": password},
    )
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    result = (login_response.get_json() or {}).get("result") or {}
    token = result.get("token")
    assert token, login_response.get_data(as_text=True)
    return {"Authorization": f"Bearer {token}"}


class TestStatisticsAnalyzerSidecar:
    """仍由 Python Analyzer sidecar 承载的统计路由。"""

    def test_get_overview(self, client, auth_headers):
        """测试获取总览统计"""
        response = client.get("/api/statistics/overview", headers=auth_headers)
        assert response.status_code == 200

        data = json.loads(response.data)
        assert data["success"] is True
        assert "result" in data

        stats = data["result"]
        assert "total_income" in stats
        assert "total_expense" in stats
        assert "net_income" in stats
        assert "bill_count" in stats
        assert isinstance(stats["bill_count"], int)

    def test_get_overview_with_date_range(self, client, auth_headers):
        """测试带日期范围的总览统计"""
        end_date = datetime.now().strftime("%Y-%m-%d")
        start_date = (datetime.now() - timedelta(days=30)).strftime("%Y-%m-%d")

        response = client.get(
            f"/api/statistics/overview?start_date={start_date}&end_date={end_date}",
            headers=auth_headers,
        )
        assert response.status_code == 200
        assert (response.get_json() or {})["success"] is True

    @pytest.mark.parametrize("granularity", ["month", "week", "day"])
    def test_get_trend(self, client, auth_headers, granularity):
        """测试趋势 Analyzer 代理路由。"""
        response = client.get(
            f"/api/statistics/trend?granularity={granularity}",
            headers=auth_headers,
        )
        assert response.status_code == 200

        data = json.loads(response.data)
        assert data["success"] is True
        assert isinstance(data["data"], list)

    def test_overview_consistency(self, client, auth_headers):
        """测试总览统计的一致性"""
        response = client.get("/api/statistics/overview", headers=auth_headers)
        data = json.loads(response.data)

        stats = data["result"]
        expected_net = round(stats["total_income"] - stats["total_expense"], 2)
        assert stats["net_income"] == expected_net

    @pytest.mark.timeout(5)
    def test_overview_performance(self, client, auth_headers):
        """测试总览统计响应时间（应在5秒内）"""
        response = client.get("/api/statistics/overview", headers=auth_headers)
        assert response.status_code == 200

    @pytest.mark.timeout(5)
    def test_trend_performance(self, client, auth_headers):
        """测试趋势统计响应时间（应在5秒内）"""
        response = client.get("/api/statistics/trend", headers=auth_headers)
        assert response.status_code == 200


class TestRustOwnedStatisticsSidecarDeletion:
    """已由 Rust 接管的统计读取路由不再注册 Flask sidecar。"""

    @pytest.mark.parametrize(
        "method,path",
        [
            ("get", "/api/statistics/category-statistics"),
            ("get", "/api/statistics/category-statistics/trends"),
            ("get", "/api/statistics/asset-trends"),
            ("get", "/api/statistics/category-pie"),
            ("get", "/api/statistics/top-merchants"),
            ("get", "/api/statistics/amounts"),
        ],
    )
    def test_rust_owned_read_routes_removed_from_sidecar(
        self,
        client,
        auth_headers,
        method,
        path,
    ):
        response = getattr(client, method)(path, headers=auth_headers)
        assert response.status_code in (404, 405), response.get_data(as_text=True)
        assert (response.get_json() or {})["success"] is False
