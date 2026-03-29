from __future__ import annotations

import tempfile
from pathlib import Path

from bill_analyser.parsers.icbc import ICBCParser
from tests.parser_test_support import build_icbc_html_xls, build_icbc_xlsx, sample_path


def test_icbc_parser_handles_csv_sample() -> None:
    parser = ICBCParser()
    bills = parser.parse(str(sample_path('icbc_statement_sample.csv')))

    assert parser.can_parse(str(sample_path('icbc_statement_sample.csv'))) is True
    assert len(bills) == 2
    assert bills[0]['source_account_id'] == 'icbc'
    assert bills[0]['amount'] < 0
    assert bills[1]['amount'] > 0


def test_icbc_parser_handles_generated_xlsx(tmp_path) -> None:
    parser = ICBCParser()
    xlsx_path = build_icbc_xlsx(tmp_path / 'icbc_statement_sample.xlsx')

    bills = parser.parse(str(xlsx_path))

    assert parser.can_parse(str(xlsx_path)) is True
    assert len(bills) == 2
    assert bills[0]['description'] == '早餐消费 | 测试早餐店 | 6222000000000001'
    assert bills[1]['amount'] == 1200.0


def test_icbc_internal_helpers_cover_content_detection(tmp_path) -> None:
    parser = ICBCParser()
    xlsx_path = build_icbc_xlsx(tmp_path / 'icbc_statement_sample.xlsx')

    is_html, error = parser._detect_file_format(str(xlsx_path))  # pylint: disable=protected-access
    extracted = parser._extract_bill_from_excel_row(  # pylint: disable=protected-access
        ('6222000000000000', '2026-01-10 08:00:00', -15.5, '测试早餐店', '6222', '早餐消费', '流水1', '早餐'),
        {
            '交易日期': 1,
            '收入/支出金额': 2,
            '对方户名': 3,
            '对方账号': 4,
            '摘要': 5,
            '交易流水号': 6,
            '交易附言': 7,
        },
    )

    assert is_html is False
    assert error is None
    assert parser._is_icbc_content('收入/支出金额 对方账号名称 交易附言') is True  # pylint: disable=protected-access
    assert parser._is_icbc_content('民生银行 支出金额 存入金额') is False  # pylint: disable=protected-access
    assert extracted is not None
    assert extracted['type'] == '支出'


def test_icbc_parser_handles_html_and_content_based_detection() -> None:
    parser = ICBCParser()
    with tempfile.TemporaryDirectory(prefix='parser-neutral-') as temp_dir:
        neutral_root = Path(temp_dir)
        neutral_xlsx = build_icbc_xlsx(neutral_root / 'statement.xlsx')
        html_xls = build_icbc_html_xls(neutral_root / 'statement.xls')

        html_bills = parser.parse(str(html_xls))

        assert parser.can_parse(str(neutral_xlsx)) is True
        assert parser.can_parse(str(html_xls)) is True
        assert len(html_bills) == 2
        assert html_bills[0]['amount'] == -15.5


def test_icbc_parser_handles_bad_inputs_and_reader_fallbacks(tmp_path, monkeypatch) -> None:
    parser = ICBCParser()
    bad_csv = tmp_path / 'bad_icbc.csv'
    bad_csv.write_text('无效内容\n', encoding='gbk')

    is_html, error = parser._detect_file_format(str(tmp_path / 'missing.xls'))  # pylint: disable=protected-access
    monkeypatch.setattr('bill_analyser.parsers.icbc.pd.read_excel', lambda *_args, **_kwargs: (_ for _ in ()).throw(ValueError('bad excel')))

    assert parser._read_excel_content(str(tmp_path / 'missing.xlsx')) == ''  # pylint: disable=protected-access
    assert parser.parse(str(bad_csv)) == []
    assert is_html is False
    assert error is not None
