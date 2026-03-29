from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

from typing import Any

import pytest

from bill_analyser.core import category_engine as category_engine_module
from bill_analyser.core.category_engine import CategoryEngine, KeywordMatcher
from bill_analyser.utils.constants import TransactionType


class FakeCategoryDB:
    """Minimal async DB stub for category-engine tests."""

    def __init__(self, categories: list[dict[str, Any]] | None = None, error: Exception | None = None) -> None:
        self.categories = categories or []
        self.error = error
        self.user_calls: list[int] = []

    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
        self.user_calls.append(user_id)
        if self.error is not None:
            raise self.error
        return list(self.categories)



def test_keyword_matcher_supports_or_not_and_regex_extract_and_cache_lifecycle() -> None:
    """关键词匹配器应覆盖复杂语法、正向关键词提取和缓存生命周期。"""
    matcher = KeywordMatcher()

    assert matcher.parse_and_match("滴滴打车", "OR:滴滴|快的&AND:打车&NOT:退款") is True
    assert matcher.parse_and_match("滴滴打车退款", "OR:滴滴|快的&AND:打车&NOT:退款") is False
    assert matcher.parse_and_match("曹操专车", "REGEX:^曹操.*车$") is True
    assert matcher.parse_and_match("美团外卖", "REGEX:^曹操.*车$") is False
    assert matcher.parse_and_match("任意文本", "AND:支付|账单") is False

    compiled_once = matcher.compile_rule("OR:滴滴|快的")
    compiled_twice = matcher.compile_rule("OR:滴滴|快的")
    assert compiled_once is compiled_twice
    assert matcher.match_compiled("滴滴出行", compiled_once) is True

    invalid_regex = matcher.compile_rule("REGEX:[broken")
    assert matcher.match_compiled("任意文本", invalid_regex) is False

    extracted = matcher.extract_positive_keywords("OR:滴滴|快的&AND:打车&NOT:退款&REGEX:^曹操.*车$")
    assert extracted == ["滴滴", "快的", "打车", "regex:^曹操.*车$"]

    matcher.clear_cache()
    assert matcher._compiled_rules_cache == {}
    assert matcher._regex_cache == {}


@pytest.mark.asyncio
async def test_category_engine_loads_rules_filters_types_and_handles_failures() -> None:
    """分类引擎应按用户/类型加载规则，并在 DB 失败时安全降级。"""
    categories = [
        {
            "main_category": "餐饮",
            "sub_category": "早餐",
            "priority": 1,
            "keywords": "OR:早餐|包子",
            "type": TransactionType.EXPENSE,
        },
        {
            "main_category": "收入",
            "sub_category": "工资",
            "priority": 2,
            "keywords": "工资",
            "type": TransactionType.INCOME,
        },
        {
            "main_category": "无关键词",
            "sub_category": "忽略",
            "priority": 3,
            "keywords": "",
            "type": TransactionType.EXPENSE,
        },
    ]
    engine = CategoryEngine()
    db = FakeCategoryDB(categories)

    await engine.load_rules_from_db(db, user_id=9, types=[TransactionType.EXPENSE])
    assert engine.is_initialized is True
    assert engine.current_user_id == 9
    assert len(engine.rules) == 1
    assert engine.rules[0]["main"] == "餐饮"
    assert "OR:早餐|包子" in engine._compiled_rules
    assert db.user_calls == [9]

    failed_engine = CategoryEngine()
    await failed_engine.load_rules_from_db(FakeCategoryDB(error=RuntimeError("db down")), user_id=1)
    assert failed_engine.rules == []



def test_category_engine_matches_by_type_filters_transfer_and_default_fallbacks() -> None:
    """match_category 应覆盖支出/收入/投资/转账类型过滤与默认回退。"""
    engine = CategoryEngine()
    engine.rules = [
        {"main": "餐饮", "sub": "早餐", "priority": 1, "keywords": "OR:早餐|包子", "type": TransactionType.EXPENSE},
        {"main": "收入", "sub": "工资", "priority": 1, "keywords": "工资", "type": TransactionType.INCOME},
        {"main": "投资", "sub": "基金", "priority": 1, "keywords": "基金", "type": TransactionType.INVESTMENT},
        {"main": "转账", "sub": "内部转账", "priority": 1, "keywords": "转账", "type": TransactionType.TRANSFER},
    ]
    engine._initialized = True
    engine._precompile_rules()

    assert engine.match_category({}) == (None, None)

    expense_bill = {"counterparty": "早餐铺", "description": "早餐", "type": "支出", "amount": -20.0}
    assert engine.match_category(expense_bill) == ("餐饮", "早餐")

    income_bill = {"counterparty": "公司", "description": "工资发放", "type": "收入", "amount": 1000.0}
    assert engine.match_category(income_bill) == ("收入", "工资")

    investment_bill = {"counterparty": "基金销售", "description": "基金申购", "type": "支出", "amount": -300.0}
    assert engine.match_category(investment_bill) == ("投资", "基金")
    assert investment_bill["type"] == "投资"

    non_transfer_bill = {"counterparty": "转账提醒", "description": "转账", "type": "支出", "amount": -50.0}
    assert engine.match_category(non_transfer_bill) == (None, None)

    transfer_bill = {
        "counterparty": "内部转账",
        "description": "转账",
        "type": "支出",
        "amount": -50.0,
        "_dedup_type": "transfer",
    }
    assert engine.match_category(transfer_bill) == ("转账", "内部转账")
    assert transfer_bill["type"] == "转账"

    explicit_type_filtered = {"counterparty": "早餐铺", "description": "早餐", "type": "支出", "amount": -20.0}
    assert engine.match_category(explicit_type_filtered, types=[TransactionType.INCOME]) == (None, None)

    zero_amount_bill = {"counterparty": "未知", "description": "未知", "type": "", "amount": 0.0}
    assert engine.match_category(zero_amount_bill) == (None, None)


@pytest.mark.asyncio
async def test_batch_match_categories_tree_and_global_engine_reload() -> None:
    """批量分类、分类树和全局引擎入口应保持当前契约。"""
    categories = [
        {
            "main_category": "餐饮",
            "sub_category": "早餐",
            "priority": 1,
            "keywords": "早餐",
            "type": TransactionType.EXPENSE,
        },
        {
            "main_category": "收入",
            "sub_category": "工资",
            "priority": 1,
            "keywords": "工资",
            "type": TransactionType.INCOME,
        },
        {
            "main_category": "转账",
            "sub_category": "内部转账",
            "priority": 1,
            "keywords": "转账",
            "type": TransactionType.TRANSFER,
        },
        {
            "main_category": "投资",
            "sub_category": "基金",
            "priority": 1,
            "keywords": "基金",
            "type": TransactionType.INVESTMENT,
        },
    ]
    engine = CategoryEngine()
    await engine.load_rules_from_db(FakeCategoryDB(categories), user_id=1)

    categorized = await engine.batch_match_categories(
        [
            {"counterparty": "早餐铺", "description": "早餐", "type": "支出", "amount": -12.0},
            {"counterparty": "公司", "description": "工资发放", "type": "收入", "amount": 1000.0},
        ]
    )
    assert categorized[0]["main_category"] == "餐饮"
    assert categorized[1]["sub_category"] == "工资"

    tree = engine.get_categories_tree()
    assert tree["支出"] == {"餐饮": ["早餐"]}
    assert tree["收入"] == {"收入": ["工资"]}
    assert tree["转账"] == {"转账": ["内部转账"]}
    assert tree["投资"] == {"投资": ["基金"]}

    category_engine_module._category_engine_v2 = CategoryEngine()
    global_engine = await category_engine_module.get_category_engine(FakeCategoryDB(categories), user_id=1)
    assert global_engine.is_initialized is True
    assert global_engine.current_user_id == 1

    await category_engine_module.get_category_engine(FakeCategoryDB(categories), user_id=2)
    assert category_engine_module._category_engine_v2.current_user_id == 2



def test_category_engine_invalidate_cache_clears_precompiled_rules() -> None:
    """invalidate_cache 应同时清空引擎规则缓存和 KeywordMatcher 缓存。"""
    engine = CategoryEngine()
    engine.rules = [
        {"main": "餐饮", "sub": "早餐", "priority": 1, "keywords": "OR:早餐|包子", "type": TransactionType.EXPENSE}
    ]
    engine._precompile_rules()
    assert engine._compiled_rules
    assert engine.keyword_matcher._compiled_rules_cache

    engine.invalidate_cache()
    assert engine._compiled_rules == {}
    assert engine.keyword_matcher._compiled_rules_cache == {}
