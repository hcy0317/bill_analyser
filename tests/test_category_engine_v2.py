"""
测试Category Engine V2模块
测试复杂关键词匹配功能
"""

import pytest
from bill_analyser.utils.constants import TransactionType
from src.core.category_engine import CategoryEngine, KeywordMatcher


class TestKeywordMatcher:
    """测试关键词匹配器"""

    def setup_method(self):
        """初始化"""
        self.matcher = KeywordMatcher()

    def test_simple_match(self):
        """测试简单匹配"""
        assert self.matcher.parse_and_match("滴滴出行支付", "滴滴") is True
        assert self.matcher.parse_and_match("美团外卖", "滴滴") is False

    def test_or_match(self):
        """测试OR逻辑"""
        rule = "OR:滴滴|快的|优步"
        assert self.matcher.parse_and_match("滴滴出行", rule) is True
        assert self.matcher.parse_and_match("快的打车", rule) is True
        assert self.matcher.parse_and_match("优步打车", rule) is True
        assert self.matcher.parse_and_match("美团外卖", rule) is False

    def test_not_match(self):
        """测试NOT排除"""
        rule = "OR:滴滴|快的&NOT:退款|取消"
        assert self.matcher.parse_and_match("滴滴出行", rule) is True
        assert self.matcher.parse_and_match("滴滴出行退款", rule) is False
        assert self.matcher.parse_and_match("快的打车取消", rule) is False

    def test_and_match(self):
        """测试AND必须"""
        rule = "OR:美团|饿了么&AND:外卖"
        assert self.matcher.parse_and_match("美团外卖", rule) is True
        assert self.matcher.parse_and_match("饿了么外卖", rule) is True
        assert self.matcher.parse_and_match("美团打车", rule) is False

    def test_complex_match(self):
        """测试复杂组合"""
        rule = "OR:滴滴|快的|优步&NOT:退款|取消&AND:打车"
        assert self.matcher.parse_and_match("滴滴打车", rule) is True
        assert self.matcher.parse_and_match("快的打车", rule) is True
        assert self.matcher.parse_and_match("滴滴打车退款", rule) is False
        assert self.matcher.parse_and_match("滴滴外卖", rule) is False

    def test_composite_visible_connectors_and_grouping(self):
        """测试新表达式中可见连接符与表达式分组语义"""
        rule = "(OR={早餐}/OR={早饭})+AND={咖啡}× NOT={退款}|OR={午餐}"
        compiled = self.matcher.compile_rule_expression(rule)

        assert self.matcher.match_compiled("早餐咖啡", compiled) is True
        assert self.matcher.match_compiled("早饭咖啡", compiled) is True
        assert self.matcher.match_compiled("早餐咖啡退款", compiled) is False
        assert self.matcher.match_compiled("午餐套餐", compiled) is True
        assert self.matcher.match_compiled("早饭摊", compiled) is False

    def test_nested_shared_bike_parentheses_change_semantics(self):
        """测试共享单车示例中多重括号会改变匹配语义"""
        grouped_rule = (
            "(OR={共享单车,摩拜,ofo,哈啰,青桔,小蓝}/"
            "(OR={549}+NOT={大丰收的}))+OR={1123}"
        )
        ungrouped_rule = (
            "OR={共享单车,摩拜,ofo,哈啰,青桔,小蓝}/"
            "(OR={549}+NOT={大丰收的})+OR={1123}"
        )

        grouped = self.matcher.compile_rule_expression(grouped_rule)
        ungrouped = self.matcher.compile_rule_expression(ungrouped_rule)

        assert self.matcher.match_compiled("共享单车", grouped) is False
        assert self.matcher.match_compiled("共享单车", ungrouped) is True
        assert self.matcher.match_compiled("共享单车 1123", grouped) is True
        assert self.matcher.match_compiled("549 1123", grouped) is True
        assert self.matcher.match_compiled("549 大丰收的 1123", grouped) is False


class FakeCategoryRulesDb:
    async def get_category_rules(
        self,
        user_id=1,  # pylint: disable=unused-argument
        enabled_only=True,  # pylint: disable=unused-argument
        include_category_priority=True,  # pylint: disable=unused-argument
    ):
        return [
            {
                "id": 1,
                "category_id": None,
                "category_type": TransactionType.TRANSFER,
                "main_category": "转账",
                "sub_category": "同名幽灵",
                "category_priority": 1,
                "priority": 1,
                "rule_expression": "OR={转账}",
                "regex_enabled": False,
            },
            {
                "id": 2,
                "category_id": 2,
                "category_type": TransactionType.INVESTMENT,
                "main_category": "投资",
                "sub_category": "利息",
                "category_priority": 1,
                "priority": 1,
                "rule_expression": "OR={结息,利息}",
                "regex_enabled": False,
            },
            {
                "id": 3,
                "category_id": 3,
                "category_type": TransactionType.INCOME,
                "main_category": "收入",
                "sub_category": "利息收入",
                "category_priority": 2,
                "priority": 2,
                "rule_expression": "OR={结息,利息}",
                "regex_enabled": False,
            },
        ]


@pytest.mark.asyncio
async def test_category_rules_fail_closed_and_bank_interest_prefers_income():
    engine = CategoryEngine()
    await engine.load_rules_from_db_v2(FakeCategoryRulesDb())

    assert [rule["id"] for rule in engine.rules] == [2, 3]

    bill = {
        "type": "收入",
        "amount": 1.23,
        "counterparty": "工商银行",
        "payment_method": "工商银行储蓄卡",
        "description": "账户结息 利息入账",
        "original_category": "银行结息",
    }

    assert engine.match_category(bill) == ("收入", "利息收入")
    assert bill["type"] == "收入"


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
