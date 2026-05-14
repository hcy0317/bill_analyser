"""统计 API sidecar 边界回归测试。"""

from __future__ import annotations

import asyncio
import time

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


class TestRustOwnedStatisticsSidecarDeletion:
    """已由 Rust 接管的统计路由不再注册 Flask sidecar。"""

    @pytest.mark.parametrize(
        "method,path",
        [
            ("get", "/api/statistics/category-statistics"),
            ("get", "/api/statistics/category-statistics/trends"),
            ("get", "/api/statistics/asset-trends"),
            ("get", "/api/statistics/category-pie"),
            ("get", "/api/statistics/top-merchants"),
            ("get", "/api/statistics/amounts"),
            ("get", "/api/statistics/overview"),
            ("get", "/api/statistics/trends"),
            ("get", "/api/statistics/comparison"),
            ("get", "/api/statistics/category"),
            ("get", "/api/statistics/trend"),
        ],
    )
    def test_rust_owned_routes_removed_from_sidecar(
        self,
        client,
        auth_headers,
        method,
        path,
    ):
        response = getattr(client, method)(path, headers=auth_headers)
        assert response.status_code in (404, 405), response.get_data(as_text=True)
        assert (response.get_json() or {})["success"] is False
