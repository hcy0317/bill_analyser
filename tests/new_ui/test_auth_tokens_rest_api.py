"""认证域 token REST 收口回归测试。"""

import pytest



def test_tokens_rest_endpoint(client, auth_headers):
    """token 列表应通过 REST 主链提供。"""
    response = client.get('/api/tokens', headers=auth_headers)
    assert response.status_code == 200

    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or []
    assert isinstance(result, list)
    if result:
        first = result[0]
        assert 'tokenId' in first
        assert 'tokenType' in first
        assert 'lastSeen' in first
        assert 'isCurrent' in first
        assert 'isCurrentToken' in first



def test_generate_api_token_rest_endpoint(client, auth_context):
    """API token 应通过 REST 主链生成。"""
    response = client.post('/api/tokens/api', headers=auth_context['headers'], json={
        'expiresInSeconds': 3600,
        'password': auth_context['password']
    })
    assert response.status_code == 200

    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert result.get('token')
    assert result.get('apiBaseUrl', '').endswith('/api')



def test_generate_mcp_token_rest_endpoint(client, auth_context):
    """MCP token 应通过 REST 主链生成。"""
    response = client.post('/api/tokens/mcp', headers=auth_context['headers'], json={
        'expiresInSeconds': 3600,
        'password': auth_context['password']
    })
    assert response.status_code == 200

    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert result.get('token')
    assert result.get('mcpUrl', '').endswith('/mcp')



def test_revoke_token_rest_endpoint(client, auth_context):
    """单个 token 会话应通过 REST 主链撤销。"""
    create_response = client.post('/api/tokens/api', headers=auth_context['headers'], json={
        'expiresInSeconds': 3600,
        'password': auth_context['password']
    })
    assert create_response.status_code == 200

    list_response = client.get('/api/tokens', headers=auth_context['headers'])
    assert list_response.status_code == 200
    tokens = (list_response.get_json() or {}).get('result') or []
    api_tokens = [token for token in tokens if token.get('tokenType') == 8 and not token.get('isCurrent')]
    assert api_tokens, f'未找到可撤销的 API token: {tokens}'

    token_id = api_tokens[0]['tokenId']
    revoke_response = client.delete(f'/api/tokens/{token_id}', headers=auth_context['headers'])
    assert revoke_response.status_code == 200

    revoke_data = revoke_response.get_json() or {}
    assert revoke_data.get('success') is True
    assert revoke_data.get('result') is True

    refreshed_tokens = (client.get('/api/tokens', headers=auth_context['headers']).get_json() or {}).get('result') or []
    assert all(token.get('tokenId') != token_id for token in refreshed_tokens)



def test_revoke_all_tokens_rest_endpoint(client, auth_context):
    """撤销其他会话应通过 REST 主链提供。"""
    extra_login = client.post('/api/auth/login', json={
        'loginName': auth_context['username'],
        'password': auth_context['password']
    })
    assert extra_login.status_code == 200

    response = client.delete('/api/tokens', headers=auth_context['headers'])
    assert response.status_code == 200

    data = response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True
    assert 'revokedCount' in data



def test_tokens_legacy_route_removed(client, auth_headers):
    """旧 token 列表路径应返回404。"""
    response = client.get('/api/v1/tokens/list.json', headers=auth_headers)
    assert response.status_code == 404


@pytest.mark.parametrize('legacy_path', [
    '/api/v1/tokens/generate/api.json',
    '/api/v1/tokens/generate/mcp.json',
    '/api/v1/tokens/revoke.json',
    '/api/v1/tokens/revoke_all.json',
])
def test_token_write_legacy_routes_removed(client, auth_headers, legacy_path):
    """旧 token 写接口路径应返回404。"""
    response = client.post(legacy_path, headers=auth_headers, json={})
    assert response.status_code == 404
