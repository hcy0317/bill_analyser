#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
查找数据库中的重复账单

用于 v6.45 修复后，查找并清理已导入的重复账单。

使用方法:
    .venv\Scripts\python scripts\find_duplicate_bills.py [--dry-run] [--delete]
    
参数:
    --dry-run  仅显示重复账单，不执行删除（默认）
    --delete   删除重复账单（保留最早导入的一条）
    
示例:
    # 查看重复账单
    .venv\Scripts\python scripts\find_duplicate_bills.py
    
    # 删除重复账单
    .venv\Scripts\python scripts\find_duplicate_bills.py --delete
"""

import sys
import os
import sqlite3
from datetime import datetime, timedelta
from collections import defaultdict

# 添加项目根目录到路径
project_root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, project_root)

DB_PATH = os.path.join(project_root, 'data', 'bills.db')


def find_duplicates(conn, time_tolerance_minutes=5, dry_run=True):
    """查找重复账单
    
    Args:
        conn: 数据库连接
        time_tolerance_minutes: 时间容差（分钟）
        dry_run: 是否仅预览不删除
        
    Returns:
        tuple: (重复组数, 删除数)
    """
    cursor = conn.cursor()
    
    # 获取所有账单，按日期和金额排序
    cursor.execute("""
        SELECT id, date, amount, description, counterparty, source_account_id, created_at
        FROM bills
        ORDER BY date, amount, id
    """)
    
    bills = cursor.fetchall()
    print(f"总账单数: {len(bills)}")
    
    # 按日期+金额分组
    groups = defaultdict(list)
    for bill in bills:
        bill_id, date_str, amount, desc, counterparty, source_id, created_at = bill
        # 用日期（精确到天）和金额作为分组键
        date_key = date_str[:10] if date_str else ''
        key = f"{date_key}_{float(amount):.2f}"
        groups[key].append({
            'id': bill_id,
            'date': date_str,
            'amount': amount,
            'description': desc,
            'counterparty': counterparty,
            'source_account_id': source_id,
            'created_at': created_at
        })
    
    # 检查每组是否有真正的重复
    duplicate_groups = []
    for key, bills_in_group in groups.items():
        if len(bills_in_group) < 2:
            continue
        
        # 检查时间是否接近
        for i in range(len(bills_in_group)):
            for j in range(i + 1, len(bills_in_group)):
                bill_a = bills_in_group[i]
                bill_b = bills_in_group[j]
                
                # 解析时间
                try:
                    dt_a = datetime.fromisoformat(bill_a['date'].replace('T', ' ').split('.')[0])
                    dt_b = datetime.fromisoformat(bill_b['date'].replace('T', ' ').split('.')[0])
                except (ValueError, AttributeError):
                    continue
                
                time_diff = abs((dt_a - dt_b).total_seconds())
                
                # 时间差在容差内
                if time_diff <= time_tolerance_minutes * 60:
                    # 检查描述或交易对手是否相似
                    desc_a = str(bill_a.get('description', '')).lower()
                    desc_b = str(bill_b.get('description', '')).lower()
                    cp_a = str(bill_a.get('counterparty', '')).lower()
                    cp_b = str(bill_b.get('counterparty', '')).lower()
                    
                    similar = False
                    if desc_a and desc_b and (desc_a in desc_b or desc_b in desc_a):
                        similar = True
                    elif cp_a and cp_b and (cp_a in cp_b or cp_b in cp_a):
                        similar = True
                    elif not desc_a and not desc_b:
                        similar = True
                    
                    if similar:
                        duplicate_groups.append((bill_a, bill_b))
    
    print(f"\n发现 {len(duplicate_groups)} 对重复账单:")
    print("-" * 80)
    
    to_delete = set()
    for bill_a, bill_b in duplicate_groups:
        # 保留 created_at 较早的，删除较晚的
        created_a = bill_a.get('created_at', '')
        created_b = bill_b.get('created_at', '')
        
        if created_a <= created_b:
            keep, remove = bill_a, bill_b
        else:
            keep, remove = bill_b, bill_a
        
        to_delete.add(remove['id'])
        
        print(f"保留 ID={keep['id']:6d}: {keep['date'][:19]} | {keep['amount']:10.2f} | "
              f"{str(keep['description'])[:30]}")
        print(f"删除 ID={remove['id']:6d}: {remove['date'][:19]} | {remove['amount']:10.2f} | "
              f"{str(remove['description'])[:30]}")
        print()
    
    print("-" * 80)
    print(f"需要删除: {len(to_delete)} 条重复账单")
    
    if not dry_run and to_delete:
        print("\n正在删除...")
        for bill_id in to_delete:
            cursor.execute("DELETE FROM bills WHERE id = ?", (bill_id,))
        conn.commit()
        print(f"已删除 {len(to_delete)} 条重复账单")
    elif to_delete:
        print("\n(使用 --delete 参数执行实际删除)")
    
    return len(duplicate_groups), len(to_delete)


def main():
    """主函数"""
    dry_run = '--delete' not in sys.argv
    
    if not os.path.exists(DB_PATH):
        print(f"错误: 数据库不存在: {DB_PATH}")
        sys.exit(1)
    
    print(f"数据库: {DB_PATH}")
    print(f"模式: {'预览 (dry-run)' if dry_run else '删除'}")
    print()
    
    conn = sqlite3.connect(DB_PATH)
    try:
        groups, deleted = find_duplicates(conn, dry_run=dry_run)
        
        if groups == 0:
            print("\n✓ 未发现重复账单")
        elif dry_run:
            print(f"\n发现 {groups} 对重复，共 {deleted} 条需要删除")
            print("使用 --delete 参数执行实际删除")
        else:
            print(f"\n✓ 已清理 {deleted} 条重复账单")
    finally:
        conn.close()


if __name__ == '__main__':
    main()
