"""账户与标签 REST 收口回归测试。"""

import time

import pytest

from tests.user_cleanup_support import register_test_user_for_cleanup


@pytest.fixture(name="auth_headers")
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    username = f"test_accounts_tags_{int(time.time() * 1000)}"
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


def test_tag_rest_routes_removed_from_flask_sidecar(client, auth_headers):
    """标签 REST 主链已由 Rust runtime 接管，不再注册 Flask sidecar 蓝图。"""
    assert client.get("/api/tags/", headers=auth_headers).status_code == 404
    assert client.post("/api/tags/", json={"name": "REST标签A"}, headers=auth_headers).status_code in (404, 405)
    assert client.put("/api/tags/1", json={"name": "REST标签A-已更新"}, headers=auth_headers).status_code in (404, 405)
    assert client.delete("/api/tags/1", headers=auth_headers).status_code in (404, 405)
    assert client.post("/api/tags/batch", json={"tags": [{"name": "批量"}]}, headers=auth_headers).status_code in (
        404,
        405,
    )
    assert client.put(
        "/api/tags/display-orders",
        json={"newDisplayOrders": [{"id": "1", "displayOrder": 1}]},
        headers=auth_headers,
    ).status_code in (404, 405)


def test_legacy_account_bulk_action_routes_removed(client, auth_headers):
    """账户批量交易旧 v1 兼容端点应已移除。"""
    move_response = client.post("/api/v1/transactions/move/all.json", json={
        "fromAccountId": "1",
        "toAccountId": "2",
        "password": "admin123"
    }, headers=auth_headers)
    assert move_response.status_code == 404

    clear_response = client.post("/api/v1/data/clear/transactions/by_account.json", json={
        "accountId": "1",
        "password": "admin123"
    }, headers=auth_headers)
    assert clear_response.status_code == 404
