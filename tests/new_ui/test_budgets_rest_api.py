"""预算 REST 收口回归测试。"""

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
        username = f'test_budgets_{suffix}'
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


def test_budget_rest_crud_and_analysis(client, auth_headers):
    """预算主链应全部走 REST 接口。"""
    unique_name = f'REST预算-{int(time.time())}'

    create_response = client.post('/api/budgets/', json={
        'name': unique_name,
        'category': 'REST测试分类',
        'sub_category': 'REST测试子分类',
        'period_type': 'monthly',
        'amount': 123.45,
        'start_date': '2026-01-01',
        'end_date': '2026-12-31',
        'alert_threshold': 75,
        'enabled': True
    }, headers=auth_headers)
    assert create_response.status_code == 201
    create_data = create_response.get_json()
    assert create_data['success'] is True
    budget_id = create_data['result']['id']

    detail_response = client.get(f'/api/budgets/{budget_id}', headers=auth_headers)
    assert detail_response.status_code == 200
    detail_data = detail_response.get_json()
    assert detail_data['success'] is True
    assert detail_data['result']['name'] == unique_name

    update_response = client.put(f'/api/budgets/{budget_id}', json={
        'name': unique_name + '-更新',
        'category': 'REST测试分类',
        'sub_category': 'REST测试子分类',
        'period_type': 'monthly',
        'amount': 222.22,
        'start_date': '2026-01-01',
        'end_date': '2026-12-31',
        'alert_threshold': 80,
        'enabled': True
    }, headers=auth_headers)
    assert update_response.status_code == 200
    update_data = update_response.get_json()
    assert update_data['success'] is True

    execution_response = client.get('/api/budgets/execution?budget_type=3&period_type=monthly', headers=auth_headers)
    assert execution_response.status_code == 200
    execution_data = execution_response.get_json()
    assert execution_data['success'] is True
    assert 'items' in execution_data['result']
    assert 'summary' in execution_data['result']

    forecast_response = client.get('/api/budgets/forecast?budget_type=3&period_type=monthly', headers=auth_headers)
    assert forecast_response.status_code == 200
    forecast_data = forecast_response.get_json()
    assert forecast_data['success'] is True
    assert 'items' in forecast_data['result']

    export_response = client.get('/api/budgets/export', headers=auth_headers)
    assert export_response.status_code == 200
    export_data = export_response.get_json()
    assert export_data['success'] is True
    assert isinstance(export_data['result'], list)

    import_name = f'REST导入预算-{int(time.time())}'
    import_response = client.post('/api/budgets/import', json=[{
        'name': import_name,
        'category': 'REST导入分类',
        'sub_category': '',
        'period_type': 'monthly',
        'amount': 88.88,
        'start_date': '2026-02-01',
        'end_date': '2026-02-28',
        'alert_threshold': 70,
        'enabled': True
    }], headers=auth_headers)
    assert import_response.status_code == 200
    import_data = import_response.get_json()
    assert import_data['success'] is True
    assert import_data['result']['created'] >= 1

    delete_response = client.delete(f'/api/budgets/{budget_id}', headers=auth_headers)
    assert delete_response.status_code == 200
    delete_data = delete_response.get_json()
    assert delete_data['success'] is True


def test_legacy_budget_v1_routes_removed(client, auth_headers):
    """预算旧 v1 兼容端点应已移除。"""
    legacy_paths = [
        ('GET', '/api/v1/budgets/list.json'),
        ('GET', '/api/v1/budgets/execution.json'),
        ('GET', '/api/v1/budgets/forecast.json'),
        ('GET', '/api/v1/budgets/export.json'),
        ('POST', '/api/v1/budgets/add.json'),
        ('POST', '/api/v1/budgets/modify.json'),
        ('POST', '/api/v1/budgets/delete.json'),
        ('POST', '/api/v1/budgets/import.json')
    ]

    for method, path in legacy_paths:
        if method == 'GET':
            response = client.get(path, headers=auth_headers)
        else:
            response = client.post(path, json={}, headers=auth_headers)
        assert response.status_code == 404, path


def test_budget_history_snapshot_and_query(client, auth_headers):
    """预算历史快照应可通过 REST 端点创建并查询。"""
    unique_name = f'REST预算快照-{int(time.time())}'

    create_response = client.post('/api/budgets/', json={
        'name': unique_name,
        'category': 'REST快照分类',
        'sub_category': '',
        'period_type': 'monthly',
        'amount': 345.67,
        'start_date': '2026-01-01',
        'end_date': '2026-12-31',
        'alert_threshold': 80,
        'enabled': True
    }, headers=auth_headers)
    assert create_response.status_code == 201
    budget_id = create_response.get_json()['result']['id']

    snapshot_response = client.post('/api/budgets/history/snapshot', json={
        'budget_type': 3,
        'period_type': 'monthly',
        'year': 2026,
        'month': 1,
        'budget_id': budget_id
    }, headers=auth_headers)
    assert snapshot_response.status_code == 200
    snapshot_data = snapshot_response.get_json()
    assert snapshot_data['success'] is True
    assert snapshot_data['result']['created_count'] >= 1
    assert snapshot_data['result']['period_start'] == '2026-01-01'
    assert snapshot_data['result']['period_end'] == '2026-01-31'

    history_response = client.get(
        f'/api/budgets/history?budget_type=3&period_type=monthly&year=2026&month=1&budget_id={budget_id}',
        headers=auth_headers
    )
    assert history_response.status_code == 200
    history_data = history_response.get_json()
    assert history_data['success'] is True
    assert history_data['result']['summary']['count'] >= 1
    assert history_data['result']['summary']['period_start'] == '2026-01-01'
    assert history_data['result']['summary']['period_end'] == '2026-01-31'
    assert any(int(item['budget_id']) == int(budget_id) for item in history_data['result']['items'])

    delete_response = client.delete(f'/api/budgets/{budget_id}', headers=auth_headers)
    assert delete_response.status_code == 200
    assert delete_response.get_json()['success'] is True


def test_budget_forecast_strategy_params(client, auth_headers):
    """预算预测应支持策略参数与历史周期参数。"""
    response = client.get(
        '/api/budgets/forecast?budget_type=3&period_type=monthly&forecast_strategy=moving_average&months_history=3',
        headers=auth_headers
    )
    assert response.status_code == 200
    data = response.get_json()
    assert data['success'] is True
    assert data['result']['summary']['forecast_strategy'] == 'moving_average'
    assert data['result']['summary']['history_periods'] == 3
    assert data['result']['period_start']
    assert data['result']['period_end']
    assert 'daysElapsed' in data['result']
    assert 'daysRemaining' in data['result']
    assert 'avg_backtest_mape' in data['result']['summary']

    forecast_items = data['result']['items']
    if forecast_items:
        item = forecast_items[0]
        assert 'backtest_mape' in item
        assert 'confidence' in item
        assert item['confidence'] in ['high', 'medium', 'low']


def test_budget_primary_secondary_rules(client, auth_headers):
    """一级/二级预算应遵循自动补父预算、总额下限与级联删除规则。"""
    category_name = f'层级预算分类-{int(time.time())}'
    period_payload = {
        'category': category_name,
        'period_type': 'monthly',
        'start_date': '2026-03-01',
        'end_date': '2026-03-31',
        'alert_threshold': 80,
        'enabled': True
    }

    create_secondary_a = client.post('/api/budgets/', json={
        'name': '二级预算-A',
        **period_payload,
        'sub_category': '子分类A',
        'amount': 100.0
    }, headers=auth_headers)
    assert create_secondary_a.status_code == 201

    first_list = client.get('/api/budgets/', query_string={'category': category_name}, headers=auth_headers)
    assert first_list.status_code == 200
    first_items = first_list.get_json()['result']
    assert len(first_items) == 2

    primary_budget = next(item for item in first_items if not item.get('sub_category'))
    assert primary_budget['amount'] == 100.0

    update_primary = client.put(f"/api/budgets/{primary_budget['id']}", json={
        'name': primary_budget.get('name', ''),
        **period_payload,
        'sub_category': '',
        'amount': 180.0
    }, headers=auth_headers)
    assert update_primary.status_code == 200

    create_secondary_b = client.post('/api/budgets/', json={
        'name': '二级预算-B',
        **period_payload,
        'sub_category': '子分类B',
        'amount': 30.0
    }, headers=auth_headers)
    assert create_secondary_b.status_code == 201

    second_list = client.get('/api/budgets/', query_string={'category': category_name}, headers=auth_headers)
    second_items = second_list.get_json()['result']
    updated_primary = next(item for item in second_items if int(item['id']) == int(primary_budget['id']))
    assert updated_primary['amount'] == 180.0

    create_secondary_c = client.post('/api/budgets/', json={
        'name': '二级预算-C',
        **period_payload,
        'sub_category': '子分类C',
        'amount': 70.0
    }, headers=auth_headers)
    assert create_secondary_c.status_code == 201

    third_list = client.get('/api/budgets/', query_string={'category': category_name}, headers=auth_headers)
    third_items = third_list.get_json()['result']
    synced_primary = next(item for item in third_items if int(item['id']) == int(primary_budget['id']))
    assert synced_primary['amount'] == 200.0

    delete_primary = client.delete(f"/api/budgets/{primary_budget['id']}", headers=auth_headers)
    assert delete_primary.status_code == 200
    assert delete_primary.get_json()['success'] is True

    final_list = client.get('/api/budgets/', query_string={'category': category_name}, headers=auth_headers)
    final_items = final_list.get_json()['result']
    assert final_items == []
