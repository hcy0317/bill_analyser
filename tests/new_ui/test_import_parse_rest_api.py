"""导入解析 REST 收口回归测试。"""

from pathlib import Path

import pytest


@pytest.mark.skip(
    reason="Legacy Flask parse_import route was deleted; Rust parser/import contracts cover this path."
)
def test_parse_import_rest_endpoint(client, auth_headers):
    """导入解析应走新的 REST 主链。"""
    sample_file = Path(__file__).resolve().parents[1] / 'fixtures' / 'import_samples' / 'wechat_statement_sample.csv'
    assert sample_file.exists(), f'样例文件不存在: {sample_file}'

    with sample_file.open('rb') as file_obj:
        response = client.post(
            '/api/bills/parse_import',
            data={
                'fileType': 'auto',
                'fileEncoding': 'utf-8',
                'file': (file_obj, sample_file.name)
            },
            headers=auth_headers,
            content_type='multipart/form-data'
        )

    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data.get('success') is True
    result = data.get('result') or {}
    items = result.get('items') or []
    assert result.get('totalCount', 0) > 0
    assert len(items) > 0

    first_item = items[0]
    assert 'time' in first_item
    assert 'type' in first_item
    assert 'amount' in first_item
    assert 'description' in first_item
    assert 'counterparty' in first_item
    assert 'paymentMethod' in first_item


def test_import_legacy_process_route_removed(client, auth_headers):
    """旧导入进度轮询路径应已移除。"""
    response = client.get(
        '/api/v1/transactions/import/process.json?client_session_id=test-session',
        headers=auth_headers
    )
    assert response.status_code == 404, response.get_data(as_text=True)


def test_import_legacy_parse_dsv_route_removed(client, auth_headers):
    """旧 DSV 解析路径应已移除。"""
    response = client.post(
        '/api/v1/transactions/parse_dsv_file.json',
        headers=auth_headers,
        data={},
        content_type='multipart/form-data'
    )
    assert response.status_code == 404, response.get_data(as_text=True)
