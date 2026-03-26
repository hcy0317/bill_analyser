"""用户资料保存持久化测试。"""

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
    """注册并登录测试用户，返回鉴权请求头。"""
    suffix = int(datetime.now().timestamp())
    username = f'test_profile_persist_{suffix}'
    password = 'Test123456!'

    register_response = client.post('/api/auth/register', json={
        'username': username,
        'email': f'{username}@example.com',
        'password': password,
        'nickname': username
    })

    assert register_response.status_code in [200, 409], (
        f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
    )

    login_response = client.post('/api/auth/login', json={
        'loginName': username,
        'password': password
    })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )

    data = login_response.get_json() or {}
    token = (data.get('result') or {}).get('token')
    assert token, f"登录响应缺少token: {data}"

    return {'Authorization': f'Bearer {token}'}


def test_profile_update_persistence(client, auth_headers):
    """更新用户资料后，再次读取应保持持久化。"""
    get_before = client.get('/api/profile', headers=auth_headers)
    assert get_before.status_code == 200

    before_data = get_before.get_json() or {}
    assert before_data.get('success') is True

    before_profile = (before_data.get('result') or {})
    assert before_profile.get('username'), '初始用户资料缺少 username'

    suffix = int(datetime.now().timestamp())
    update_payload = {
        'nickname': f'persist_nickname_{suffix}',
        'language': 'en',
        'fiscalYearStart': 4,
        'cashAccountId': '9001',
        'cashTransferCategoryId': '9002',
        'investmentPlatformKeywords': ['蚂蚁财富', '京东金融'],
        'investmentProductKeywords': ['指数基金', 'REITs'],
        'investmentExcludeKeywords': ['还款', '账单']
    }

    update_response = client.put('/api/profile', json=update_payload, headers=auth_headers)
    assert update_response.status_code == 200, update_response.get_data(as_text=True)

    update_data = update_response.get_json() or {}
    assert update_data.get('success') is True

    update_user = ((update_data.get('result') or {}).get('user') or {})
    assert update_user.get('nickname') == update_payload['nickname']
    assert update_user.get('language') == update_payload['language']
    assert update_user.get('fiscalYearStart') == update_payload['fiscalYearStart']
    assert str(update_user.get('cashAccountId')) == update_payload['cashAccountId']
    assert str(update_user.get('cashTransferCategoryId')) == update_payload['cashTransferCategoryId']
    assert update_user.get('investmentPlatformKeywords') == update_payload['investmentPlatformKeywords']
    assert update_user.get('investmentProductKeywords') == update_payload['investmentProductKeywords']
    assert update_user.get('investmentExcludeKeywords') == update_payload['investmentExcludeKeywords']

    get_after = client.get('/api/profile', headers=auth_headers)
    assert get_after.status_code == 200

    after_data = get_after.get_json() or {}
    assert after_data.get('success') is True

    after_profile = (after_data.get('result') or {})
    assert after_profile.get('nickname') == update_payload['nickname']
    assert after_profile.get('language') == update_payload['language']
    assert after_profile.get('fiscalYearStart') == update_payload['fiscalYearStart']
    assert str(after_profile.get('cashAccountId')) == update_payload['cashAccountId']
    assert str(after_profile.get('cashTransferCategoryId')) == update_payload['cashTransferCategoryId']
    assert after_profile.get('investmentPlatformKeywords') == update_payload['investmentPlatformKeywords']
    assert after_profile.get('investmentProductKeywords') == update_payload['investmentProductKeywords']
    assert after_profile.get('investmentExcludeKeywords') == update_payload['investmentExcludeKeywords']


def test_legacy_profile_routes_removed(client, auth_headers):
    """旧的 profile get/update rewrite 路径应已移除。"""
    legacy_profile = client.get('/api/v1/users/profile.json', headers=auth_headers)
    assert legacy_profile.status_code == 404

    legacy_get = client.get('/api/v1/users/profile/get.json', headers=auth_headers)
    assert legacy_get.status_code == 404

    legacy_update = client.post(
        '/api/v1/users/profile/update.json',
        json={'nickname': 'legacy_should_fail'},
        headers=auth_headers
    )
    assert legacy_update.status_code == 404


def test_register_initializes_investment_categories_and_default_accounts(client):
    """注册后应初始化投资分类与默认账户模板。"""
    suffix = int(datetime.now().timestamp())
    username = f'test_register_init_{suffix}'
    password = 'Test123456!'

    register_payload = {
        'username': username,
        'email': f'{username}@example.com',
        'password': password,
        'nickname': username,
        'language': 'zh_Hans',
        'defaultCurrency': 'CNY',
        'firstDayOfWeek': 1,
        'categories': [
            {
                'name': '投资本金',
                'type': 5,
                'icon': '800',
                'color': 'ff9500',
                'subCategories': [
                    {
                        'name': '基金投资',
                        'type': 5,
                        'icon': '810',
                        'color': 'ff9500'
                    }
                ]
            }
        ]
    }

    register_response = client.post('/api/auth/register', json=register_payload)
    assert register_response.status_code == 200, register_response.get_data(as_text=True)

    register_data = register_response.get_json() or {}
    assert register_data.get('success') is True
    result = register_data.get('result') or {}
    assert result.get('presetCategoriesSaved') is True
    assert result.get('presetAccountsSaved') is True

    from src.api.app import app
    db = app.config.get('DB_INSTANCE')
    assert db is not None

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        user = loop.run_until_complete(db.get_user_by_username(username))
        assert user is not None
        user_id = int(user['id'])

        categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
        accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
    finally:
        loop.close()

    assert any(int(c.get('type', 0)) == 5 for c in categories), '未初始化投资类型分类'

    account_names = {str(a.get('name', '')).strip() for a in accounts}
    expected_accounts = {'现金', '借记卡', '信用卡', '支付宝', '微信'}
    assert expected_accounts.issubset(account_names), f'默认账户缺失: {expected_accounts - account_names}'
