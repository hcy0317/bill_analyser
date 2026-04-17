"""认证域 2FA 状态 REST 收口回归测试。"""

import time

import pytest

from tests.user_cleanup_support import register_test_user_for_cleanup


@pytest.fixture(name="auth_headers")
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    username = f"test_2fa_status_{int(time.time() * 1000)}"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]
    register_response = client.post("/api/auth/register", json={
        "username": username,
        "email": f"{username}@example.com",
        "password": "Test123456!",
        "nickname": username
    })
    assert register_response.status_code in [200, 201], (
        f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
    )
    register_test_user_for_cleanup(db, username)

    login_response = client.post("/api/auth/login", json={
        "loginName": username,
        "password": "Test123456!"
    })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    token = (data.get("result") or {}).get("token")
    assert token, f"登录响应缺少token: {data}"
    return {"Authorization": f"Bearer {token}"}



def test_two_factor_status_rest_endpoint(client, auth_headers):
    """2FA状态应通过 REST 主链提供。"""
    response = client.get("/api/2fa/status", headers=auth_headers)
    assert response.status_code == 200

    data = response.get_json() or {}
    assert data.get("success") is True
    result = data.get("result") or {}
    assert "enable" in result
    assert "isEnabled" in result



def test_two_factor_status_legacy_route_removed(client, auth_headers):
    """旧2FA状态路径应返回404。"""
    response = client.get("/api/v1/users/2fa/status.json", headers=auth_headers)
    assert response.status_code == 404
