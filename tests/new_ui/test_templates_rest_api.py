"""模板 REST sidecar 收口回归测试。"""

import time

import pytest

from tests.user_cleanup_support import register_test_user_for_cleanup


@pytest.fixture(name="auth_headers")
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    username = f"test_templates_{int(time.time() * 1000)}"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]
    register_response = client.post(
        "/api/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": "Test123456!",
            "nickname": username,
        },
    )
    assert register_response.status_code in [200, 201], (
        f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
    )
    register_test_user_for_cleanup(db, username)

    login_response = client.post(
        "/api/auth/login",
        json={
            "loginName": username,
            "password": "Test123456!",
        },
    )

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    token = (data.get("result") or {}).get("token")
    assert token, f"登录响应缺少token: {data}"
    return {"Authorization": f"Bearer {token}"}


def test_template_rest_routes_removed_from_flask_sidecar(client, auth_headers):
    """模板 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
    assert client.get("/api/templates/", headers=auth_headers).status_code == 404
    assert client.post(
        "/api/templates/",
        json={"templateType": 1, "name": "REST普通模板A"},
        headers=auth_headers,
    ).status_code in (404, 405)
    assert client.get("/api/templates/1?templateType=1", headers=auth_headers).status_code == 404
    assert client.put(
        "/api/templates/1?templateType=1",
        json={"templateType": 1, "name": "REST普通模板A-已更新"},
        headers=auth_headers,
    ).status_code in (404, 405)
    assert client.delete("/api/templates/1?templateType=1", headers=auth_headers).status_code in (404, 405)
    assert client.put(
        "/api/templates/display-orders",
        json={"templateType": 1, "newDisplayOrders": [{"id": "1", "displayOrder": 1}]},
        headers=auth_headers,
    ).status_code in (404, 405)
