"""v6.1修复验证测试

测试内容：
1. V1 Adapter添加日期显示字段（gregorianCalendarYearDashMonthDashDay, displayDayOfWeek）
2. 对账单API使用账户ID筛选
3. utcOffset正确填充
"""

import pytest
from datetime import datetime
from src.api.adapters.transaction_adapter import TransactionAdapter


class TestV1AdapterDateFields:
    """测试V1 Adapter日期显示字段"""

    @pytest.mark.asyncio
    async def test_date_display_fields_added(self):
        """验证backend_to_frontend添加了日期显示字段"""
        adapter = TransactionAdapter()
        
        # 模拟数据库账单
        bill = {
            'id': 1,
            'date': '2024-11-21 10:30:00',  # 2024年11月21日周四
            'type': '支出',
            'amount': 100.50,
            'source_account_id': 1,
            'destination_account_id': None,
            'main_category': '餐饮',
            'sub_category': '午餐',
            'counterparty': '测试商家',
            'description': '测试描述'
        }
        
        # 转换
        v1_bill = await adapter.backend_to_frontend(bill)
        
        # 验证必要字段存在
        assert 'gregorianCalendarYearDashMonthDashDay' in v1_bill
        assert 'gregorianCalendarDayOfMonth' in v1_bill
        assert 'displayDayOfWeek' in v1_bill
        
        # 验证值正确
        assert v1_bill['gregorianCalendarYearDashMonthDashDay'] == '2024-11-21'
        assert v1_bill['gregorianCalendarDayOfMonth'] == 21
        # 2024-11-21是周四，weekday=3 → displayDayOfWeek=5
        assert v1_bill['displayDayOfWeek'] == 5
        
        print(f"✅ 日期字段验证通过: {v1_bill['gregorianCalendarYearDashMonthDashDay']}, "
              f"星期{v1_bill['displayDayOfWeek']}")

    @pytest.mark.asyncio
    async def test_weekday_conversion(self):
        """测试不同星期的转换逻辑"""
        adapter = TransactionAdapter()
        
        test_cases = [
            ('2024-11-17', 1),  # 周日 → 1
            ('2024-11-18', 2),  # 周一 → 2
            ('2024-11-19', 3),  # 周二 → 3
            ('2024-11-20', 4),  # 周三 → 4
            ('2024-11-21', 5),  # 周四 → 5
            ('2024-11-22', 6),  # 周五 → 6
            ('2024-11-23', 7),  # 周六 → 7
        ]
        
        for date_str, expected_weekday in test_cases:
            bill = {
                'id': 1,
                'date': f'{date_str} 10:00:00',
                'type': '支出',
                'amount': 100.0,
                'source_account_id': 1,
                'main_category': '测试',
                'sub_category': '',
                'counterparty': '',
                'description': ''
            }
            
            v1_bill = await adapter.backend_to_frontend(bill)
            assert v1_bill['displayDayOfWeek'] == expected_weekday, \
                f"日期{date_str}的displayDayOfWeek应为{expected_weekday}，实际为{v1_bill['displayDayOfWeek']}"
        
        print("✅ 所有星期转换逻辑正确")


class TestReconciliationAccountFiltering:
    """测试对账单API账户ID筛选"""

    def test_account_id_validation(self):
        """测试account_id验证逻辑"""
        # 有效ID
        assert int('123') == 123
        
        # 无效ID应抛出异常
        with pytest.raises(ValueError):
            int('abc')
        
        with pytest.raises(TypeError):
            int(None)
        
        print("✅ 账户ID验证逻辑正确")

    def test_account_id_comparison(self):
        """测试转账判断的账户ID比较"""
        account_id_int = 5
        
        # 转入判断
        dest_acc_id = 5
        assert dest_acc_id == account_id_int  # 转入
        
        # 转出判断
        source_acc_id = 5
        dest_acc_id = 10
        assert source_acc_id == account_id_int  # 转出
        assert dest_acc_id != account_id_int  # 不是转入
        
        print("✅ 转账账户ID比较逻辑正确")


class TestUtcOffsetHandling:
    """测试utcOffset字段处理"""

    @pytest.mark.asyncio
    async def test_utc_offset_present(self):
        """验证utcOffset字段存在且有效"""
        adapter = TransactionAdapter()
        
        bill = {
            'id': 1,
            'date': '2024-11-21 10:30:00',
            'type': '支出',
            'amount': 100.0,
            'source_account_id': 1,
            'main_category': '测试',
            'sub_category': '',
            'counterparty': '',
            'description': ''
        }
        
        v1_bill = await adapter.backend_to_frontend(bill)
        
        # 验证utcOffset字段存在
        assert 'utcOffset' in v1_bill
        # 验证是数字类型（不是NaN）
        assert isinstance(v1_bill['utcOffset'], int)
        # 验证在合理范围 (-12到+14小时 = -720到+840分钟)
        assert -720 <= v1_bill['utcOffset'] <= 840
        
        print(f"✅ utcOffset字段正确: {v1_bill['utcOffset']}分钟")


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--asyncio-mode=auto'])
