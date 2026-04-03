from __future__ import annotations

import tempfile
from pathlib import Path

import pandas as pd
import pytest
from openpyxl import Workbook

from bill_analyser.parsers.ccb import CCBParser
from tests.parser_test_support import build_ccb_xlsx, sample_path


def test_ccb_parser_handles_generated_xlsx(tmp_path) -> None:
    parser = CCBParser()
    xlsx_path = build_ccb_xlsx(tmp_path / "ccb_statement_sample.xlsx")

    bills = parser.parse(str(xlsx_path))

    assert parser.can_parse(str(xlsx_path)) is True
    assert len(bills) == 2
    assert bills[0]["source_account_id"] == "ccb"
    assert bills[0]["amount"] == -18.6
    assert bills[1]["amount"] == 200.0


def test_ccb_helpers_and_unknown_file() -> None:
    parser = CCBParser()

    assert parser._build_trade_time({"交易日期": "20260112", "交易时间": "081500"}) == "2026-01-12 08:15:00"  # pylint: disable=protected-access
    assert parser._build_trade_time({"记账日": "20260112"}) == "2026-01-12"  # pylint: disable=protected-access
    assert parser._build_trade_time({}) == ""  # pylint: disable=protected-access
    assert parser.can_parse(str(sample_path("generic_statement_sample.csv"))) is False


def test_ccb_parser_handles_content_detection_and_missing_headers(tmp_path, monkeypatch) -> None:
    parser = CCBParser()
    with tempfile.TemporaryDirectory(prefix="parser-neutral-") as temp_dir:
        neutral_xlsx = Path(temp_dir) / "statement.xlsx"
        neutral_xlsx.write_bytes(b"placeholder")
    workbook = Workbook()
    sheet = workbook.active
    assert sheet is not None
    sheet.append(["无效表头"])
    bad_xlsx = tmp_path / "bad_ccb.xlsx"
    workbook.save(bad_xlsx)
    workbook.close()

    monkeypatch.setattr("bill_analyser.parsers.ccb.pd.read_excel", lambda *_args, **_kwargs: pd.DataFrame([
        ["中国建设银行", "开户机构：", "记账日", "交易日期", "支出", "收入", "账户余额"]
    ]))

    assert parser.can_parse(str(neutral_xlsx)) is True

    assert parser.parse(str(bad_xlsx)) == []


def test_ccb_parser_detects_content_without_filename_and_skips_zero_amount_rows(tmp_path, monkeypatch) -> None:
    parser = CCBParser()
    dataframe = pd.DataFrame(
        [
            ["中国建设银行", "", "", "", "", "", "", ""],
            ["开户机构：测试支行", "", "", "", "", "", "", ""],
            ["", "", "", "", "", "", "", ""],
            ["记账日", "交易日期", "交易时间", "摘要", "支出", "收入", "账户余额", "对方户名"],
            ["20260112", "20260112", "081500", "无效零金额", "", "", "1000.0", ""],
            ["20260112", "20260112", "182000", "工资入账", "", "200.0", "1200.0", ""],
        ]
    )
    statement_path = tmp_path / "statement.xlsx"
    statement_path.write_bytes(b"placeholder")
    monkeypatch.setattr("bill_analyser.parsers.ccb.pd.read_excel", lambda *_args, **_kwargs: dataframe)

    assert parser.can_parse(str(statement_path)) is True

    bills = parser.parse(str(statement_path))

    assert len(bills) == 1
    assert bills[0]["counterparty"] == "工资入账"
    assert bills[0]["amount"] == 200.0


@pytest.mark.parametrize("other_bank_marker", ["储种", "对方开户行", "支出金额"])
def test_ccb_parser_rejects_other_bank_indicators_even_with_ccb_signals(
    tmp_path,
    monkeypatch,
    other_bank_marker: str,
) -> None:
    parser = CCBParser()
    monkeypatch.setattr(
        "bill_analyser.parsers.ccb.pd.read_excel",
        lambda *_args, **_kwargs: pd.DataFrame(
            [["中国建设银行", "记账日", "交易日期", "支出", "收入", other_bank_marker]]
        ),
    )

    assert parser.can_parse("neutral_statement.xlsx") is False


def test_ccb_parser_accepts_column_patterns_without_strong_markers(tmp_path, monkeypatch) -> None:
    parser = CCBParser()
    monkeypatch.setattr(
        "bill_analyser.parsers.ccb.pd.read_excel",
        lambda *_args, **_kwargs: pd.DataFrame([["记账日", "交易日期", "支出", "收入", "账户余额"]]),
    )

    assert parser.can_parse("neutral_statement.xlsx") is True


def test_ccb_parser_short_circuits_on_filename_match(monkeypatch) -> None:
    parser = CCBParser()

    def _raise_if_called(*_args, **_kwargs) -> pd.DataFrame:
        raise AssertionError("pd.read_excel should not be called for filename short-circuit")

    monkeypatch.setattr("bill_analyser.parsers.ccb.pd.read_excel", _raise_if_called)

    assert parser.can_parse("fake_ccb_statement.xlsx") is True


def test_ccb_parser_returns_false_when_probe_raises(tmp_path, monkeypatch) -> None:
    parser = CCBParser()

    def _raise_probe_error(*_args, **_kwargs) -> pd.DataFrame:
        raise RuntimeError("broken excel probe")

    monkeypatch.setattr("bill_analyser.parsers.ccb.pd.read_excel", _raise_probe_error)

    assert parser.can_parse("neutral_statement.xlsx") is False


def test_ccb_parser_skips_bad_rows_and_uses_date_and_counterparty_fallbacks(
    tmp_path,
    monkeypatch,
) -> None:
    parser = CCBParser()
    statement_path = tmp_path / "neutral_statement.xlsx"
    statement_path.write_bytes(b"placeholder")
    monkeypatch.setattr(
        "bill_analyser.parsers.ccb.pd.read_excel",
        lambda *_args, **_kwargs: pd.DataFrame(
            [
                ["中国建设银行", "", "", "", "", "", "", ""],
                ["开户机构：测试支行", "", "", "", "", "", "", ""],
                ["记账日", "交易日期", "交易时间", "摘要", "支出", "收入", "账户余额", "对方户名"],
                ["20260112", "", "", "摘要不会覆盖对方户名", "18.6", "", "1000.0", "优先对方户名"],
                ["", "", "", "无日期行", "10.0", "", "990.0", "空日期"],
                ["20260112", "20260112", "082000", "坏金额", "", "abc", "990.0", "坏金额行"],
                ["20260113", "20260113", "091500", "工资入账", "", "200.0", "1200.0", ""],
            ]
        ),
    )

    bills = parser.parse(str(statement_path))

    assert len(bills) == 2
    assert bills[0]["date"] == "2026-01-12 00:00:00"
    assert bills[0]["counterparty"] == "优先对方户名"
    assert bills[0]["amount"] == -18.6
    assert bills[1]["counterparty"] == "工资入账"
    assert bills[1]["amount"] == 200.0
