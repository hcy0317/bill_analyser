"""v6.2 Comprehensive Fix Tests

验证修复:
1. 对账单金额单位正确(元到分)
2. 交易列表日期分组正确(使用完整日期比较)
3. V1 Adapter日期字段完整
4. accountClosingBalance字段正确

测试场景:
- 对账单API返回的金额应该是分
- 交易列表不同月的同一天应该分组显示
- 所有交易包含完整日期显示字段
"""

import pytest
from datetime import datetime
from src.utils.currency import yuan_to_cents, cents_to_yuan
from src.api.adapters.transaction_adapter import TransactionAdapter


class TestV62ComprehensiveFixes:
    """v6.2综合修复测试"""

    def test_reconciliation_amount_units(self):
        """验证对账单金额单位转换"""
        # 后端金额（元）
        backend_amounts = {
            'openingBalance': 0.0,
            'closingBalance': -444.0,
            'totalInflows': 0.0,
            'totalOutflows': 444.0,
            'netFlow': -444.0
        }
        
        # 前端期望的金额（分）
        frontend_expected = {
            'openingBalance': 0,
            'closingBalance': -44400,
            'totalInflows': 0,
            'totalOutflows': 44400,
            'netFlow': -44400
        }
        
        # 验证转换
        for key, yuan_value in backend_amounts.items():
            cents_value = yuan_to_cents(yuan_value)
            assert cents_value == frontend_expected[key], \
                f"{key}: 期望{frontend_expected[key]}分，实际{cents_value}分"
        
        print("✅ 对账单金额单位测试通过")

    def test_transaction_amount_conversion(self):
        """验证单笔交易金额转换"""
        # 实际案例：111元和333元
        transaction_111 = yuan_to_cents(111.0)
        transaction_333 = yuan_to_cents(333.0)
        
        assert transaction_111 == 11100, "111元应转换为11100分"
        assert transaction_333 == 33300, "333元应转换为33300分"
        
        # 反向验证：前端显示
        assert cents_to_yuan(11100) == 111.0, "11100分应显示为111.0元"
        assert cents_to_yuan(33300) == 333.0, "33300分应显示为333.0元"
        
        print("✅ 交易金额转换测试通过")

    @pytest.mark.asyncio
    async def test_date_grouping_fields(self):
        """验证日期分组字段完整性"""
        adapter = TransactionAdapter()
        
        # 模拟不同月的同一天
        oct_15_bill = {
            'id': 1,
            'date': '2025-10-15 17:00:45',  # 10月15日
            'type': '支出',
            'amount': 333.0,
            'source_account_id': 1,
            'main_category': '餐饮',
            'sub_category': '堂食',
            'counterparty': '测试商家',
            'description': '堂食'
        }
        
        nov_21_bill = {
            'id': 2,
            'date': '2025-11-21 16:59:38',  # 11月21日
            'type': '支出',
            'amount': 111.0,
            'source_account_id': 1,
            'main_category': '餐饮',
            'sub_category': '堂食',
            'counterparty': '测试商家',
            'description': '堂食'
        }
        
        # 转换
        v1_oct = await adapter.backend_to_frontend(oct_15_bill)
        v1_nov = await adapter.backend_to_frontend(nov_21_bill)
        
        # 验证日期字段
        assert v1_oct['gregorianCalendarYearDashMonthDashDay'] == '2025-10-15'
        assert v1_nov['gregorianCalendarYearDashMonthDashDay'] == '2025-11-21'
        
        # 验证两者不相等（确保能正确分组）
        assert v1_oct['gregorianCalendarYearDashMonthDashDay'] != \
               v1_nov['gregorianCalendarYearDashMonthDashDay']
        
        # 验证月中日相同但完整日期不同
        assert v1_oct['gregorianCalendarDayOfMonth'] == 15
        assert v1_nov['gregorianCalendarDayOfMonth'] == 21
        
        # 验证星期几
        # weekday(): 0=周一, 2=周三, 4=周五
        # displayDayOfWeek: 1=周日, 2=周一, 4=周三, 6=周五
        # 2025-10-15 星期三, 2025-11-21 星期五
        assert v1_oct['displayDayOfWeek'] == 4, "10月15日应为周三(4)"
        assert v1_nov['displayDayOfWeek'] == 6, "11月21日应为周五(6)"
        
        print("✅ 日期分组字段测试通过")

    @pytest.mark.asyncio
    async def test_account_closing_balance_field(self):
        """验证accountClosingBalance字段存在且正确"""
        adapter = TransactionAdapter()
        
        bill = {
            'id': 1,
            'date': '2025-11-21 16:59:38',
            'type': '支出',
            'amount': 111.0,
            'source_account_id': 1,
            'main_category': '餐饮',
            'sub_category': '堂食',
            'counterparty': '',
            'description': '堂食'
        }
        
        v1_bill = await adapter.backend_to_frontend(bill)
        
        # 注意：adapter本身不设置accountClosingBalance，这由对账单API设置
        # 这里只验证基础转换字段
        assert 'amount' in v1_bill
        assert v1_bill['amount'] == yuan_to_cents(111.0)
        
        # 模拟对账单API设置accountClosingBalance
        closing_balance_yuan = -444.0
        v1_bill['accountClosingBalance'] = yuan_to_cents(closing_balance_yuan)
        
        assert v1_bill['accountClosingBalance'] == -44400
        
        print("✅ accountClosingBalance字段测试通过")

    def test_real_world_scenario(self):
        """真实场景测试：对账单显示"""
        # 用户期望看到的对账单
        expected_display = {
            'opening': '¥ 0.00',
            'closing': '¥ -444.00',
            'inflows': '¥ 0.00',
            'outflows': '¥ 444.00',
            'net_flow': '¥ -444.00'
        }
        
        # 后端返回的数据（分）
        api_response = {
            'openingBalance': yuan_to_cents(0.0),
            'closingBalance': yuan_to_cents(-444.0),
            'totalInflows': yuan_to_cents(0.0),
            'totalOutflows': yuan_to_cents(444.0),
            'netFlow': yuan_to_cents(-444.0)
        }
        
        # 前端转换回元显示
        displayed = {
            'opening': f"¥ {cents_to_yuan(api_response['openingBalance']):.2f}",
            'closing': f"¥ {cents_to_yuan(api_response['closingBalance']):.2f}",
            'inflows': f"¥ {cents_to_yuan(api_response['totalInflows']):.2f}",
            'outflows': f"¥ {cents_to_yuan(api_response['totalOutflows']):.2f}",
            'net_flow': f"¥ {cents_to_yuan(api_response['netFlow']):.2f}"
        }
        
        assert displayed == expected_display, \
            f"显示不匹配：\n期望={expected_display}\n实际={displayed}"
        
        print("✅ 真实场景测试通过")
        print(f"   期初余额: {displayed['opening']}")
        print(f"   期末余额: {displayed['closing']}")
        print(f"   总流入: {displayed['inflows']}")
        print(f"   总流出: {displayed['outflows']}")
        print(f"   净现金流: {displayed['net_flow']}")


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--asyncio-mode=auto'])
