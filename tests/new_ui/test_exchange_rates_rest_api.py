"""汇率域 REST 收口回归测试。"""

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


@pytest.fixture(scope='module', name='auth_context')
def _auth_context_fixture(client):
    """注册并登录测试用户。"""
    suffix = int(datetime.now().timestamp())
    username = f'test_exchange_rest_{suffix}'
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

    return {
        'username': username,
        'headers': {'Authorization': f'Bearer {token}'}
    }


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(auth_context):  # pylint: disable=unused-argument
    """返回鉴权头。"""
    return auth_context['headers']


@pytest.fixture(scope='module', name='db')
def _db_fixture():
    """返回数据库实例。"""
    from src.api.app import app
    return app.config['DB_INSTANCE']


def test_exchange_rates_rest_endpoints(client, auth_context, auth_headers, db):
    """用户自定义汇率应走新的 REST 主链。"""
    user = asyncio.run(db.get_user_by_username(auth_context['username']))
    assert user is not None
    asyncio.run(db.update_user(int(user['id']), {'default_currency': 'CNY'}))

    update_response = client.put('/api/statistics/exchange-rates/custom', json={
        'currency': 'USD',
        'rate': '0.5'
    }, headers=auth_headers)
    assert update_response.status_code == 200, update_response.get_data(as_text=True)

    update_data = update_response.get_json() or {}
    assert update_data.get('success') is True
    update_result = update_data.get('result') or {}
    assert update_result.get('currency') == 'USD'
    assert update_result.get('rate') == '0.5'
    assert isinstance(update_result.get('updateTime'), int)

    latest_response = client.get('/api/statistics/exchange-rates', headers=auth_headers)
    assert latest_response.status_code == 200, latest_response.get_data(as_text=True)
    latest_data = latest_response.get_json() or {}
    assert latest_data.get('success') is True
    latest_result = latest_data.get('result') or {}
    assert latest_result.get('dataSource') == 'user_custom'
    assert latest_result.get('baseCurrency') == 'CNY'
    exchange_rates = latest_result.get('exchangeRates') or []
    usd_rate = next((item for item in exchange_rates if item.get('currency') == 'USD'), None)
    assert usd_rate is not None
    assert usd_rate.get('rate') == '0.5'

    delete_response = client.delete('/api/statistics/exchange-rates/custom/USD', headers=auth_headers)
    assert delete_response.status_code == 200, delete_response.get_data(as_text=True)
    delete_data = delete_response.get_json() or {}
    assert delete_data.get('success') is True
    assert delete_data.get('result') is True

    latest_after_delete = client.get('/api/statistics/exchange-rates', headers=auth_headers)
    assert latest_after_delete.status_code == 200, latest_after_delete.get_data(as_text=True)
    latest_after_delete_data = latest_after_delete.get_json() or {}
    assert latest_after_delete_data.get('success') is True
    assert (latest_after_delete_data.get('result') or {}).get('dataSource') != 'user_custom'


@pytest.mark.parametrize('legacy_path,method', [
    ('/api/v1/exchange_rates/user_custom/update.json', 'post'),
    ('/api/v1/exchange_rates/user_custom/delete.json', 'post')
])
def test_exchange_rates_legacy_routes_removed(client, auth_headers, legacy_path, method):
    """旧自定义汇率路径应已移除。"""
    response = getattr(client, method)(legacy_path, json={'currency': 'USD', 'rate': '0.5'}, headers=auth_headers)
    assert response.status_code == 404
