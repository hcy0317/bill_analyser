"""
测试去重引擎 - Deduplication Engine Tests

测试支付宝/微信与银行账单去重以及账户间转账去重功能
"""

import pytest
from datetime import datetime, timedelta
from src.utils.deduplication import DeduplicationEngine, DeduplicationMode


class TestDeduplicationEngine:
    """去重引擎测试类"""

    def test_init_simple_mode(self):
        """测试Simple模式初始化"""
        engine = DeduplicationEngine(DeduplicationMode.SIMPLE)
        assert engine.mode == DeduplicationMode.SIMPLE
        assert engine.amount_tolerance == 0.01
        assert engine.similarity_threshold == 0.90

    def test_init_aggressive_mode(self):
        """测试Aggressive模式初始化"""
        engine = DeduplicationEngine(DeduplicationMode.AGGRESSIVE)
        assert engine.mode == DeduplicationMode.AGGRESSIVE
        assert engine.amount_tolerance == 0.05
        assert engine.similarity_threshold == 0.80

    def test_exact_match_by_transaction_no(self):
        """测试通过交易号完全匹配"""
        engine = DeduplicationEngine(DeduplicationMode.SIMPLE)

        payment_bill = {
            'source': 'alipay',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'transaction_no': 'TXN123456',
            'description': '消费'
        }

        bank_bill = {
            'source': 'icbc',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'transaction_no': 'TXN123456',
            'description': '支付宝消费'
        }

        assert engine._is_duplicate_payment_bank(payment_bill, bank_bill) is True

    def test_no_match_different_transaction_no(self):
        """测试交易号不同时不匹配（Simple模式）"""
        engine = DeduplicationEngine(DeduplicationMode.SIMPLE)

        payment_bill = {
            'source': 'alipay',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'transaction_no': 'TXN123456',
            'description': '消费'
        }

        bank_bill = {
            'source': 'icbc',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'transaction_no': 'TXN789012',
            'description': '支付宝消费'
        }

        assert engine._is_duplicate_payment_bank(payment_bill, bank_bill) is False

    def test_advanced_match_by_amount_and_time(self):
        """测试通过金额和时间窗口匹配（Advanced模式）"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        payment_bill = {
            'source': 'wechat',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'transaction_no': '',
            'description': '微信消费'
        }

        bank_bill = {
            'source': 'cmbc',
            'date': '2025-01-15 10:05:00',
            'amount': -100.00,
            'transaction_no': '',
            'description': '微信支付'
        }

        assert engine._is_duplicate_payment_bank(payment_bill, bank_bill) is True

    def test_no_match_amount_difference_too_large(self):
        """测试金额差异过大时不匹配"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        payment_bill = {
            'source': 'alipay',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'description': '消费'
        }

        bank_bill = {
            'source': 'icbc',
            'date': '2025-01-15 10:00:00',
            'amount': -105.00,
            'description': '支付宝消费'
        }

        assert engine._is_duplicate_payment_bank(payment_bill, bank_bill) is False

    def test_no_match_time_window_exceeded(self):
        """测试时间窗口超出时不匹配"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        payment_bill = {
            'source': 'alipay',
            'date': '2025-01-15 10:00:00',
            'amount': -100.00,
            'description': '消费'
        }

        bank_bill = {
            'source': 'icbc',
            'date': '2025-01-17 10:00:00',  # 2天后
            'amount': -100.00,
            'description': '支付宝消费'
        }

        assert engine._is_duplicate_payment_bank(payment_bill, bank_bill) is False

    def test_deduplicate_payment_bank_bills(self):
        """测试支付宝/微信与银行账单去重"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        bills = [
            {
                'source': 'alipay',
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'description': '消费'
            },
            {
                'source': 'icbc',
                'date': '2025-01-15 10:05:00',
                'amount': -100.00,
                'description': '支付宝消费'
            },
            {
                'source': 'wechat',
                'date': '2025-01-16 10:00:00',
                'amount': -50.00,
                'description': '餐饮'
            }
        ]

        result_bills, removed_bills = engine._deduplicate_payment_bank(bills)

        # 应该移除银行账单（因为支付宝账单信息更详细）
        assert len(result_bills) == 2
        assert len(removed_bills) == 1
        assert removed_bills[0]['source'] == 'icbc'

    def test_find_transfer_pairs(self):
        """测试查找转账对"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'description': '转账到支付宝'
            },
            {
                'date': '2025-01-15 10:05:00',
                'amount': 100.00,
                'description': '支付宝充值'
            },
            {
                'date': '2025-01-16 10:00:00',
                'amount': -50.00,
                'description': '购物'
            }
        ]

        pairs = engine._find_transfer_pairs(bills)

        # 应该找到一对转账
        assert len(pairs) == 1
        assert abs(pairs[0][0]['amount']) == abs(pairs[0][1]['amount'])

    def test_deduplicate_transfers(self):
        """测试账户间转账去重"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'description': '转账到支付宝'
            },
            {
                'date': '2025-01-15 10:05:00',
                'amount': 100.00,
                'description': '支付宝充值'
            },
            {
                'date': '2025-01-16 10:00:00',
                'amount': -50.00,
                'description': '购物'
            }
        ]

        result_bills, removed_bills = engine._deduplicate_transfers(bills)

        # 应该移除转账对
        assert len(result_bills) == 1
        assert len(removed_bills) == 2
        assert result_bills[0]['description'] == '购物'

    def test_deduplicate_all_comprehensive(self):
        """测试完整去重流程"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        bills = [
            # 支付宝账单（与银行重复）
            {
                'source': 'alipay',
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'description': '消费'
            },
            # 银行账单
            {
                'source': 'icbc',
                'date': '2025-01-15 10:05:00',
                'amount': -100.00,
                'description': '支付宝消费'
            },
            # 转账对 - 银行转出
            {
                'source': 'cmbc',
                'date': '2025-01-16 10:00:00',
                'amount': -50.00,
                'description': '转账到微信'
            },
            # 转账对 - 微信收入（这笔也会被识别为与cmbc重复）
            {
                'source': 'wechat',
                'date': '2025-01-16 10:05:00',
                'amount': 50.00,
                'description': '微信充值'
            },
            # 有效账单
            {
                'source': 'wechat',
                'date': '2025-01-17 10:00:00',
                'amount': -30.00,
                'description': '餐饮'
            }
        ]

        result_bills, stats = engine.deduplicate_all(bills)

        # 预期：
        # 1. 支付宝账单被移除（与银行重复）
        # 2. 微信充值账单被移除（与银行重复，因为金额和时间接近）
        # 3. 最终剩余：银行账单、cmbc转账、微信餐饮
        assert len(result_bills) >= 2  # 至少保留2条（银行和有效账单）
        assert stats['original_count'] == 5
        assert stats['payment_bank_duplicates'] >= 1  # 至少移除支付宝
        assert stats['final_count'] == len(result_bills)

    def test_parse_date_various_formats(self):
        """测试解析多种日期格式"""
        engine = DeduplicationEngine(DeduplicationMode.SIMPLE)

        # 测试各种日期格式
        dates = [
            ('2025-01-15 10:00:00', datetime(2025, 1, 15, 10, 0, 0)),
            ('2025-01-15', datetime(2025, 1, 15, 0, 0, 0)),
            ('2025/01/15 10:00:00', datetime(2025, 1, 15, 10, 0, 0)),
            ('20250115', datetime(2025, 1, 15, 0, 0, 0)),
        ]

        for date_str, expected in dates:
            result = engine._parse_date(date_str)
            assert result == expected

    def test_calculate_similarity(self):
        """测试文本相似度计算"""
        engine = DeduplicationEngine(DeduplicationMode.SIMPLE)

        # 完全相同
        assert engine._calculate_similarity('测试文本', '测试文本') == 1.0

        # 完全不同
        assert engine._calculate_similarity('测试', '完全不同的文本') < 0.5

        # 部分相似
        similarity = engine._calculate_similarity('支付宝消费', '支付宝转账')
        assert 0.5 < similarity < 1.0

    def test_get_deduplication_report(self):
        """测试生成去重报告"""
        engine = DeduplicationEngine(DeduplicationMode.ADVANCED)

        stats = {
            'original_count': 100,
            'payment_bank_duplicates': 20,
            'transfer_duplicates': 10,
            'final_count': 70
        }

        report = engine.get_deduplication_report(stats)

        assert '去重模式: advanced' in report
        assert '原始账单数: 100' in report
        assert '支付工具-银行重复: 20' in report
        assert '账户间转账重复: 10' in report
        assert '最终账单数: 70' in report


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
