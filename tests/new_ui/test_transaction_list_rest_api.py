"""交易列表 REST 主链测试。"""

import json
from datetime import datetime

import pytest


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    suffix = int(datetime.now().timestamp())
    username = f'test_transaction_list_rest_{suffix}'
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

    data = login_response.get_json() or {}
    token = (data.get('result') or {}).get('token')
    assert token
    return {'Authorization': f'Bearer {token}'}


def test_transactions_list_rest_endpoint(client, auth_headers):
    """交易列表应可通过 REST 主链访问。"""
    response = client.get(
        '/api/bills/?max_time=9999999999999&min_time=0&type=0&page_size=10&page=1&with_count=true',
        headers=auth_headers
    )
    assert response.status_code == 200

    data = json.loads(response.data)
    assert data['success'] is True
    assert 'result' in data
    assert 'items' in data['result']
    assert 'totalCount' in data['result']


def test_transactions_list_by_month_rest_endpoint(client, auth_headers):
    """按月交易列表应可通过 REST 主链访问。"""
    response = client.get(
        '/api/bills/by-month?year=2025&month=1&type=0',
        headers=auth_headers
    )
    assert response.status_code == 200

    data = json.loads(response.data)
    assert data['success'] is True
    assert 'result' in data
    assert 'items' in data['result']
    assert 'totalCount' in data['result']


def test_transactions_list_legacy_routes_removed(client, auth_headers):
    """旧交易列表 v1 路径应已移除。"""
    list_response = client.get(
        '/api/v1/transactions/list.json?max_time=9999999999999&min_time=0&type=0&page=1&count=10',
        headers=auth_headers
    )
    by_month_response = client.get(
        '/api/v1/transactions/list/by_month.json?year=2025&month=1&type=0',
        headers=auth_headers
    )

    assert list_response.status_code == 404
    assert by_month_response.status_code == 404