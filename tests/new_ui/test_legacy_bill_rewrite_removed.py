"""账单域历史 rewrite 已移除的回归测试。"""

import pytest


@pytest.mark.parametrize(('method', 'path', 'json_body'), [
    ('post', '/api/v1/transactions/add.json', {'description': 'legacy add'}),
    ('post', '/api/v1/transactions/modify.json', {'id': 1, 'description': 'legacy modify'}),
    ('post', '/api/v1/transactions/delete.json', {'id': 1}),
    ('post', '/api/v1/transactions/import.json', {'transactions': []}),
    ('get', '/api/v1/transactions/reconciliation_statements.json?account_id=1&start_time=1&end_time=2', None),
    ('get', '/api/v1/transaction/categories/list.json', None),
    ('post', '/api/v1/transaction/categories/add.json', {'name': 'legacy category'}),
    ('post', '/api/v1/transaction/categories/add_batch.json', {'items': []}),
])
def test_legacy_bill_and_category_rewrite_routes_removed(client, auth_headers, method, path, json_body):
    """已停止使用的账单/分类 rewrite 路径应返回 404。"""
    response = getattr(client, method)(path, json=json_body, headers=auth_headers)
    assert response.status_code == 404, response.get_data(as_text=True)


def test_legacy_parse_import_rewrite_removed(client, auth_headers):
    """旧 parse_import rewrite 路径应返回 404。"""
    response = client.post('/api/v1/transactions/parse_import.json', headers=auth_headers)
    assert response.status_code == 404, response.get_data(as_text=True)