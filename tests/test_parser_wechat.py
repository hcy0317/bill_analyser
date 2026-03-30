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


def test_wechat_parser_detects_signature_headers_and_unsupported_extension(tmp_path) -> None:
    parser = WeChatParser()
    signature_csv = tmp_path / 'signature_wechat.csv'
    signature_csv.write_text(
        '交易时间,交易类型,交易对方,商品,收/支,金额(元)\n'
        '2026-01-15 08:01:00,商户消费,测试早餐店,鲜肉包,支出,¥5.00\n',
        encoding='utf-8-sig',
    )
    partial_csv = tmp_path / 'partial_wechat.csv'
    partial_csv.write_text('交易时间,金额(元)\n2026-01-15 08:01:00,¥5.00\n', encoding='utf-8')

    unsupported_file = tmp_path / 'wechat_statement.txt'
    unsupported_file.write_text('placeholder', encoding='utf-8')
    parser.supported_extensions.append('.txt')

    assert parser.can_parse(str(signature_csv)) is True
    assert parser.can_parse(str(partial_csv)) is False
    assert parser.parse(str(unsupported_file)) == []


def test_wechat_parser_handles_csv_row_errors_and_sheetless_workbooks(tmp_path, monkeypatch) -> None:
    parser = WeChatParser()
    valid_csv = tmp_path / 'valid_wechat.csv'
    valid_csv.write_text(
        '微信支付账单\n'
        '交易时间,交易类型,交易对方,商品,收/支,金额(元),支付方式,当前状态,交易单号,商户单号,备注\n'
        '2026-01-15 08:01:00,商户消费,测试早餐店,鲜肉包,支出,¥5.00,零钱,支付成功,4200,WX-1,/\n',
        encoding='utf-8',
    )

    def explode_row(_row):
        raise RuntimeError('bad row')

    monkeypatch.setattr(parser, '_extract_bill_from_csv_row', explode_row)

    class SheetlessWorkbook:
        active = None

        def close(self) -> None:
            return None

    monkeypatch.setattr('bill_analyser.parsers.wechat.openpyxl.load_workbook', lambda *_args, **_kwargs: SheetlessWorkbook())

    assert parser.parse(str(valid_csv)) == []
    assert parser._can_parse_xlsx(str(tmp_path / 'sheetless.xlsx')) is False  # pylint: disable=protected-access
    assert parser._parse_xlsx(str(tmp_path / 'sheetless.xlsx')) == []  # pylint: disable=protected-access
    assert parser._extract_bill_from_xlsx_row(None, {'金额(元)': 0}) is None  # type: ignore[arg-type]  # pylint: disable=protected-access
