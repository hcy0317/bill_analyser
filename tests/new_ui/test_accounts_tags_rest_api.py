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


def test_tag_rest_lifecycle(client, auth_headers):
    """标签 CRUD 与排序应全部走 REST 接口。"""
    create_response = client.post("/api/tags/", json={
        "name": "REST标签A",
        "color": "#FF0000",
        "icon": "1"
    }, headers=auth_headers)
    assert create_response.status_code == 201
    create_data = create_response.get_json()
    assert create_data["success"] is True
    assert create_data["result"]["name"] == "REST标签A"
    tag_id = create_data["result"]["id"]

    second_response = client.post("/api/tags/", json={
        "name": "REST标签B",
        "color": "#00FF00",
        "icon": "2"
    }, headers=auth_headers)
    assert second_response.status_code == 201
    second_tag_id = second_response.get_json()["result"]["id"]
    assert second_tag_id

    update_response = client.put(f"/api/tags/{tag_id}", json={
        "id": str(tag_id),
        "name": "REST标签A-已更新"
    }, headers=auth_headers)
    assert update_response.status_code == 200
    update_data = update_response.get_json()
    assert update_data["success"] is True
    assert update_data["result"]["name"] == "REST标签A-已更新"

    hide_response = client.put(f"/api/tags/{tag_id}", json={
        "id": str(tag_id),
        "hidden": True
    }, headers=auth_headers)
    assert hide_response.status_code == 200
    hide_data = hide_response.get_json()
    assert hide_data["success"] is True
    assert hide_data["result"]["hidden"] in [True, 1]

    move_response = client.put("/api/tags/display-orders", json={
        "newDisplayOrders": [
            {"id": str(tag_id), "displayOrder": 2},
            {"id": str(second_tag_id), "displayOrder": 1}
        ]
    }, headers=auth_headers)
    assert move_response.status_code == 200
    move_data = move_response.get_json()
    assert move_data["success"] is True
    assert move_data["result"] is True

    batch_response = client.post("/api/tags/batch", json={
        "tags": [
            {"name": "REST批量标签1"},
            {"name": "REST批量标签2"}
        ],
        "skipExists": True
    }, headers=auth_headers)
    assert batch_response.status_code == 201
    batch_data = batch_response.get_json()
    assert batch_data["success"] is True
    assert len(batch_data["result"]) == 2

    delete_response = client.delete(f"/api/tags/{tag_id}", headers=auth_headers)
    assert delete_response.status_code == 200
    delete_data = delete_response.get_json()
    assert delete_data["success"] is True
    assert delete_data["result"] is True


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
