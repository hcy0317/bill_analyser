from __future__ import annotations

from bill_analyser.parsers.alipay import AlipayParser
from tests.parser_test_support import sample_path


def test_alipay_parser_handles_csv_sample() -> None:
    parser = AlipayParser()
    bills = parser.parse(str(sample_path("alipay_statement_sample.csv")))

    assert parser.can_parse(str(sample_path("alipay_statement_sample.csv"))) is True
    assert len(bills) == 6
    assert bills[0]["source_account_id"] == "alipay"
    assert any(bill["amount"] < 0 for bill in bills)
    assert any(bill["amount"] > 0 for bill in bills)
    assert any(bill["original_category"] == "投资理财" for bill in bills)


def test_alipay_extract_row_skips_footer_and_keeps_fields() -> None:
    parser = AlipayParser()

    assert parser._extract_bill_from_row({"交易时间": "共6笔记录"}) is None  # pylint: disable=protected-access
    row_bill = parser._extract_bill_from_row(  # pylint: disable=protected-access
        {
            "交易时间": "2026-01-15 08:30:00",
            "收/支": "支出",
            "交易对方": "测试早餐店",
            "对方账号": "test@example.com",
            "商品说明": "早餐套餐",
            "金额": "15.00",
            "交易分类": "商户消费",
        }
    )

    assert row_bill is not None
    assert row_bill["counterparty"] == "测试早餐店"
    assert row_bill["original_category"] == "商户消费"


def test_alipay_parser_rejects_generic_sample() -> None:
    parser = AlipayParser()

    assert parser.can_parse(str(sample_path("generic_statement_sample.csv"))) is False


def test_alipay_parser_handles_missing_headers_and_encoding_failures(tmp_path, monkeypatch) -> None:
    parser = AlipayParser()
    bad_csv = tmp_path / "alipay_bad.csv"
    bad_csv.write_text("无效表头\n仅说明\n", encoding="utf-8")

    assert parser.parse(str(bad_csv)) == []
    monkeypatch.setattr(parser, "_parse_with_encoding", lambda *_args, **_kwargs: [])
    assert parser.parse(str(sample_path("alipay_statement_sample.csv"))) == []


def test_alipay_parser_covers_os_errors_and_validate_short_circuit(tmp_path, monkeypatch) -> None:
    parser = AlipayParser()
    missing_like_path = tmp_path / "missing_alipay.csv"

    def _raise_open_error(*_args, **_kwargs):
        raise OSError("cannot open file")

    monkeypatch.setattr("builtins.open", _raise_open_error)
    assert parser.can_parse(str(missing_like_path)) is False


def test_alipay_parser_handles_row_level_exceptions(tmp_path, monkeypatch) -> None:
    parser = AlipayParser()
    parser.validate_file = lambda _file_path: False  # type: ignore[method-assign]
    assert parser.parse(str(tmp_path / "ignored.csv")) == []

    parser = AlipayParser()
    csv_path = tmp_path / "alipay_rows.csv"
    csv_path.write_text(
        "支付宝账单\n"
        "交易时间,交易分类,交易对方,对方账号,商品说明,收/支,金额,收/付款方式,交易状态,交易订单号,商家订单号,备注\n"
        "2026-01-15 08:30:00,商户消费,测试早餐店,test@example.com,早餐套餐,支出,15.00,余额,交易成功,ALI-1,MER-1,/\n",
        encoding="utf-8",
    )

    monkeypatch.setattr(
        parser,
        "_extract_bill_from_row",
        lambda _row: (_ for _ in ()).throw(RuntimeError("bad row")),
    )
    assert parser._parse_with_encoding(str(csv_path), "utf-8") == []


def test_alipay_extract_row_uses_alternate_time_and_field_names() -> None:
    parser = AlipayParser()

    row_bill = parser._extract_bill_from_row(  # pylint: disable=protected-access
        {
            "交易创建时间": "2026-01-16 09:45:00",
            "对方": "测试转账对象",
            "商品名称": "午餐套餐",
            "金额(元)": "23.50",
            "交易分类": "转账",
        }
    )

    assert row_bill is not None
    assert row_bill["date"] == "2026-01-16 09:45:00"
    assert row_bill["counterparty"] == "测试转账对象"
    assert row_bill["description"] == "午餐套餐"
    assert row_bill["amount"] == "23.50"


def test_alipay_parse_with_encoding_skips_rows_that_extract_to_none(tmp_path) -> None:
    parser = AlipayParser()
    csv_path = tmp_path / "alipay_skip_none.csv"
    csv_path.write_text(
        "支付宝账单\n"
        "交易时间,交易分类,交易对方,对方账号,商品说明,收/支,金额,收/付款方式,交易状态,交易订单号,商家订单号,备注\n"
        "共2笔记录,统计项,,,,,,,,,,\n"
        "2026-01-15 08:30:00,商户消费,测试早餐店,test@example.com,早餐套餐,支出,15.00,余额,交易成功,ALI-1,MER-1,/\n",
        encoding="utf-8",
    )

    bills = parser._parse_with_encoding(str(csv_path), "utf-8")  # pylint: disable=protected-access

    assert len(bills) == 1
    assert bills[0]["counterparty"] == "测试早餐店"
