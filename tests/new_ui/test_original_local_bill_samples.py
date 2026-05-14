"""使用本地 bills/ 原始账单的回归测试（不会把账单文件纳入 git）。"""

from __future__ import annotations

from io import BytesIO
from pathlib import Path

import pytest

from bill_analyser.parsers.factory import ParserFactory
from tests.real_sample_support import discover_local_original_bill_cases


LOCAL_ORIGINAL_BILL_CASES = discover_local_original_bill_cases()


def _auto_parse_original_file(client, auth_headers, sample_path: Path):
    with sample_path.open('rb') as file_obj:
        return client.post(
            '/api/bills/parse_import',
            data={
                'fileType': 'auto',
                'file': (BytesIO(file_obj.read()), sample_path.name),
            },
            headers=auth_headers,
            content_type='multipart/form-data',
        )


@pytest.mark.skipif(not LOCAL_ORIGINAL_BILL_CASES, reason='未发现本地 bills/ 原始账单样本')
@pytest.mark.parametrize('case', LOCAL_ORIGINAL_BILL_CASES, ids=[case['id'] for case in LOCAL_ORIGINAL_BILL_CASES])
def test_local_original_bill_files_detect_and_parse_with_dedicated_parser(case):
    """每类本地原始账单都应能被真实 parser 检测并解析出账单。"""
    sample_path = case['path']
    factory = ParserFactory()

    detected = factory.detect_parser(str(sample_path))
    assert detected is not None, sample_path.name
    assert detected.get('id') == case['parser_id'], sample_path.name

    bills = factory.parse(str(sample_path), parser_type=case['parser_id'])
    assert bills, sample_path.name
    assert any(str(bill.get('date') or '').strip() for bill in bills), bills[0]


@pytest.mark.skipif(not LOCAL_ORIGINAL_BILL_CASES, reason='未发现本地 bills/ 原始账单样本')
@pytest.mark.skip(
    reason="Flask parse_import route was deleted; dedicated parser coverage remains below Rust/Python parser contracts."
)
@pytest.mark.parametrize('case', LOCAL_ORIGINAL_BILL_CASES, ids=[case['id'] for case in LOCAL_ORIGINAL_BILL_CASES])
def test_local_original_bill_files_complete_auto_parse_import_route(client, auth_headers, case):
    """每类本地原始账单都应能走通 parse_import 自动识别链路。"""
    response = _auto_parse_original_file(client, auth_headers, case['path'])

    assert response.status_code == 200, response.get_data(as_text=True)
    data = response.get_json() or {}
    assert data.get('success') is True, data

    result = data.get('result') or {}
    assert result.get('parserType') == case['parser_id'], result
    assert result.get('detectedParserType') == case['parser_id'], result
    assert result.get('totalCount', 0) > 0, result

    items = result.get('items') or []
    assert items, result
    first_item = items[0]
    assert first_item.get('parserSource') == case['parser_id'], first_item
    assert int(first_item.get('sourceAmount') or 0) > 0, first_item
