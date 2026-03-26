"""检查测试数据"""
import asyncio
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from src.core.db import Database

async def main():
    db = Database()
    
    # 查询ID=222和223的账单
    bill_222 = await db.get_bill_by_id(222)
    bill_223 = await db.get_bill_by_id(223)
    
    print("\n=== 账单详情 ===")
    print(f"\n账单#222:")
    print(f"  ID: {bill_222.get('id')}")
    print(f"  日期: {bill_222.get('date')}")
    print(f"  类型: {bill_222.get('type')}")
    print(f"  金额: {bill_222.get('amount')}元")
    print(f"  源账户ID: {bill_222.get('source_account_id')}")
    print(f"  目标账户ID: {bill_222.get('destination_account_id')}")
    print(f"  账户名称(channel): {bill_222.get('channel')}")
    print(f"  分类: {bill_222.get('main_category')}-{bill_222.get('sub_category')}")
    
    print(f"\n账单#223:")
    print(f"  ID: {bill_223.get('id')}")
    print(f"  日期: {bill_223.get('date')}")
    print(f"  类型: {bill_223.get('type')}")
    print(f"  金额: {bill_223.get('amount')}元")
    print(f"  源账户ID: {bill_223.get('source_account_id')}")
    print(f"  目标账户ID: {bill_223.get('destination_account_id')}")
    print(f"  账户名称(channel): {bill_223.get('channel')}")
    print(f"  分类: {bill_223.get('main_category')}-{bill_223.get('sub_category')}")
    
    # 查询农业银行账户信息
    account = await db.get_account_by_id(3)
    print(f"\n=== 农业银行账户 ===")
    print(f"  ID: {account['id']}")
    print(f"  名称: {account['name']}")

if __name__ == '__main__':
    asyncio.run(main())
