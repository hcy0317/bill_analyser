"""
测试新的规则引擎V2
"""
import json
from pathlib import Path
from src.utils.logic import RuleEngineV2

def main():
    # 加载新规则
    rules_path = Path('src/config/categories_v2.json')
    with open(rules_path, 'r', encoding='utf-8') as f:
        rules = json.load(f)
    
    # 创建引擎
    engine = RuleEngineV2()
    engine.load_rules(rules)
    
    # 测试案例
    test_cases = [
        ("美团外卖", "外卖订单", "餐饮/外卖"),
        ("拼多多平台商户", "商品订单", "购物/网购"),
        ("滴滴出行", "网约车费用", "交通/打车"),
        ("星巴克", "咖啡", "餐饮/咖啡茶饮"),
        ("陈红梅", "转账备注:生活费", "往来/转账"),
        ("余额宝", "理财收益", "投资/理财"),
    ]
    
    print("测试分类引擎:\n")
    correct = 0
    for counterparty, description, expected in test_cases:
        result = engine.classify(counterparty, description)
        if result:
            actual = f"{result['main']}/{result['sub']}"
            status = "✅" if actual == expected else "❌"
            if actual == expected:
                correct += 1
        else:
            actual = "未分类"
            status = "❌"
        
        print(f"{status} {counterparty:15s} | {description:20s}")
        print(f"   预期: {expected:20s} 实际: {actual}")
        print()
    
    print(f"准确率: {correct}/{len(test_cases)} ({correct/len(test_cases)*100:.1f}%)")

if __name__ == '__main__':
    main()
