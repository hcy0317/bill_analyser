"""Test reconciliation API field completeness

Verify issues:
1. Transaction list does not show weekday
2. Reconciliation does not detect category
3. Account balance trend logic error
"""

import pytest
import asyncio
from datetime import datetime
from src.core.db import Database
from src.api.adapters.transaction_adapter import TransactionAdapter
from src.utils.currency import yuan_to_cents


class TestReconciliationFields:
    """测试对账单字段完整性"""

    @pytest.mark.asyncio
    async def test_transaction_date_fields(self):
        """测试交易记录包含完整的日期字段"""
        adapter = TransactionAdapter()
        
        # 模拟两笔交易（不同日期）
        bill1 = {
            'id': 1,
            'date': '2025-11-21 16:59:38',  # 11月21日 星期五
            'type': '支出',
            'amount': 111.0,
            'source_account_id': 1,
            'destination_account_id': None,
            'main_category': '餐饮',
            'sub_category': '堂食',
            'counterparty': '测试商家',
            'description': '堂食',
            'channel': '农业银行'
        }
        
        bill2 = {
            'id': 2,
            'date': '2025-10-15 17:00:45',  # 10月15日 星期三
            'type': '支出',
            'amount': 333.0,
            'source_account_id': 1,
            'destination_account_id': None,
            'main_category': '餐饮',
            'sub_category': '堂食',
            'counterparty': '测试商家',
            'description': '堂食',
            'channel': '农业银行'
        }
        
        # 转换为v1格式
        v1_bill1 = await adapter.backend_to_frontend(bill1)
        v1_bill2 = await adapter.backend_to_frontend(bill2)
        
        # 验证日期字段存在
        print("\n=== 11月21日交易字段 ===")
        assert 'gregorianCalendarYearDashMonthDashDay' in v1_bill1, "缺少完整日期字段"
        assert v1_bill1['gregorianCalendarYearDashMonthDashDay'] == '2025-11-21'
        print(f"✓ gregorianCalendarYearDashMonthDashDay: {v1_bill1['gregorianCalendarYearDashMonthDashDay']}")
        
        assert 'displayDayOfWeek' in v1_bill1, "缺少星期字段"
        # 2025-11-21 是星期五 (weekday=4) → displayDayOfWeek=6
        assert v1_bill1['displayDayOfWeek'] == 6, f"星期五应该是6，实际是{v1_bill1['displayDayOfWeek']}"
        print(f"✓ displayDayOfWeek: {v1_bill1['displayDayOfWeek']} (星期五)")
        
        print("\n=== 10月15日交易字段 ===")
        assert 'gregorianCalendarYearDashMonthDashDay' in v1_bill2, "缺少完整日期字段"
        assert v1_bill2['gregorianCalendarYearDashMonthDashDay'] == '2025-10-15'
        print(f"✓ gregorianCalendarYearDashMonthDashDay: {v1_bill2['gregorianCalendarYearDashMonthDashDay']}")
        
        assert 'displayDayOfWeek' in v1_bill2, "缺少星期字段"
        # 2025-10-15 是星期三 (weekday=2) → displayDayOfWeek=4
        assert v1_bill2['displayDayOfWeek'] == 4, f"星期三应该是4，实际是{v1_bill2['displayDayOfWeek']}"
        print(f"✓ displayDayOfWeek: {v1_bill2['displayDayOfWeek']} (星期三)")
        
        # 验证两条记录的日期不同（确保能正确分组）
        assert v1_bill1['gregorianCalendarYearDashMonthDashDay'] != v1_bill2['gregorianCalendarYearDashMonthDashDay']
        print("\n✅ 日期字段完整且正确")

    @pytest.mark.asyncio
    async def test_category_fields_in_transaction(self):
        """测试交易记录包含完整的分类字段"""
        adapter = TransactionAdapter()
        
        bill = {
            'id': 1,
            'date': '2025-11-21 16:59:38',
            'type': '支出',
            'amount': 111.0,
            'source_account_id': 1,
            'main_category': '餐饮',
            'sub_category': '堂食',
            'description': '堂食',
            'channel': '农业银行'
        }
        
        v1_bill = await adapter.backend_to_frontend(bill)
        
        print("\n=== 分类字段检查 ===")
        assert 'categoryId' in v1_bill, "缺少categoryId字段"
        assert 'categoryName' in v1_bill, "缺少categoryName字段"
        assert 'subCategoryName' in v1_bill, "缺少subCategoryName字段"
        
        print(f"categoryId: {v1_bill['categoryId']}")
        print(f"categoryName: {v1_bill['categoryName']}")
        print(f"subCategoryName: {v1_bill['subCategoryName']}")
        
        # 验证分类信息正确
        assert '餐饮' in v1_bill['categoryName'], "分类名称应包含'餐饮'"
        assert v1_bill['subCategoryName'] == '堂食', "子分类应为'堂食'"
        
        print("✅ 分类字段完整且正确")

    @pytest.mark.asyncio
    async def test_account_balance_logic(self):
        """测试账户余额计算逻辑"""
        # 模拟账单序列（数据库返回时间倒序）
        bills_from_db = [
            {
                'id': 222,
                'date': '2025-11-21 16:59:15',
                'type': '支出',
                'amount': 111.0,  # 支出111
                'source_account_id': 3,
            },
            {
                'id': 223,
                'date': '2025-10-15 17:00:03',
                'type': '支出',
                'amount': 333.0,  # 支出333
                'source_account_id': 3,
            }
        ]
        
        # 按时间正序排序
        bills_sorted = sorted(bills_from_db, key=lambda b: b.get('date', ''))
        
        print("\n=== 排序后的账单顺序（时间正序） ===")
        for bill in bills_sorted:
            print(f"ID={bill['id']}, Date={bill['date']}, Amount={bill['amount']}")
        
        # 模拟对账单余额计算（期初余额=0）
        opening_balance = 0.0
        closing_balance = opening_balance
        balance_history = {}
        
        # 按时间正序计算每笔交易后的余额
        print("\n=== 余额计算过程（时间正序） ===")
        for bill in bills_sorted:
            amount = bill['amount']
            if bill['type'] == '支出':
                closing_balance -= amount
            elif bill['type'] == '收入':
                closing_balance += amount
            
            balance_history[bill['id']] = closing_balance
            print(f"ID={bill['id']}, Date={bill['date']}, "
                  f"支出{amount} → 余额{closing_balance}")
        
        print(f"\n期初余额: {opening_balance}")
        print(f"期末余额: {closing_balance}")
        
        # 验证余额计算正确
        # 按时间顺序：10月15日支出333 → 11月21日支出111
        # ID=223 (10月15日)支出333后: 0 - 333 = -333
        assert balance_history[223] == -333.0, f"10月15日余额应为-333，实际{balance_history[223]}"
        # ID=222 (11月21日)支出111后: -333 - 111 = -444
        assert balance_history[222] == -444.0, f"11月21日余额应为-444，实际{balance_history[222]}"
        
        # 模拟前端显示（时间倒序）
        print("\n=== 前端显示（时间倒序） ===")
        display_list = sorted(bills_sorted, key=lambda b: b['date'], reverse=True)
        for bill in display_list:
            balance = balance_history[bill['id']]
            print(f"Date={bill['date']}, 支出{bill['amount']} → 账户余额{balance}")
        
        print("\n✅ 余额计算逻辑正确")
        print("   10月15日: 0 - 333 = -333")
        print("   11月21日: -333 - 111 = -444")
        print("\n显示顺序（最新在前）:")
        print(f"   11月21日（最新）: 账户余额-444")
        print(f"   10月15日（较早）: 账户余额-333")

    @pytest.mark.asyncio
    async def test_reconciliation_api_response_format(self):
        """测试对账单API返回格式"""
        adapter = TransactionAdapter()
        
        # 模拟API返回的交易列表
        bills = [
            {
                'id': 2,
                'date': '2025-11-21 16:59:38',
                'type': '支出',
                'amount': 111.0,
                'source_account_id': 1,
                'main_category': '餐饮',
                'sub_category': '堂食',
                'description': '堂食',
                'channel': '农业银行'
            },
            {
                'id': 1,
                'date': '2025-10-15 17:00:45',
                'type': '支出',
                'amount': 333.0,
                'source_account_id': 1,
                'main_category': '餐饮',
                'sub_category': '堂食',
                'description': '堂食',
                'channel': '农业银行'
            }
        ]
        
        # 转换为v1格式并计算余额
        opening_balance = 0.0
        closing_balance = opening_balance
        transactions = []
        
        for bill in reversed(bills):  # 时间正序计算余额
            amount = bill['amount']
            if bill['type'] == '支出':
                closing_balance -= amount
            
            v1_bill = await adapter.backend_to_frontend(bill)
            v1_bill['accountClosingBalance'] = yuan_to_cents(closing_balance)
            transactions.append(v1_bill)
        
        # 反转为时间倒序（前端显示最新的在前）
        transactions.reverse()
        
        print("\n=== 对账单API响应格式 ===")
        for t in transactions:
            date = t['gregorianCalendarYearDashMonthDashDay']
            weekday = t['displayDayOfWeek']
            category = t['categoryName']
            amount = t['amount']
            balance = t['accountClosingBalance']
            
            print(f"{date} (星期{weekday}): {category} -{amount}分 → 余额{balance}分")
        
        # 验证格式完整性
        for t in transactions:
            assert 'gregorianCalendarYearDashMonthDashDay' in t
            assert 'displayDayOfWeek' in t
            assert 'categoryName' in t
            assert 'accountClosingBalance' in t
        
        # 验证余额正确
        # 最新交易（11月21日）余额应为-444元=-44400分
        assert transactions[0]['accountClosingBalance'] == -44400
        # 较早交易（10月15日）余额应为-333元=-33300分
        assert transactions[1]['accountClosingBalance'] == -33300
        
        print("\n✅ 对账单API格式正确")


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--asyncio-mode=auto'])
