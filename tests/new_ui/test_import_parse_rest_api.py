"""导入解析 REST 收口回归测试。"""

import asyncio
from datetime import datetime
from pathlib import Path

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
    username = f'test_import_parse_rest_{suffix}'
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


def test_parse_import_rest_endpoint(client, auth_headers):
    """导入解析应走新的 REST 主链。"""
    sample_file = Path('c:/Users/hcy/OneDrive/Github/bill_analyser/bills/微信支付账单(20240101-20240331).csv')
    assert sample_file.exists(), f'样例文件不存在: {sample_file}'

    with sample_file.open('rb') as file_obj:
        response = client.post(
            '/api/bills/parse_import',
            data={
                'fileType': 'auto',
                'fileEncoding': 'utf-8',
                'file': (file_obj, sample_file.name)
            },
            headers=auth_headers,
            content_type='multipart/form-data'
        )

    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    items = result.get('items') or []
    assert result.get('totalCount', 0) > 0
    assert len(items) > 0

    first_item = items[0]
    assert 'time' in first_item
    assert 'type' in first_item
    assert 'amount' in first_item
    assert 'description' in first_item
    assert 'counterparty' in first_item
    assert 'paymentMethod' in first_item


def test_import_legacy_process_route_removed(client, auth_headers):
    """旧导入进度轮询路径应已移除。"""
    response = client.get(
        '/api/v1/transactions/import/process.json?client_session_id=test-session',
        headers=auth_headers
    )
    assert response.status_code == 404, response.get_data(as_text=True)


def test_import_legacy_parse_dsv_route_removed(client, auth_headers):
    """旧 DSV 解析路径应已移除。"""
    response = client.post(
        '/api/v1/transactions/parse_dsv_file.json',
        headers=auth_headers,
        data={},
        content_type='multipart/form-data'
    )
    assert response.status_code == 404, response.get_data(as_text=True)