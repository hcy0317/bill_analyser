"""数据管理与系统版本 REST 收口回归测试。"""

import asyncio
import time
from datetime import datetime

import pytest


async def _get_user_id(db, username: str) -> int:
    user = await db.get_user_by_username(username)
    assert user is not None
    return int(user['id'])


async def _seed_user_records(db, user_id: int, suffix: str) -> dict:
    """为测试用户写入一组账户/分类/账单数据。"""
    now = datetime.now().isoformat()

    category_id = await db.create_category({
        'type': 3,
        'main_category': f'测试主分类{suffix}',
        'sub_category': f'测试子分类{suffix}',
        'priority': 0,
        'keywords': '',
        'description': '测试导出分类',
        'icon': 'mdi-tag',
        'color': '2196f3',
        'hidden': False
    }, user_id=user_id)
    assert category_id is not None

    account_id = await db.create_account({
        'name': f'测试账户{suffix}',
        'type': 1,
        'category': 'asset',
        'currency': 'CNY',
        'icon': 'mdi-wallet',
        'color': '4caf50',
        'balance': 0.0,
        'initial_balance': 0.0,
        'hidden': False,
        'display_order': 0,
        'comment': '测试账户',
        'aliases': f'测试账户别名{suffix}',
    }, user_id=user_id)

    bill_id = await db.create_bill({
        'date': '2025-01-02 12:34:56',
        'type': '支出',
        'amount': -12.34,
        'main_category': f'测试主分类{suffix}',
        'sub_category': f'测试子分类{suffix}',
        'source_account_id': account_id,
        'counterparty': f'测试商户{suffix}',
        'payment_method': f'测试支付方式{suffix}',
        'description': f'导出测试账单{suffix}',
        'created_at': now,
        'updated_at': now
    }, user_id=user_id)
    assert bill_id is not None

    return {
        'category_id': category_id,
        'account_id': account_id,
        'bill_id': bill_id,
        'suffix': suffix
    }


def test_system_version_rest_endpoint(client):
    """系统版本应走新的 REST 主链。"""
    response = client.get('/api/system/version')
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    assert 'version' in result
    assert 'commitHash' in result
    assert 'buildTime' in result


@pytest.mark.parametrize('legacy_path', [
    '/api/v1/systems/version.json',
    '/api/v1/data/export.csv',
    '/api/v1/data/export.tsv',
    '/api/v1/data/clear/all.json',
    '/api/v1/data/clear/transactions.json',
])
def test_data_management_legacy_routes_removed(client, auth_headers, legacy_path):
    """旧数据管理与系统版本路径应已移除。"""
    if legacy_path.endswith('.csv') or legacy_path.endswith('.tsv') or legacy_path.endswith('version.json'):
        response = client.get(legacy_path, headers=auth_headers)
    else:
        response = client.post(legacy_path, json={'password': 'anything'}, headers=auth_headers)
    assert response.status_code == 404


def test_export_user_data_csv_and_tsv(client, auth_context, auth_headers, db):
    """数据导出应支持 CSV/TSV REST 路径。"""
    user_id = asyncio.run(_get_user_id(db, auth_context['username']))
    suffix = str(int(time.time() * 1000))
    asyncio.run(_seed_user_records(db, user_id, suffix))

    csv_response = client.get('/api/data/export.csv', headers=auth_headers)
    assert csv_response.status_code == 200, csv_response.get_data(as_text=True)
    assert csv_response.content_type.startswith('text/csv')
    csv_text = csv_response.get_data(as_text=True)
    assert f'导出测试账单{suffix}' in csv_text
    assert 'source_account' in csv_text

    tsv_response = client.get('/api/data/export.tsv', headers=auth_headers)
    assert tsv_response.status_code == 200, tsv_response.get_data(as_text=True)
    assert tsv_response.content_type.startswith('text/tab-separated-values')
    tsv_text = tsv_response.get_data(as_text=True)
    assert f'导出测试账单{suffix}' in tsv_text
    assert '\tsource_account\t' in tsv_text or tsv_text.startswith('\ufeffid\tdate')


def test_clear_user_transactions_rest_endpoint(client, auth_context, auth_headers, db, operation_password):
    """清空交易应仅删除账单并保留账户/分类。"""
    user_id = asyncio.run(_get_user_id(db, auth_context['username']))
    suffix = f'tx_{int(time.time() * 1000)}'
    seed = asyncio.run(_seed_user_records(db, user_id, suffix))

    response = client.post('/api/data/clear/transactions', json={
        'password': operation_password
    }, headers=auth_headers)
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True
    assert data.get('deletedCount', 0) >= 1

    remaining_bills = asyncio.run(db.get_bills(user_id=user_id))
    assert remaining_bills == []

    remaining_accounts = asyncio.run(db.get_all_accounts(user_id=user_id))
    remaining_categories = asyncio.run(db.get_all_categories(user_id=user_id))
    assert any(int(account['id']) == int(seed['account_id']) for account in remaining_accounts)
    assert any(int(category['id']) == int(seed['category_id']) for category in remaining_categories)


def test_clear_all_user_data_rest_endpoint(client, auth_context, auth_headers, db, operation_password):
    """清空全部数据应删除账户、分类与账单等业务数据。"""
    user_id = asyncio.run(_get_user_id(db, auth_context['username']))
    suffix = f'all_{int(time.time() * 1000)}'
    seed = asyncio.run(_seed_user_records(db, user_id, suffix))

    response = client.post('/api/data/clear/all', json={
        'password': operation_password
    }, headers=auth_headers)
    assert response.status_code == 200, response.get_data(as_text=True)

    data = response.get_json() or {}
    assert data.get('success') is True
    assert data.get('result') is True
    counts = data.get('counts') or {}
    assert counts.get('accounts', 0) >= 1
    assert counts.get('categories', 0) >= 1

    remaining_accounts = asyncio.run(db.get_all_accounts(user_id=user_id))
    remaining_categories = asyncio.run(db.get_all_categories(user_id=user_id))
    remaining_bills = asyncio.run(db.get_bills(user_id=user_id))

    assert remaining_accounts == []
    assert remaining_bills == []
    assert all(int(category.get('id', 0)) != int(seed['category_id']) for category in remaining_categories)
    assert all(suffix not in (category.get('main_category') or '') for category in remaining_categories)
