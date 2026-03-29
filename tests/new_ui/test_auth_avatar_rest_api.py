"""认证域头像 REST 收口回归测试。"""

import asyncio
from datetime import datetime
from io import BytesIO

import pytest


PNG_BYTES = (
    b'\x89PNG\r\n\x1a\n'
    b'\x00\x00\x00\rIHDR'
    b'\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89'
    b'\x00\x00\x00\x0cIDATx\x9cc```\x00\x00\x00\x04\x00\x01\x0b\xe7\x02\x9d'
    b'\x00\x00\x00\x00IEND\xaeB`\x82'
)



def test_avatar_rest_endpoints(client, auth_headers):
    """头像更新与删除应走 REST 主链。"""
    upload_response = client.post(
        '/api/profile/avatar',
        data={
            'avatar': (BytesIO(PNG_BYTES), 'avatar.png')
        },
        headers=auth_headers,
        content_type='multipart/form-data'
    )
    assert upload_response.status_code == 200, upload_response.get_data(as_text=True)

    upload_data = upload_response.get_json() or {}
    assert upload_data.get('success') is True
    uploaded_profile = upload_data.get('result') or {}
    assert uploaded_profile.get('avatar', '').startswith('data:image/png;base64,')
    assert uploaded_profile.get('avatarProvider') == 'internal'

    remove_response = client.delete('/api/profile/avatar', headers=auth_headers)
    assert remove_response.status_code == 200, remove_response.get_data(as_text=True)

    remove_data = remove_response.get_json() or {}
    assert remove_data.get('success') is True
    removed_profile = remove_data.get('result') or {}
    assert removed_profile.get('avatar') == ''
    assert removed_profile.get('avatarProvider') == 'internal'


@pytest.mark.parametrize('legacy_path,method', [
    ('/api/v1/users/avatar/update.json', 'post'),
    ('/api/v1/users/avatar/remove.json', 'post')
])
def test_avatar_legacy_routes_removed(client, auth_headers, legacy_path, method):
    """旧头像路径应已移除。"""
    response = getattr(client, method)(legacy_path, headers=auth_headers)
    assert response.status_code == 404


def test_verify_email_resend_rest_endpoint(client, auth_headers):
    """验证邮件重发应走 REST 主链。"""
    response = client.post('/api/profile/email/resend-verification', headers=auth_headers)
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True


def test_verify_email_resend_legacy_route_removed(client, auth_headers):
    """旧验证邮件重发路径应已移除。"""
    response = client.post('/api/v1/users/verify_email/resend.json', headers=auth_headers)
    assert response.status_code == 404


def test_verify_email_by_token_rest_endpoint(client, auth_context):
    """邮箱验证应走新的 REST 主链。"""
    from src.api.routes import auth as auth_module

    config = auth_module.load_auth_config()
    token = auth_module.generate_action_token(
        user_id=asyncio.run(
            __import__('src.api.app', fromlist=['app']).app.config['DB_INSTANCE']
            .get_user_by_username(auth_context['username'])
        )['id'],
        username=auth_context['username'],
        email=f"{auth_context['username']}@example.com",
        config=config,
        token_type='verify_email'
    )

    response = client.post('/api/auth/email/verify', json={
        'token': token,
        'requestNewToken': True
    })
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert result.get('newToken')
    assert (result.get('user') or {}).get('emailVerified') is True


def test_verify_email_guest_resend_rest_endpoint(client, auth_context):
    """未登录用户重发验证邮件应走新的 REST 主链。"""
    response = client.post('/api/auth/email/resend-verification', json={
        'email': f"{auth_context['username']}@example.com",
        'password': auth_context['password']
    })
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True


def test_logout_rest_endpoint(client, auth_context):
    """登出应走 REST 主链。"""
    login_response = client.post('/api/auth/login', json={
        'loginName': auth_context['username'],
        'password': auth_context['password']
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    login_data = login_response.get_json() or {}
    token = (login_data.get('result') or {}).get('token')
    assert token

    response = client.post('/api/auth/logout', headers={
        'Authorization': f'Bearer {token}'
    })
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True


def test_password_reset_rest_endpoints(client, monkeypatch):
    """找回密码与重置密码应走新的 REST 主链。"""
    from src.api.routes import auth as auth_module
    from src.api.app import app

    suffix = int(datetime.now().timestamp() * 1000)
    username = f'test_reset_rest_{suffix}'
    old_password = 'OldPass123!'
    email = f'{username}@example.com'

    register_response = client.post('/api/auth/register', json={
        'username': username,
        'email': email,
        'password': old_password,
        'nickname': username
    })
    assert register_response.status_code in [200, 409], register_response.get_data(as_text=True)

    original_config = auth_module.load_auth_config

    def _patched_config():
        config = original_config().copy()
        config['enable_user_forget_password'] = True
        return config

    monkeypatch.setattr(auth_module, 'load_auth_config', _patched_config)

    request_response = client.post('/api/auth/password/forgot', json={
        'email': email
    })
    assert request_response.status_code == 200, request_response.get_data(as_text=True)
    request_data = request_response.get_json() or {}
    assert request_data.get('success') is True
    assert request_data.get('result') is True

    db = app.config['DB_INSTANCE']
    user = asyncio.run(db.get_user_by_username(username))
    token = auth_module.generate_action_token(
        user_id=user['id'],
        username=user['username'],
        email=user['email'],
        config=_patched_config(),
        token_type='reset_password'
    )

    new_password = 'NewPass123!'
    reset_response = client.post('/api/auth/password/reset', json={
        'email': user['email'],
        'password': new_password,
        'token': token
    })
    assert reset_response.status_code == 200, reset_response.get_data(as_text=True)
    reset_data = reset_response.get_json() or {}
    assert reset_data.get('success') is True
    assert reset_data.get('result') is True

    login_response = client.post('/api/auth/login', json={
        'loginName': username,
        'password': new_password
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)


def test_oauth2_authorize_rest_endpoint_disabled_by_default(client):
    """OAuth2 回调授权应有 REST 入口，并在默认配置下明确返回禁用。"""
    response = client.post('/api/auth/oauth2/authorize', json={
        'password': 'irrelevant',
        'token': 'optional-current-token'
    }, headers={
        'Authorization': 'Bearer callback-token'
    })
    assert response.status_code == 403, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is False
    assert data.get('error') == 'OAuth2 disabled'


@pytest.mark.parametrize('legacy_path,method', [
    ('/api/authorize.json', 'post'),
    ('/api/register.json', 'post'),
    ('/api/logout.json', 'post'),
    ('/api/verify_email/by_token.json', 'post'),
    ('/api/verify_email/resend.json', 'post'),
    ('/api/forget_password/request.json', 'post'),
    ('/api/forget_password/reset/by_token.json', 'post'),
    ('/api/oauth2/authorize.json', 'post')
])
def test_auth_entry_legacy_routes_removed(client, legacy_path, method):
    """旧登录/注册/登出路径应已移除。"""
    response = getattr(client, method)(legacy_path)
    assert response.status_code == 404


def test_external_auths_rest_endpoints(client, auth_context):
    """第三方登录列表与解绑应走 REST 主链。"""
    from src.api.app import app

    db = app.config['DB_INSTANCE']
    user = asyncio.run(db.get_user_by_username(auth_context['username']))
    assert user is not None

    asyncio.run(db.create_user_external_auth({
        'user_id': user['id'],
        'external_auth_category': 'oauth2',
        'external_auth_type': 'github',
        'external_user_id': 'github-user-1',
        'external_username': 'octocat'
    }))

    list_response = client.get('/api/profile/external-auths', headers=auth_context['headers'])
    assert list_response.status_code == 200, list_response.get_data(as_text=True)
    list_data = list_response.get_json() or {}
    assert list_data.get('success') is True
    result = list_data.get('result') or []
    github_auth = next((item for item in result if item.get('externalAuthType') == 'github'), None)
    assert github_auth is not None
    assert github_auth.get('externalAuthCategory') == 'oauth2'
    assert github_auth.get('linked') is True
    assert github_auth.get('externalUsername') == 'octocat'

    unlink_response = client.post('/api/profile/external-auths/unlink', json={
        'externalAuthType': 'github',
        'password': auth_context['password']
    }, headers=auth_context['headers'])
    assert unlink_response.status_code == 200, unlink_response.get_data(as_text=True)
    unlink_data = unlink_response.get_json() or {}
    assert unlink_data.get('success') is True
    assert unlink_data.get('result') is True

    after_response = client.get('/api/profile/external-auths', headers=auth_context['headers'])
    assert after_response.status_code == 200
    after_data = after_response.get_json() or {}
    after_items = after_data.get('result') or []
    assert all(item.get('externalAuthType') != 'github' or item.get('linked') is not True for item in after_items)


@pytest.mark.parametrize('legacy_path,method', [
    ('/api/v1/users/external_auth/list.json', 'get'),
    ('/api/v1/users/external_auth/unlink.json', 'post')
])
def test_external_auths_legacy_routes_removed(client, auth_headers, legacy_path, method):
    """旧第三方登录路径应已移除。"""
    response = getattr(client, method)(legacy_path, headers=auth_headers)
    assert response.status_code == 404


def test_cloud_settings_rest_endpoints(client, auth_context):
    """应用云同步设置应走 REST 主链。"""
    headers = auth_context['headers']

    initial_response = client.get('/api/profile/cloud-settings', headers=headers)
    assert initial_response.status_code == 200, initial_response.get_data(as_text=True)
    initial_data = initial_response.get_json() or {}
    assert initial_data.get('success') is True
    assert initial_data.get('result') is False

    settings = [
        {
            'settingKey': 'showAccountBalance',
            'settingValue': 'true'
        },
        {
            'settingKey': 'itemsCountInTransactionListPage',
            'settingValue': '25'
        },
    ]

    update_response = client.put('/api/profile/cloud-settings', json={
        'settings': settings,
        'fullUpdate': True
    }, headers=headers)
    assert update_response.status_code == 200, update_response.get_data(as_text=True)
    update_data = update_response.get_json() or {}
    assert update_data.get('success') is True
    assert update_data.get('result') is True

    get_response = client.get('/api/profile/cloud-settings', headers=headers)
    assert get_response.status_code == 200, get_response.get_data(as_text=True)
    get_data = get_response.get_json() or {}
    assert get_data.get('success') is True
    result = get_data.get('result') or []
    result_map = {item['settingKey']: item['settingValue'] for item in result}
    assert result_map == {
        'showAccountBalance': 'true',
        'itemsCountInTransactionListPage': '25'
    }

    relogin_response = client.post('/api/auth/login', json={
        'loginName': auth_context['username'],
        'password': auth_context['password']
    })
    assert relogin_response.status_code == 200, relogin_response.get_data(as_text=True)
    relogin_data = relogin_response.get_json() or {}
    relogin_result = relogin_data.get('result') or {}
    relogin_settings = relogin_result.get('applicationCloudSettings') or []
    relogin_settings_map = {item['settingKey']: item['settingValue'] for item in relogin_settings}
    assert relogin_settings_map == result_map

    delete_response = client.delete('/api/profile/cloud-settings', headers=headers)
    assert delete_response.status_code == 200, delete_response.get_data(as_text=True)
    delete_data = delete_response.get_json() or {}
    assert delete_data.get('success') is True
    assert delete_data.get('result') is True

    after_delete_response = client.get('/api/profile/cloud-settings', headers=headers)
    assert after_delete_response.status_code == 200
    after_delete_data = after_delete_response.get_json() or {}
    assert after_delete_data.get('success') is True
    assert after_delete_data.get('result') is False


@pytest.mark.parametrize('legacy_path,method', [
    ('/api/v1/users/settings/cloud/get.json', 'get'),
    ('/api/v1/users/settings/cloud/update.json', 'post'),
    ('/api/v1/users/settings/cloud/disable.json', 'post')
])
def test_cloud_settings_legacy_routes_removed(client, auth_headers, legacy_path, method):
    """旧应用云同步设置路径应已移除。"""
    response = getattr(client, method)(legacy_path, headers=auth_headers)
    assert response.status_code == 404
