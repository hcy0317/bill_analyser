#!/usr/bin/env python3
"""
详细分析：检查微信和农业银行账单的时间范围
"""
import sys
import os

project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from pathlib import Path
from src.parsers.factory import ParserFactory
from datetime import datetime

def analyze_time_ranges():
    """分析账单时间范围"""
    
    bills_dir = Path(project_root) / 'bills'
    factory = ParserFactory()
    
    # 微信账单 2024年7-8月
    wechat_file = bills_dir / '微信支付账单(20240701-20240831).csv'
    wechat_bills = factory.parse(str(wechat_file))
    
    # 农业银行账单
    abc_file = bills_dir / 'detail20250825.xlsx'
    abc_bills = factory.parse(str(abc_file))
    
    print("="*70)
    print("账单时间范围分析")
    print("="*70)
    
    # 分析微信账单时间
    wechat_dates = []
    for bill in wechat_bills:
        try:
            dt = datetime.strptime(bill.get('date', ''), '%Y-%m-%d %H:%M:%S')
            wechat_dates.append(dt)
        except:
            pass
    
    if wechat_dates:
        print(f"\n微信账单 ({len(wechat_bills)} 条):")
        print(f"  最早: {min(wechat_dates)}")
        print(f"  最晚: {max(wechat_dates)}")
    
    # 分析农业银行账单时间
    abc_dates = []
    for bill in abc_bills:
        try:
            dt = datetime.strptime(bill.get('date', ''), '%Y-%m-%d %H:%M:%S')
            abc_dates.append(dt)
        except:
            pass
    
    if abc_dates:
        print(f"\n农业银行账单 ({len(abc_bills)} 条):")
        print(f"  最早: {min(abc_dates)}")
        print(f"  最晚: {max(abc_dates)}")
    
    # 检查是否有时间重叠
    if wechat_dates and abc_dates:
        wechat_range = (min(wechat_dates), max(wechat_dates))
        abc_range = (min(abc_dates), max(abc_dates))
        
        has_overlap = wechat_range[0] <= abc_range[1] and wechat_range[1] >= abc_range[0]
        
        print(f"\n时间范围是否重叠: {'是' if has_overlap else '否'}")
        
        if not has_overlap:
            print("  ⚠️ 两个账单文件的时间范围完全不重叠，无法检测重复！")
            print("  ⚠️ 微信账单是2024年7-8月，农业银行账单可能是2025年的数据")
    
    # 让我们看看农业银行账单中有没有2024年8月的数据
    print("\n农业银行账单中2024年8月的数据:")
    aug_2024_bills = []
    for bill in abc_bills:
        try:
            dt = datetime.strptime(bill.get('date', ''), '%Y-%m-%d %H:%M:%S')
            if dt.year == 2024 and dt.month == 8:
                aug_2024_bills.append(bill)
        except:
            pass
    
    if aug_2024_bills:
        print(f"  找到 {len(aug_2024_bills)} 条2024年8月的农业银行账单:")
        for i, bill in enumerate(aug_2024_bills[:10]):
            print(f"    [{i+1}] {bill.get('date')} | ¥{bill.get('amount'):.2f} | {bill.get('counterparty', '')[:20]}")
    else:
        print("  未找到2024年8月的农业银行账单")
    
    # 检查另一个农业银行文件
    print("\n检查 detail20250826.xlsx...")
    abc_file2 = bills_dir / 'detail20250826.xlsx'
    abc_bills2 = factory.parse(str(abc_file2))
    
    abc2_aug_2024 = []
    for bill in abc_bills2:
        try:
            dt = datetime.strptime(bill.get('date', ''), '%Y-%m-%d %H:%M:%S')
            if dt.year == 2024 and dt.month == 8:
                abc2_aug_2024.append(bill)
        except:
            pass
    
    if abc2_aug_2024:
        print(f"  找到 {len(abc2_aug_2024)} 条2024年8月的账单:")
        for i, bill in enumerate(abc2_aug_2024[:10]):
            print(f"    [{i+1}] {bill.get('date')} | ¥{bill.get('amount'):.2f} | {bill.get('counterparty', '')[:20]}")
    else:
        print("  未找到2024年8月的农业银行账单")

if __name__ == '__main__':
    analyze_time_ranges()
