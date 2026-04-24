"""
测试Category Engine V2模块
测试复杂关键词匹配功能
"""

import pytest
from src.core.category_engine import KeywordMatcher


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


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
