"""认证域用户数据统计 REST 收口回归测试。"""

import time

import pytest


@pytest.fixture(scope='module', name='auth_headers')
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    login_response = client.post('/api/auth/login', json={
        'loginName': 'admin',
        'password': 'admin123'
    })

    if login_response.status_code != 200:
        suffix = int(time.time())
        username = f'test_auth_stats_{suffix}'
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



def test_user_data_statistics_rest_endpoint(client, auth_headers):
    """用户数据统计应通过 REST 主链提供。"""
    response = client.get('/api/data/statistics', headers=auth_headers)
    assert response.status_code == 200

    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert 'billCount' in result
    assert 'accountCount' in result
    assert 'categoryCount' in result
    assert 'tagCount' in result
    assert 'templateCount' in result



def test_user_data_statistics_legacy_route_removed(client, auth_headers):
    """旧用户数据统计路径应返回404。"""
    response = client.get('/api/v1/data/statistics.json', headers=auth_headers)
    assert response.status_code == 404
