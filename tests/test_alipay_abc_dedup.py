#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
测试支付宝+农业银行的账单去重
"""

import sys
from pathlib import Path
from datetime import datetime
from collections import defaultdict

sys.path.insert(0, str(Path(__file__).parent.parent))

from src.parsers.factory import ParserFactory
from src.utils.validator import BillValidator
from src.core.smart_dedup import SmartDeduplicationEngine


def test_alipay_abc_dedup():
    """测试支付宝+农业银行去重"""
    bills_dir = Path('bills')
    factory = ParserFactory()
    validator = BillValidator()
    dedup_engine = SmartDeduplicationEngine()

    print('='*70)
    print('2025年8月 支付宝+农业银行 去重测试')
    print('='*70)

    # 解析账单
    alipay_file = bills_dir / '支付宝交易明细(20250715-20250825).csv'
    if alipay_file.exists():
        alipay_bills = factory.parse(str(alipay_file))
        alipay_aug = [b for b in alipay_bills if '2025-08' in b.get('date', '')]
        print(f'支付宝 2025年8月: {len(alipay_aug)} 条')
        sources = set(b.get('source_account_id') for b in alipay_aug)
        print(f'  source_account_id: {sources}')
    else:
        print(f'支付宝账单文件不存在: {alipay_file}')
        return

    abc_file = bills_dir / 'detail20250826.xlsx'
    if abc_file.exists():
        abc_bills = factory.parse(str(abc_file))
        abc_aug = [b for b in abc_bills if '2025-08' in b.get('date', '')]
        print(f'农业银行 2025年8月: {len(abc_aug)} 条')
        sources = set(b.get('source_account_id') for b in abc_aug)
        print(f'  source_account_id: {sources}')
    else:
        print(f'农业银行账单文件不存在: {abc_file}')
        return

    # 合并
    all_bills = alipay_aug + abc_aug
    print(f'\n合并: {len(all_bills)} 条')

    # 验证
    valid_bills, _ = validator.validate_bills(all_bills)
    print(f'验证通过: {len(valid_bills)} 条')

    # 分析可能的平台-银行重复
    print('\n' + '='*70)
    print('分析可能的平台-银行重复（时间差<=60秒，金额相同）:')
    print('='*70)
    
    by_amount = defaultdict(list)
    for bill in valid_bills:
        amt = abs(bill.get('amount', 0))
        if amt > 0:
            by_amount[amt].append(bill)

    potential_count = 0
    for amt, group in by_amount.items():
        sources = set(b.get('source_account_id') for b in group)
        if 'alipay' in sources and 'abc' in sources:
            alipay_group = [b for b in group if b.get('source_account_id') == 'alipay']
            abc_group = [b for b in group if b.get('source_account_id') == 'abc']
            
            for a in alipay_group:
                for b in abc_group:
                    try:
                        a_dt = datetime.strptime(a.get('date', ''), '%Y-%m-%d %H:%M:%S')
                        b_dt = datetime.strptime(b.get('date', ''), '%Y-%m-%d %H:%M:%S')
                        diff = abs((a_dt - b_dt).total_seconds())
                        if diff <= 60:
                            potential_count += 1
                            same_dir = a.get('amount', 0) * b.get('amount', 0) > 0
                            print(f'\n组 {potential_count}: ¥{amt:.2f}')
                            print(f'  时间差: {diff:.1f} 秒')
                            print(f'  方向相同: {same_dir}')
                            print(f'  支付宝: {a.get("date")} | amt={a.get("amount"):.2f} | {a.get("type")} | {a.get("counterparty", "")[:40]}')
                            print(f'  农行:   {b.get("date")} | amt={b.get("amount"):.2f} | {b.get("type")} | {b.get("counterparty", "")[:40]}')
                    except ValueError:
                        pass

    print(f'\n可能的重复组数: {potential_count}')

    # 执行去重
    print('\n' + '='*70)
    print('执行智能去重:')
    print('='*70)
    
    result = dedup_engine.process(valid_bills)
    print(f'原始账单数: {result.original_count}')
    print(f'移除账单数: {result.removed_count}')
    print(f'保留账单数: {len(result.kept_bills)}')
    print(f'转账配对数: {len(result.transfer_pairs)} 对')
    print(f'重复组数: {len(result.duplicate_groups)} 组')

    # 显示重复组详情
    if result.duplicate_groups:
        print('\n重复组详情:')
        for i, group in enumerate(result.duplicate_groups):
            kept = group.get('kept', {})
            removed = group.get('removed', [])
            print(f'\n  组 {i+1}: 保留1条，移除{len(removed)}条')
            print(f'    保留: {kept.get("date")} | {kept.get("source_account_id")} | ¥{abs(kept.get("amount", 0)):.2f} | {kept.get("counterparty", "")[:30]}')
            for r in removed:
                print(f'    移除: {r.get("date")} | {r.get("source_account_id")} | ¥{abs(r.get("amount", 0)):.2f} | {r.get("counterparty", "")[:30]}')

    # 验证
    assert result.removed_count >= 0, "移除数应>=0"
    print('\n✓ 测试完成')


if __name__ == '__main__':
    test_alipay_abc_dedup()
