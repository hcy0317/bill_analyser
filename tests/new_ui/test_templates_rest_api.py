"""模板 REST 收口回归测试。"""

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
        username = f'test_templates_{suffix}'
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


def test_normal_template_rest_lifecycle(client, auth_headers):
    """普通模板应支持 REST CRUD、隐藏和排序。"""
    create_response = client.post('/api/templates/', json={
        'templateType': 1,
        'name': 'REST普通模板A',
        'type': 3,
        'categoryId': '101',
        'sourceAccountId': '11',
        'destinationAccountId': '0',
        'sourceAmount': 12345,
        'destinationAmount': 0,
        'hideAmount': False,
        'tagIds': ['1', '2'],
        'comment': '普通模板测试'
    }, headers=auth_headers)
    assert create_response.status_code == 201
    create_data = create_response.get_json()
    assert create_data['success'] is True
    template = create_data['result']
    assert template['templateType'] == 1
    assert template['name'] == 'REST普通模板A'
    assert template['tagIds'] == ['1', '2']
    template_id = template['id']

    second_response = client.post('/api/templates/', json={
        'templateType': 1,
        'name': 'REST普通模板B',
        'type': 2,
        'categoryId': '102',
        'sourceAccountId': '12',
        'destinationAccountId': '0',
        'sourceAmount': 888,
        'destinationAmount': 0,
        'hideAmount': True,
        'tagIds': [],
        'comment': '第二个普通模板'
    }, headers=auth_headers)
    assert second_response.status_code == 201
    second_template_id = second_response.get_json()['result']['id']

    list_response = client.get('/api/templates/?templateType=1', headers=auth_headers)
    assert list_response.status_code == 200
    list_data = list_response.get_json()
    assert list_data['success'] is True
    assert any(item['id'] == template_id for item in list_data['result'])
    assert all(item['templateType'] == 1 for item in list_data['result'])

    detail_response = client.get(f'/api/templates/{template_id}?templateType=1', headers=auth_headers)
    assert detail_response.status_code == 200
    detail_data = detail_response.get_json()
    assert detail_data['result']['name'] == 'REST普通模板A'
    assert detail_data['result']['categoryId'] == '101'

    update_response = client.put(f'/api/templates/{template_id}?templateType=1', json={
        'templateType': 1,
        'name': 'REST普通模板A-已更新',
        'sourceAmount': 54321,
        'comment': '已更新备注'
    }, headers=auth_headers)
    assert update_response.status_code == 200
    update_data = update_response.get_json()
    assert update_data['success'] is True
    assert update_data['result']['name'] == 'REST普通模板A-已更新'
    assert update_data['result']['sourceAmount'] == 54321

    hide_response = client.put(f'/api/templates/{template_id}?templateType=1', json={
        'templateType': 1,
        'hidden': True
    }, headers=auth_headers)
    assert hide_response.status_code == 200
    hide_data = hide_response.get_json()
    assert hide_data['success'] is True
    assert hide_data['result']['hidden'] in [True, 1]

    move_response = client.put('/api/templates/display-orders', json={
        'templateType': 1,
        'newDisplayOrders': [
            {'id': str(template_id), 'displayOrder': 2},
            {'id': str(second_template_id), 'displayOrder': 1}
        ]
    }, headers=auth_headers)
    assert move_response.status_code == 200
    move_data = move_response.get_json()
    assert move_data['success'] is True
    assert move_data['result'] is True

    delete_response = client.delete(f'/api/templates/{template_id}?templateType=1', headers=auth_headers)
    assert delete_response.status_code == 200
    delete_data = delete_response.get_json()
    assert delete_data['success'] is True
    assert delete_data['result'] is True


def test_scheduled_template_rest_lifecycle(client, auth_headers):
    """定时模板应支持独立列表、详情与删除。"""
    create_response = client.post('/api/templates/', json={
        'templateType': 2,
        'name': 'REST定时模板A',
        'type': 3,
        'categoryId': '201',
        'sourceAccountId': '21',
        'destinationAccountId': '0',
        'sourceAmount': 1000,
        'destinationAmount': 0,
        'hideAmount': False,
        'tagIds': ['9'],
        'comment': '定时模板测试',
        'scheduledFrequencyType': 2,
        'scheduledFrequency': '1,15',
        'scheduledStartDate': '2026-03-01',
        'scheduledEndDate': '2026-12-31',
        'utcOffset': 480
    }, headers=auth_headers)
    assert create_response.status_code == 201
    create_data = create_response.get_json()
    assert create_data['success'] is True
    assert create_data['result']['templateType'] == 2
    assert create_data['result']['scheduledFrequencyType'] == 2
    assert create_data['result']['scheduledFrequency'] == '1,15'
    scheduled_id = create_data['result']['id']

    list_response = client.get('/api/templates/?templateType=2', headers=auth_headers)
    assert list_response.status_code == 200
    list_data = list_response.get_json()
    assert list_data['success'] is True
    assert any(item['id'] == scheduled_id for item in list_data['result'])
    assert all(item['templateType'] == 2 for item in list_data['result'])

    detail_response = client.get(f'/api/templates/{scheduled_id}?templateType=2', headers=auth_headers)
    assert detail_response.status_code == 200
    detail_data = detail_response.get_json()
    assert detail_data['success'] is True
    assert detail_data['result']['scheduledStartDate'] == '2026-03-01'
    assert detail_data['result']['scheduledEndDate'] == '2026-12-31'

    delete_response = client.delete(f'/api/templates/{scheduled_id}?templateType=2', headers=auth_headers)
    assert delete_response.status_code == 200
    delete_data = delete_response.get_json()
    assert delete_data['success'] is True
    assert delete_data['result'] is True


