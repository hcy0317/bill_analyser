"""认证域 token refresh REST 收口回归测试。"""


def test_token_refresh_rest_endpoint(client, auth_context):
    """refresh token 应通过 REST 主链提供。"""
    refresh_token = auth_context.get('refresh_token')
    assert refresh_token, f'登录响应缺少 refreshToken: {auth_context}'

    refresh_response = client.post('/api/tokens/refresh', json={
        'refreshToken': refresh_token
    })
    assert refresh_response.status_code == 200

    data = refresh_response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert result.get('token')
    assert result.get('newToken')
    assert result.get('refreshToken')



def test_token_refresh_legacy_route_removed(client):
    """旧 refresh token 路径应返回404。"""
    response = client.post('/api/v1/tokens/refresh.json', json={
        'refreshToken': 'legacy-token'
    })
    assert response.status_code == 404
