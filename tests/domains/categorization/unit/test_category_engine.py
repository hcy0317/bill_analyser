from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

from typing import Any

import pytest

from bill_analyser.core import category_engine as category_engine_module
from bill_analyser.core.category_engine import CategoryEngine, KeywordMatcher
from bill_analyser.core.db_category_rules import DatabaseCategoryRulesMixin
from bill_analyser.utils.constants import TransactionType


class FakeCategoryDB:
    """Minimal async DB stub for category-engine tests."""

    def __init__(
        self,
        category_rules: list[dict[str, Any]] | None = None,
        error: Exception | None = None,
    ) -> None:
        self.category_rules = category_rules or []
        self.error = error
        self.user_calls: list[int] = []
        self.legacy_categories_requested = False

    async def get_category_rules(
        self,
        user_id: int = 1,
        category_id: int | None = None,
        enabled_only: bool = True,
    ) -> list[dict[str, Any]]:
        _ = (category_id, enabled_only)
        self.user_calls.append(user_id)
        if self.error is not None:
            raise self.error
        return list(self.category_rules)

    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
        _ = user_id
        self.legacy_categories_requested = True
        raise AssertionError("category_engine should not fallback to legacy categories.keywords")



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


def test_keyword_matcher_rule_expression_ast_preserves_old_and_new_semantics() -> None:
    """分类规则表达式应兼容旧语法、新语法，并支持括号优先级。"""
    matcher = KeywordMatcher()

    legacy = matcher.compile_rule_expression("OR:星巴克|瑞幸&AND:咖啡&NOT:退款")
    assert matcher.match_compiled("星巴克咖啡", legacy) is True
    assert matcher.match_compiled("瑞幸咖啡", legacy) is True
    assert matcher.match_compiled("星巴克退款咖啡", legacy) is False
    assert matcher.match_compiled("星巴克早餐", legacy) is False

    composite = matcher.compile_rule_expression("OR={星巴克,瑞幸}+AND={咖啡}+NOT={退款}")
    assert matcher.match_compiled("星巴克咖啡", composite) is True
    assert matcher.match_compiled("瑞幸咖啡", composite) is True
    assert matcher.match_compiled("星巴克退款咖啡", composite) is False
    assert matcher.match_compiled("星巴克早餐", composite) is False

    parenthesized = matcher.compile_rule_expression(
        "(OR={星巴克}+AND={咖啡})|OR={瑞幸}"
    )
    assert matcher.match_compiled("星巴克咖啡", parenthesized) is True
    assert matcher.match_compiled("瑞幸拿铁", parenthesized) is True
    assert matcher.match_compiled("星巴克早餐", parenthesized) is False


def test_keyword_matcher_rule_expression_regex_switch_and_regex_clause() -> None:
    """regex_enabled 与 REGEX 子句都应进入真实匹配语义。"""
    matcher = KeywordMatcher()

    regex_enabled_rule = matcher.compile_rule_expression(
        "OR={^星巴克.*咖啡$}+NOT={退款}",
        regex_enabled=True,
    )
    assert matcher.match_compiled("星巴克冰咖啡", regex_enabled_rule) is True
    assert matcher.match_compiled("门店 星巴克冰咖啡", regex_enabled_rule) is False
    assert matcher.match_compiled("星巴克冰咖啡退款", regex_enabled_rule) is False

    regex_clause_rule = matcher.compile_rule_expression("REGEX={^瑞幸.*咖啡$}+NOT={退款}")
    assert matcher.match_compiled("瑞幸生椰咖啡", regex_clause_rule) is True
    assert matcher.match_compiled("门店 瑞幸生椰咖啡", regex_clause_rule) is False
    assert matcher.match_compiled("瑞幸生椰咖啡退款", regex_clause_rule) is False


def test_keyword_migration_escapes_expression_delimiters_as_literals() -> None:
    """旧关键词迁移时不应把逗号、加号、花括号和竖线误当新语法分隔符。"""
    matcher = KeywordMatcher()

    plain_legacy = "商户A,咖啡+拿铁{热}|杯"
    plain_expr = DatabaseCategoryRulesMixin._convert_old_keyword_syntax(plain_legacy)
    assert plain_expr == r"OR={商户A\,咖啡\+拿铁\{热\}\|杯}"

    plain_compiled = matcher.compile_rule_expression(plain_expr)
    assert matcher.match_compiled("订单 商户A,咖啡+拿铁{热}|杯", plain_compiled) is True
    assert matcher.match_compiled("订单 商户A 咖啡 拿铁 热 杯", plain_compiled) is False

    legacy_expr = DatabaseCategoryRulesMixin._convert_old_keyword_syntax(
        r"OR:商户A\|联名|普通,门店&AND:上海\&浦东|午餐+套餐&NOT:退款\|撤销"
    )
    assert legacy_expr == (
        r"OR={商户A\|联名,普通\,门店}"
        r"+AND={上海&浦东,午餐\+套餐}"
        r"+NOT={退款\|撤销}"
    )

    legacy_compiled = matcher.compile_rule_expression(legacy_expr)
    assert matcher.match_compiled("商户A|联名 上海&浦东 午餐+套餐", legacy_compiled) is True
    assert matcher.match_compiled("普通,门店 上海&浦东 午餐+套餐", legacy_compiled) is True
    assert matcher.match_compiled("商户A|联名 上海&浦东", legacy_compiled) is False
    assert (
        matcher.match_compiled(
            "商户A|联名 上海&浦东 午餐+套餐 退款|撤销",
            legacy_compiled,
        )
        is False
    )


def test_keyword_migration_preserves_legacy_regex_patterns_with_delimiters() -> None:
    """旧 REGEX 规则迁移后仍按正则匹配，并保留正则内的新语法分隔字符。"""
    matcher = KeywordMatcher()

    regex_expr = DatabaseCategoryRulesMixin._convert_old_keyword_syntax(
        r"REGEX:^商户\d{2},(咖啡|茶)\+$"
    )
    regex_compiled = matcher.compile_rule_expression(regex_expr)

    assert regex_expr.startswith("REGEX={")
    assert matcher.match_compiled("商户12,咖啡+", regex_compiled) is True
    assert matcher.match_compiled("商户12,咖啡", regex_compiled) is False
    assert matcher.match_compiled("商户AB,咖啡+", regex_compiled) is False


@pytest.mark.asyncio
async def test_category_engine_loads_rules_filters_types_and_handles_failures() -> None:
    """分类引擎应按用户/类型加载规则，并在 DB 失败时安全降级。"""
    category_rules = [
        {
            "id": 101,
            "category_id": 1,
            "main_category": "餐饮",
            "sub_category": "早餐",
            "priority": 1,
            "rule_expression": "OR={早餐,包子}",
            "category_type": TransactionType.EXPENSE,
        },
        {
            "id": 102,
            "category_id": 2,
            "main_category": "收入",
            "sub_category": "工资",
            "priority": 2,
            "rule_expression": "OR={工资}",
            "category_type": TransactionType.INCOME,
        },
    ]
    engine = CategoryEngine()
    db = FakeCategoryDB(category_rules)

    await engine.load_rules_from_db(db, user_id=9, types=[TransactionType.EXPENSE])
    assert engine.is_initialized is True
    assert engine.current_user_id == 9
    assert len(engine.rules) == 1
    assert engine.rules[0]["main"] == "餐饮"
    assert engine.rules[0]["keywords"] == "OR={早餐,包子}"
    assert engine.rules[0]["_compiled_v2"].or_blocks == [["早餐", "包子"]]
    assert db.user_calls == [9]
    assert db.legacy_categories_requested is False

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
    category_rules = [
        {
            "id": 201,
            "category_id": 10,
            "main_category": "餐饮",
            "sub_category": "早餐",
            "priority": 1,
            "rule_expression": "OR={早餐}",
            "category_type": TransactionType.EXPENSE,
        },
        {
            "id": 202,
            "category_id": 11,
            "main_category": "收入",
            "sub_category": "工资",
            "priority": 1,
            "rule_expression": "OR={工资}",
            "category_type": TransactionType.INCOME,
        },
        {
            "id": 203,
            "category_id": 12,
            "main_category": "转账",
            "sub_category": "内部转账",
            "priority": 1,
            "rule_expression": "OR={转账}",
            "category_type": TransactionType.TRANSFER,
        },
        {
            "id": 204,
            "category_id": 13,
            "main_category": "投资",
            "sub_category": "基金",
            "priority": 1,
            "rule_expression": "OR={基金}",
            "category_type": TransactionType.INVESTMENT,
        },
    ]
    engine = CategoryEngine()
    await engine.load_rules_from_db(FakeCategoryDB(category_rules), user_id=1)

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
    global_engine = await category_engine_module.get_category_engine(FakeCategoryDB(category_rules), user_id=1)
    assert global_engine.is_initialized is True
    assert global_engine.current_user_id == 1

    await category_engine_module.get_category_engine(FakeCategoryDB(category_rules), user_id=2)
    assert category_engine_module._category_engine_v2.current_user_id == 2


@pytest.mark.asyncio
async def test_category_engine_matches_new_rule_expression_without_legacy_fallback() -> None:
    """从 category_rules 加载的新语法表达式应直接参与运行时匹配。"""
    engine = CategoryEngine()
    db = FakeCategoryDB(
        [
            {
                "id": 301,
                "category_id": 30,
                "main_category": "餐饮",
                "sub_category": "早餐",
                "priority": 1,
                "rule_expression": "OR={早餐,包子}+NOT={退款}",
                "category_type": TransactionType.EXPENSE,
            }
        ]
    )

    await engine.load_rules_from_db(db, user_id=5, types=[TransactionType.EXPENSE])

    matched_bill = {"counterparty": "早餐铺", "description": "早餐套餐", "type": "支出", "amount": -18.0}
    assert engine.match_category(matched_bill) == ("餐饮", "早餐")

    blocked_bill = {"counterparty": "早餐铺", "description": "早餐退款", "type": "支出", "amount": -18.0}
    assert engine.match_category(blocked_bill) == (None, None)
    assert db.legacy_categories_requested is False



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
