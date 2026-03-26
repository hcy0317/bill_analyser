"""账单域历史 rewrite 已移除的回归测试。"""

from datetime import datetime

import pytest


@pytest.fixture(scope='module', name='client')
def _client_fixture():
    """创建测试客户端。"""
    import asyncio

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
    username = f'test_legacy_bill_rewrite_removed_{suffix}'
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

    token = ((login_response.get_json() or {}).get('result') or {}).get('token')
    assert token
    return {'Authorization': f'Bearer {token}'}


@pytest.mark.parametrize(('method', 'path', 'json_body'), [
    ('post', '/api/v1/transactions/add.json', {'description': 'legacy add'}),
    ('post', '/api/v1/transactions/modify.json', {'id': 1, 'description': 'legacy modify'}),
    ('post', '/api/v1/transactions/delete.json', {'id': 1}),
    ('post', '/api/v1/transactions/import.json', {'transactions': []}),
    ('get', '/api/v1/transactions/reconciliation_statements.json?account_id=1&start_time=1&end_time=2', None),
    ('get', '/api/v1/transaction/categories/list.json', None),
    ('post', '/api/v1/transaction/categories/add.json', {'name': 'legacy category'}),
    ('post', '/api/v1/transaction/categories/add_batch.json', {'items': []}),
])
def test_legacy_bill_and_category_rewrite_routes_removed(client, auth_headers, method, path, json_body):
    """已停止使用的账单/分类 rewrite 路径应返回 404。"""
    response = getattr(client, method)(path, json=json_body, headers=auth_headers)
    assert response.status_code == 404, response.get_data(as_text=True)


def test_legacy_parse_import_rewrite_removed(client, auth_headers):
    """旧 parse_import rewrite 路径应返回 404。"""
    response = client.post('/api/v1/transactions/parse_import.json', headers=auth_headers)
    assert response.status_code == 404, response.get_data(as_text=True)