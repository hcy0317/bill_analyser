#!/usr/bin/env python3
"""
查找2025年8月的账单文件
"""
import sys
import os

project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

from pathlib import Path
from src.parsers.factory import ParserFactory
from datetime import datetime

def find_aug_2025_bills():
    """查找包含2025年8月数据的账单文件"""
    
    bills_dir = Path(project_root) / 'bills'
    factory = ParserFactory()
    
    print("="*70)
    print("查找2025年8月的账单文件")
    print("="*70)
    
    # 遍历所有账单文件
    for file_path in sorted(bills_dir.iterdir()):
        if not file_path.is_file():
            continue
            
        try:
            bills = factory.parse(str(file_path))
            if not bills:
                continue
            
            # 统计2025年8月的账单
            aug_2025_count = 0
            for bill in bills:
                try:
                    dt = datetime.strptime(bill.get('date', ''), '%Y-%m-%d %H:%M:%S')
                    if dt.year == 2025 and dt.month == 8:
                        aug_2025_count += 1
                except:
                    pass
            
            if aug_2025_count > 0:
                source_ids = set(b.get('source_account_id') for b in bills)
                print(f"\n{file_path.name}")
                print(f"  来源: {source_ids}")
                print(f"  总条数: {len(bills)}")
                print(f"  2025年8月: {aug_2025_count} 条")
                
        except Exception as e:
            pass

if __name__ == '__main__':
    find_aug_2025_bills()
