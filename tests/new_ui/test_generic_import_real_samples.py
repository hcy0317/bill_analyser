"""真实账单样本的通用导入全链路回归测试。"""

import json
import re
from io import BytesIO
from pathlib import Path

import pytest

from tests.real_sample_support import discover_real_sample_cases

REAL_SAMPLE_CASES = discover_real_sample_cases()


def _preview_sample_file(client, auth_headers, sample_path: Path):
    with sample_path.open('rb') as file_obj:
        return client.post(
            '/api/bills/import/preview',
            data={'file': (BytesIO(file_obj.read()), sample_path.name)},
            headers=auth_headers,
            content_type='multipart/form-data',
        )


def _parse_sample_file(client, auth_headers, sample_path: Path, suggestion: dict[str, object]):
    with sample_path.open('rb') as file_obj:
        return client.post(
            '/api/bills/parse_import',
            data={
                'fileType': sample_path.suffix.lower().lstrip('.'),
                'columnMapping': json.dumps(suggestion.get('columnMapping') or {}, ensure_ascii=False),
                'transactionTypeMapping': json.dumps(
                    suggestion.get('transactionTypeMapping') or {}, ensure_ascii=False
                ),
                'hasHeaderLine': 'true',
                'file': (BytesIO(file_obj.read()), sample_path.name),
            },
            headers=auth_headers,
            content_type='multipart/form-data',
        )


def _assert_preview_result(case: dict[str, object], preview_result: dict[str, object]):
    assert preview_result.get('detectedHeaderRow') == case['expected_header_row']
    assert (preview_result.get('headers') or [])[: len(case['expected_headers'])] == case['expected_headers']
    assert preview_result.get('previewRows'), 'previewRows 不应为空'


def _assert_mapping_suggestion(case: dict[str, object], suggestion: dict[str, object]):
    column_mapping = suggestion.get('columnMapping') or {}

    assert column_mapping, f"未能为 {case['path'].name} 建议列映射"
    assert '1' in column_mapping, f"{case['path'].name} 缺少交易时间映射"
    assert len(column_mapping) >= case['min_mapping_count'], (
        f"{case['path'].name} 建议映射过少: {column_mapping}"
    )


def _assert_parsed_result(parse_result: dict[str, object]):
    assert parse_result.get('totalCount', 0) > 0, parse_result

    items = parse_result.get('items') or []
    assert items, parse_result
    first_item = items[0]

    time_text = str(first_item.get('timeText') or '').strip()
    assert re.fullmatch(r'\d{4}[-/]\d{1,2}[-/]\d{1,2}(?:\s+\d{1,2}:\d{2}(?::\d{2})?)?', time_text), first_item

    transaction_type = first_item.get('type')
    assert isinstance(transaction_type, int) and transaction_type in {2, 3, 4, 5}, first_item

    source_amount = first_item.get('sourceAmount')
    assert isinstance(source_amount, int) and source_amount > 0, first_item

    assert any(
        str(first_item.get(field) or '').strip() for field in ('comment', 'counterparty', 'paymentMethod')
    ), first_item


@pytest.mark.parametrize('case', REAL_SAMPLE_CASES, ids=[case['id'] for case in REAL_SAMPLE_CASES])
def test_preview_real_samples_detect_true_header_row(client, auth_headers, case):
    """每个真实账单样本都应识别出真正表头。"""
    sample_path = case['path']
    assert sample_path.exists(), f'样本不存在: {sample_path}'

    response = _preview_sample_file(client, auth_headers, sample_path)

    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data.get('success') is True, data
    result = data.get('result') or {}

    _assert_preview_result(case, result)


@pytest.mark.parametrize('case', REAL_SAMPLE_CASES, ids=[case['id'] for case in REAL_SAMPLE_CASES])
def test_real_samples_can_complete_generic_import_route(client, auth_headers, case):
    """每个真实账单样本都应打通 预览→建议映射→解析为通用格式 的链路。"""
    sample_path = case['path']
    assert sample_path.exists(), f'样本不存在: {sample_path}'

    preview_response = _preview_sample_file(client, auth_headers, sample_path)
    assert preview_response.status_code == 200, preview_response.get_data(as_text=True)
    preview_data = preview_response.get_json() or {}
    assert preview_data.get('success') is True, preview_data
    preview_result = preview_data.get('result') or {}
    _assert_preview_result(case, preview_result)

    suggest_response = client.post(
        '/api/bills/import/configs/suggest',
        json={
            'fileFormat': sample_path.suffix.lower().lstrip('.'),
            'headers': preview_result.get('headers') or [],
            'sampleRows': preview_result.get('previewRows') or [],
        },
        headers=auth_headers,
    )
    assert suggest_response.status_code == 200, suggest_response.get_data(as_text=True)
    suggest_data = suggest_response.get_json() or {}
    assert suggest_data.get('success') is True, suggest_data
    suggestion = suggest_data.get('result') or {}
    _assert_mapping_suggestion(case, suggestion)

    parse_response = _parse_sample_file(client, auth_headers, sample_path, suggestion)
    assert parse_response.status_code == 200, parse_response.get_data(as_text=True)
    parse_data = parse_response.get_json() or {}
    assert parse_data.get('success') is True, parse_data

    parse_result = parse_data.get('result') or {}
    _assert_parsed_result(parse_result)