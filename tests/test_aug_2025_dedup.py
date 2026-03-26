#!/usr/bin/env python3
"""
测试2025年8月的微信+农业银行账单去重
这是用户报告的真实场景
"""
import sys
import os

project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from pathlib import Path
from src.parsers.factory import ParserFactory
from src.utils.validator import BillValidator
from src.core.smart_dedup import SmartDeduplicationEngine
from datetime import datetime

def test_aug_2025_dedup():
    """测试2025年8月微信+农业银行去重"""
    
    bills_dir = Path(project_root) / 'bills'
    factory = ParserFactory()
    validator = BillValidator()
    dedup_engine = SmartDeduplicationEngine()
    
    # 2025年8月的账单文件
    wechat_file = bills_dir / '微信支付账单流水文件(20250604-20250825)——【解压密码可在微信支付公众号查看】.xlsx'
    abc_file = bills_dir / 'detail20250826.xlsx'
    
    print("="*70)
    print("2025年8月微信+农业银行账单去重测试")
    print("="*70)
    print(f"微信账单: {wechat_file.name}")
    print(f"农业银行账单: {abc_file.name}")
    
    # 解析微信账单
    print("\n[1] 解析微信账单...")
    wechat_bills = factory.parse(str(wechat_file))
    # 只取2025年8月的
    wechat_aug = [b for b in wechat_bills if '2025-08' in b.get('date', '')]
    print(f"    总共解析 {len(wechat_bills)} 条，2025年8月: {len(wechat_aug)} 条")
    print(f"    source_account_id: {set(b.get('source_account_id') for b in wechat_aug)}")
    
    # 解析农业银行账单
    print("\n[2] 解析农业银行账单...")
    abc_bills = factory.parse(str(abc_file))
    # 只取2025年8月的
    abc_aug = [b for b in abc_bills if '2025-08' in b.get('date', '')]
    print(f"    总共解析 {len(abc_bills)} 条，2025年8月: {len(abc_aug)} 条")
    print(f"    source_account_id: {set(b.get('source_account_id') for b in abc_aug)}")
    
    # 合并
    all_bills = wechat_aug + abc_aug
    print(f"\n[3] 合并后共 {len(all_bills)} 条账单")
    
    # 验证
    print("\n[4] 验证账单...")
    valid_bills, invalid_bills = validator.validate_bills(all_bills)
    print(f"    有效: {len(valid_bills)}, 无效: {len(invalid_bills)}")
    
    # 分析可能的重复
    print("\n[5] 分析可能的平台-银行重复...")
    from collections import defaultdict
    by_amount = defaultdict(list)
    for bill in valid_bills:
        amt = abs(bill.get('amount', 0))
        if amt > 0:
            by_amount[amt].append(bill)
    
    potential_dups = []
    for amt, group in by_amount.items():
        sources = set(b.get('source_account_id') for b in group)
        if 'wechat' in sources and 'abc' in sources:
            wechat_group = [b for b in group if b.get('source_account_id') == 'wechat']
            abc_group = [b for b in group if b.get('source_account_id') == 'abc']
            potential_dups.append({
                'amount': amt,
                'wechat': wechat_group,
                'abc': abc_group
            })
    
    print(f"    找到 {len(potential_dups)} 个金额相同的跨平台组")
    
    if potential_dups:
        print("\n    可能的重复详情:")
        for dup in sorted(potential_dups, key=lambda x: -x['amount'])[:5]:
            print(f"\n    金额: ¥{dup['amount']:.2f}")
            print(f"        微信 ({len(dup['wechat'])} 条):")
            for w in dup['wechat'][:2]:
                print(f"          {w.get('date')} | {w.get('counterparty', '')[:20]} | 方向:{w.get('type')}")
            print(f"        农行 ({len(dup['abc'])} 条):")
            for a in dup['abc'][:2]:
                print(f"          {a.get('date')} | {a.get('counterparty', '')[:20]} | 方向:{a.get('type')}")
            
            # 检查时间差
            for w in dup['wechat']:
                for a in dup['abc']:
                    try:
                        w_dt = datetime.strptime(w.get('date', ''), '%Y-%m-%d %H:%M:%S')
                        a_dt = datetime.strptime(a.get('date', ''), '%Y-%m-%d %H:%M:%S')
                        time_diff = abs((w_dt - a_dt).total_seconds())
                        if time_diff <= 60:
                            w_type = w.get('type', '')
                            a_type = a.get('type', '')
                            w_amt = w.get('amount', 0)
                            a_amt = a.get('amount', 0)
                            same_direction = w_amt * a_amt > 0
                            print(f"        ⚠️ 时间接近! 差: {time_diff:.1f}秒")
                            print(f"           微信: {w_type} {w_amt:.2f}")
                            print(f"           农行: {a_type} {a_amt:.2f}")
                            print(f"           方向相同: {same_direction}")
                    except:
                        pass
    
    # 执行去重
    print("\n[6] 执行智能去重...")
    result = dedup_engine.process(valid_bills)
    
    print(f"\n去重结果:")
    print(f"    原始数量: {result.original_count}")
    print(f"    移除数量: {result.removed_count}")
    print(f"    保留数量: {len(result.kept_bills)}")
    print(f"    转账配对: {len(result.transfer_pairs)} 对")
    print(f"    分账组: {len(result.split_groups)} 组")
    print(f"    重复组: {len(result.duplicate_groups)} 组")
    
    if result.duplicate_groups:
        print("\n[7] 重复组详情:")
        for i, group in enumerate(result.duplicate_groups[:5]):
            print(f"    重复组 [{i+1}]:")
            for bill in group:
                print(f"      {bill.get('date')} | ¥{bill.get('amount'):.2f} | {bill.get('source_account_id')} | {bill.get('counterparty', '')[:20]}")

if __name__ == '__main__':
    test_aug_2025_dedup()
