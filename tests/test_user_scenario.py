#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
用户场景测试 - 验证去重引擎对实际账单的处理
"""

import sys
import os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from datetime import datetime
from src.core.smart_dedup import SmartDeduplicationEngine, DeduplicationType


def is_platform(source: str) -> bool:
    """判断是否为支付平台来源"""
    return source.lower() in SmartDeduplicationEngine.PLATFORM_SOURCES if source else False


def is_bank(source: str) -> bool:
    """判断是否为银行来源"""
    return source.lower() in SmartDeduplicationEngine.BANK_SOURCES if source else False


def test_user_scenario_investment():
    """测试用户场景：农业银行账单 vs 支付宝账单（投资）"""
    print("\n" + "=" * 60)
    print("测试场景：农业银行账单 vs 支付宝账单（投资）")
    print("=" * 60)
    
    # 模拟用户场景：农业银行账单 vs 支付宝账单
    abc_bill = {
        'date': '2025-08-20 09:23:53',
        'amount': -50.00,
        'source_account_id': 'abc',  # 农业银行解析器
        'counterparty': '蚂蚁（杭州）基金销售有限公司',
        'payment_method': '农业银行',
        'description': 'NA2025082061879040930531090310104蚂蚁（杭州）基金销售有限公司'
    }

    alipay_bill = {
        'date': '2025-08-20 09:23:52',
        'amount': -50.00,
        'source_account_id': 'alipay',  # 支付宝解析器
        'counterparty': '蚂蚁财富-蚂蚁（杭州）基金销售有限公司',
        'payment_method': '中国农业银行储蓄卡(4071)',
        'description': '蚂蚁财富-南方红利低波50ETF联接A-买入'
    }

    # 创建引擎实例
    engine = SmartDeduplicationEngine()
    
    # 检查各条件
    source1 = abc_bill['source_account_id']
    source2 = alipay_bill['source_account_id']
    print(f"\n[条件检查]")
    print(f"  source1 = {source1}")
    print(f"    is_platform = {is_platform(source1)}")
    print(f"    is_bank = {is_bank(source1)}")
    print(f"  source2 = {source2}")
    print(f"    is_platform = {is_platform(source2)}")
    print(f"    is_bank = {is_bank(source2)}")
    
    # 时间差
    dt1 = datetime.fromisoformat(abc_bill['date'])
    dt2 = datetime.fromisoformat(alipay_bill['date'])
    time_diff = abs((dt1 - dt2).total_seconds())
    print(f"  时间差: {time_diff}秒 (<= 30秒? {time_diff <= 30})")
    
    # 金额比较
    amt1 = abc_bill['amount']
    amt2 = alipay_bill['amount']
    print(f"  金额: {amt1} vs {amt2}")
    print(f"    相等: {abs(amt1 - amt2) <= 0.01}")
    print(f"    同方向: {amt1 * amt2 > 0}")
    
    # 是否为平台-银行配对
    is_platform_bank = (
        (is_platform(source1) and is_bank(source2)) or
        (is_bank(source1) and is_platform(source2))
    )
    print(f"  是平台-银行配对: {is_platform_bank}")
    
    # 执行去重
    result = engine.process([abc_bill, alipay_bill])
    print(f"\n[去重结果]")
    print(f"  原始账单数: {result.original_count}")
    print(f"  保留账单数: {len(result.kept_bills)}")
    print(f"  移除数量: {result.removed_count}")
    print(f"  重复组数: {len(result.duplicate_groups)}")
    print(f"  转账配对: {len(result.transfer_pairs)}")
    print(f"  分账单组: {len(result.split_groups)}")
    
    # 检查重复组详情
    for i, group in enumerate(result.duplicate_groups):
        print(f"\n  重复组 {i+1}:")
        print(f"    类型: {group.type.value}")
        print(f"    原因: {group.reason}")
        print(f"    保留: {group.keep_bill.get('counterparty', '')[:30]}")
        for rb in group.remove_bills:
            print(f"    移除: {rb.get('counterparty', '')[:30]}")
    
    # 检查账单状态
    print(f"\n[账单状态]")
    for bill in [abc_bill, alipay_bill]:
        cp = bill['counterparty'][:30]
        removed = bill.get('_removed', False)
        print(f"  {cp}: _removed={removed}")
    
    # 验证
    if result.removed_count == 1:
        print("\n✅ 测试通过！")
        return True
    else:
        print(f"\n❌ 测试失败！期望移除1条，实际移除{result.removed_count}条")
        return False


def test_user_scenario_transfer():
    """测试用户场景：微信转账 vs 农业银行转账"""
    print("\n" + "=" * 60)
    print("测试场景：微信转账 vs 农业银行转账")
    print("=" * 60)
    
    # 微信支付记录
    wechat_bill = {
        'date': '2025-08-20 17:30:46',
        'amount': -1860.00,
        'source_account_id': 'wechat',
        'counterparty': '微信支付微信转账',
        'payment_method': '农业银行',
        'description': 'UA0820563404958312微信支付-微信转账'
    }
    
    # 农业银行记录
    abc_bill = {
        'date': '2025-08-20 17:30:46',
        'amount': -1860.00,
        'source_account_id': 'abc',
        'counterparty': '陈红梅',
        'payment_method': '农业银行储蓄卡(4071)',
        'description': '转账备注:差旅费转账'
    }
    
    engine = SmartDeduplicationEngine()
    
    # 检查条件
    source1 = wechat_bill['source_account_id']
    source2 = abc_bill['source_account_id']
    print(f"\n[条件检查]")
    print(f"  wechat: is_platform={is_platform(source1)}, is_bank={is_bank(source1)}")
    print(f"  abc: is_platform={is_platform(source2)}, is_bank={is_bank(source2)}")
    
    # 时间差
    dt1 = datetime.fromisoformat(wechat_bill['date'])
    dt2 = datetime.fromisoformat(abc_bill['date'])
    time_diff = abs((dt1 - dt2).total_seconds())
    print(f"  时间差: {time_diff}秒")
    
    # 金额
    amt1 = wechat_bill['amount']
    amt2 = abc_bill['amount']
    print(f"  金额: {amt1} vs {amt2}, 相等且同向: {amt1 == amt2}")
    
    # 是否为平台-银行配对
    is_platform_bank = (
        (is_platform(source1) and is_bank(source2)) or
        (is_bank(source1) and is_platform(source2))
    )
    print(f"  是平台-银行配对: {is_platform_bank}")
    
    # 执行去重
    result = engine.process([wechat_bill, abc_bill])
    print(f"\n[去重结果]")
    print(f"  原始账单数: {result.original_count}")
    print(f"  保留账单数: {len(result.kept_bills)}")
    print(f"  移除数量: {result.removed_count}")
    print(f"  重复组数: {len(result.duplicate_groups)}")
    
    # 检查重复组详情
    for i, group in enumerate(result.duplicate_groups):
        print(f"\n  重复组 {i+1}:")
        print(f"    类型: {group.type.value}")
        print(f"    原因: {group.reason}")
    
    # 检查账单状态
    print(f"\n[账单状态]")
    for bill in [wechat_bill, abc_bill]:
        cp = bill['counterparty'][:30]
        removed = bill.get('_removed', False)
        print(f"  {cp}: _removed={removed}")
    
    # 验证
    if result.removed_count == 1:
        print("\n✅ 测试通过！")
        return True
    else:
        print(f"\n❌ 测试失败！期望移除1条，实际移除{result.removed_count}条")
        return False


def test_similar_bills_dedup():
    """测试类似账单去重（同金额同方向但不同来源）"""
    print("\n" + "=" * 60)
    print("测试场景：类似账单去重（同金额同方向不同来源）")
    print("=" * 60)
    
    # 两条类似的银行账单
    bill1 = {
        'date': '2025-08-20 10:00:00',
        'amount': -100.00,
        'source_account_id': 'abc',
        'counterparty': '蚂蚁财富投资',
        'payment_method': '农业银行',
        'description': '蚂蚁财富基金买入'
    }
    
    bill2 = {
        'date': '2025-08-20 10:00:01',
        'amount': -100.00,
        'source_account_id': 'cmbc',  # 民生银行
        'counterparty': '蚂蚁财富投资服务',
        'payment_method': '民生银行',
        'description': '蚂蚁财富购买基金'
    }
    
    engine = SmartDeduplicationEngine()
    result = engine.process([bill1, bill2])
    
    print(f"\n[去重结果]")
    print(f"  原始账单数: {result.original_count}")
    print(f"  移除数量: {result.removed_count}")
    
    for i, group in enumerate(result.duplicate_groups):
        print(f"\n  重复组 {i+1}:")
        print(f"    类型: {group.type.value}")
        print(f"    原因: {group.reason}")
    
    # 类似账单去重应该识别这个场景
    if result.removed_count == 1:
        print("\n✅ 测试通过！")
        return True
    else:
        # 可能因为相似度不够高没有去重
        print(f"\n⚠️ 可能需要检查：移除{result.removed_count}条")
        return False


if __name__ == '__main__':
    results = []
    results.append(("投资场景", test_user_scenario_investment()))
    results.append(("转账场景", test_user_scenario_transfer()))
    results.append(("类似账单", test_similar_bills_dedup()))
    
    print("\n" + "=" * 60)
    print("测试结果汇总")
    print("=" * 60)
    for name, passed in results:
        status = "✅ 通过" if passed else "❌ 失败"
        print(f"  {name}: {status}")
