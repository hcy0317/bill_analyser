"""
分类引擎缓存失效测试

测试修改分类关键词后能否正确重新预编译规则
"""

import pytest
from bill_analyser.core.category_engine import CategoryEngine, KeywordMatcher
from bill_analyser.utils.constants import TransactionType


class TestKeywordMatcherCache:
    """测试 KeywordMatcher 缓存功能"""

    def test_cache_basic(self):
        """测试基本缓存功能"""
        matcher = KeywordMatcher()
        
        # 编译规则
        rule = "OR:滴滴|快的"
        compiled1 = matcher.compile_rule(rule)
        compiled2 = matcher.compile_rule(rule)
        
        # 应该返回同一个缓存对象
        assert compiled1 is compiled2
        
    def test_cache_clear(self):
        """测试缓存清除功能"""
        matcher = KeywordMatcher()
        
        # 编译规则
        rule = "OR:滴滴|快的"
        compiled1 = matcher.compile_rule(rule)
        
        # 清除缓存
        matcher.clear_cache()
        
        # 再次编译应该是新对象
        compiled2 = matcher.compile_rule(rule)
        
        # 应该是不同的对象（虽然内容相同）
        assert compiled1 is not compiled2
        
    def test_cache_clear_multiple_rules(self):
        """测试清除多条规则的缓存"""
        matcher = KeywordMatcher()
        
        # 编译多条规则
        rules = ["OR:滴滴|快的", "AND:支付|转账", "NOT:退款"]
        compiled_before = [matcher.compile_rule(r) for r in rules]
        
        # 验证缓存存在
        assert len(matcher._compiled_rules_cache) == 3
        
        # 清除缓存
        matcher.clear_cache()
        
        # 验证缓存已清空
        assert len(matcher._compiled_rules_cache) == 0
        
        # 再次编译
        compiled_after = [matcher.compile_rule(r) for r in rules]
        
        # 应该是不同的对象
        for before, after in zip(compiled_before, compiled_after):
            assert before is not after


class TestCategoryEngineInvalidation:
    """测试 CategoryEngine 缓存失效功能"""
    
    def test_invalidate_cache(self):
        """测试 invalidate_cache 方法"""
        engine = CategoryEngine()
        engine.rules = [
            {'main': '交通', 'sub': '打车', 'keywords': 'OR:滴滴|快的', 'type': TransactionType.EXPENSE}
        ]
        
        # 预编译规则
        engine._precompile_rules()
        
        # 验证规则已编译
        assert len(engine._compiled_rules) == 1
        assert len(engine.keyword_matcher._compiled_rules_cache) >= 1
        
        # 失效缓存
        engine.invalidate_cache()
        
        # 验证缓存已清空
        assert len(engine._compiled_rules) == 0
        assert len(engine.keyword_matcher._compiled_rules_cache) == 0
        
    def test_precompile_clears_cache(self):
        """测试 _precompile_rules 方法会清除旧缓存"""
        engine = CategoryEngine()
        
        # 第一组规则
        engine.rules = [
            {'main': '交通', 'sub': '打车', 'keywords': 'OR:滴滴|快的', 'type': TransactionType.EXPENSE}
        ]
        engine._precompile_rules()
        
        # 修改规则
        engine.rules = [
            {'main': '餐饮', 'sub': '外卖', 'keywords': 'OR:美团|饿了么', 'type': TransactionType.EXPENSE}
        ]
        
        # 重新预编译（应该清除旧缓存）
        engine._precompile_rules()
        
        # 验证旧规则已清除
        assert 'OR:滴滴|快的' not in engine._compiled_rules
        # 验证新规则已编译
        assert 'OR:美团|饿了么' in engine._compiled_rules
        
    def test_rule_update_reflected_after_precompile(self):
        """测试规则更新后重新预编译能反映新规则"""
        engine = CategoryEngine()
        
        # 初始规则
        engine.rules = [
            {'main': '交通', 'sub': '打车', 'keywords': '滴滴', 'type': TransactionType.EXPENSE}
        ]
        engine._precompile_rules()
        
        # 验证初始匹配
        text = "滴滴出行"
        assert engine._match_keywords_fast(text, engine.rules[0]['keywords'])
        
        text2 = "曹操专车"
        assert not engine._match_keywords_fast(text2, engine.rules[0]['keywords'])
        
        # 修改规则关键词
        engine.rules = [
            {'main': '交通', 'sub': '打车', 'keywords': 'OR:滴滴|曹操', 'type': TransactionType.EXPENSE}
        ]
        engine._precompile_rules()
        
        # 验证新规则生效
        assert engine._match_keywords_fast(text, engine.rules[0]['keywords'])
        assert engine._match_keywords_fast(text2, engine.rules[0]['keywords'])


class TestCacheInvalidationIntegration:
    """集成测试：模拟真实使用场景"""
    
    def test_simulate_rule_modification_flow(self):
        """模拟用户修改规则后重新分类的流程"""
        engine = CategoryEngine()
        
        # 1. 初始加载规则
        engine.rules = [
            {'main': '餐饮', 'sub': '外卖', 'keywords': '美团外卖', 'type': TransactionType.EXPENSE, 'priority': 1}
        ]
        engine._precompile_rules()
        engine._initialized = True
        
        # 2. 创建测试账单
        bill = {
            'counterparty': '美团外卖',
            'description': '外卖订单',
            'type': '支出',
            'amount': -50.0
        }
        
        # 3. 匹配分类
        main_cat, sub_cat = engine.match_category(bill)
        assert main_cat == '餐饮'
        assert sub_cat == '外卖'
        
        # 4. 模拟用户添加新关键词
        engine.rules = [
            {'main': '餐饮', 'sub': '外卖', 'keywords': 'OR:美团外卖|饿了么', 'type': TransactionType.EXPENSE, 'priority': 1}
        ]
        engine._precompile_rules()  # 重新预编译
        engine._initialized = True
        
        # 5. 测试新关键词
        bill2 = {
            'counterparty': '饿了么',
            'description': '外卖订单',
            'type': '支出',
            'amount': -35.0
        }
        
        main_cat2, sub_cat2 = engine.match_category(bill2)
        assert main_cat2 == '餐饮'
        assert sub_cat2 == '外卖'
        
    def test_empty_rules_after_invalidate(self):
        """测试失效缓存后没有规则的情况"""
        engine = CategoryEngine()
        engine.rules = []
        engine._precompile_rules()
        engine._initialized = True
        
        bill = {
            'counterparty': '测试',
            'description': '测试',
            'type': '支出',
            'amount': -10.0
        }
        
        main_cat, sub_cat = engine.match_category(bill)
        assert main_cat is None
        assert sub_cat is None
