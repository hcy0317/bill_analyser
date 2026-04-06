from __future__ import annotations

import tempfile
from pathlib import Path

import pandas as pd

from bill_analyser.parsers.abc import ABCParser
from tests.parser_test_support import build_abc_xlsx, sample_path


def test_abc_parser_handles_csv_sample() -> None:
    parser = ABCParser()
    bills = parser.parse(str(sample_path("abc_statement_sample.csv")))

    assert parser.can_parse(str(sample_path("abc_statement_sample.csv"))) is True
    assert len(bills) == 2
    assert bills[0]["source_account_id"] == "abc"
    assert bills[0]["description"] == "早餐 | 门店消费 | 测试早餐店"
    assert bills[1]["amount"] == 200.0


def test_abc_parser_handles_generated_xlsx(tmp_path) -> None:
    parser = ABCParser()
    xlsx_path = build_abc_xlsx(tmp_path / "abc_statement_sample.xlsx")

    bills = parser.parse(str(xlsx_path))

    assert parser.can_parse(str(xlsx_path)) is True
    assert len(bills) == 2
    assert bills[0]["amount"] == -18.6
    assert bills[1]["description"] == "差旅报销 | 公司报销 | 测试报销账户"


def test_abc_description_merging_and_content_checks() -> None:
    parser = ABCParser()

    assert parser._merge_description_fields("早餐", "门店消费", "早餐") == "早餐 | 门店消费"  # pylint: disable=protected-access
    assert parser._check_abc_content("中国农业银行股份有限公司 交易日期 交易时间 交易金额 对方户名 交易用途 本次余额") is True  # pylint: disable=protected-access
    assert parser._check_abc_content("支出金额 存入金额 民生银行") is False  # pylint: disable=protected-access


def test_abc_helpers_and_failure_paths(tmp_path, monkeypatch) -> None:
    parser = ABCParser()
    bad_csv = tmp_path / "bad_abc.csv"
    bad_csv.write_text("无效表头\n", encoding="gbk")
    bad_xlsx = tmp_path / "bad_abc.xlsx"
    build_abc_xlsx(bad_xlsx)

    assert parser._get_date_field({"记账日期": "2026-01-12"}) == "2026-01-12"  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"交易金额": "-12.30"}) == ("12.3", "支出")  # pylint: disable=protected-access
    assert parser._find_header_row(pd.DataFrame([["无效"]])) == -1  # pylint: disable=protected-access
    assert parser._parse_csv(str(bad_csv)) == []  # pylint: disable=protected-access

    monkeypatch.setattr(parser, "_parse_csv", lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("bad csv")))
    assert parser.parse(str(sample_path("abc_statement_sample.csv"))) == []


def test_abc_parser_can_parse_neutral_filenames(monkeypatch) -> None:
    parser = ABCParser()

    with tempfile.TemporaryDirectory(prefix="parser-neutral-") as temp_dir:
        neutral_root = Path(temp_dir)
        neutral_csv = neutral_root / "statement.csv"
        neutral_csv.write_text("占位,占位\n", encoding="gbk")
        neutral_xlsx = build_abc_xlsx(neutral_root / "statement.xlsx")

        csv_df = pd.DataFrame([["中国农业银行股份有限公司", "交易日期", "交易时间", "交易金额", "对方户名", "交易用途", "本次余额"]])
        xlsx_df = pd.DataFrame([["账户明细查询", "活期明细", "交易日期", "交易时间", "交易金额"]])

        monkeypatch.setattr("bill_analyser.parsers.abc.pd.read_csv", lambda *_args, **_kwargs: csv_df)
        monkeypatch.setattr("bill_analyser.parsers.abc.pd.read_excel", lambda *_args, **_kwargs: xlsx_df)

        assert parser.can_parse(str(neutral_csv)) is True
        assert parser.can_parse(str(neutral_xlsx)) is True


def test_abc_helper_branches_cover_invalid_dates_amounts_and_zero_rows() -> None:
    parser = ABCParser()

    assert parser._merge_description_fields("", "早餐") == "早餐"  # pylint: disable=protected-access
    assert parser._merge_description_fields("单字段") == "单字段"  # pylint: disable=protected-access
    assert parser._get_date_field({}) == ""  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"收入金额": "-5"}) == ("0", "支出")  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"收入金额": "oops"}) == ("0", "支出")  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"支出金额": "-5"}) == ("0", "支出")  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"支出金额": "oops"}) == ("0", "支出")  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"交易金额": "oops"}) == ("0", "支出")  # pylint: disable=protected-access
    assert parser._get_amount_and_type({"交易金额": "-15"}) == ("15.0", "支出")  # pylint: disable=protected-access
    assert parser._parse_csv_row({"交易金额": "18.60"}) is None  # pylint: disable=protected-access
    assert parser._parse_csv_row({"交易日期": "2026-01-12", "交易金额": "oops"}) is None  # pylint: disable=protected-access
    assert parser._parse_excel_row({"交易金额": "18.60"}) is None  # pylint: disable=protected-access
    assert parser._parse_excel_row({"交易日期": "2026-01-12", "交易金额": "oops"}) is None  # pylint: disable=protected-access


def test_abc_parse_paths_cover_row_errors_header_miss_and_validate_short_circuit(monkeypatch) -> None:
    parser = ABCParser()

    monkeypatch.setattr(
        parser,
        "read_lines_with_fallback",
        lambda _file_path: (
            [
                "农业银行账单\n",
                "交易日期,交易金额\n",
                ",18.60\n",
                "2026-01-12,18.60\n",
            ],
            "utf-8",
        ),
    )
    parsed_csv = parser._parse_csv("mixed_rows.csv")  # pylint: disable=protected-access
    assert len(parsed_csv) == 1

    monkeypatch.setattr(
        parser,
        "_parse_csv_row",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("bad csv row")),
    )
    assert parser._parse_csv("broken.csv") == []  # pylint: disable=protected-access

    monkeypatch.setattr("bill_analyser.parsers.abc.pd.read_excel", lambda *_args, **_kwargs: pd.DataFrame([["无效表头"]]))
    assert parser._parse_excel("missing_header.xlsx") == []  # pylint: disable=protected-access

    monkeypatch.setattr(
        "bill_analyser.parsers.abc.pd.read_excel",
        lambda *_args, **_kwargs: pd.DataFrame([
            ["交易日期", "交易金额"],
            [None, "18.60"],
            ["2026-01-12", "18.60"],
        ]),
    )
    parser = ABCParser()
    parsed_excel = parser._parse_excel("mixed_rows.xlsx")  # pylint: disable=protected-access
    assert len(parsed_excel) == 1

    parser = ABCParser()
    monkeypatch.setattr(
        parser,
        "_parse_excel_row",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("bad excel row")),
    )
    assert parser._parse_excel("bad_rows.xlsx") == []  # pylint: disable=protected-access

    monkeypatch.setattr(parser, "validate_file", lambda _file_path: False)
    assert parser.parse("ignored.csv") == []


def test_abc_can_parse_covers_reader_fallbacks_and_exception_paths(monkeypatch) -> None:
    parser = ABCParser()

    assert parser.can_parse("statement.txt") is False

    read_csv_calls: list[str | None] = []

    def _read_csv_with_retry(*_args, **kwargs) -> pd.DataFrame:
        encoding = kwargs.get("encoding")
        read_csv_calls.append(encoding)
        if len(read_csv_calls) == 1:
            raise UnicodeDecodeError(str(encoding or "utf-8"), b"x", 0, 1, "bad csv")
        return pd.DataFrame([["中国农业银行股份有限公司", "交易日期", "交易时间", "交易金额"]])

    monkeypatch.setattr("bill_analyser.parsers.abc.pd.read_csv", _read_csv_with_retry)
    monkeypatch.setattr(parser, "_check_abc_content", lambda _content: True)
    assert parser.can_parse("statement.csv") is True
    assert len(read_csv_calls) >= 2

    monkeypatch.setattr(
        "bill_analyser.parsers.abc.pd.read_csv",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(UnicodeDecodeError("utf-8", b"x", 0, 1, "still bad")),
    )
    assert parser.can_parse("statement.csv") is False

    monkeypatch.setattr(
        "bill_analyser.parsers.abc.pd.read_excel",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("broken excel probe")),
    )
    assert parser.can_parse("statement.xlsx") is False

    monkeypatch.setattr("bill_analyser.parsers.abc.pd.read_csv", lambda *_args, **_kwargs: pd.DataFrame([["交易日期", "交易金额"]]))
    monkeypatch.setattr(
        parser,
        "_check_abc_content",
        lambda _content: (_ for _ in ()).throw(RuntimeError("boom")),
    )
    assert parser.can_parse("statement.csv") is False
