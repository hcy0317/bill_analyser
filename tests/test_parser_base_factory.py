from __future__ import annotations

from pathlib import Path

from bill_analyser.parsers.base import ParserBase, StandardBill
from bill_analyser.parsers import factory as factory_module
from bill_analyser.parsers.factory import ParserFactory
from tests.parser_test_support import sample_path


class DummyParser(ParserBase):
    PARSER_ID = 'dummy'
    PARSER_NAME = 'Dummy'

    def can_parse(self, file_path: str) -> bool:
        return file_path.endswith('.txt')

    def parse(self, file_path: str):
        return []


def test_standard_bill_round_trip_and_type_hints() -> None:
    bill = StandardBill.from_dict(
        {
            'date': '2026-03-28 00:00:00',
            'amount': '12.34',
            'type': '支出',
            'description': '早餐',
            'source_account_id': 'wechat',
        }
    )

    assert bill.amount == 12.34
    assert bill.to_dict()['description'] == '早餐'


def test_parser_base_helpers_and_post_process(tmp_path: Path) -> None:
    parser = DummyParser()
    sample_file = tmp_path / 'sample.txt'
    sample_file.write_text('ok', encoding='utf-8')

    assert parser.validate_file(str(sample_file)) is False
    parser.supported_extensions.append('.txt')
    assert parser.validate_file(str(sample_file)) is True

    assert parser.normalize_date('2026/01/02') == '2026-01-02 00:00:00'
    assert parser.normalize_date('bad-date') == 'bad-date'
    assert parser.normalize_amount('¥1,234.50') == 1234.5
    assert parser.normalize_amount('not-a-number') == 0.0
    assert parser.normalize_type('投资理财') == '投资'
    assert parser.normalize_type('未知类型') == '支出'
    assert parser.aggregate_description(
        {
            'description': '早餐',
            'counterparty': '测试早餐店',
            'remark': '早餐',
            'payment_method': '零钱',
        }
    ) == '早餐 | 测试早餐店 | 零钱'

    processed = parser.post_process(
        [
            {
                'date': '2026-01-02',
                'amount': '18.60',
                'type': '支出',
                'description': '早餐',
                'counterparty': '测试早餐店',
                'payment_method': '零钱',
            },
            {
                'date': '2026-01-03 09:00:00',
                'amount': '88.00',
                'type': '投资理财',
                'description': '报销',
                'original_category': '投资理财',
            },
            {
                'amount': '0',
                'type': '支出',
            },
        ]
    )

    assert len(processed) == 2
    assert processed[0]['amount'] == -18.6
    assert processed[0]['source_account_id'] == 'dummy'
    assert processed[1]['type'] == '收入'
    assert processed[1]['amount'] == 88.0


def test_parser_factory_detects_known_and_unknown_samples() -> None:
    factory = ParserFactory()

    wechat_info = factory.detect_parser(str(sample_path('wechat_statement_sample.csv')))
    unknown_info = factory.detect_parser(str(sample_path('generic_statement_sample.csv')))

    assert wechat_info is not None
    assert wechat_info['id'] == 'wechat'
    assert unknown_info is None
    assert factory.get_parser_by_id('WeChat') is not None
    assert factory.get_parser_by_id('missing') is None
    assert '.csv' in factory.get_supported_formats()
    assert any(item['name'] == 'WeChatParser' for item in factory.get_parser_info())

    parsed = factory.parse_multiple([
        str(sample_path('wechat_statement_sample.csv')),
        str(sample_path('alipay_statement_sample.csv')),
    ])
    assert len(parsed) >= 4


def test_parser_base_handles_directory_and_bad_records(tmp_path: Path) -> None:
    parser = DummyParser()
    parser.supported_extensions.append('.txt')
    directory = tmp_path / 'folder'
    directory.mkdir()

    assert parser.validate_file(str(directory)) is False
    assert parser.aggregate_description({'description': '/', 'remark': 'nan'}) == ''
    assert parser.post_process([None]) == []  # type: ignore[list-item]


def test_factory_explicit_selection_and_error_paths(monkeypatch, tmp_path: Path) -> None:
    factory = ParserFactory()
    existing_file = sample_path('wechat_statement_sample.csv')
    missing_file = tmp_path / 'missing.csv'

    assert factory.get_parser(str(existing_file), parser_type='wechat') is not None
    assert factory.get_parser(str(existing_file), parser_type='missing') is not None
    assert factory.get_parser(str(missing_file)) is None
    assert factory.detect_parser(str(missing_file)) is None

    class ExplodingParser(ParserBase):
        PARSER_ID = 'boom'
        PARSER_NAME = 'Boom'

        def can_parse(self, file_path: str) -> bool:
            raise RuntimeError('boom')

        def parse(self, file_path: str):
            raise RuntimeError('boom')

    class MatchingParser(ParserBase):
        PARSER_ID = 'match'
        PARSER_NAME = 'Match'

        def can_parse(self, file_path: str) -> bool:
            return file_path.endswith('.csv')

        def parse(self, file_path: str):
            return [
                {
                    'date': '2026-01-01 08:00:00',
                    'type': '收入',
                    'amount': '10',
                    'description': '匹配成功',
                }
            ]

    factory.parsers = [ExplodingParser(), MatchingParser()]

    detected = factory.detect_parser(str(existing_file))
    assert detected is not None
    assert detected['id'] == 'match'

    class ParseBoomParser(MatchingParser):
        def parse(self, file_path: str):
            raise RuntimeError('parse failed')

    monkeypatch.setattr(factory, 'get_parser', lambda *args, **kwargs: ParseBoomParser())
    assert factory.parse(str(existing_file)) == []


def test_factory_module_level_helpers_delegate_to_global_factory(monkeypatch) -> None:
    factory = ParserFactory()
    monkeypatch.setattr(factory_module, '_factory', factory)

    assert factory_module.get_parser(str(sample_path('wechat_statement_sample.csv'))) is not None
    assert factory_module.parse_file(str(sample_path('wechat_statement_sample.csv')))
    assert factory_module.parse_multiple_files([str(sample_path('wechat_statement_sample.csv'))])
    assert '.csv' in factory_module.get_supported_formats()
