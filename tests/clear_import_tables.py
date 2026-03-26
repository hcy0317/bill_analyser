#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""清空导入相关临时表"""

import sqlite3
import os

db_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'data', 'bills.db')
conn = sqlite3.connect(db_path)
c = conn.cursor()

# 清空三阶段临时表
c.execute('DELETE FROM import_sessions')
c.execute('DELETE FROM bills_parser_template')
c.execute('DELETE FROM bills_preview')
conn.commit()

print('已清空导入相关的临时表')
print(f'import_sessions: {c.execute("SELECT COUNT(*) FROM import_sessions").fetchone()[0]}')
print(f'bills_parser_template: {c.execute("SELECT COUNT(*) FROM bills_parser_template").fetchone()[0]}')
print(f'bills_preview: {c.execute("SELECT COUNT(*) FROM bills_preview").fetchone()[0]}')
conn.close()
