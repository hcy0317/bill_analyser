from __future__ import annotations

import tempfile
from pathlib import Path

from bill_analyser.parsers.cmbc import CMBCParser
from openpyxl import Workbook

from tests.parser_test_support import build_cmbc_html_xls, build_cmbc_xlsx, sample_path


def test_cmbc_parser_handles_csv_sample() -> None:
    parser = CMBCParser()
    bills = parser.parse(str(sample_path('cmbc_statement_sample.csv')))

    assert parser.can_parse(str(sample_path('cmbc_statement_sample.csv'))) is True
    assert len(bills) == 2
    assert bills[0]['source_account_id'] == 'cmbc'
    assert bills[0]['amount'] < 0
    assert bills[1]['amount'] > 0


def test_cmbc_parser_handles_html_xls_sample(tmp_path) -> None:
    parser = CMBCParser()
    html_path = build_cmbc_html_xls(tmp_path / 'cmbc_statement_sample.xls')

    bills = parser.parse(str(html_path))

    assert parser._is_html_file(str(html_path)) is True  # pylint: disable=protected-access
    assert parser.can_parse(str(html_path)) is True
    assert len(bills) == 2
    assert bills[0]['description'] == '工作餐 | 测试午餐店 | 手机银行 | 6226000000000001'
    assert bills[1]['amount'] == 88.0


def test_cmbc_parser_rejects_generic_sample() -> None:
    parser = CMBCParser()

    assert parser.can_parse(str(sample_path('generic_statement_sample.csv'))) is False


def test_cmbc_parser_handles_generated_xlsx_and_content_detection() -> None:
    parser = CMBCParser()
    with tempfile.TemporaryDirectory(prefix='parser-neutral-') as temp_dir:
        xlsx_path = build_cmbc_xlsx(Path(temp_dir) / 'statement.xlsx')

        bills = parser.parse(str(xlsx_path))

        assert parser.can_parse(str(xlsx_path)) is True
        assert len(bills) == 2
        assert bills[0]['amount'] == -32.8
        assert bills[1]['amount'] == 88.0


def test_cmbc_parser_handles_bad_excel_input(tmp_path, monkeypatch) -> None:
    parser = CMBCParser()
    workbook = Workbook()
    sheet = workbook.active
    assert sheet is not None
    sheet.append(['无效表头'])
    bad_xlsx = tmp_path / 'bad_cmbc.xlsx'
    workbook.save(bad_xlsx)
    workbook.close()

    monkeypatch.setattr('bill_analyser.parsers.cmbc.pd.read_excel', lambda *_args, **_kwargs: (_ for _ in ()).throw(ValueError('bad excel')))

    assert parser.parse(str(bad_xlsx)) == []
    assert parser._is_html_file(str(bad_xlsx)) is False  # pylint: disable=protected-access


def test_cmbc_parser_detects_utf8sig_neutral_csv_and_skips_blank_dates(tmp_path) -> None:
    parser = CMBCParser()
    neutral_csv = tmp_path / 'cmbc_statement_utf8sig.csv'
    neutral_csv.write_text(
        '中国民生银行股份有限公司个人账户对账单,,,,,\n'
        '交易时间,交易金额,收/支,交易对手,交易说明,摘要\n'
        ',32.80,支出,测试午餐店,工作餐,门店消费\n'
        '20170102 09:30:00,88.00,收入,测试报销账户,报销到账,公司报销\n',
        encoding='utf-8-sig',
    )

    bills = parser.parse(str(neutral_csv))

    assert parser.can_parse(str(neutral_csv)) is True
    assert len(bills) == 1
    assert bills[0]['amount'] == 88.0
