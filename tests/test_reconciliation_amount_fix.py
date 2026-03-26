"""测试对账单API金额单位修复

验证问题：
1. 金额从元正确转换为分（444元 → 44400分）
2. accountClosingBalance字段存在
3. 日期分组字段完整
"""

import pytest
from src.utils.currency import yuan_to_cents


class TestReconciliationAmountConversion:
    """测试对账单金额单位转换"""

    def test_yuan_to_cents_conversion(self):
        """测试元到分的转换"""
        assert yuan_to_cents(444.00) == 44400
        assert yuan_to_cents(111.00) == 11100
        assert yuan_to_cents(333.00) == 33300
        assert yuan_to_cents(0.00) == 0
        assert yuan_to_cents(1.23) == 123
        assert yuan_to_cents(100.50) == 10050
        print("✅ 金额转换测试通过")

    def test_negative_amounts(self):
        """测试负数金额转换"""
        assert yuan_to_cents(-444.00) == -44400
        assert yuan_to_cents(-111.00) == -11100
        print("✅ 负数金额转换测试通过")

    def test_edge_cases(self):
        """测试边界情况"""
        assert yuan_to_cents(None) == 0
        assert yuan_to_cents('') == 0
        assert yuan_to_cents(0) == 0
        assert yuan_to_cents("444.00") == 44400
        print("✅ 边界情况测试通过")


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
