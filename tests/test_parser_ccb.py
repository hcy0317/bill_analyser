from __future__ import annotations

import tempfile
from pathlib import Path

from openpyxl import Workbook
import pandas as pd

from bill_analyser.parsers.ccb import CCBParser
from tests.parser_test_support import build_ccb_xlsx, sample_path


def test_ccb_parser_handles_generated_xlsx(tmp_path) -> None:
    parser = CCBParser()
    xlsx_path = build_ccb_xlsx(tmp_path / 'ccb_statement_sample.xlsx')

    bills = parser.parse(str(xlsx_path))

    assert parser.can_parse(str(xlsx_path)) is True
    assert len(bills) == 2
    assert bills[0]['source_account_id'] == 'ccb'
    assert bills[0]['amount'] == -18.6
    assert bills[1]['amount'] == 200.0


def test_ccb_helpers_and_unknown_file() -> None:
    parser = CCBParser()

    assert parser._build_trade_time({'交易日期': '20260112', '交易时间': '081500'}) == '2026-01-12 08:15:00'  # pylint: disable=protected-access
    assert parser._build_trade_time({'记账日': '20260112'}) == '2026-01-12'  # pylint: disable=protected-access
    assert parser._build_trade_time({}) == ''  # pylint: disable=protected-access
    assert parser.can_parse(str(sample_path('generic_statement_sample.csv'))) is False


def test_ccb_parser_handles_content_detection_and_missing_headers(tmp_path, monkeypatch) -> None:
    parser = CCBParser()
    with tempfile.TemporaryDirectory(prefix='parser-neutral-') as temp_dir:
        neutral_xlsx = Path(temp_dir) / 'statement.xlsx'
        neutral_xlsx.write_bytes(b'placeholder')
    workbook = Workbook()
    sheet = workbook.active
    assert sheet is not None
    sheet.append(['无效表头'])
    bad_xlsx = tmp_path / 'bad_ccb.xlsx'
    workbook.save(bad_xlsx)
    workbook.close()

    monkeypatch.setattr('bill_analyser.parsers.ccb.pd.read_excel', lambda *_args, **_kwargs: pd.DataFrame([
        ['中国建设银行', '开户机构：', '记账日', '交易日期', '支出', '收入', '账户余额']
    ]))

    assert parser.can_parse(str(neutral_xlsx)) is True

    assert parser.parse(str(bad_xlsx)) == []


def test_ccb_parser_detects_content_without_filename_and_skips_zero_amount_rows(tmp_path, monkeypatch) -> None:
    parser = CCBParser()
    dataframe = pd.DataFrame(
        [
            ['中国建设银行', '', '', '', '', '', '', ''],
            ['开户机构：测试支行', '', '', '', '', '', '', ''],
            ['', '', '', '', '', '', '', ''],
            ['记账日', '交易日期', '交易时间', '摘要', '支出', '收入', '账户余额', '对方户名'],
            ['20260112', '20260112', '081500', '无效零金额', '', '', '1000.0', ''],
            ['20260112', '20260112', '182000', '工资入账', '', '200.0', '1200.0', ''],
        ]
    )
    statement_path = tmp_path / 'statement.xlsx'
    statement_path.write_bytes(b'placeholder')
    monkeypatch.setattr('bill_analyser.parsers.ccb.pd.read_excel', lambda *_args, **_kwargs: dataframe)

    assert parser.can_parse(str(statement_path)) is True

    bills = parser.parse(str(statement_path))

    assert len(bills) == 1
    assert bills[0]['counterparty'] == '工资入账'
    assert bills[0]['amount'] == 200.0
