from __future__ import annotations

import json

from bill_analyser.core import investment_settings as investment_settings_module



def test_dedupe_keywords_trims_values_and_removes_case_insensitive_duplicates() -> None:
    """关键词去重应保留顺序、去空值并按大小写不敏感去重。"""
    assert investment_settings_module._dedupe_keywords([
        " 基金 ",
        "基金",
        "ETF",
        "etf",
        "",
        "  ",
        "理财",
    ]) == ["基金", "ETF", "理财"]



def test_normalize_keyword_list_handles_none_sequences_json_and_delimiters() -> None:
    """关键词标准化应兼容空值、序列、JSON 字符串和多分隔符文本。"""
    assert investment_settings_module.normalize_keyword_list(None, ["基金", "基金", "ETF"]) == ["基金", "ETF"]
    assert investment_settings_module.normalize_keyword_list(("基金", " ETF ", "基金")) == ["基金", "ETF"]
    assert investment_settings_module.normalize_keyword_list('["基金", " ETF ", ""]') == ["基金", "ETF"]
    assert investment_settings_module.normalize_keyword_list("基金，ETF|REITs\n理财;定投；组合、黄金") == [
        "基金",
        "ETF",
        "REITs",
        "理财",
        "定投",
        "组合",
        "黄金",
    ]



def test_normalize_keyword_list_handles_invalid_json_and_blank_text() -> None:
    """非法 JSON 文本和空白字符串也应稳定回退。"""
    assert investment_settings_module.normalize_keyword_list("[bad json") == ["[bad json"]
    assert investment_settings_module.normalize_keyword_list("   ", ["理财", "理财", "基金"]) == ["理财", "基金"]



def test_serialize_keyword_list_returns_compact_json_string() -> None:
    """序列化结果应等于标准化后的关键词列表。"""
    serialized = investment_settings_module.serialize_keyword_list(["基金", " ETF ", "基金"])
    assert json.loads(serialized) == ["基金", "ETF"]



def test_build_user_investment_keyword_settings_merges_user_values_with_defaults() -> None:
    """用户投资设置应对缺失字段使用默认值，并标准化自定义关键词。"""
    default_settings = investment_settings_module.build_user_investment_keyword_settings(None)
    assert default_settings["platform_keywords"] == investment_settings_module.DEFAULT_INVESTMENT_PLATFORM_KEYWORDS
    assert default_settings["product_keywords"] == investment_settings_module.DEFAULT_INVESTMENT_PRODUCT_KEYWORDS
    assert default_settings["exclude_keywords"] == investment_settings_module.DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS

    custom_settings = investment_settings_module.build_user_investment_keyword_settings(
        {
            "investment_platform_keywords": "蚂蚁财富, 招银理财, 蚂蚁财富",
            "investment_product_keywords": '["基金", "ETF", "基金"]',
        }
    )
    assert custom_settings == {
        "platform_keywords": ["蚂蚁财富", "招银理财"],
        "product_keywords": ["基金", "ETF"],
        "exclude_keywords": investment_settings_module.DEFAULT_INVESTMENT_EXCLUDE_KEYWORDS,
    }
