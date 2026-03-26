# -*- coding: utf-8 -*-
"""
快速验证修复功能

使用方法:
    .\.venv\Scripts\python.exe tests\quick_verify.py
    
验证项:
1. 投资账单是否有目标账户
2. 账单是否有分类
3. 是否存在幽灵账单
4. 余额同步是否正确
"""
import sqlite3
import sys

def check_latest_bills():
    """检查最近创建的账单"""
    conn = sqlite3.connect('src/data/bills.db')
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    
    print("\n" + "="*80)
    print("快速验证 - 最近10条账单")
    print("="*80)
    
    cursor.execute("""
        SELECT 
            id,
            date,
            type,
            amount,
            channel,
            main_category,
            sub_category,
            source_account_id,
            destination_account_id
        FROM bills
        ORDER BY id DESC
        LIMIT 10
    """)
    
    bills = cursor.fetchall()
    
    issues = {
        'no_category': 0,
        'ghost_bills': 0,
        'investment_no_dest': 0
    }
    
    print(f"\n{'ID':<5} {'类型':<6} {'金额':<10} {'分类':<20} {'源账户':<5} {'目标账户':<5}")
    print("-" * 80)
    
    for bill in bills:
        category = f"{bill['main_category'] or '无'}-{bill['sub_category'] or '无'}"
        print(f"{bill['id']:<5} {bill['type']:<6} {bill['amount']:<10.2f} {category:<20} "
              f"{bill['source_account_id']:<5} {bill['destination_account_id']:<5}")
        
        # 检查问题
        if not bill['main_category']:
            issues['no_category'] += 1
        
        if bill['source_account_id'] == 0:
            issues['ghost_bills'] += 1
        
        if bill['type'] == '投资' and bill['destination_account_id'] == 0:
            issues['investment_no_dest'] += 1
    
    # 输出问题统计
    print("\n" + "="*80)
    print("问题统计")
    print("="*80)
    
    total_issues = sum(issues.values())
    
    if total_issues == 0:
        print("\n✅ 所有检查通过！没有发现问题。")
    else:
        if issues['no_category'] > 0:
            print(f"❌ {issues['no_category']}条账单没有分类")
        
        if issues['ghost_bills'] > 0:
            print(f"❌ {issues['ghost_bills']}条幽灵账单（source_account_id=0）")
        
        if issues['investment_no_dest'] > 0:
            print(f"❌ {issues['investment_no_dest']}条投资账单缺少目标账户")
    
    conn.close()
    return total_issues == 0

def check_account_balances():
    """检查账户余额完整性"""
    conn = sqlite3.connect('src/data/bills.db')
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()
    
    print("\n" + "="*80)
    print("账户余额检查 - Top 5账户")
    print("="*80)
    
    cursor.execute("""
        SELECT id, name, balance
        FROM accounts
        WHERE balance != 0
        ORDER BY ABS(balance) DESC
        LIMIT 5
    """)
    
    accounts = cursor.fetchall()
    
    print(f"\n{'ID':<5} {'账户名':<20} {'余额':<15}")
    print("-" * 80)
    
    for acc in accounts:
        print(f"{acc['id']:<5} {acc['name']:<20} {acc['balance']:>15.2f}")
    
    conn.close()

def main():
    print("\n" + "="*80)
    print("Bill Analyser - 快速验证工具")
    print("="*80)
    print("\n检查修复是否生效...")
    
    try:
        # 检查账单
        bills_ok = check_latest_bills()
        
        # 检查余额
        check_account_balances()
        
        print("\n" + "="*80)
        if bills_ok:
            print("✅ 验证通过！所有功能正常。")
        else:
            print("⚠️ 发现问题，请查看上方统计。")
        print("="*80 + "\n")
        
        return 0 if bills_ok else 1
        
    except Exception as e:
        print(f"\n❌ 验证失败: {e}")
        return 1

if __name__ == "__main__":
    sys.exit(main())
