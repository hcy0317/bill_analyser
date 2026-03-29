from __future__ import annotations

from openpyxl import Workbook

from bill_analyser.parsers.wechat import WeChatParser
from tests.parser_test_support import build_wechat_xlsx, sample_path


def test_wechat_parser_handles_csv_sample() -> None:
    parser = WeChatParser()
    bills = parser.parse(str(sample_path('wechat_statement_sample.csv')))

    assert parser.can_parse(str(sample_path('wechat_statement_sample.csv'))) is True
    assert len(bills) == 7
    assert bills[0]['source_account_id'] == 'wechat'
    assert any(bill['amount'] < 0 for bill in bills)
    assert any(bill['amount'] > 0 for bill in bills)


def test_wechat_parser_handles_generated_xlsx(tmp_path) -> None:
    parser = WeChatParser()
    xlsx_path = build_wechat_xlsx(tmp_path / 'wechat_statement_sample.xlsx')

    bills = parser.parse(str(xlsx_path))

    assert parser.can_parse(str(xlsx_path)) is True
    assert len(bills) == 2
    assert bills[0]['description'] == '鲜肉包 | 测试早餐店 | 零钱 | 商户消费'
    assert bills[0]['amount'] == -5.0
    assert bills[1]['amount'] == 30.0


def test_wechat_parser_rejects_unknown_csv() -> None:
    parser = WeChatParser()

    assert parser.can_parse(str(sample_path('generic_statement_sample.csv'))) is False


def test_wechat_parser_handles_missing_headers_and_bad_workbooks(tmp_path) -> None:
    parser = WeChatParser()
    bad_csv = tmp_path / 'bad_wechat.csv'
    bad_csv.write_text('备注,金额\n无效,1\n', encoding='utf-8')

    workbook = Workbook()
    sheet = workbook.active
    assert sheet is not None
    sheet.append(['不是微信账单'])
    bad_xlsx = tmp_path / 'bad_wechat.xlsx'
    workbook.save(bad_xlsx)
    workbook.close()

    assert parser.parse(str(bad_csv)) == []
    assert parser.can_parse(str(bad_xlsx)) is False
    assert parser.parse(str(bad_xlsx)) == []
