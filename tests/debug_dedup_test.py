#!/usr/bin/env python3
"""
诊断脚本：测试智能去重是否正常工作

测试场景：
1. 两条来自不同来源（wechat vs abc）的账单，同时间同金额同方向 → 应该被去重
2. 两条来自不同来源的账单，同时间金额相反 → 应该被识别为转账配对
"""

import sys
import os

# 设置路径
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from src.core.smart_dedup import SmartDeduplicationEngine


def test_platform_bank_dedup():
    """测试支付平台与银行账单去重"""
    print("\n" + "=" * 60)
    print("测试1: 支付平台与银行账单去重")
    print("=" * 60)

    engine = SmartDeduplicationEngine()

    # 模拟用户场景：同一笔消费在微信和农业银行都有记录
    bills = [
        {
            'date': '2025-08-20 17:30:46',
            'amount': -1860.00,  # 支出为负
            'description': '微信支付微信转账',
            'counterparty': '陈红梅',
            'source_account_id': 'wechat',  # 来自微信
            'payment_method': '微信支付',
            'type': '支出'
        },
        {
            'date': '2025-08-20 17:30:46',
            'amount': -1860.00,  # 支出为负
            'description': '转账',
            'counterparty': '陈红梅',
            'source_account_id': 'abc',  # 来自农业银行
            'payment_method': '农业银行储蓄卡(4071)',
            'type': '支出'
        }
    ]

    print(f"输入账单数: {len(bills)}")
    for i, b in enumerate(bills):
        print(f"  [{i+1}] 来源={b['source_account_id']}, 金额={b['amount']}, "
              f"时间={b['date']}, 描述={b['description'][:20]}")

    result = engine.process(bills)

    print(f"\n去重结果:")
    print(f"  原始账单数: {result.original_count}")
    print(f"  保留账单数: {len(result.kept_bills)}")
    print(f"  移除账单数: {result.removed_count}")
    print(f"  重复组数: {len(result.duplicate_groups)}")

    if result.duplicate_groups:
        for i, group in enumerate(result.duplicate_groups):
            print(f"  重复组 {i+1}: {group.type}, 原因: {group.reason}")
            print(f"    保留: {group.keep_bill.get('source_account_id')}")
            for rb in group.remove_bills:
                print(f"    移除: {rb.get('source_account_id')}")

    expected = len(result.kept_bills) == 1 and result.removed_count == 1
    print(f"\n预期: 保留1条, 移除1条 → {'✅ 通过' if expected else '❌ 失败'}")
    return expected


def test_transfer_pairing():
    """测试转账配对"""
    print("\n" + "=" * 60)
    print("测试2: 转账配对（金额相反）")
    print("=" * 60)

    engine = SmartDeduplicationEngine()

    # 模拟转账场景：支付宝转出，农业银行转入
    bills = [
        {
            'date': '2025-08-20 17:30:46',
            'amount': -1860.00,  # 转出为负
            'description': '转账到农行',
            'counterparty': '',
            'source_account_id': 'alipay',  # 来自支付宝
            'payment_method': '支付宝',
            'type': '支出'
        },
        {
            'date': '2025-08-20 17:30:46',
            'amount': 1860.00,  # 转入为正
            'description': '支付宝转入',
            'counterparty': '',
            'source_account_id': 'abc',  # 来自农业银行
            'payment_method': '农业银行储蓄卡(4071)',
            'type': '收入'
        }
    ]

    print(f"输入账单数: {len(bills)}")
    for i, b in enumerate(bills):
        print(f"  [{i+1}] 来源={b['source_account_id']}, 金额={b['amount']}, "
              f"时间={b['date']}, 描述={b['description'][:20]}")

    result = engine.process(bills)

    print(f"\n配对结果:")
    print(f"  转账配对数: {len(result.transfer_pairs)}")

    if result.transfer_pairs:
        for i, (out_bill, in_bill) in enumerate(result.transfer_pairs):
            print(f"  配对 {i+1}:")
            print(f"    转出: {out_bill.get('source_account_id')} -> 目标: {out_bill.get('destination_account_id')}")
            print(f"    转入: {in_bill.get('source_account_id')} <- 来源: (原{in_bill.get('destination_account_id')})")
            print(f"    金额: {abs(out_bill.get('amount'))}")

    # 转账配对不会移除账单，两条都保留但设置type为'转账'
    bills_with_transfer_type = [b for b in result.kept_bills if b.get('type') == '转账']

    expected = len(result.transfer_pairs) == 1 and len(bills_with_transfer_type) == 2
    print(f"\n预期: 发现1对转账, 两条账单type都变为'转账' → {'✅ 通过' if expected else '❌ 失败'}")
    return expected


def test_similar_bills_dedup():
    """测试类似账单去重"""
    print("\n" + "=" * 60)
    print("测试3: 类似账单去重（相似度匹配）")
    print("=" * 60)

    engine = SmartDeduplicationEngine()

    # 模拟场景：两个不同银行记录同一笔消费，counterparty相似
    bills = [
        {
            'date': '2025-08-20 10:00:00',
            'amount': -50.00,
            'description': '消费',
            'counterparty': '星巴克咖啡北京朝阳店',
            'source_account_id': 'icbc',  # 来自工商银行
            'payment_method': '工商银行',
            'type': '支出'
        },
        {
            'date': '2025-08-20 10:00:00',
            'amount': -50.00,
            'description': '银联消费',
            'counterparty': '星巴克咖啡朝阳区店',  # 略有不同但相似
            'source_account_id': 'cmbc',  # 来自民生银行
            'payment_method': '民生银行',
            'type': '支出'
        }
    ]

    print(f"输入账单数: {len(bills)}")
    for i, b in enumerate(bills):
        print(f"  [{i+1}] 来源={b['source_account_id']}, 金额={b['amount']}, "
              f"counterparty={b['counterparty'][:20]}")

    result = engine.process(bills)

    print(f"\n去重结果:")
    print(f"  原始账单数: {result.original_count}")
    print(f"  保留账单数: {len(result.kept_bills)}")
    print(f"  移除账单数: {result.removed_count}")
    print(f"  重复组数: {len(result.duplicate_groups)}")

    if result.duplicate_groups:
        for i, group in enumerate(result.duplicate_groups):
            print(f"  重复组 {i+1}: {group.type}, 原因: {group.reason}")

    expected = len(result.kept_bills) == 1 and result.removed_count == 1
    print(f"\n预期: 保留1条, 移除1条 → {'✅ 通过' if expected else '❌ 失败'}")
    return expected


def test_same_source_no_dedup():
    """测试同来源账单不应去重"""
    print("\n" + "=" * 60)
    print("测试4: 同来源账单不应去重（保护正常账单）")
    print("=" * 60)

    engine = SmartDeduplicationEngine()

    # 模拟场景：同一来源的两条不同消费（时间和金额相同但确实是两笔交易）
    bills = [
        {
            'date': '2025-08-20 10:00:00',
            'amount': -50.00,
            'description': '购买咖啡',
            'counterparty': '星巴克',
            'source_account_id': 'wechat',  # 同一来源
            'payment_method': '微信支付',
            'type': '支出'
        },
        {
            'date': '2025-08-20 10:00:00',
            'amount': -50.00,
            'description': '购买蛋糕',
            'counterparty': '星巴克',
            'source_account_id': 'wechat',  # 同一来源
            'payment_method': '微信支付',
            'type': '支出'
        }
    ]

    print(f"输入账单数: {len(bills)}")
    for i, b in enumerate(bills):
        print(f"  [{i+1}] 来源={b['source_account_id']}, 描述={b['description']}")

    result = engine.process(bills)

    print(f"\n结果:")
    print(f"  保留账单数: {len(result.kept_bills)}")
    print(f"  移除账单数: {result.removed_count}")

    # 同来源的账单不应该互相去重（除非完全相同）
    expected = len(result.kept_bills) == 2 and result.removed_count == 0
    print(f"\n预期: 两条都保留（同来源不去重）→ {'✅ 通过' if expected else '❌ 失败'}")
    return expected


def main():
    print("=" * 60)
    print("智能去重诊断测试")
    print("=" * 60)

    results = []

    results.append(("支付平台与银行去重", test_platform_bank_dedup()))
    results.append(("转账配对", test_transfer_pairing()))
    results.append(("类似账单去重", test_similar_bills_dedup()))
    results.append(("同来源保护", test_same_source_no_dedup()))

    print("\n" + "=" * 60)
    print("测试汇总")
    print("=" * 60)

    passed = sum(1 for _, r in results if r)
    total = len(results)

    for name, result in results:
        status = "✅ 通过" if result else "❌ 失败"
        print(f"  {name}: {status}")

    print(f"\n总计: {passed}/{total} 通过")

    return passed == total


if __name__ == '__main__':
    success = main()
    sys.exit(0 if success else 1)
