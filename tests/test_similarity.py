"""测试用户报告的去重场景"""
import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from src.core.smart_dedup import SmartDeduplicationEngine

# 创建去重引擎
engine = SmartDeduplicationEngine()

# 用户报告的账单数据
test_bills = [
    # 第1组 - 同时间同金额，不同counterparty（0%相似度）
    {
        'date': '2025-08-20 17:30:46',
        'amount': -1860.00,
        'type': '支出',
        'source_account_id': 'abc',
        'counterparty': '微信支付微信转账',
        'description': 'UA0820563404958312微信支付-微信转账 | 微信支付微信转账',
    },
    {
        'date': '2025-08-20 17:30:46',
        'amount': -1860.00,
        'type': '支出',
        'source_account_id': 'abc',
        'counterparty': '陈红梅',
        'description': '转账备注:差旅费转账 | 陈红梅 | 农业银行储蓄卡(4071) | 转账',
    },
    # 第2组 - 差1秒同金额，高相似度counterparty
    {
        'date': '2025-08-20 09:23:53',
        'amount': -50.00,
        'type': '支出',
        'source_account_id': 'abc',
        'counterparty': '蚂蚁（杭州）基金销售有限公司',
        'description': 'NA2025082061879040930531090310104蚂蚁（杭州）基金销售有限公司',
    },
    {
        'date': '2025-08-20 09:23:52',
        'amount': -50.00,
        'type': '支出',
        'source_account_id': 'abc',
        'counterparty': '蚂蚁财富-蚂蚁（杭州）基金销售有限公司',
        'description': '蚂蚁财富-南方红利低波50ETF联接A-买入 | 中国农业银行储蓄卡(4071) | 投资理财',
        'original_category': '投资理财',
    },
]

print("\n=== 测试用户报告的去重场景 ===")
print(f"原始账单数量: {len(test_bills)}")

# 执行去重
result = engine.process(test_bills)

print(f"\n去重结果:")
print(f"  - 原始数量: {result.original_count}")
print(f"  - 保留数量: {len(result.kept_bills)}")
print(f"  - 移除数量: {result.removed_count}")
print(f"  - 重复组数: {len(result.duplicate_groups)}")

for group in result.duplicate_groups:
    print(f"\n重复组 ({group.type.value}):")
    print(f"  原因: {group.reason}")
    print(f"  保留: {group.keep_bill.get('counterparty')[:30]}")
    for rb in group.remove_bills:
        print(f"  移除: {rb.get('counterparty')[:30]}")

print(f"\n转账配对数: {len(result.transfer_pairs)}")
# 注意：v6.42.1版本中，投资配对已移除，使用分类引擎识别投资类型
print(f"分账单组数: {len(result.split_groups)}")

# 分析为什么不匹配
from difflib import SequenceMatcher

def similarity(cp1, cp2):
    s1 = str(cp1 or '').strip().lower()
    s2 = str(cp2 or '').strip().lower()
    return SequenceMatcher(None, s1, s2).ratio()

print("\n=== counterparty相似度分析 ===")
print(f"第1组: '微信支付微信转账' vs '陈红梅' = {similarity('微信支付微信转账', '陈红梅'):.2%}")
print(f"第2组: '蚂蚁（杭州）基金销售有限公司' vs '蚂蚁财富-蚂蚁（杭州）基金销售有限公司' = {similarity('蚂蚁（杭州）基金销售有限公司', '蚂蚁财富-蚂蚁（杭州）基金销售有限公司'):.2%}")
