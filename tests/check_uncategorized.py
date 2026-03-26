import sqlite3

conn = sqlite3.connect('src/data/bills.db')
cursor = conn.cursor()

# 查询未分类账单
cursor.execute('''
    SELECT date, counterparty, amount, description
    FROM bills 
    WHERE main_category IS NULL
    ORDER BY date DESC
''')
uncategorized = cursor.fetchall()

print(f'未分类账单 (共{len(uncategorized)}条):\n')
for r in uncategorized:
    print(f'{r[0]} | {r[1]:20s} | ¥{r[2]:8.2f} | {r[3][:40] if r[3] else ""}')

conn.close()
