"""统计旧兼容子路径 404 回归测试。"""

# pylint: disable=import-outside-toplevel

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
    """获取认证请求头。"""
    login_response = client.post('/api/auth/login', json={
        'loginName': 'admin',
        'password': 'admin123'
    })

    if login_response.status_code != 200:
        suffix = int(datetime.now().timestamp())
        username = f'test_stats_legacy_{suffix}'
        register_response = client.post('/api/auth/register', json={
            'username': username,
            'email': f'{username}@example.com',
            'password': 'Test123456!',
            'nickname': username
        })
        assert register_response.status_code in [200, 409], (
            f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
        )

        login_response = client.post('/api/auth/login', json={
            'loginName': username,
            'password': 'Test123456!'
        })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    token = (data.get('result') or {}).get('token')
    assert token, f"登录响应缺少token: {data}"
    return {'Authorization': f'Bearer {token}'}


def test_legacy_category_statistics_route_removed(client, auth_headers):
    """旧分类统计兼容路径应返回404。"""
    response = client.get(
        '/api/statistics/transaction-statistics',
        query_string={'startTime': 0, 'endTime': 0},
        headers=auth_headers
    )
    assert response.status_code == 404



def test_legacy_trend_statistics_route_removed(client, auth_headers):
    """旧趋势统计兼容路径应返回404。"""
    response = client.get(
        '/api/statistics/transaction-statistics/trends',
        query_string={'startYearMonth': '202501', 'endYearMonth': '202512'},
        headers=auth_headers
    )
    assert response.status_code == 404



def test_legacy_asset_trends_route_removed(client, auth_headers):
    """旧资产趋势兼容路径应返回404。"""
    response = client.get(
        '/api/statistics/transaction-statistics/asset-trends',
        query_string={'startTime': 0, 'endTime': 0},
        headers=auth_headers
    )
    assert response.status_code == 404


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--tb=short'])
