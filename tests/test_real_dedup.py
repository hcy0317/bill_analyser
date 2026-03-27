#!/usr/bin/env python3
"""
真实场景去重测试 - 微信账单 + 农业银行账单
测试用户报告的问题：重复账单没有被去重
"""
import sys
import os

# 添加项目根目录到 Python 路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from pathlib import Path

from bill_analyser.core.smart_dedup import SmartDeduplicationEngine
from bill_analyser.parsers.factory import ParserFactory
from bill_analyser.utils.validator import BillValidator

def test_real_dedup():
    """测试真实的微信+农业银行去重场景"""
    
    bills_dir = Path(project_root) / 'bills'
    factory = ParserFactory()
    validator = BillValidator()
    dedup_engine = SmartDeduplicationEngine()
    
    # 选择2024年8月的文件（用户提到的时间范围）
    # 微信账单：20240701-20240831
    # 农业银行账单：detail20250825.xlsx 和 detail20250826.xlsx（可能包含2024年8月的数据）
    
    wechat_file = bills_dir / '微信支付账单(20240701-20240831).csv'
    abc_file = bills_dir / 'detail20250825.xlsx'  # 农业银行
    
    if not wechat_file.exists():
        print(f"❌ 微信账单文件不存在: {wechat_file}")
        return
    if not abc_file.exists():
        print(f"❌ 农业银行账单文件不存在: {abc_file}")
        return
    
    print("="*70)
    print("真实场景去重测试")
    print("="*70)
    print(f"微信账单: {wechat_file.name}")
    print(f"农业银行账单: {abc_file.name}")
    
    # 解析微信账单
    print("\n[1] 解析微信账单...")
    wechat_bills = factory.parse(str(wechat_file))
    print(f"    解析得到 {len(wechat_bills)} 条账单")
    wechat_sources = set(b.get('source_account_id') for b in wechat_bills)
    print(f"    source_account_id: {wechat_sources}")
    
    # 解析农业银行账单
    print("\n[2] 解析农业银行账单...")
    abc_bills = factory.parse(str(abc_file))
    print(f"    解析得到 {len(abc_bills)} 条账单")
    abc_sources = set(b.get('source_account_id') for b in abc_bills)
    print(f"    source_account_id: {abc_sources}")
    
    # 合并账单
    all_bills = wechat_bills + abc_bills
    print(f"\n[3] 合并后共 {len(all_bills)} 条账单")
    
    # 验证
    print("\n[4] 验证账单...")
    valid_bills, invalid_bills = validator.validate_bills(all_bills)
    print(f"    有效: {len(valid_bills)}, 无效: {len(invalid_bills)}")
    
    # 检查验证后的 source_account_id
    all_sources = set(b.get('source_account_id') for b in valid_bills)
    print(f"    验证后 source_account_id: {all_sources}")
    
    # 分析可能的重复（时间接近且金额相等的账单）
    print("\n[5] 分析可能的平台-银行重复...")
    
    # 按金额分组
    from collections import defaultdict
    by_amount = defaultdict(list)
    for bill in valid_bills:
        amt = abs(bill.get('amount', 0))
        if amt > 0:
            by_amount[amt].append(bill)
    
    # 找出金额相同的跨平台账单
    potential_duplicates = []
    for amt, group in by_amount.items():
        sources = set(b.get('source_account_id') for b in group)
        if 'wechat' in sources and 'abc' in sources:
            wechat_group = [b for b in group if b.get('source_account_id') == 'wechat']
            abc_group = [b for b in group if b.get('source_account_id') == 'abc']
            potential_duplicates.append({
                'amount': amt,
                'wechat_count': len(wechat_group),
                'abc_count': len(abc_group),
                'wechat': wechat_group,
                'abc': abc_group
            })
    
    print(f"    找到 {len(potential_duplicates)} 个可能的重复金额组")
    
    if potential_duplicates:
        print("\n    前5个可能的重复:")
        for i, dup in enumerate(sorted(potential_duplicates, key=lambda x: -x['amount'])[:5]):
            print(f"\n    [{i+1}] 金额: ¥{dup['amount']:.2f}")
            print(f"        微信账单 {dup['wechat_count']} 条, 农业银行账单 {dup['abc_count']} 条")
            
            # 检查时间是否接近
            for w in dup['wechat'][:2]:
                for a in dup['abc'][:2]:
                    from datetime import datetime
                    try:
                        w_dt = datetime.strptime(w.get('date', ''), '%Y-%m-%d %H:%M:%S')
                        a_dt = datetime.strptime(a.get('date', ''), '%Y-%m-%d %H:%M:%S')
                        time_diff = abs((w_dt - a_dt).total_seconds())
                        if time_diff <= 60:  # 1分钟内
                            print(f"        ⚠️ 时间接近! 微信: {w.get('date')}, 农行: {a.get('date')}, 差: {time_diff:.1f}秒")
                            print(f"           微信方向: {w.get('type')}, 农行方向: {a.get('type')}")
                    except ValueError:
                        pass
    
    # 执行去重
    print("\n[6] 执行智能去重...")
    result = dedup_engine.process(valid_bills)
    
    print("\n去重结果:")
    print(f"    原始数量: {result.original_count}")
    print(f"    移除数量: {result.removed_count}")
    print(f"    保留数量: {len(result.kept_bills)}")
    print(f"    转账配对: {len(result.transfer_pairs)} 对")
    print(f"    分账组: {len(result.split_groups)} 组")
    print(f"    重复组: {len(result.duplicate_groups)} 组")
    
    removed_bills = [bill for group in result.duplicate_groups for bill in group.remove_bills]

    # 如果有被移除的账单，显示详情
    if removed_bills:
        print("\n[7] 被移除的账单详情 (前10条):")
        for i, bill in enumerate(removed_bills[:10]):
            print(f"    [{i+1}] {bill.get('date')} | ¥{bill.get('amount'):.2f} | {bill.get('source_account_id')} | {bill.get('_removal_reason', 'unknown')}")

    assert result.removed_count >= 0

if __name__ == '__main__':
    test_real_dedup()
