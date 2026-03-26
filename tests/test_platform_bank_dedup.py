#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
测试平台-银行去重功能

验证 v6.48 修复后，使用 _parser_id 字段正确识别平台和银行来源
"""

import sys
import os
import asyncio
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from src.core.smart_dedup import SmartDeduplicationEngine


def test_platform_bank_dedup():
    """测试平台-银行去重"""
    engine = SmartDeduplicationEngine()
    
    # 模拟两条账单：一个支付宝，一个农业银行
    # 这是用户报告的实际场景
    bills = [
        {
            '_template_id': 1,
            '_parser_id': 'abc',  # 农业银行
            'date': '2025-08-25 09:30:13',
            'amount': -50.0,
            'type': '支出',
            'counterparty': '蚂蚁（杭州）基金销售有限公司',
            'description': '基金购买',
            'source_account_id': 1,  # 数字账户ID
        },
        {
            '_template_id': 48,
            '_parser_id': 'alipay',  # 支付宝
            'date': '2025-08-25 09:30:12',
            'amount': -50.0,
            'type': '支出',
            'counterparty': '国泰黄金-蚂蚁（杭州）基金销售有限公司',
            'description': '理财购买',
            'source_account_id': 2,  # 不同的账户ID
        },
    ]
    
    print("=" * 60)
    print("测试平台-银行去重 (使用 _parser_id 字段)")
    print("=" * 60)
    print(f"输入账单数: {len(bills)}")
    for i, b in enumerate(bills):
        print(f"  [{i+1}] template_id={b['_template_id']}, parser_id={b['_parser_id']}, "
              f"date={b['date']}, amount={b['amount']}")
    print()
    
    # 执行去重
    result = engine.process(bills)
    
    print(f"保留账单数: {len(result.kept_bills)}")
    print(f"移除账单数: {result.removed_count}")
    print(f"去重组数: {len(result.duplicate_groups)}")
    print()
    
    # 检查结果
    if len(result.kept_bills) == 1:
        kept_bill = result.kept_bills[0]
        print(f"保留的账单:")
        print(f"  - template_id: {kept_bill.get('_template_id')}")
        print(f"  - parser_id: {kept_bill.get('_parser_id')}")
        print(f"  - _dedup_type: {kept_bill.get('_dedup_type')}")
        print(f"  - _merged_template_ids: {kept_bill.get('_merged_template_ids')}")
        
        # 验证
        if kept_bill.get('_dedup_type') == 'platform_bank':
            print("\n✅ 去重类型正确: platform_bank")
        else:
            print(f"\n❌ 去重类型错误: {kept_bill.get('_dedup_type')}")
        
        if kept_bill.get('_parser_id') == 'alipay':
            print("✅ 正确保留平台账单 (alipay)")
        else:
            print(f"❌ 未保留平台账单，而是保留了: {kept_bill.get('_parser_id')}")
        
        merged_ids = kept_bill.get('_merged_template_ids', [])
        if 1 in merged_ids:
            print(f"✅ 合并的模板ID正确: {merged_ids}")
        else:
            print(f"❌ 合并的模板ID不正确: {merged_ids}")
    else:
        print(f"❌ 预期保留1条账单，实际保留了 {len(result.kept_bills)} 条")
        for b in result.kept_bills:
            print(f"  - template_id={b.get('_template_id')}, parser_id={b.get('_parser_id')}")
    
    print()
    
    # 打印去重组详情
    if result.duplicate_groups:
        print("去重组详情:")
        for i, group in enumerate(result.duplicate_groups):
            print(f"  组 {i+1}: type={group.type.name}, reason={group.reason}")
    else:
        print("❌ 没有检测到任何去重组")


if __name__ == '__main__':
    test_platform_bank_dedup()
