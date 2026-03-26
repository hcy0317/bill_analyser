"""快速检查数据库中的账单数据"""
import asyncio
import sys
sys.path.insert(0, 'src')

from src.core.db import Database

async def check_bills():
    db = Database()
    try:
        # 查询所有账单
        bills, total = await db.query_bills(page=1, page_size=10)
        print(f"\n数据库中共有 {total} 条账单:")
        print("="*80)
        
        for i, bill in enumerate(bills, 1):
            print(f"\n账单 #{i}:")
            print(f"  ID: {bill['id']}")
            print(f"  日期: {bill['date']}")
            print(f"  类型: {bill['type']}")
            print(f"  金额: {bill['amount']}")
            print(f"  来源账户ID: {bill.get('source_account_id')}")
            print(f"  目标账户ID: {bill.get('destination_account_id')}")
            print(f"  描述: {bill.get('description', 'N/A')}")
        
        print("\n" + "="*80)
        
    finally:
        await db.close()

if __name__ == '__main__':
    asyncio.run(check_bills())
