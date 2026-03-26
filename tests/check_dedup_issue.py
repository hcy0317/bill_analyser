"""分析去重问题：检查bills_parser_template中ID 1和48为何未被去重."""
import sqlite3
import os

# 连接数据库
db_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data', 'bills.db')
conn = sqlite3.connect(db_path)
conn.row_factory = sqlite3.Row

print("=" * 100)
print("分析去重问题：bills_parser_template中ID 1和48")
print("=" * 100)

# 查看parser_template表中的记录
print("\n[1] bills_parser_template 表中 ID=1 和 ID=48 的记录:")
print("-" * 100)
cursor = conn.execute("""
    SELECT id, session_id, parser_date, parser_amount, parser_type, 
           parser_counterparty, parser_payment_method, parser_id, parser_is_processed
    FROM bills_parser_template 
    WHERE id IN (1, 48)
""")
records = cursor.fetchall()
for r in records:
    print(f"ID: {r['id']}")
    print(f"  session_id: {r['session_id']}")
    print(f"  日期: {r['parser_date']}")
    print(f"  金额: {r['parser_amount']}")
    print(f"  类型: {r['parser_type']}")
    print(f"  交易对方: {r['parser_counterparty']}")
    print(f"  支付方式: {r['parser_payment_method']}")
    print(f"  解析器ID: {r['parser_id']}")
    print(f"  是否已处理: {r['parser_is_processed']}")
    print()

# 查看preview表中对应的记录
print("\n[2] bills_preview 表中的相关记录:")
print("-" * 100)
cursor = conn.execute("""
    SELECT id, session_id, preview_date, preview_amount, preview_type,
           preview_counterparty, dedup_type, dedup_source_ids
    FROM bills_preview
    ORDER BY id
    LIMIT 20
""")
previews = cursor.fetchall()
for p in previews:
    print(f"ID: {p['id']}")
    print(f"  session_id: {p['session_id']}")
    print(f"  日期: {p['preview_date']}")
    print(f"  金额: {p['preview_amount']}")
    print(f"  类型: {p['preview_type']}")
    print(f"  交易对方: {p['preview_counterparty'][:50] if p['preview_counterparty'] else ''}")
    print(f"  去重类型: {p['dedup_type']}")
    print(f"  去重源IDs: {p['dedup_source_ids']}")
    print()

# 查看parser_template表的所有记录数量和分布
print("\n[3] bills_parser_template 表统计:")
print("-" * 100)
cursor = conn.execute("SELECT COUNT(*) as total FROM bills_parser_template")
total = cursor.fetchone()['total']
print(f"总记录数: {total}")

cursor = conn.execute("SELECT parser_id, COUNT(*) as cnt FROM bills_parser_template GROUP BY parser_id")
for r in cursor.fetchall():
    print(f"  {r['parser_id']}: {r['cnt']} 条")

# 检查可能重复的记录（相同金额和接近时间）
print("\n[4] 检查ID=1和ID=48是否应该被去重:")
print("-" * 100)

# 获取ID=1和48的详细信息
cursor = conn.execute("""
    SELECT * FROM bills_parser_template WHERE id IN (1, 48)
""")
bill_1 = None
bill_48 = None
for r in cursor.fetchall():
    if r['id'] == 1:
        bill_1 = dict(r)
    elif r['id'] == 48:
        bill_48 = dict(r)

if bill_1 and bill_48:
    print(f"账单1: 日期={bill_1['parser_date']}, 金额={bill_1['parser_amount']}, 来源={bill_1['parser_id']}")
    print(f"账单48: 日期={bill_48['parser_date']}, 金额={bill_48['parser_amount']}, 来源={bill_48['parser_id']}")
    
    # 比较
    from datetime import datetime
    try:
        dt1 = datetime.fromisoformat(bill_1['parser_date'].replace('Z', '+00:00'))
        dt2 = datetime.fromisoformat(bill_48['parser_date'].replace('Z', '+00:00'))
        time_diff = abs((dt1 - dt2).total_seconds())
        print(f"\n时间差: {time_diff} 秒")
    except Exception as e:
        print(f"时间解析错误: {e}")
    
    amt1 = float(bill_1['parser_amount']) if bill_1['parser_amount'] else 0
    amt2 = float(bill_48['parser_amount']) if bill_48['parser_amount'] else 0
    print(f"金额差: {abs(amt1 - amt2)}")
    print(f"金额符号相同: {amt1 * amt2 > 0}")
    print(f"来源不同: {bill_1['parser_id'] != bill_48['parser_id']}")
    
    # 检查去重条件
    is_platform_1 = bill_1['parser_id'] in ['wechat', 'alipay']
    is_platform_48 = bill_48['parser_id'] in ['wechat', 'alipay']
    is_bank_1 = not is_platform_1
    is_bank_48 = not is_platform_48
    
    print(f"\n账单1是平台: {is_platform_1}, 是银行: {is_bank_1}")
    print(f"账单48是平台: {is_platform_48}, 是银行: {is_bank_48}")
    print(f"是平台-银行对: {(is_platform_1 and is_bank_48) or (is_bank_1 and is_platform_48)}")

conn.close()
print("\n" + "=" * 100)
