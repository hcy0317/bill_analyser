"""查看数据库账户信息"""
import asyncio
import sys
from pathlib import Path

# 添加项目根目录到Python路径
sys.path.insert(0, str(Path(__file__).parent.parent))

from src.core.db import Database

async def main():
    db = Database()
    accounts = await db.get_all_accounts()
    print("\n所有账户:")
    print("=" * 50)
    for acc in accounts:
        print(f"ID={acc['id']}, Name={acc['name']}, Balance={acc.get('balance', 0)}")
    print("=" * 50)
    print(f"\n总共{len(accounts)}个账户\n")

if __name__ == '__main__':
    asyncio.run(main())
