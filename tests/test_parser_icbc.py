"""Regression and edge-case tests for the ICBC parser."""

from __future__ import annotations

import tempfile
from pathlib import Path
from typing import Any

import pandas as pd
from openpyxl import Workbook

from bill_analyser.parsers.factory import ParserFactory
from bill_analyser.parsers.icbc import ICBCParser
from tests.parser_test_support import build_icbc_html_xls, build_icbc_xlsx, sample_path

# pylint: disable=missing-function-docstring,missing-class-docstring,too-few-public-methods


def test_icbc_parser_handles_csv_sample() -> None:
    parser = ICBCParser()
    bills = parser.parse(str(sample_path("icbc_statement_sample.csv")))

    assert parser.can_parse(str(sample_path("icbc_statement_sample.csv"))) is True
    assert len(bills) == 2
    assert bills[0]["source_account_id"] == "icbc"
    assert bills[0]["amount"] < 0
    assert bills[1]["amount"] > 0


def test_icbc_parser_handles_generated_xlsx(tmp_path) -> None:
    parser = ICBCParser()
    xlsx_path = build_icbc_xlsx(tmp_path / "icbc_statement_sample.xlsx")

    bills = parser.parse(str(xlsx_path))

    assert parser.can_parse(str(xlsx_path)) is True
    assert len(bills) == 2
    assert bills[0]["description"] == "早餐消费 | 测试早餐店 | 6222000000000001"
    assert bills[1]["amount"] == 1200.0


def test_icbc_internal_helpers_cover_content_detection(tmp_path) -> None:
    parser = ICBCParser()
    xlsx_path = build_icbc_xlsx(tmp_path / "icbc_statement_sample.xlsx")

    is_html, error = parser._detect_file_format(str(xlsx_path))  # pylint: disable=protected-access
    extracted = parser._extract_bill_from_excel_row(  # pylint: disable=protected-access
        ("6222000000000000", "2026-01-10 08:00:00", -15.5, "测试早餐店", "6222", "早餐消费", "流水1", "早餐"),
        {
            "交易日期": 1,
            "收入/支出金额": 2,
            "对方户名": 3,
            "对方账号": 4,
            "摘要": 5,
            "交易流水号": 6,
            "交易附言": 7,
        },
    )

    assert is_html is False
    assert error is None
    assert parser._is_icbc_content("收入/支出金额 对方账号名称 交易附言") is True  # pylint: disable=protected-access
    assert parser._is_icbc_content("民生银行 支出金额 存入金额") is False  # pylint: disable=protected-access
    assert extracted is not None
    assert extracted["type"] == "支出"


def test_icbc_parser_handles_html_and_content_based_detection() -> None:
    parser = ICBCParser()
    with tempfile.TemporaryDirectory(prefix="parser-neutral-") as temp_dir:
        neutral_root = Path(temp_dir)
        neutral_xlsx = build_icbc_xlsx(neutral_root / "statement.xlsx")
        html_xls = build_icbc_html_xls(neutral_root / "statement.xls")

        html_bills = parser.parse(str(html_xls))

        assert parser.can_parse(str(neutral_xlsx)) is True
        assert parser.can_parse(str(html_xls)) is True
        assert len(html_bills) == 2
        assert html_bills[0]["amount"] == -15.5


def test_icbc_parser_handles_bad_inputs_and_reader_fallbacks(tmp_path, monkeypatch) -> None:
    parser = ICBCParser()
    bad_csv = tmp_path / "bad_icbc.csv"
    bad_csv.write_text("无效内容\n", encoding="gbk")

    is_html, error = parser._detect_file_format(str(tmp_path / "missing.xls"))  # pylint: disable=protected-access
    monkeypatch.setattr(
        "bill_analyser.parsers.icbc.pd.read_excel",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(ValueError("bad excel")),
    )

    assert parser._read_excel_content(str(tmp_path / "missing.xlsx")) == ""  # pylint: disable=protected-access
    assert parser.parse(str(bad_csv)) == []
    assert is_html is False
    assert error is not None


def test_icbc_parser_detects_neutral_utf8sig_csv_and_reads_html_content(tmp_path) -> None:
    parser = ICBCParser()
    neutral_csv = tmp_path / "statement.csv"
    neutral_csv.write_text(
        "中国工商银行历史明细,,,,,,\n"
        "交易日期,交易金额,对方户名,对方账号,摘要,交易流水号,收/支\n"
        "2026-01-10 08:00:00,-15.50,测试早餐店,6222000000000001,早餐消费,ICBC-0001,支出\n"
        "2026-01-11 19:30:00,1200.00,测试工资账户,6222000000000002,工资入账,ICBC-0002,收入\n",
        encoding="utf-8-sig",
    )
    html_xls = build_icbc_html_xls(tmp_path / "statement.xls")

    assert parser.can_parse(str(neutral_csv)) is True
    assert len(parser.parse(str(neutral_csv))) == 2
    assert "中国工商银行" in parser._read_html_content(str(html_xls))  # pylint: disable=protected-access


def test_icbc_parse_accepts_csv_when_header_is_first_line(monkeypatch) -> None:
    parser = ICBCParser()

    monkeypatch.setattr(parser, "validate_file", lambda _file_path: True)
    monkeypatch.setattr(
        parser,
        "read_lines_with_fallback",
        lambda _file_path: (
            [
                "交易日期,交易金额,收/支,对方户名,对方账号,摘要,交易流水号\n",
                "2026-01-03 08:00:00,-15.50,支出,测试早餐店,6222000000000001,早餐消费,ICBC-0003\n",
            ],
            "utf-8",
        ),
    )

    parsed_bills = parser.parse("header_first.csv")

    assert len(parsed_bills) == 1
    assert parsed_bills[0]["type"] == "支出"
    assert parsed_bills[0]["amount"] == -15.5


def test_icbc_factory_detects_neutral_csv_with_first_line_header(tmp_path) -> None:
    factory = ParserFactory()
    neutral_csv = tmp_path / "statement.csv"
    neutral_csv.write_text(
        "交易日期,交易金额,收/支,对方户名,对方账号,摘要,交易流水号\n"
        "2026-01-03 08:00:00,-15.50,支出,测试早餐店,6222000000000001,早餐消费,ICBC-0003\n",
        encoding="utf-8-sig",
    )

    parser = factory.get_parser(str(neutral_csv))
    parsed_bills = factory.parse(str(neutral_csv))

    assert parser is not None
    assert parser.PARSER_ID == "icbc"
    assert len(parsed_bills) == 1
    assert parsed_bills[0]["amount"] == -15.5


def test_icbc_factory_preserves_positive_income_without_explicit_type_columns(tmp_path) -> None:
    factory = ParserFactory()
    neutral_csv = tmp_path / "income_only.csv"
    neutral_csv.write_text(
        "交易日期,交易金额,对方户名,对方账号,摘要,交易流水号\n"
        "2026-01-03 08:00:00,1200.00,测试工资账户,6222000000000002,工资入账,ICBC-0004\n",
        encoding="utf-8-sig",
    )

    parsed_bills = factory.parse(str(neutral_csv))

    assert len(parsed_bills) == 1
    assert parsed_bills[0]["type"] == "收入"
    assert parsed_bills[0]["amount"] == 1200.0


def test_icbc_parse_html_xls_handles_empty_tables_missing_headers_and_bad_rows(monkeypatch) -> None:
    parser = ICBCParser()

    monkeypatch.setattr("bill_analyser.parsers.icbc.pd.read_html", lambda *_args, **_kwargs: [])
    assert not parser._parse_html_xls("empty.xls")  # pylint: disable=protected-access

    no_header_df = pd.DataFrame([["无效表头"], ["仍然无效"]])
    monkeypatch.setattr(
        "bill_analyser.parsers.icbc.pd.read_html",
        lambda *_args, **_kwargs: [no_header_df],
    )
    assert not parser._parse_html_xls("no_header.xls")  # pylint: disable=protected-access

    class BadStringValue:
        def __str__(self) -> str:
            raise RuntimeError("bad row payload")

    html_df = pd.DataFrame(
        [
            ["交易日期", "收入/支出金额", "对方户名", "对方账号", "摘要", "交易流水号"],
            [None, "10.00", "跳过空日期", "A001", "空日期", "L001"],
            ["2026-01-03 08:00:00", "bad", "坏金额", "A002", "坏金额", "L002"],
            ["2026-01-04 08:00:00", "0", "零金额", "A003", "零金额", "L003"],
            ["2026-01-05 08:00:00", "88.00", "坏摘要", "A004", BadStringValue(), "L004"],
            ["2026-01-06 08:00:00", "-12.50", "最终有效", "A005", "早餐消费", "L005"],
        ]
    )
    monkeypatch.setattr(
        "bill_analyser.parsers.icbc.pd.read_html",
        lambda *_args, **_kwargs: [html_df],
    )
    parsed_bills = parser._parse_html_xls("edge_cases.xls")  # pylint: disable=protected-access

    assert len(parsed_bills) == 1
    assert parsed_bills[0]["counterparty"] == "最终有效"
    assert parsed_bills[0]["amount"] == -12.5

    def _raise_html_error(*_args: Any, **_kwargs: Any) -> list[pd.DataFrame]:
        raise RuntimeError("broken html parser")

    monkeypatch.setattr("bill_analyser.parsers.icbc.pd.read_html", _raise_html_error)
    assert not parser._parse_html_xls("broken_html.xls")  # pylint: disable=protected-access


def test_icbc_parse_excel_covers_missing_headers_invalid_rows_and_top_level_errors(
    tmp_path,
    monkeypatch,
) -> None:
    parser = ICBCParser()

    missing_header_path = tmp_path / "missing_header.xlsx"
    missing_header_workbook = Workbook()
    missing_header_sheet = missing_header_workbook.active
    assert missing_header_sheet is not None
    missing_header_sheet.append(["首行说明"])
    missing_header_workbook.save(missing_header_path)
    missing_header_workbook.close()

    assert not parser._parse_excel(str(missing_header_path))  # pylint: disable=protected-access

    edge_path = tmp_path / "edge_cases.xlsx"
    edge_workbook = Workbook()
    edge_sheet = edge_workbook.active
    assert edge_sheet is not None
    edge_sheet.append(["说明行"])
    edge_sheet.append(
        ["账号", "交易日期", "收入/支出金额", "对方户名", "对方账号", "摘要", "交易流水号", "交易附言"]
    )
    edge_sheet.append(["6222", None, -10.0, "空日期", "A001", "空日期", "L001", "附言"])
    edge_sheet.append(["6222", "2026-01-01 08:00:00", "bad", "坏金额", "A002", "坏金额", "L002", "附言"])
    edge_sheet.append(["6222", "2026-01-02 08:00:00", "0", "零金额", "A003", "零金额", "L003", "附言"])
    edge_sheet.append(["6222", "2026-01-03 08:00:00", -15.5, "最终有效", "A004", "早餐消费", "L004", "附言"])
    edge_workbook.save(edge_path)
    edge_workbook.close()

    parsed_bills = parser._parse_excel(str(edge_path))  # pylint: disable=protected-access
    assert len(parsed_bills) == 1
    assert parsed_bills[0]["counterparty"] == "最终有效"
    assert parsed_bills[0]["amount"] == -15.5

    def _raise_workbook_error(*_args: Any, **_kwargs: Any) -> Workbook:
        raise RuntimeError("broken workbook")

    monkeypatch.setattr("bill_analyser.parsers.icbc.openpyxl.load_workbook", _raise_workbook_error)
    assert not parser._parse_excel("broken.xlsx")  # pylint: disable=protected-access
