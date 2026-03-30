"""账户与标签 REST 收口回归测试。"""

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
        username = f'test_accounts_tags_{suffix}'
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


def test_tag_rest_lifecycle(client, auth_headers):
    """标签 CRUD 与排序应全部走 REST 接口。"""
    create_response = client.post('/api/tags/', json={
        'name': 'REST标签A',
        'color': '#FF0000',
        'icon': '1'
    }, headers=auth_headers)
    assert create_response.status_code == 201
    create_data = create_response.get_json()
    assert create_data['success'] is True
    assert create_data['result']['name'] == 'REST标签A'
    tag_id = create_data['result']['id']

    second_response = client.post('/api/tags/', json={
        'name': 'REST标签B',
        'color': '#00FF00',
        'icon': '2'
    }, headers=auth_headers)
    assert second_response.status_code == 201
    second_tag_id = second_response.get_json()['result']['id']
    assert second_tag_id

    update_response = client.put(f'/api/tags/{tag_id}', json={
        'name': 'REST标签A-已更新'
    }, headers=auth_headers)
    assert update_response.status_code == 200
    update_data = update_response.get_json()
    assert update_data['success'] is True
    assert update_data['result']['name'] == 'REST标签A-已更新'

    hide_response = client.put(f'/api/tags/{tag_id}', json={
        'hidden': True
    }, headers=auth_headers)
    assert hide_response.status_code == 200
    hide_data = hide_response.get_json()
    assert hide_data['success'] is True
    assert hide_data['result']['hidden'] in [True, 1]

    move_response = client.put('/api/tags/display-orders', json={
        'newDisplayOrders': [
            {'id': str(tag_id), 'displayOrder': 2},
            {'id': str(second_tag_id), 'displayOrder': 1}
        ]
    }, headers=auth_headers)
    assert move_response.status_code == 200
    move_data = move_response.get_json()
    assert move_data['success'] is True
    assert move_data['result'] is True

    batch_response = client.post('/api/tags/batch', json={
        'tags': [
            {'name': 'REST批量标签1'},
            {'name': 'REST批量标签2'}
        ],
        'skipExists': True
    }, headers=auth_headers)
    assert batch_response.status_code == 201
    batch_data = batch_response.get_json()
    assert batch_data['success'] is True
    assert len(batch_data['result']) == 2

    delete_response = client.delete(f'/api/tags/{tag_id}', headers=auth_headers)
    assert delete_response.status_code == 200
    delete_data = delete_response.get_json()
    assert delete_data['success'] is True
    assert delete_data['result'] is True


def test_account_rest_hide_move_and_delete_subaccount(client, auth_headers):
    """账户隐藏、排序、子账户删除应全部走 REST 接口。"""
    create_parent_response = client.post('/api/accounts/', json={
        'name': 'REST父账户',
        'category': 1,
        'type': 2,
        'icon': '1',
        'color': 'ffcc00',
        'currency': 'CNY',
        'balance': 0,
        'comment': '父账户测试',
        'hidden': False,
        'aliases': [],
        'subAccounts': [
            {
                'name': 'REST子账户',
                'category': 1,
                'type': 1,
                'icon': '1',
                'color': '00ccff',
                'currency': 'CNY',
                'balance': 1000,
                'comment': '子账户测试',
                'hidden': False,
                'aliases': []
            }
        ]
    }, headers=auth_headers)
    assert create_parent_response.status_code == 201
    parent_data = create_parent_response.get_json()
    assert parent_data['success'] is True
    parent_account = parent_data['result']
    parent_id = parent_account['id']
    assert parent_account['subAccounts']
    sub_account_id = parent_account['subAccounts'][0]['id']

    create_second_response = client.post('/api/accounts/', json={
        'name': 'REST排序账户',
        'category': 1,
        'type': 1,
        'icon': '1',
        'color': '00ff00',
        'currency': 'CNY',
        'balance': 0,
        'comment': '排序测试',
        'hidden': False,
        'aliases': []
    }, headers=auth_headers)
    assert create_second_response.status_code == 201
    second_account_id = create_second_response.get_json()['result']['id']

    hide_response = client.put(f'/api/accounts/{parent_id}', json={
        'hidden': True
    }, headers=auth_headers)
    assert hide_response.status_code == 200
    hide_data = hide_response.get_json()
    assert hide_data['success'] is True
    assert hide_data['result']['hidden'] in [True, 1]

    move_response = client.put('/api/accounts/display-orders', json={
        'newDisplayOrders': [
            {'id': str(parent_id), 'displayOrder': 2},
            {'id': str(second_account_id), 'displayOrder': 1}
        ]
    }, headers=auth_headers)
    assert move_response.status_code == 200
    move_data = move_response.get_json()
    assert move_data['success'] is True
    assert move_data['result'] is True

    delete_sub_response = client.delete(f'/api/accounts/{sub_account_id}', headers=auth_headers)
    assert delete_sub_response.status_code == 200
    delete_sub_data = delete_sub_response.get_json()
    assert delete_sub_data['success'] is True
    assert delete_sub_data['result'] is True

    parent_detail_response = client.get(f'/api/accounts/{parent_id}', headers=auth_headers)
    assert parent_detail_response.status_code == 200
    parent_detail_data = parent_detail_response.get_json()
    assert parent_detail_data['success'] is True
    assert not parent_detail_data['result'].get('subAccounts')


def test_account_rest_move_and_clear_transactions(client, auth_headers, operation_password):
    """账户相关剩余批量动作应走 REST 扩展接口。"""
    source_response = client.post('/api/accounts/', json={
        'name': 'REST迁移源账户',
        'category': 1,
        'type': 1,
        'icon': '1',
        'color': 'ff0000',
        'currency': 'CNY',
        'balance': 0,
        'comment': '源账户',
        'hidden': False,
        'aliases': []
    }, headers=auth_headers)
    assert source_response.status_code == 201
    source_id = source_response.get_json()['result']['id']

    target_response = client.post('/api/accounts/', json={
        'name': 'REST迁移目标账户',
        'category': 1,
        'type': 1,
        'icon': '1',
        'color': '00ff00',
        'currency': 'CNY',
        'balance': 0,
        'comment': '目标账户',
        'hidden': False,
        'aliases': []
    }, headers=auth_headers)
    assert target_response.status_code == 201
    target_id = target_response.get_json()['result']['id']

    bill_response = client.post('/api/bills', json={
        'time': 1704067200000,
        'type': 3,
        'sourceAmount': 1000,
        'sourceAccountId': str(source_id),
        'categoryId': '0',
        'tagIds': [],
        'comment': '账户批量迁移测试'
    }, headers=auth_headers)
    assert bill_response.status_code in [200, 201]

    move_response = client.post(f'/api/accounts/{source_id}/transactions/move', json={
        'toAccountId': str(target_id),
        'password': operation_password
    }, headers=auth_headers)
    assert move_response.status_code == 200
    move_data = move_response.get_json()
    assert move_data['success'] is True
    assert move_data['result'] is True
    assert move_data['moved_count'] >= 1

    clear_response = client.post(f'/api/accounts/{target_id}/transactions/clear', json={
        'password': operation_password
    }, headers=auth_headers)
    assert clear_response.status_code == 200
    clear_data = clear_response.get_json()
    assert clear_data['success'] is True
    assert clear_data['result'] is True
    assert clear_data['deleted_count'] >= 1


def test_legacy_account_bulk_action_routes_removed(client, auth_headers):
    """账户批量交易旧 v1 兼容端点应已移除。"""
    move_response = client.post('/api/v1/transactions/move/all.json', json={
        'fromAccountId': '1',
        'toAccountId': '2',
        'password': 'admin123'
    }, headers=auth_headers)
    assert move_response.status_code == 404

    clear_response = client.post('/api/v1/data/clear/transactions/by_account.json', json={
        'accountId': '1',
        'password': 'admin123'
    }, headers=auth_headers)
    assert clear_response.status_code == 404
