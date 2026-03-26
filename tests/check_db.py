import sqlite3

conn = sqlite3.connect('src/data/bills.db')
cursor = conn.cursor()

# 总数
cursor.execute('SELECT COUNT(*) FROM bills')
total = cursor.fetchone()[0]
print(f'总记录数: {total}')

# 分类统计
cursor.execute('''
    SELECT main_category, sub_category, COUNT(*) 
    FROM bills 
    WHERE main_category IS NOT NULL 
    GROUP BY main_category, sub_category 
    ORDER BY COUNT(*) DESC 
    LIMIT 10
''')
results = cursor.fetchall()
print('\n前10个分类:')
for row in results:
    print(f'  {row[0]}/{row[1]}: {row[2]}条')

# 未分类统计
cursor.execute('SELECT COUNT(*) FROM bills WHERE main_category IS NULL')
uncategorized = cursor.fetchone()[0]
print(f'\n未分类: {uncategorized}条')

# 最近5条
cursor.execute('''
    SELECT date, counterparty, amount, main_category, sub_category, description
    FROM bills 
    ORDER BY date DESC 
    LIMIT 5
''')
recent = cursor.fetchall()
print('\n最近5条记录:')
for r in recent:
    cat = f"{r[3] or '未分类'}/{r[4] or ''}"
    print(f'  {r[0]} | {r[1][:15]:15s} | ¥{r[2]:8.2f} | {cat:20s} | {r[5][:20] if r[5] else ""}')

conn.close()
