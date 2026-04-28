# pyright: reportPrivateUsage=false
from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any, cast

import pytest

from bill_analyser.api.routes import bills as bills_module

bills_module = cast("Any", bills_module)


@pytest.mark.parametrize(
    ("raw_value", "expected"),
    [
        ("1234", "1234"),
        ("1,234", "1234"),
        ("1,234.56", "1234.56"),
        ("1,234,567", "1234567"),
        ("1.234,56", "1234.56"),
        ("1.234.567", "1234567"),
        ("1,234,567.89", "1234567.89"),
        ("1.234.567,89", "1234567.89"),
        ("1234,56", "1234.56"),
        ("1.234", "1234"),
        ("0.123", "0.123"),
        ("-1.234,56", "-1234.56"),
        ("1,2,3.4.5.6", "123.4.5.6"),
    ],
)
def test_auto_detect_and_normalize_amount_supports_mixed_locales(raw_value: str, expected: str) -> None:
    """金额标准化 helper 应正确处理英式、欧式和边界格式。"""
    assert bills_module._auto_detect_and_normalize_amount(raw_value) == expected


def test_parse_generic_import_amount_and_signed_amount_handle_currency_and_parentheses() -> None:
    """金额解析应支持货币符号、括号负数与带符号结果。"""
    assert bills_module._parse_generic_import_amount("¥1,234.56") == pytest.approx(1234.56)
    assert bills_module._parse_generic_import_amount("1.234,56") == pytest.approx(1234.56)
    assert bills_module._parse_generic_import_amount("(1,234.56)") == pytest.approx(1234.56)
    assert bills_module._parse_generic_import_signed_amount("(1,234.56)") == pytest.approx(-1234.56)
    assert bills_module._parse_generic_import_signed_amount("not-a-number") is None


def test_detect_generic_import_header_row_index_skips_preamble_rows() -> None:
    """表头检测应跳过前导说明行，定位到真正表头。"""
    rows = [
        ["账单导出说明", "", "", ""],
        ["统计周期", "2026-01-01 至 2026-01-31", "", ""],
        ["交易时间", "交易类型", "金额", "账户"],
        ["2026-01-01 08:30:00", "支出", "12.34", "支付宝"],
    ]

    assert bills_module._detect_generic_import_header_row_index(rows) == 2
    trimmed_rows, header_index = bills_module._trim_generic_import_rows_to_header(rows)
    assert header_index == 2
    assert trimmed_rows[0] == ["交易时间", "交易类型", "金额", "账户"]


def test_detect_generic_import_header_row_index_falls_back_when_no_clear_header() -> None:
    """当没有清晰表头时应回退到首行。"""
    rows = [
        ["2026-01-01 08:30:00", "支出", "12.34", "支付宝"],
        ["2026-01-02 09:00:00", "收入", "88.00", "银行卡"],
    ]

    assert bills_module._detect_generic_import_header_row_index(rows) == 0


def test_detect_generic_import_header_row_index_respects_scan_limit() -> None:
    """表头超出扫描上限时应保持回退策略。"""
    rows = [[f"说明{i}", "", "", ""] for i in range(31)]
    rows.append(["交易时间", "交易类型", "金额", "账户"])
    rows.append(["2026-01-01 08:30:00", "支出", "12.34", "支付宝"])

    assert bills_module._detect_generic_import_header_row_index(rows) == 0


def test_query_filters_and_import_scoring_helpers_cover_invalid_and_context_paths() -> None:
    """查询筛选与列评分 helper 应覆盖异常输入和上下文加分分支。"""

    class _FakeFilterDB:
        async def get_all_categories(self, *, user_id: int) -> list[dict[str, Any]]:
            _ = user_id
            return [
                {"id": 10, "main_category": "餐饮", "sub_category": "早餐"},
                {"id": 11, "main_category": "学习", "sub_category": "课本"},
            ]

    filters: dict[str, Any] = {}
    asyncio.run(
        bills_module._apply_common_transaction_filters(
            {
                "keyword": "%E6%97%A9%E9%A4%90",
                "accountIds": "1,2",
                "categoryIds": "10,99",
                "tagIds": "3,4",
                "amountFilter": "positive",
            },
            filters,
            _FakeFilterDB(),
            7,
        )
    )
    assert filters == {
        "keyword": "早餐",
        "account_ids": [1, 2],
        "categories": [{"main": "餐饮", "sub": "早餐"}],
        "tag_ids": [3, 4],
        "amount_filter": "positive",
    }

    invalid_filters: dict[str, Any] = {}
    asyncio.run(
        bills_module._apply_common_transaction_filters(
            {"accountIds": "1,a", "categoryIds": "oops", "tagIds": "x"},
            invalid_filters,
            _FakeFilterDB(),
            7,
        )
    )
    assert invalid_filters == {}
    assert bills_module._parse_int_list("") == []

    assert bills_module.allowed_file("demo.csv") is True
    assert bills_module.allowed_file("demo.exe") is False
    assert bills_module.allowed_picture_file("avatar.png") is True
    assert bills_module.allowed_picture_file("avatar.txt") is False

    assert bills_module._score_header_keyword_match("交易时间", 1) >= bills_module.IMPORT_HEADER_EXACT_SCORE
    assert bills_module._score_header_keyword_match("账户余额", 6) == 0.0
    assert bills_module._score_header_keyword_match("对方账户", 6) == 0.0
    assert bills_module._score_header_keyword_match("原始分类", 4) >= bills_module.IMPORT_HEADER_EXACT_SCORE
    assert bills_module._score_header_keyword_match("支付方式", 6) >= bills_module.IMPORT_HEADER_SEMANTIC_SCORE
    assert bills_module._score_header_keyword_match("对方户名", 9) >= bills_module.IMPORT_HEADER_SEMANTIC_MEDIUM_SCORE
    assert bills_module._score_header_keyword_match("商品说明", 14) >= bills_module.IMPORT_HEADER_SEMANTIC_SCORE
    assert bills_module._score_header_keyword_match("对方账户", 9) >= bills_module.IMPORT_HEADER_TYPE_HINT_SCORE
    assert bills_module._score_header_keyword_match("对方金额", 11) >= bills_module.IMPORT_HEADER_TYPE_HINT_SCORE
    assert bills_module._score_header_keyword_match("付款账户", 6) >= bills_module.IMPORT_HEADER_CONTEXT_SCORE
    assert bills_module._score_header_keyword_match("交易金额", 8) >= bills_module.IMPORT_HEADER_CONTEXT_SCORE

    column_pairs = bills_module._extract_import_config_header_type_pairs(
        {
            "field_mappings": {"columnMapping": {"1": "0", "bad": "x", "8": 9}},
            "sample_headers": ["交易时间", "金额"],
            "use_count": 3,
        }
    )
    assert len(column_pairs) == 1
    assert column_pairs[0][0] == bills_module._normalize_import_suggestion_text("交易时间")

    legacy_pairs = bills_module._extract_import_config_header_type_pairs(
        {
            "field_mappings": {"date": "交易时间", "memo": "摘要", "unknown": "忽略"},
            "sample_headers": [],
            "use_count": 2,
        }
    )
    assert {pair[0] for pair in legacy_pairs} == {
        bills_module._normalize_import_suggestion_text("交易时间"),
        bills_module._normalize_import_suggestion_text("摘要"),
    }
    assert {pair[1] for pair in legacy_pairs} == {1, 14}

    assert bills_module._build_auto_transaction_type_mapping([], None) == {}
    auto_mapping = bills_module._build_auto_transaction_type_mapping(
        [["支出"], "bad-row", ["支出"], ["收入"], ["未知"]],
        0,
    )
    assert auto_mapping == {"支出": 3, "收入": 2}


def test_import_mapping_data_like_and_header_scoring_cover_remaining_edge_branches() -> None:
    """导入建议与表头/数据评分 helper 应覆盖空表头、重复候选与弱匹配分支。"""
    suggestion = bills_module._build_import_mapping_suggestion(
        ["", "交易时间", "时间"],
        configs=[],
        sample_rows=[],
    )
    assert suggestion["columnMapping"]["1"] == 1
    assert suggestion["suggestions"][0]["header"] == "交易时间"

    assert bills_module._is_generic_import_data_like_cell("") is False
    assert bills_module._is_generic_import_data_like_cell("2026-03-01") is True
    assert bills_module._is_generic_import_data_like_cell("01/03/2026 08:30:00") is True
    assert bills_module._is_generic_import_data_like_cell("普通表头") is False

    low_score, low_types = bills_module._score_generic_import_header_row(["仅一列"])
    assert low_score == -1.0
    assert low_types == set()

    noisy_score, noisy_types = bills_module._score_generic_import_header_row(["foo", "bar", "2026-03-01", "12.34"])
    assert noisy_score < bills_module.IMPORT_HEADER_MIN_REVIEW_SCORE
    assert noisy_types == set()

    mixed_score, _ = bills_module._score_generic_import_header_row(["交易时间", "()", "金额"])
    assert mixed_score > 0

    assert bills_module._score_generic_import_data_row([]) == 0.0
    assert bills_module._detect_generic_import_header_row_index([]) == 0


def test_read_text_with_fallback_supports_utf8_and_gbk(tmp_path: Path) -> None:
    """文本读取应按编码回退链正确识别 utf-8 与 gbk。"""
    utf8_file = tmp_path / "utf8.csv"
    utf8_file.write_text("交易时间,金额\n2026-01-01,12.34\n", encoding="utf-8")

    gbk_file = tmp_path / "gbk.csv"
    gbk_file.write_bytes("交易时间,金额\n2026-01-01,12.34\n".encode("gbk"))

    utf8_text, utf8_encoding = bills_module._read_text_with_fallback(utf8_file)
    gbk_text, gbk_encoding = bills_module._read_text_with_fallback(gbk_file, requested_encoding="utf-8")

    assert "交易时间" in utf8_text
    assert utf8_encoding == "utf-8"
    assert "交易时间" in gbk_text
    assert gbk_encoding in {"gbk", "gb18030"}


def test_detect_csv_delimiter_prefers_sniffer_and_fallback_counts() -> None:
    """CSV 分隔符检测应能识别常见分隔符并在失败时回退到计数策略。"""
    assert bills_module._detect_csv_delimiter("a,b,c\n1,2,3\n") == ","
    assert bills_module._detect_csv_delimiter("a;b;c\n1;2;3\n") == ";"
    assert bills_module._detect_csv_delimiter("a|b|c\n1|2|3\n", fallback=",") == "|"


def test_detect_csv_delimiter_falls_back_when_sniffer_errors(monkeypatch: pytest.MonkeyPatch) -> None:
    """当 Sniffer 抛错时应退回到计数逻辑。"""

    class _BrokenSniffer:
        @staticmethod
        def sniff(_sample_text: str) -> Any:
            raise bills_module.csv.Error("boom")

    monkeypatch.setattr(bills_module.csv, "Sniffer", lambda: _BrokenSniffer())
    assert bills_module._detect_csv_delimiter("a\tb\tc\n1\t2\t3\n", fallback=",") == "\t"


def test_read_text_and_table_loaders_cover_failure_xlsx_xls_and_unsupported_paths(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """文本与表格读取 helper 应覆盖失败、xlsx/xls 与不支持格式分支。"""
    broken_file = tmp_path / "broken.csv"
    broken_file.write_bytes(b"\xff\xfe\xff")

    def _raise_decode(*_args: Any, **_kwargs: Any) -> Any:
        raise UnicodeDecodeError("utf-8", b"", 0, 1, "boom")

    monkeypatch.setattr("builtins.open", _raise_decode)
    with pytest.raises(UnicodeDecodeError):
        bills_module._read_text_with_fallback(broken_file)

    monkeypatch.undo()

    original_openpyxl = bills_module.openpyxl
    original_pd = bills_module.pd
    try:
        monkeypatch.setattr(bills_module, "openpyxl", None)
        with pytest.raises(ImportError, match="openpyxl"):
            bills_module._load_generic_import_rows(tmp_path / "demo.xlsx")

        class _FakeSheet:
            @staticmethod
            def iter_rows(values_only: bool = True) -> list[tuple[Any, ...]]:
                _ = values_only
                return [("交易时间", "金额"), ("2026-03-01", 12.34)]

        class _FakeWorkbook:
            def __init__(self) -> None:
                self.worksheets = [_FakeSheet()]
                self.closed = False

            def close(self) -> None:
                self.closed = True

        fake_workbook = _FakeWorkbook()
        monkeypatch.setattr(
            bills_module,
            "openpyxl",
            type("FakeOpenPyxl", (), {"load_workbook": staticmethod(lambda *_args, **_kwargs: fake_workbook)})(),
        )
        xlsx_rows, xlsx_encoding, xlsx_delimiter = bills_module._load_generic_import_rows(tmp_path / "demo.xlsx")
        assert xlsx_rows == [["交易时间", "金额"], ["2026-03-01", "12.34"]]
        assert xlsx_encoding == "utf-8"
        assert xlsx_delimiter == ""
        assert fake_workbook.closed is True

        monkeypatch.setattr(bills_module, "pd", None)
        with pytest.raises(ImportError, match="pandas"):
            bills_module._load_generic_import_rows(tmp_path / "demo.xls")

        class _FakeTable:
            def __init__(self, rows: list[tuple[Any, ...]]) -> None:
                self._rows = rows

            def fillna(self, _value: str) -> _FakeTable:
                return self

            def itertuples(self, index: bool = False, name: Any = None) -> list[tuple[Any, ...]]:
                _ = (index, name)
                return self._rows

        fake_pd = type(
            "FakePandas",
            (),
            {
                "read_html": staticmethod(lambda *_args, **_kwargs: [_FakeTable([("交易时间", "金额"), ("2026-03-01", 12.34)])]),
                "read_excel": staticmethod(lambda *_args, **_kwargs: _FakeTable([])),
            },
        )()
        monkeypatch.setattr(bills_module, "pd", fake_pd)
        xls_rows, xls_encoding, xls_delimiter = bills_module._load_generic_import_rows(tmp_path / "demo.xls")
        assert xls_rows == [["交易时间", "金额"], ["2026-03-01", "12.34"]]
        assert xls_encoding == "utf-8"
        assert xls_delimiter == ""

        fake_pd_excel_fallback = type(
            "FakePandasFallback",
            (),
            {
                "read_html": staticmethod(lambda *_args, **_kwargs: []),
                "read_excel": staticmethod(lambda *_args, **_kwargs: _FakeTable([("A", "B"), (1, 2)])),
            },
        )()
        monkeypatch.setattr(bills_module, "pd", fake_pd_excel_fallback)
        fallback_rows, _, _ = bills_module._load_generic_import_rows(tmp_path / "fallback.xls")
        assert fallback_rows == [["A", "B"], ["1", "2"]]

        fake_pd_fail = type(
            "FakePandasFail",
            (),
            {
                "read_html": staticmethod(lambda *_args, **_kwargs: []),
                "read_excel": staticmethod(lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("excel boom"))),
            },
        )()
        monkeypatch.setattr(bills_module, "pd", fake_pd_fail)
        with pytest.raises(ValueError, match="未能读取 xls 文件"):
            bills_module._load_generic_import_rows(tmp_path / "broken.xls")

        with pytest.raises(ValueError, match="Unsupported file format"):
            bills_module._load_generic_import_rows(tmp_path / "demo.json")
    finally:
        bills_module.openpyxl = original_openpyxl
        bills_module.pd = original_pd


@pytest.mark.parametrize(
    ("raw_value", "expected_prefix"),
    [
        ("2026-03-01 08:30:00", "2026-03-01 08:30:00"),
        ("2026/03/01 08:30", "2026-03-01 08:30:00"),
        ("20260301", "2026-03-01 00:00:00"),
        ("31/01/2026", "2026-01-31 00:00:00"),
    ],
)
def test_parse_generic_import_time_supports_multiple_input_formats(raw_value: str, expected_prefix: str) -> None:
    """时间解析 helper 应兼容多种常见输入格式。"""
    timestamp = bills_module._parse_generic_import_time(raw_value)
    rendered = bills_module.datetime.fromtimestamp(timestamp).strftime("%Y-%m-%d %H:%M:%S")
    assert rendered == expected_prefix


def test_misc_parse_helpers_cover_empty_datetime_custom_separators_and_context_edges() -> None:
    """剩余解析 helper 应覆盖空值、datetime、自定义分隔符与上下文边界。"""
    now_timestamp = bills_module._parse_generic_import_time(None)
    assert isinstance(now_timestamp, int)

    dt_value = bills_module.datetime(2026, 3, 1, 8, 30, 0)
    assert bills_module._parse_generic_import_time(dt_value) == int(dt_value.timestamp())
    assert isinstance(bills_module._parse_generic_import_time("   "), int)
    assert isinstance(bills_module._parse_generic_import_time("not-a-date"), int)

    assert bills_module._parse_generic_import_amount(5) == pytest.approx(5.0)
    assert bills_module._parse_generic_import_amount("   ") == 0.0
    assert bills_module._parse_generic_import_amount("1.234,56", decimal_separator=",", grouping_symbol=".") == pytest.approx(1234.56)
    assert bills_module._parse_generic_import_amount("not-a-number", decimal_separator=",", grouping_symbol=".") == 0.0

    assert bills_module._parse_generic_import_signed_amount(7) == pytest.approx(7.0)
    assert bills_module._parse_generic_import_signed_amount("   ") is None
    assert bills_module._parse_generic_import_signed_amount("1.234,56", decimal_separator=",", grouping_symbol=".") == pytest.approx(1234.56)
    assert bills_module._parse_generic_import_signed_amount("oops", decimal_separator=",", grouping_symbol=".") is None

    assert bills_module._get_mapped_cell(["a"], {"1": "x"}, 1) == ""
    assert bills_module._get_mapped_cell(["a"], {"1": 5}, 1) == ""
    assert bills_module._get_mapped_cell(["a"], {}, 1) == ""

    assert bills_module._infer_generic_import_type_from_context([], ["摘要"]) is None
    assert bills_module._infer_generic_import_type_from_context(["普通备注"], ["摘要"]) is None
    assert bills_module._infer_generic_import_type_from_context(["工资补发"], ["摘要"]) == "收入"
    assert bills_module._infer_generic_import_type_from_context(["超市消费"], ["摘要"]) == "支出"
    assert bills_module._infer_generic_import_type_from_context(["工资补发", "还款"], ["摘要", "备注"]) is None
    assert bills_module._infer_generic_import_type_from_context(["工资补发"], [""]) == "收入"

    assert bills_module._find_generic_import_header_index(["", "   "], [""]) is None
    assert bills_module._find_generic_import_header_index(["交易时间"], ["不存在"]) is None
    assert bills_module._find_generic_import_header_index(["交易时间"], ["交易时间"]) == 0

    assert bills_module._build_generic_import_trade_time(["2026-03-01"], [], {"1": "bad"}, "2026-03-01") == "2026-03-01"
    assert bills_module._build_generic_import_trade_time(["2026-03-01", "08:30:00"], ["交易日期", "交易时间"], {"1": 9}, "") == "2026-03-01 08:30:00"
    assert bills_module._build_generic_import_trade_time(["", "08:30:00"], ["交易日期", "交易时间"], {"1": 9}, "") == "08:30:00"
    assert (
        bills_module._build_generic_import_trade_time(
            ["2026-03-01 08:30:00"], ["交易时间"], {"1": 0}, "2026-03-01 08:30:00"
        )
        == "2026-03-01 08:30:00"
    )
    assert bills_module._is_generic_import_repeated_header_row([], []) is False

    class _BadIdentifier:
        def __bool__(self) -> bool:
            return True

        def __str__(self) -> str:
            raise ValueError("bad id")

    assert bills_module._safe_int_identifier(_BadIdentifier()) is None


def test_match_account_by_name_prefers_exact_alias_then_fuzzy() -> None:
    """账户匹配应优先精确名称、其次别名，最后再做模糊匹配。"""
    accounts = [
        {"id": 1, "name": "支付宝", "aliases": "alipay, 支付宝钱包", "comment": ""},
        {"id": 2, "name": "支付宝红包", "aliases": "", "comment": "hongbao, gift"},
        {"id": 3, "name": "微信支付", "aliases": "wechat pay", "comment": "微信"},
    ]

    assert bills_module._match_account_by_name(accounts, "支付宝")["id"] == 1
    assert bills_module._match_account_by_name(accounts, "支付宝钱包")["id"] == 1
    assert bills_module._match_account_by_name(accounts, "gift")["id"] == 2
    assert bills_module._match_account_by_name(accounts, "微信")["id"] == 3
    assert bills_module._match_account_by_name(accounts, "支付宝红包账户")["id"] == 2
    assert bills_module._match_account_by_name(accounts, "") is None


def test_build_import_mapping_suggestion_combines_keyword_and_history_scores() -> None:
    """列映射建议应综合关键词和历史模板权重输出稳定结果。"""
    headers = ["交易时间", "收支", "金额", "付款账户", "摘要"]
    configs = [
        {
            "field_mappings": {"columnMapping": {"1": 0, "3": 1, "8": 2, "6": 3, "14": 4}},
            "sample_headers": headers,
            "use_count": 12,
        }
    ]
    sample_rows = [["2026-01-01 08:30:00", "支出", "12.34", "支付宝", "早餐"]]

    suggestion = bills_module._build_import_mapping_suggestion(headers, configs, sample_rows)

    assert suggestion["includeHeader"] is True
    assert suggestion["columnMapping"] == {"1": 0, "3": 1, "8": 2, "6": 3, "14": 4}
    assert suggestion["transactionTypeMapping"]["支出"] == 3
    assert any(item["score"] >= 10 for item in suggestion["suggestions"])


def test_build_import_mapping_suggestion_skips_duplicate_column_type_candidates_and_empty_time() -> None:
    """列映射建议应跳过重复列类型候选，并处理空时间值。"""
    suggestion = bills_module._build_import_mapping_suggestion(
        ["金额", "交易金额", "备注"],
        configs=[],
        sample_rows=[],
    )

    assert suggestion["columnMapping"]["8"] in {0, 1}
    assert len([item for item in suggestion["suggestions"] if item["columnType"] == 8]) == 1
    assert isinstance(bills_module._parse_generic_import_time(""), int)


def test_helper_scoring_and_matching_cover_empty_keyword_low_score_and_no_context_paths(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """评分与匹配 helper 应覆盖空关键字、低分候选和无有效上下文路径。"""
    monkeypatch.setattr(bills_module, "IMPORT_COLUMN_TYPE_KEYWORDS", {1: ("", "交易时间")})
    assert bills_module._score_header_keyword_match("交易时间", 1) >= bills_module.IMPORT_HEADER_EXACT_SCORE

    monkeypatch.setattr(bills_module, "IMPORT_COLUMN_TYPE_KEYWORDS", {1: ("交易时间",)})
    monkeypatch.setattr(bills_module, "_score_header_keyword_match", lambda *_args, **_kwargs: 1.0)
    low_score_suggestion = bills_module._build_import_mapping_suggestion(["低分列"], configs=[], sample_rows=[])
    assert low_score_suggestion["columnMapping"] == {}

    assert bills_module._auto_detect_and_normalize_amount("1.234.567,89") == "1234567.89"
    assert bills_module._infer_generic_import_type_from_context(["", "   "], ["摘要"]) is None
    assert bills_module._find_generic_import_header_index(["交易时间"], [""]) is None
    assert bills_module._match_account_by_name([{"name": "", "aliases": "", "comment": ""}], "不存在") is None


def test_parse_form_helpers_and_identifier_helpers_cover_json_bool_and_id_edges() -> None:
    """表单 JSON/布尔解析与安全 ID 转换应覆盖常见边界。"""
    assert bills_module._parse_json_form_field('{"a": 1}', {}) == {"a": 1}
    assert bills_module._parse_json_form_field("{bad-json}", {"fallback": True}) == {"fallback": True}
    assert bills_module._parse_json_form_field("", [1]) == [1]

    assert bills_module._parse_bool_form_field("true") is True
    assert bills_module._parse_bool_form_field("On") is True
    assert bills_module._parse_bool_form_field("0") is False
    assert bills_module._parse_bool_form_field("", default=True) is True

    assert bills_module._safe_int_identifier("123") == 123
    assert bills_module._safe_int_identifier("001") == 1
    assert bills_module._safe_int_identifier("12a") is None
    assert bills_module._safe_int_identifier(None) is None


def test_type_mapping_helpers_cover_numeric_keyword_and_fallback_paths() -> None:
    """交易类型映射 helper 应覆盖数字、关键字与兜底支出分支。"""
    assert bills_module._frontend_type_number_to_backend_label("1") == "余额调整"
    assert bills_module._frontend_type_number_to_backend_label("income") == "收入"
    assert bills_module._frontend_type_number_to_backend_label("退款") == "收入"
    assert bills_module._frontend_type_number_to_backend_label("mystery") == "支出"

    mapping = {"支出": 3, "收入": 2, "TRANSFER": 4}
    assert bills_module._map_generic_import_type("支出", mapping) == "支出"
    assert bills_module._map_generic_import_type("income", mapping) == "收入"
    assert bills_module._map_generic_import_type("transfer", mapping) == "转账"
    assert bills_module._map_generic_import_type("", {}) == "支出"


def test_generic_import_header_helpers_cover_search_trade_time_and_repeated_header_detection() -> None:
    """表头查找、时间拼装和重复表头识别应保持稳定。"""
    headers = ["交易日期", "交易时间", "金额", "账户", "备注"]
    row = ["2026-03-01", "08:30:00", "12.34", "支付宝", "早餐"]
    column_mapping = {"1": 0, "8": 2, "6": 3, "14": 4}

    assert bills_module._find_generic_import_header_index(headers, ["交易时间"]) == 1
    assert bills_module._find_generic_import_header_index(headers, ["时间"], exclude={1}) is None
    assert bills_module._build_generic_import_trade_time(row, headers, column_mapping, row[0]) == "2026-03-01 08:30:00"
    assert bills_module._build_generic_import_trade_time(row, headers, {"1": 1}, row[1]) == "08:30:00"
    assert bills_module._is_generic_import_repeated_header_row(headers, headers) is True
    assert bills_module._is_generic_import_repeated_header_row(row, headers) is False


def test_resolve_generic_import_type_and_amount_supports_direction_columns_context_and_signed_amounts() -> None:
    """类型与金额推断应兼容方向列、上下文推断和单列正负数。"""
    headers = ["交易日期", "收/支", "收入金额", "支出金额", "摘要", "金额"]
    column_mapping = {"1": 0, "8": 5, "14": 4}

    expense_row = ["2026-03-01", "", "", "56.70", "购买早餐", ""]
    expense_type, expense_amount, expense_raw_type = bills_module._resolve_generic_import_type_and_amount(
        expense_row,
        headers=headers,
        column_mapping=column_mapping,
        transaction_type_mapping={},
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
    )
    assert expense_type == "支出"
    assert expense_amount == pytest.approx(56.70)
    assert expense_raw_type == "支出"

    income_row = ["2026-03-02", "", "88.00", "", "工资补发", ""]
    income_type, income_amount, income_raw_type = bills_module._resolve_generic_import_type_and_amount(
        income_row,
        headers=headers,
        column_mapping=column_mapping,
        transaction_type_mapping={},
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
    )
    assert income_type == "收入"
    assert income_amount == pytest.approx(88.00)
    assert income_raw_type == "收入"

    signed_headers = ["交易时间", "金额", "摘要"]
    signed_row = ["2026-03-03 09:00:00", "-100.50", "超市消费"]
    signed_type, signed_amount, signed_raw_type = bills_module._resolve_generic_import_type_and_amount(
        signed_row,
        headers=signed_headers,
        column_mapping={"1": 0, "8": 1, "14": 2},
        transaction_type_mapping={},
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        amount_column_has_signed_values=True,
    )
    assert signed_type == "支出"
    assert signed_amount == pytest.approx(100.50)
    assert signed_raw_type == ""

    transfer_row = ["2026-03-04 10:00:00", "12.00", "工资补发"]
    transfer_type, _, _ = bills_module._resolve_generic_import_type_and_amount(
        transfer_row,
        headers=signed_headers,
        column_mapping={"1": 0, "3": 1, "8": 1, "14": 2},
        transaction_type_mapping={"12.00": 4},
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
    )
    assert transfer_type == "收入"


def test_generic_import_signed_amount_column_detection_requires_both_positive_and_negative_samples() -> None:
    """单金额列正负号探测需要同时看到正值和负值。"""
    rows = [["100.00"], ["-20.00"], ["0"]]
    assert bills_module._generic_import_amount_column_has_signed_values(rows, {"8": 0}) is True
    assert bills_module._generic_import_amount_column_has_signed_values([["100.00"], ["0"]], {"8": 0}) is False
    assert bills_module._generic_import_amount_column_has_signed_values(rows, {}) is False
    assert bills_module._generic_import_amount_column_has_signed_values(rows, {"8": "bad"}) is False
    assert bills_module._generic_import_amount_column_has_signed_values([[]], {"8": 0}) is False


def test_resolve_generic_import_type_and_amount_covers_positive_prefix_and_signed_positive_inference() -> None:
    """类型金额推断应覆盖正号前缀和无显式类型下的带符号收入推断。"""
    plus_type, plus_amount, plus_raw_type = bills_module._resolve_generic_import_type_and_amount(
        ["+88.00"],
        headers=[],
        column_mapping={"8": 0, "3": "bad"},
        transaction_type_mapping={},
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        amount_column_has_signed_values=False,
    )
    assert plus_type == "收入"
    assert plus_amount == pytest.approx(88.0)
    assert plus_raw_type == ""

    inferred_type, inferred_amount, inferred_raw_type = bills_module._resolve_generic_import_type_and_amount(
        ["88.00"],
        headers=[],
        column_mapping={"8": 0},
        transaction_type_mapping={},
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        amount_column_has_signed_values=True,
    )
    assert inferred_type == "收入"
    assert inferred_amount == pytest.approx(88.0)
    assert inferred_raw_type == ""


def test_mapping_payload_and_import_item_helpers_cover_account_category_and_fallback_branches(tmp_path: Path) -> None:
    """映射 payload、导入项转换与图片 data URL helper 应覆盖关键分支。"""
    accounts = [
        {"id": 1, "name": "支付宝", "currency": "CNY"},
        {"id": 2, "name": "招商银行", "currency": "USD"},
        {"id": None, "name": "忽略账户", "currency": "CNY"},
    ]
    categories = [
        {"id": 10, "main_category": "餐饮", "sub_category": "早餐"},
        {"id": 11, "main_category": "工资", "sub_category": ""},
    ]

    account_mapping = bills_module._build_account_mapping_payload(accounts)
    category_mapping = bills_module._build_category_mapping_payload(categories)
    assert account_mapping["name_to_id"] == {"支付宝": 1, "招商银行": 2}
    assert category_mapping["name_to_id"] == {("餐饮", "早餐"): 10, ("工资", ""): 11}

    base_bill = {
        "type": "转账",
        "main_category": "餐饮",
        "sub_category": "早餐",
        "amount": 12.34,
        "destination_amount": 56.78,
        "trade_time": "2026-03-01 08:30:00",
        "account": "支付宝",
        "account_currency": "CNY",
        "related_account": "招商银行",
        "related_account_currency": "USD",
        "description": "早餐",
        "payment_method": "支付宝",
        "original_tag_names": "should-be-reset",
        "is_manually_annotated": 1,
    }
    converted = bills_module._convert_bill_to_import_item(base_bill)
    assert converted["type"] == 4
    assert converted["sourceAmount"] == 1234
    assert converted["destinationAmount"] == 5678
    assert converted["originalCategoryName"] == "餐饮/早餐"
    assert converted["originalTagNames"] == []
    assert converted["isManuallyAnnotated"] is True

    enriched_bill = dict(base_bill)
    enriched_bill.update({"source_account_id": 1, "destination_account_id": 2})
    converted_with_mapping = bills_module._convert_bill_to_import_item_with_mappings(
        enriched_bill,
        account_mappings=account_mapping,
        category_mappings=category_mapping,
    )
    assert converted_with_mapping["categoryId"] == "10"
    assert converted_with_mapping["sourceAccountId"] == "1"
    assert converted_with_mapping["destinationAccountId"] == "2"
    assert converted_with_mapping["accountName"] == "支付宝"
    assert converted_with_mapping["originalDestinationAccountCurrency"] == "USD"

    main_only_bill = {
        "type": "支出",
        "main_category": "工资",
        "sub_category": "奖金",
        "amount": 1,
        "date": "2026-03-01",
        "source_account_id": "999",
        "destination_account_id": "0",
        "original_tag_names": [],
    }
    main_only_item = bills_module._convert_bill_to_import_item_with_mappings(
        main_only_bill,
        account_mappings=account_mapping,
        category_mappings=category_mapping,
    )
    assert main_only_item["categoryId"] == "11"

    unknown_type_bill = {"type": "未知类型", "amount": 1, "date": "2026-03-01", "original_tag_names": []}
    unknown_item = bills_module._convert_bill_to_import_item(unknown_type_bill)
    assert unknown_item["type"] == 3
    assert unknown_item["sourceAmount"] == 100

    image_path = tmp_path / "avatar.unknown"
    image_path.write_bytes(b"binary-image")
    assert bills_module._build_picture_data_url(image_path).startswith("data:application/octet-stream;base64,")


class _AsyncRunnerLoop:
    """Use asyncio.run to emulate the ad-hoc event loop object bills helpers expect."""

    @staticmethod
    def run_until_complete(coro: Any) -> Any:
        return asyncio.run(coro)


class _FakeReviewDB:
    def __init__(self) -> None:
        self.accounts = [
            {"id": 1, "name": "支付宝", "currency": "CNY"},
            {"id": 2, "name": "招商银行", "currency": "USD"},
        ]
        self.categories = [
            {"id": 10, "main_category": "餐饮", "sub_category": "早餐"},
            {"id": 11, "main_category": "学习", "sub_category": "课本"},
        ]

    async def get_all_accounts(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.accounts]

    async def get_all_categories(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.categories]


class _FakeReviewCategoryEngine:
    def __init__(self) -> None:
        self.loaded_with: list[tuple[Any, int]] = []

    async def load_rules_from_db(self, db: Any, *, user_id: int) -> None:
        self.loaded_with.append((db, user_id))

    async def batch_match_categories(self, bills: list[dict[str, Any]], types: Any = None) -> list[dict[str, Any]]:
        _ = types
        updated: list[dict[str, Any]] = []
        for bill in bills:
            copied = dict(bill)
            copied["main_category"] = "系统分类"
            copied["sub_category"] = "系统子类"
            updated.append(copied)
        return updated


class _FakeReviewBillService:
    def __init__(self) -> None:
        self.category_engine = _FakeReviewCategoryEngine()

    async def _apply_import_learning_rules(
        self,
        bills: list[dict[str, Any]],
        *,
        user_id: int,
        type_only: bool,
        record_usage: bool,
    ) -> int:
        _ = (user_id, record_usage)
        if type_only:
            for bill in bills:
                bill["type"] = "收入"
            return 2

        for bill in bills:
            if bill.get("description") == "keep-learning":
                bill["main_category"] = "学习"
                bill["sub_category"] = "课本"
        return 3

    async def _detect_investment_candidates(self, bills: list[dict[str, Any]], *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        for bill in bills:
            if bill.get("description") == "investment":
                bill["type"] = "投资"
        return bills

    async def _match_accounts(self, bills: list[dict[str, Any]], user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        for index, bill in enumerate(bills, start=1):
            bill["source_account_id"] = index
        return bills

    async def _detect_cash_transfers(self, bills: list[dict[str, Any]], *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        for bill in bills:
            bill.setdefault("destination_account_id", 0)
        return bills


class _FakePrepareDB:
    def __init__(self, *, accounts: list[dict[str, Any]], category: dict[str, Any] | None = None) -> None:
        self.accounts = accounts
        self.category = category

    async def get_all_accounts(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.accounts]

    async def get_category_by_id(self, category_id: int, *, user_id: int) -> dict[str, Any] | None:
        _ = (category_id, user_id)
        return dict(self.category) if self.category else None


class _FakePrepareAdapter:
    def __init__(self, backend_data: dict[str, Any], metadata: dict[str, Any]) -> None:
        self.backend_data = backend_data
        self.metadata = metadata

    def frontend_to_backend(self, _frontend_data: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
        return dict(self.backend_data), dict(self.metadata)


class _FakePrepareCategoryEngine:
    def __init__(self, result: tuple[str | None, str | None]) -> None:
        self.result = result

    def match_category(self, _backend_data: dict[str, Any]) -> tuple[str | None, str | None]:
        return self.result


class _FakeCreateDB:
    def __init__(self, *, created_bill_id: int) -> None:
        self.created_bill_id = created_bill_id
        self.saved_tags: list[tuple[int, list[int], int]] = []
        self.synced_accounts: list[int] = []

    async def create_bill(self, payload: dict[str, Any], *, user_id: int) -> int:
        _ = (payload, user_id)
        return self.created_bill_id

    async def add_tags_to_bill(self, bill_id: int, tag_ids: list[int], *, user_id: int) -> None:
        self.saved_tags.append((bill_id, list(tag_ids), user_id))

    async def sync_account_balance(self, account_id: int) -> None:
        self.synced_accounts.append(account_id)

    async def get_bill_by_id(self, bill_id: int, *, user_id: int) -> dict[str, Any]:
        _ = user_id
        return {"id": bill_id, "source_account_id": 11, "destination_account_id": 22}

    async def get_tags_for_bill(self, bill_id: int, *, user_id: int) -> list[dict[str, Any]]:
        _ = (bill_id, user_id)
        return [{"id": 7, "name": "早餐"}]


class _FakeCreateAdapter:
    async def backend_to_frontend(self, bill: dict[str, Any], *, tags: list[dict[str, Any]]) -> dict[str, Any]:
        return {"id": str(bill["id"]), "tagCount": len(tags)}


class _FakeBalanceSyncDB:
    def __init__(self, *, raise_on_sync: bool = False) -> None:
        self.raise_on_sync = raise_on_sync
        self.synced_accounts: list[int] = []

    async def sync_account_balance(self, account_id: int) -> None:
        if self.raise_on_sync:
            raise RuntimeError("sync failed")
        self.synced_accounts.append(account_id)


class _FakeCategoryLookupDB:
    def __init__(self, categories: list[dict[str, Any]]) -> None:
        self.categories = categories

    async def get_all_categories(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.categories]


def test_prepare_import_review_bills_restores_explicit_values_and_builds_mapping_payloads() -> None:
    """导入预览富化应恢复显式字段，并输出账户/分类映射 payload。"""
    db = _FakeReviewDB()
    service = _FakeReviewBillService()
    bills = [
        {
            "trade_time": "2026-03-01 08:30:00",
            "type": "支出",
            "main_category": "餐饮",
            "sub_category": "早餐",
            "original_category": "餐饮/早餐",
            "description": "explicit-known",
            "_import_has_explicit_type": True,
            "_import_has_explicit_category": True,
        },
        {
            "trade_time": "2026-03-02 09:00:00",
            "type": "支出",
            "main_category": "失效分类",
            "sub_category": "旧子类",
            "original_category": "失效分类/旧子类",
            "description": "explicit-unknown",
            "_import_has_explicit_category": True,
        },
        {
            "trade_time": "2026-03-03 10:00:00",
            "type": "支出",
            "description": "keep-learning",
        },
    ]

    enriched_bills, account_mapping, category_mapping, stats = asyncio.run(
        bills_module._prepare_import_review_bills(bills, db, service, 9)
    )

    assert stats == {"learning_seeded": 2, "learning_replayed": 3}
    assert service.category_engine.loaded_with and service.category_engine.loaded_with[0][1] == 9

    assert enriched_bills[0]["date"] == "2026-03-01 08:30:00"
    assert enriched_bills[0]["type"] == "支出"
    assert enriched_bills[0]["main_category"] == "餐饮"
    assert enriched_bills[0]["sub_category"] == "早餐"
    assert enriched_bills[0]["source_account_id"] == 1

    assert enriched_bills[1]["main_category"] == ""
    assert enriched_bills[1]["sub_category"] == ""
    assert enriched_bills[1]["original_category"] == "失效分类/旧子类"

    assert enriched_bills[2]["main_category"] == "学习"
    assert enriched_bills[2]["sub_category"] == "课本"
    assert account_mapping["name_to_id"] == {"支付宝": 1, "招商银行": 2}
    assert category_mapping["name_to_id"] == {("餐饮", "早餐"): 10, ("学习", "课本"): 11}


def test_parse_import_file_with_column_mapping_skips_repeated_headers_and_invalid_dates(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """列映射解析应跳过重复表头与坏日期，且输出可直接入 staging 的 date 字段。"""
    monkeypatch.setattr(
        bills_module,
        "_load_generic_import_rows",
        lambda *_args, **_kwargs: (
            [
                ["交易时间", "交易类型", "金额", "对方金额", "账户", "标签", "备注"],
                ["交易时间", "交易类型", "金额", "对方金额", "账户", "标签", "备注"],
                ["", "", "", "", "", "", ""],
                ["2026-03-01 08:30:00", "支出", "12.34", "0", "支付宝", "早餐|午餐", "正常行"],
                ["不是日期", "支出", "11.00", "0", "支付宝", "午餐", "坏日期行"],
            ],
            "utf-8",
            ",",
        ),
    )

    items, actual_encoding, actual_delimiter = bills_module._parse_import_file_with_column_mapping(
        Path("dummy.csv"),
        column_mapping={"1": 0, "3": 1, "8": 2, "11": 3, "6": 4, "13": 5, "14": 6},
        transaction_type_mapping={"支出": 3},
        has_header_line=True,
        time_format="%Y-%m-%d %H:%M:%S",
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        tag_separator="|",
        file_encoding="utf-8",
        delimiter=",",
    )

    assert actual_encoding == "utf-8"
    assert actual_delimiter == ","
    assert len(items) == 1
    assert items[0]["date"] == "2026-03-01 08:30:00"
    assert items[0]["trade_time"] == "2026-03-01 08:30:00"
    assert items[0]["description"] == "正常行"
    assert items[0]["original_tag_names"] == ["早餐", "午餐"]


def test_parse_import_file_with_column_mapping_covers_empty_rows_and_zero_amount_skips(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """列映射解析应覆盖空输入与金额全零跳过路径。"""
    monkeypatch.setattr(
        bills_module,
        "_load_generic_import_rows",
        lambda *_args, **_kwargs: ([], "utf-8", ","),
    )
    empty_items, empty_encoding, empty_delimiter = bills_module._parse_import_file_with_column_mapping(
        Path("empty.csv"),
        column_mapping={"1": 0},
        transaction_type_mapping={},
        has_header_line=True,
        time_format="",
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        tag_separator=";",
        file_encoding="utf-8",
        delimiter=",",
    )
    assert empty_items == []
    assert empty_encoding == "utf-8"
    assert empty_delimiter == ","

    monkeypatch.setattr(
        bills_module,
        "_load_generic_import_rows",
        lambda *_args, **_kwargs: (
            [["交易时间", "收/支", "金额", "对方金额"], ["2026-03-01", "支出", "0", "0"]],
            "utf-8",
            ",",
        ),
    )
    zero_items, _, _ = bills_module._parse_import_file_with_column_mapping(
        Path("zero.csv"),
        column_mapping={"1": 0, "3": 1, "8": 2, "11": 3},
        transaction_type_mapping={},
        has_header_line=True,
        time_format="",
        amount_decimal_separator=".",
        amount_digit_grouping_symbol="",
        tag_separator=";",
        file_encoding="utf-8",
        delimiter=",",
    )
    assert zero_items == []


def test_prepare_import_review_bills_without_db_or_service_returns_empty_mappings() -> None:
    """当 db 或 service 缺失时，导入预览准备应优雅返回空映射。"""
    enriched_bills, account_mapping, category_mapping, stats = asyncio.run(
        bills_module._prepare_import_review_bills([{"description": "早餐"}], None, None, 1)
    )
    assert enriched_bills == [{"description": "早餐"}]
    assert account_mapping == {"id_to_account": {}, "name_to_id": {}, "id_to_name": {}}
    assert category_mapping == {"id_to_category": {}, "name_to_id": {}}
    assert stats == {"learning_seeded": 0, "learning_replayed": 0}


def test_parse_import_file_with_auto_mapping_covers_empty_no_match_and_success_paths(tmp_path: Path) -> None:
    """自动映射解析应覆盖空输入、无匹配和成功三条路径。"""
    empty_file = tmp_path / "empty.csv"
    empty_file.write_text("", encoding="utf-8")
    empty_items, empty_suggestion, _, _ = bills_module._parse_import_file_with_auto_mapping(empty_file, configs=[])
    assert empty_items == []
    assert empty_suggestion == {"columnMapping": {}, "transactionTypeMapping": {}}

    unknown_file = tmp_path / "unknown.csv"
    unknown_file.write_text("foo,bar\n1,2\n", encoding="utf-8")
    unknown_items, unknown_suggestion, _, _ = bills_module._parse_import_file_with_auto_mapping(unknown_file, configs=[])
    assert unknown_items == []
    assert unknown_suggestion["columnMapping"] == {}

    mapped_file = tmp_path / "mapped.csv"
    mapped_file.write_text(
        "导出说明,,,,\n交易时间,交易类型,金额,账户,摘要\n2026-03-01 08:30:00,支出,12.34,支付宝,早餐\n",
        encoding="utf-8",
    )
    configs = [
        {
            "field_mappings": {"columnMapping": {"1": 0, "3": 1, "8": 2, "6": 3, "14": 4}},
            "sample_headers": ["交易时间", "交易类型", "金额", "账户", "摘要"],
            "use_count": 10,
        }
    ]
    mapped_items, mapped_suggestion, actual_encoding, actual_delimiter = bills_module._parse_import_file_with_auto_mapping(
        mapped_file,
        configs=configs,
    )
    assert actual_encoding == "utf-8"
    assert actual_delimiter == ","
    assert mapped_suggestion["columnMapping"] == {"1": 0, "3": 1, "8": 2, "6": 3, "14": 4}
    assert len(mapped_items) == 1
    assert mapped_items[0]["account"] == "支付宝"
    assert mapped_items[0]["description"] == "早餐"


def test_prepare_backend_bill_for_create_handles_investment_category_lookup_and_account_fallback() -> None:
    """创建前预处理应补齐投资目标账户、分类和默认源账户。"""
    db = _FakePrepareDB(
        accounts=[
            {"id": 11, "name": "默认账户"},
            {"id": 22, "name": "投资账户"},
        ],
        category={"main_category": "投资理财", "sub_category": "基金"},
    )
    adapter = _FakePrepareAdapter(
        backend_data={
            "type": "投资",
            "amount": 88.0,
            "payment_method": "支付宝",
            "source_account_id": 0,
            "destination_account_id": 0,
        },
        metadata={"auto_invest_account": True, "category_id": "77"},
    )

    backend_data, metadata = bills_module._prepare_backend_bill_for_create(
        {"comment": "基金买入"},
        db,
        _FakePrepareCategoryEngine((None, None)),
        adapter,
        _AsyncRunnerLoop(),
        1,
    )

    assert metadata == {"auto_invest_account": True, "category_id": "77"}
    assert backend_data["description"] == "基金买入"
    assert backend_data["counterparty"] == "基金买入"
    assert backend_data["source_account_id"] == 11
    assert backend_data["destination_account_id"] == 22
    assert backend_data["destination_amount"] == 88.0
    assert backend_data["main_category"] == "投资理财"
    assert backend_data["sub_category"] == "基金"
    assert backend_data["created_at"]
    assert backend_data["updated_at"]


def test_prepare_backend_bill_for_create_supports_rule_match_and_missing_account_failure() -> None:
    """创建前预处理应支持规则匹配兜底，并在无可用账户时抛出明确错误。"""
    matched_backend_data, _ = bills_module._prepare_backend_bill_for_create(
        {},
        _FakePrepareDB(accounts=[{"id": 1, "name": "微信"}], category=None),
        _FakePrepareCategoryEngine(("餐饮", "早餐")),
        _FakePrepareAdapter(
            backend_data={
                "type": "支出",
                "amount": 12.0,
                "payment_method": "微信",
                "source_account_id": 1,
            },
            metadata={"category_id": "bad"},
        ),
        _AsyncRunnerLoop(),
        1,
    )
    assert matched_backend_data["description"] == "微信"
    assert matched_backend_data["counterparty"] == "微信"
    assert matched_backend_data["main_category"] == "餐饮"
    assert matched_backend_data["sub_category"] == "早餐"

    with pytest.raises(ValueError, match="No account available"):
        bills_module._prepare_backend_bill_for_create(
            {},
            _FakePrepareDB(accounts=[], category=None),
            _FakePrepareCategoryEngine((None, None)),
            _FakePrepareAdapter(
                backend_data={"type": "支出", "amount": 1.0, "payment_method": "支付宝", "source_account_id": 0},
                metadata={},
            ),
            _AsyncRunnerLoop(),
            1,
        )

    default_category_backend_data, _ = bills_module._prepare_backend_bill_for_create(
        {},
        _FakePrepareDB(accounts=[{"id": 1, "name": "微信"}], category=None),
        _FakePrepareCategoryEngine((None, None)),
        _FakePrepareAdapter(
            backend_data={
                "type": "未知类型",
                "amount": 9.9,
                "payment_method": "微信",
                "source_account_id": 1,
            },
            metadata={},
        ),
        _AsyncRunnerLoop(),
        1,
    )
    assert default_category_backend_data["main_category"] == "其他"
    assert default_category_backend_data["sub_category"] == ""


def test_prepare_backend_bill_for_create_uses_default_mapping_and_handles_missing_invest_target() -> None:
    """创建前预处理应覆盖默认分类映射与投资目标账户缺失分支。"""

    class _LocalLoop:
        @staticmethod
        def run_until_complete(coro: Any) -> Any:
            return asyncio.run(coro)

    class _FallbackDb:
        async def get_all_accounts(self, *, user_id: int) -> list[dict[str, Any]]:
            _ = user_id
            return [{"id": 5, "name": "普通账户"}]

        async def get_category_by_id(self, category_id: int, *, user_id: int) -> dict[str, Any] | None:
            _ = (category_id, user_id)
            return None

    fallback_db = _FallbackDb()
    category_engine = _FakePrepareCategoryEngine((None, None))

    invest_backend, _ = bills_module._prepare_backend_bill_for_create(
        {"comment": "投资测试"},
        fallback_db,
        category_engine,
        _FakePrepareAdapter(
            backend_data={
                "type": "投资",
                "amount": 66.0,
                "payment_method": "现金",
                "source_account_id": 5,
                "destination_account_id": 0,
            },
            metadata={"auto_invest_account": True},
        ),
        _LocalLoop(),
        1,
    )
    assert invest_backend["destination_account_id"] == 0
    assert invest_backend["main_category"] == bills_module.DEFAULT_BILL_CATEGORY_MAPPING["投资"][0]

    mapped_backend, _ = bills_module._prepare_backend_bill_for_create(
        {"comment": "收入测试"},
        fallback_db,
        category_engine,
        _FakePrepareAdapter(
            backend_data={
                "type": "收入",
                "amount": 8.8,
                "payment_method": "现金",
                "source_account_id": 5,
            },
            metadata={},
        ),
        _LocalLoop(),
        1,
    )
    assert (mapped_backend["main_category"], mapped_backend["sub_category"]) == bills_module.DEFAULT_BILL_CATEGORY_MAPPING[
        "收入"
    ]


def test_create_bill_and_build_response_handles_success_and_failed_creation() -> None:
    """创建账单后处理应保存标签、同步余额，并在创建失败时抛错。"""
    db = _FakeCreateDB(created_bill_id=123)
    bill_id, frontend_bill = bills_module._create_bill_and_build_response(
        {"source_account_id": 11, "destination_account_id": 22},
        {"tag_ids": [7]},
        db,
        _FakeCreateAdapter(),
        _AsyncRunnerLoop(),
        5,
    )
    assert bill_id == 123
    assert frontend_bill == {"id": "123", "tagCount": 1}
    assert db.saved_tags == [(123, [7], 5)]
    assert db.synced_accounts == [11, 22]

    with pytest.raises(RuntimeError, match="Failed to create bill"):
        bills_module._create_bill_and_build_response(
            {"source_account_id": 11, "destination_account_id": 22},
            {},
            _FakeCreateDB(created_bill_id=0),
            _FakeCreateAdapter(),
            _AsyncRunnerLoop(),
            5,
        )


def test_sync_balance_category_lookup_and_date_parsing_helpers_cover_remaining_simple_paths() -> None:
    """余额同步、分类查找和日期解析 helper 应覆盖成功、失败与回退路径。"""
    sync_db = _FakeBalanceSyncDB()
    asyncio.run(bills_module.sync_balances_for_bill(sync_db, {"source_account_id": 11, "destination_account_id": 22}))
    assert sync_db.synced_accounts == [11, 22]

    exploding_sync_db = _FakeBalanceSyncDB(raise_on_sync=True)
    asyncio.run(bills_module.sync_balances_for_bill(exploding_sync_db, {"source_account_id": 11}))
    assert exploding_sync_db.synced_accounts == []

    category_db = _FakeCategoryLookupDB(
        [
            {"id": 10, "main_category": "餐饮", "sub_category": "早餐"},
            {"id": 11, "main_category": "工资", "sub_category": ""},
        ]
    )
    assert bills_module.get_category_id_from_names("餐饮", "早餐", category_db, user_id=9) == "10"
    assert bills_module.get_category_id_from_names("工资", "", category_db, user_id=9) == "11"
    assert bills_module.get_category_id_from_names("不存在", "", category_db, user_id=9) == "0"
    assert bills_module.get_category_id_from_names("", "", category_db, user_id=9) == "0"

    assert bills_module.parse_bill_date("2026-03-01 08:30:00").strftime("%Y-%m-%d %H:%M:%S") == "2026-03-01 08:30:00"
    assert bills_module.parse_bill_date("2026-03-01").strftime("%Y-%m-%d") == "2026-03-01"
    fallback_date = bills_module.parse_bill_date("bad-date")
    assert isinstance(fallback_date, bills_module.datetime)
