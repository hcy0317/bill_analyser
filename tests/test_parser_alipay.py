from __future__ import annotations

from bill_analyser.parsers.alipay import AlipayParser
from tests.parser_test_support import sample_path


def test_alipay_parser_handles_csv_sample() -> None:
    parser = AlipayParser()
    bills = parser.parse(str(sample_path('alipay_statement_sample.csv')))

    assert parser.can_parse(str(sample_path('alipay_statement_sample.csv'))) is True
    assert len(bills) == 6
    assert bills[0]['source_account_id'] == 'alipay'
    assert any(bill['amount'] < 0 for bill in bills)
    assert any(bill['amount'] > 0 for bill in bills)
    assert any(bill['original_category'] == '投资理财' for bill in bills)


def test_alipay_extract_row_skips_footer_and_keeps_fields() -> None:
    parser = AlipayParser()

    assert parser._extract_bill_from_row({'交易时间': '共6笔记录'}) is None  # pylint: disable=protected-access
    row_bill = parser._extract_bill_from_row(  # pylint: disable=protected-access
        {
            '交易时间': '2026-01-15 08:30:00',
            '收/支': '支出',
            '交易对方': '测试早餐店',
            '对方账号': 'test@example.com',
            '商品说明': '早餐套餐',
            '金额': '15.00',
            '交易分类': '商户消费',
        }
    )

    assert row_bill is not None
    assert row_bill['counterparty'] == '测试早餐店'
    assert row_bill['original_category'] == '商户消费'


def test_alipay_parser_rejects_generic_sample() -> None:
    parser = AlipayParser()

    assert parser.can_parse(str(sample_path('generic_statement_sample.csv'))) is False


def test_alipay_parser_handles_missing_headers_and_encoding_failures(tmp_path, monkeypatch) -> None:
    parser = AlipayParser()
    bad_csv = tmp_path / 'alipay_bad.csv'
    bad_csv.write_text('无效表头\n仅说明\n', encoding='utf-8')

    assert parser.parse(str(bad_csv)) == []
    monkeypatch.setattr(parser, '_parse_with_encoding', lambda *_args, **_kwargs: [])
    assert parser.parse(str(sample_path('alipay_statement_sample.csv'))) == []
