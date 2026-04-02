"""Regression and edge-case tests for the CMBC parser."""

from __future__ import annotations

import tempfile
from pathlib import Path
from typing import Any

import pandas as pd
from openpyxl import Workbook

from bill_analyser.parsers.cmbc import CMBCParser
from tests.parser_test_support import build_cmbc_html_xls, build_cmbc_xlsx, sample_path

# pylint: disable=line-too-long,missing-function-docstring,too-few-public-methods,use-implicit-booleaness-not-comparison


def test_cmbc_parser_handles_csv_sample() -> None:
    parser = CMBCParser()
    bills = parser.parse(str(sample_path("cmbc_statement_sample.csv")))

    assert parser.can_parse(str(sample_path("cmbc_statement_sample.csv"))) is True
    assert len(bills) == 2
    assert bills[0]["source_account_id"] == "cmbc"
    assert bills[0]["amount"] < 0
    assert bills[1]["amount"] > 0


def test_cmbc_parser_handles_html_xls_sample(tmp_path) -> None:
    parser = CMBCParser()
    html_path = build_cmbc_html_xls(tmp_path / "cmbc_statement_sample.xls")

    bills = parser.parse(str(html_path))

    assert parser._is_html_file(str(html_path)) is True  # pylint: disable=protected-access
    assert parser.can_parse(str(html_path)) is True
    assert len(bills) == 2
    assert bills[0]["description"] == "工作餐 | 测试午餐店 | 手机银行 | 6226000000000001"
    assert bills[1]["amount"] == 88.0


def test_cmbc_parser_rejects_generic_sample() -> None:
    parser = CMBCParser()

    assert parser.can_parse(str(sample_path("generic_statement_sample.csv"))) is False


def test_cmbc_parser_handles_generated_xlsx_and_content_detection() -> None:
    parser = CMBCParser()
    with tempfile.TemporaryDirectory(prefix="parser-neutral-") as temp_dir:
        xlsx_path = build_cmbc_xlsx(Path(temp_dir) / "statement.xlsx")

        bills = parser.parse(str(xlsx_path))

        assert parser.can_parse(str(xlsx_path)) is True
        assert len(bills) == 2
        assert bills[0]["amount"] == -32.8
        assert bills[1]["amount"] == 88.0


def test_cmbc_parser_handles_bad_excel_input(tmp_path, monkeypatch) -> None:
    parser = CMBCParser()
    workbook = Workbook()
    sheet = workbook.active
    assert sheet is not None
    sheet.append(["无效表头"])
    bad_xlsx = tmp_path / "bad_cmbc.xlsx"
    workbook.save(bad_xlsx)
    workbook.close()

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_excel", lambda *_args, **_kwargs: (_ for _ in ()).throw(ValueError("bad excel")))

    assert not parser.parse(str(bad_xlsx))
    assert parser._is_html_file(str(bad_xlsx)) is False  # pylint: disable=protected-access


def test_cmbc_parser_detects_utf8sig_neutral_csv_and_skips_blank_dates(tmp_path) -> None:
    parser = CMBCParser()
    neutral_csv = tmp_path / "cmbc_statement_utf8sig.csv"
    neutral_csv.write_text(
        "中国民生银行股份有限公司个人账户对账单,,,,,\n"
        "交易时间,交易金额,收/支,交易对手,交易说明,摘要\n"
        ",32.80,支出,测试午餐店,工作餐,门店消费\n"
        "20170102 09:30:00,88.00,收入,测试报销账户,报销到账,公司报销\n",
        encoding="utf-8-sig",
    )

    bills = parser.parse(str(neutral_csv))

    assert parser.can_parse(str(neutral_csv)) is True
    assert len(bills) == 1
    assert bills[0]["amount"] == 88.0


def test_cmbc_can_parse_covers_extension_encoding_fallback_and_top_level_failure(monkeypatch) -> None:
    parser = CMBCParser()

    def _raise_decode_error(*_args: Any, **kwargs: Any) -> pd.DataFrame:
        encoding = kwargs.get("encoding", "utf-8")
        raise UnicodeDecodeError(str(encoding), b"x", 0, 1, "bad encoding")

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_csv", _raise_decode_error)

    assert parser.can_parse("statement.txt") is False
    assert parser.can_parse("neutral_statement.csv") is False
    assert parser.can_parse(None) is False  # type: ignore[arg-type]


def test_cmbc_can_parse_excel_falls_back_from_html_and_handles_reader_errors(monkeypatch) -> None:
    parser = CMBCParser()

    excel_like_df = pd.DataFrame(
        [
            ["交易时间", "支出金额", "存入金额", "账户余额", "对方账号", "对方名称", "对方开户行"],
            ["20170102 09:30:00", "32.80", "", "100.00", "6226", "测试商户", "测试开户行"],
        ]
    )

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", lambda *_args, **_kwargs: [excel_like_df])
    assert parser.can_parse("neutral_statement.xls") is True

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", lambda *_args, **_kwargs: [])
    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_excel", lambda *_args, **_kwargs: excel_like_df)
    assert parser.can_parse("neutral_statement.xlsx") is True

    def _raise_reader_error(*_args: Any, **_kwargs: Any) -> pd.DataFrame:
        raise RuntimeError("broken reader")

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", _raise_reader_error)
    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_excel", _raise_reader_error)
    assert parser.can_parse("broken_statement.xlsx") is False


def test_cmbc_is_html_file_returns_false_when_open_fails(monkeypatch) -> None:
    parser = CMBCParser()

    def _raise_open_error(*_args: Any, **_kwargs: Any) -> Any:
        raise OSError("cannot open file")

    monkeypatch.setattr("builtins.open", _raise_open_error)

    assert parser._is_html_file("missing.xls") is False  # pylint: disable=protected-access


def test_cmbc_parse_html_xls_handles_empty_tables_missing_headers_and_bad_rows(monkeypatch) -> None:
    parser = CMBCParser()

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", lambda *_args, **_kwargs: [])
    assert not parser._parse_html_xls("empty.xls")  # pylint: disable=protected-access

    no_header_df = pd.DataFrame([["无效表头"], ["仍然无效"]])
    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", lambda *_args, **_kwargs: [no_header_df])
    assert not parser._parse_html_xls("no_header.xls")  # pylint: disable=protected-access

    class BadStringValue:
        """Sentinel object whose string conversion fails for row-level error coverage."""

        def __str__(self) -> str:
            raise RuntimeError("bad row payload")

    html_df = pd.DataFrame(
        [
            ["交易时间", "支出金额", "存入金额", "账户余额", "对方账号", "对方名称", "交易方式", "摘要", None],
            ["交易时间", "10", "", "100", "A001", "跳过表头", "手机银行", "重复表头"],
            ["20170103\t08:00:00", "", "bad-credit", "100", "A002", "坏收入", "手机银行", "坏收入行"],
            ["20170103\t09:00:00", "bad-debit", "", "100", "A003", "坏支出", "手机银行", "坏支出行"],
            ["20170103\t10:00:00", "", "", "100", "A004", "零金额", "手机银行", "零金额行"],
            ["20170103\t11:00:00", "", "88.00", "100", "A005", "有效收入", "手机银行", BadStringValue()],
            ["20170103\t12:00:00", "", "66.00", "100", "A006", "最终有效", "手机银行", "有效行", "忽略列"],
        ]
    )
    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", lambda *_args, **_kwargs: [html_df])
    parsed_bills = parser._parse_html_xls("edge_cases.xls")  # pylint: disable=protected-access

    assert len(parsed_bills) == 1
    assert parsed_bills[0]["counterparty"] == "最终有效"
    assert parsed_bills[0]["amount"] == "66.0"

    def _raise_html_error(*_args: Any, **_kwargs: Any) -> list[pd.DataFrame]:
        raise RuntimeError("broken html parser")

    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_html", _raise_html_error)
    assert not parser._parse_html_xls("broken_html.xls")  # pylint: disable=protected-access


def test_cmbc_parse_covers_validate_failure_csv_header_and_row_errors(monkeypatch) -> None:
    parser = CMBCParser()

    monkeypatch.setattr(parser, "validate_file", lambda _file_path: False)
    assert not parser.parse("ignored.csv")

    monkeypatch.setattr(parser, "validate_file", lambda _file_path: True)
    monkeypatch.setattr(parser, "read_lines_with_fallback", lambda _file_path: (["没有表头\n"], "utf-8"))
    assert not parser.parse("missing_header.csv")

    monkeypatch.setattr(
        parser,
        "read_lines_with_fallback",
        lambda _file_path: (
            [
                "中国民生银行股份有限公司个人账户对账单\n",
                "交易日期,交易金额,收/支,交易对手,交易说明\n",
                "2026-01-01,abc,,坏数据商户,坏金额\n",
                "2026-01-02,12.50,,报销账户,自动判定收入\n",
            ],
            "utf-8",
        ),
    )
    parsed_bills = parser.parse("row_errors.csv")

    assert len(parsed_bills) == 1
    assert parsed_bills[0]["type"] == "收入"
    assert parsed_bills[0]["amount"] == 12.5


def test_cmbc_parse_accepts_csv_when_header_is_first_line(monkeypatch) -> None:
    parser = CMBCParser()

    monkeypatch.setattr(parser, "validate_file", lambda _file_path: True)
    monkeypatch.setattr(
        parser,
        "read_lines_with_fallback",
        lambda _file_path: (
            [
                "交易日期,交易金额,收/支,交易对手,交易说明\n",
                "2026-01-03,8.80,支出,便利店,早餐\n",
            ],
            "utf-8",
        ),
    )

    parsed_bills = parser.parse("header_first.csv")

    assert len(parsed_bills) == 1
    assert parsed_bills[0]["type"] == "支出"
    assert parsed_bills[0]["amount"] == -8.8


def test_cmbc_parse_excel_covers_missing_headers_invalid_rows_and_new_old_amount_branches(monkeypatch) -> None:
    parser = CMBCParser()
    monkeypatch.setattr(parser, "validate_file", lambda _file_path: True)
    monkeypatch.setattr(parser, "_is_html_file", lambda _file_path: False)

    monkeypatch.setattr(
        "bill_analyser.parsers.cmbc.pd.read_excel",
        lambda *_args, **_kwargs: pd.DataFrame([["无效表头"]]),
    )
    assert not parser.parse("missing_header.xlsx")

    new_format_df = pd.DataFrame(
        [
            ["交易时间", "支出金额", "存入金额", "账户余额", "对方名称", "摘要"],
            ["nan", "10.00", "", "100", "跳过空日期", "空日期"],
            ["2026-01-03", "", "bad-credit", "100", "坏收入", "坏收入"],
            ["2026-01-04", "bad-debit", "", "100", "坏支出", "坏支出"],
            ["2026-01-05", "", "88.00", "100", "有效收入", "有效收入"],
        ]
    )
    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_excel", lambda *_args, **_kwargs: new_format_df)
    new_format_bills = parser.parse("new_format.xlsx")
    assert len(new_format_bills) == 1
    assert new_format_bills[0]["counterparty"] == "有效收入"
    assert new_format_bills[0]["amount"] == 88.0

    old_format_df = pd.DataFrame(
        [
            ["交易日期", "金额", "对方名称", "摘要"],
            ["nan", "10.00", "跳过日期", "空日期"],
            ["2026-01-06", "bad-amount", "坏金额", "坏金额"],
            ["2026-01-07", "0", "零金额", "零金额"],
            ["2026-01-08", "18.50", "最终有效", "旧格式收入"],
        ]
    )
    monkeypatch.setattr("bill_analyser.parsers.cmbc.pd.read_excel", lambda *_args, **_kwargs: old_format_df)
    old_format_bills = parser.parse("old_format.xlsx")
    assert len(old_format_bills) == 1
    assert old_format_bills[0]["counterparty"] == "最终有效"
    assert old_format_bills[0]["amount"] == 18.5
