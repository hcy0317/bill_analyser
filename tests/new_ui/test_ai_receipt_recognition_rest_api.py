"""AI识图 REST 收口回归测试。"""

import asyncio
from datetime import datetime

import pytest


@pytest.fixture(scope='module', name='client')
def _client_fixture():
    """创建测试客户端。"""

    async def init_services():
        from src.api.app import initialize
        await initialize()

    asyncio.run(init_services())

    from src.api.app import app
    app.config['TESTING'] = True

    with app.test_client() as test_client:
        yield test_client


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(client):
    """返回鉴权请求头。"""
    suffix = int(datetime.now().timestamp())
    username = f'test_ai_receipt_rest_{suffix}'
    password = 'Test123456!'

    register_response = client.post('/api/auth/register', json={
        'username': username,
        'email': f'{username}@example.com',
        'password': password,
        'nickname': username
    })
    assert register_response.status_code in [200, 409], register_response.get_data(as_text=True)

    login_response = client.post('/api/auth/login', json={
        'loginName': username,
        'password': password
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    login_data = login_response.get_json() or {}
    token = (login_data.get('result') or {}).get('token')
    assert token

    return {'Authorization': f'Bearer {token}'}


def test_ai_receipt_recognition_rest_disabled_safe(client, auth_headers):
    """AI识图新 REST 主链当前应返回未实现或路由未启用。"""
    response = client.post('/api/ml/receipt-recognition', headers=auth_headers)
    assert response.status_code in (404, 501), response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is False
    error_message = f"{data.get('errorMessage') or ''} {data.get('message') or ''}".lower()
    assert 'not implemented' in error_message or 'not found' in error_message


def test_ai_receipt_recognition_legacy_route_removed(client, auth_headers):
    """旧 AI 识图 v1 路径应已移除。"""
    response = client.post('/api/v1/llm/transactions/recognize_receipt_image.json', headers=auth_headers)
    assert response.status_code == 404