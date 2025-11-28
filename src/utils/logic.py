"""
Enhanced Logic Parser - 增强的逻辑表达式解析器

支持的表达式格式:
- OR:关键词1|关键词2|关键词3  (任一匹配)
- AND:关键词1|关键词2  (全部匹配)
- NOT:关键词1|关键词2  (排除)
- OR:外卖|美团&NOT:退款  (组合)
"""

import re
from typing import Any, Dict, List, Optional, Set

try:
    from .logger import get_logger
except ImportError:
    # 测试时使用
    import sys
    from pathlib import Path
    sys.path.insert(0, str(Path(__file__).parent.parent.parent))
    from src.utils.logger import get_logger


class EnhancedLogicParser:
    """增强的逻辑表达式解析器"""

    def __init__(self):
        """初始化解析器"""
        self.logger = get_logger('EnhancedLogicParser')

    def parse_expression(self, expression: str) -> Dict[str, List[str]]:
        """
        解析逻辑表达式为结构化数据
        
        Args:
            expression: 逻辑表达式,如 "OR:外卖|美团&NOT:退款"
            
        Returns:
            Dict: {'or': [...], 'and': [...], 'not': [...]}
        """
        result = {
            'or': [],
            'and': [],
            'not': []
        }
        
        # 按&分割不同的逻辑组
        parts = expression.split('&')
        
        for part in parts:
            part = part.strip()
            if not part:
                continue
                
            # 匹配 OR:/AND:/NOT: 前缀
            if part.startswith('OR:'):
                keywords = part[3:].split('|')
                result['or'].extend([k.strip() for k in keywords if k.strip()])
            elif part.startswith('AND:'):
                keywords = part[4:].split('|')
                result['and'].extend([k.strip() for k in keywords if k.strip()])
            elif part.startswith('NOT:'):
                keywords = part[4:].split('|')
                result['not'].extend([k.strip() for k in keywords if k.strip()])
            else:
                # 没有前缀,默认为OR
                keywords = part.split('|')
                result['or'].extend([k.strip() for k in keywords if k.strip()])
        
        return result

    def match(self, expression: str, text: str) -> bool:
        """
        判断文本是否匹配表达式
        
        Args:
            expression: 逻辑表达式
            text: 要匹配的文本
            
        Returns:
            bool: 是否匹配
        """
        if not expression or not text:
            return False
            
        parsed = self.parse_expression(expression)
        text_lower = text.lower()
        
        # 检查NOT条件(排除)
        if parsed['not']:
            for keyword in parsed['not']:
                if keyword.lower() in text_lower:
                    return False
        
        # 检查AND条件(必须全部匹配)
        if parsed['and']:
            for keyword in parsed['and']:
                if keyword.lower() not in text_lower:
                    return False
        
        # 检查OR条件(任一匹配)
        if parsed['or']:
            for keyword in parsed['or']:
                if keyword.lower() in text_lower:
                    return True
            return False  # OR条件存在但都不匹配
        
        # 只有AND和NOT条件,前面都通过了
        if parsed['and'] or parsed['not']:
            return True
            
        return False

    def match_bill(self, expression: str, counterparty: str, description: str) -> bool:
        """
        匹配账单(对方+说明)
        
        Args:
            expression: 逻辑表达式
            counterparty: 对方
            description: 说明
            
        Returns:
            bool: 是否匹配
        """
        # 合并对方和说明一起匹配
        combined_text = f"{counterparty} {description}"
        return self.match(expression, combined_text)


class RuleEngineV2:
    """规则引擎 V2 - 使用新的逻辑表达式"""

    def __init__(self):
        """初始化规则引擎"""
        self.logger = get_logger('RuleEngineV2')
        self.parser = EnhancedLogicParser()
        self.rules: Dict[str, Dict[str, Dict]] = {}

    def load_rules(self, rules_dict: Dict[str, Dict[str, Dict]]):
        """
        加载规则字典
        
        Args:
            rules_dict: 规则字典,格式:
                {
                    "餐饮": {
                        "外卖": {
                            "priority": 10,
                            "logic_expression": "OR:外卖|美团&NOT:退款",
                            "enabled": true
                        }
                    }
                }
        """
        self.rules = rules_dict
        total_count = sum(len(subs) for subs in rules_dict.values())
        self.logger.info(f"加载了 {len(rules_dict)} 个主分类, {total_count} 条规则")

    def classify(self, counterparty: str, description: str) -> Optional[Dict[str, str]]:
        """
        分类账单
        
        Args:
            counterparty: 对方
            description: 说明
            
        Returns:
            Optional[Dict]: {'main': '主分类', 'sub': '子分类'} 或 None
        """
        best_match = None
        best_priority = -1
        
        # 遍历所有规则
        for main_category, sub_categories in self.rules.items():
            for sub_category, rule_config in sub_categories.items():
                # 检查是否启用
                if not rule_config.get('enabled', True):
                    continue
                    
                # 获取优先级和表达式
                priority = rule_config.get('priority', 0)
                expression = rule_config.get('logic_expression', '')
                
                if not expression:
                    continue
                
                # 匹配
                if self.parser.match_bill(expression, counterparty, description):
                    # 选择优先级最高的
                    if priority > best_priority:
                        best_priority = priority
                        best_match = {
                            'main': main_category,
                            'sub': sub_category
                        }
        
        return best_match


def convert_old_rules_to_new_format(old_rules: List[Dict]) -> Dict[str, Dict[str, Dict]]:
    """
    将旧格式规则转换为新格式
    
    Args:
        old_rules: 旧格式规则列表
        
    Returns:
        Dict: 新格式规则字典
    """
    new_rules = {}
    
    # 用于合并同一分类的关键词
    temp_rules = {}
    
    for idx, rule in enumerate(old_rules):
        main = rule.get('main', '')
        sub = rule.get('sub', '')
        conditions = rule.get('conditions', [])
        
        if not main or not sub:
            continue
        
        # 提取关键词
        keywords = []
        for cond in conditions:
            # 解析 "对方包含'关键词'" 或 "说明包含'关键词'"
            match = re.search(r"包含'([^']+)'", cond)
            if match:
                keywords.append(match.group(1))
        
        if not keywords:
            continue
        
        # 创建唯一键
        key = f"{main}/{sub}"
        
        # 合并同一分类的关键词
        if key not in temp_rules:
            temp_rules[key] = {
                'main': main,
                'sub': sub,
                'keywords': [],
                'priority': 100 - idx  # 第一次出现的优先级
            }
        
        temp_rules[key]['keywords'].extend(keywords)
    
    # 转换为最终格式
    for key, rule_data in temp_rules.items():
        main = rule_data['main']
        sub = rule_data['sub']
        
        # 去重关键词
        unique_keywords = list(dict.fromkeys(rule_data['keywords']))
        
        # 构建OR表达式
        expression = "OR:" + "|".join(unique_keywords)
        
        # 添加到新格式
        if main not in new_rules:
            new_rules[main] = {}
        
        new_rules[main][sub] = {
            'priority': rule_data['priority'],
            'description': f'{main}/{sub}',
            'logic_expression': expression,
            'keywords': unique_keywords,
            'enabled': True
        }
    
    return new_rules


# 为了向后兼容，添加别名
RuleEngine = RuleEngineV2


if __name__ == '__main__':
    # 测试
    parser = EnhancedLogicParser()
    
    # 测试OR
    assert parser.match("OR:外卖|美团|饿了么", "美团外卖订单")
    assert not parser.match("OR:外卖|美团|饿了么", "京东购物")
    
    # 测试AND
    assert parser.match("AND:还款|信用卡", "信用卡还款")
    assert not parser.match("AND:还款|信用卡", "信用卡消费")
    
    # 测试NOT
    assert not parser.match("NOT:退款|取消", "订单退款")
    assert parser.match("NOT:退款|取消", "订单完成")
    
    # 测试组合
    assert parser.match("OR:外卖|美团&NOT:退款", "美团外卖")
    assert not parser.match("OR:外卖|美团&NOT:退款", "美团外卖退款")
    
    print("✅ 所有测试通过!")
