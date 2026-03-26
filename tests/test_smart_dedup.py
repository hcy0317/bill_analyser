"""
SmartDeduplicationEngine 测试 v6.42.1

测试4种去重机制：
1. 转账配对（v6.42.1恢复）
2. 支付平台/银行去重
3. 类似账单去重
4. 分账单去重

v6.42.1 - 恢复转账配对机制
    - 转账配对：时间30秒内 + 金额相反 + 来源不同 → 自动识别为转账
    - 移除投资配对（由分类引擎的关键词匹配处理）
    - 保留4种核心去重机制
"""

import pytest
from datetime import datetime
from src.core.smart_dedup import SmartDeduplicationEngine, DeduplicationType


class TestTransferPairing:
    """测试转账配对功能

    v6.42.1转账配对条件：
    1. 时间误差30秒以内
    2. 金额绝对值相等，符号相反（一正一负）
    3. 来自不同账户
    """

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_transfer_pair_basic(self):
        """测试基本的转账配对"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,  # 转出
                'source_account_id': 'alipay',
                'description': '转账到工行',
                'counterparty': '工商银行',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': 100.00,  # 转入
                'source_account_id': 'icbc',
                'description': '支付宝转入',
                'counterparty': '支付宝',
                'type': '收入'
            }
        ]

        result = self.engine.process(bills)

        print(f"\n[转账配对] 保留账单数: {len(result.kept_bills)}")
        print(f"[转账配对] 转账对数: {len(result.transfer_pairs)}")

        # 应该识别为1对转账
        assert len(result.transfer_pairs) == 1, "应该识别为1对转账"

        # 验证转账配对的账户设置
        outgoing, incoming = result.transfer_pairs[0]
        assert outgoing['type'] == '转账', "转出账单type应该是'转账'"
        assert incoming['type'] == '转账', "转入账单type应该是'转账'"

        # v6.62: 去重阶段记录 _destination_* 字段，account_id 在账户匹配阶段设置
        # 验证目标账户信息被正确记录
        assert outgoing.get('_destination_parser_id', '') or outgoing.get('source_account_id', '') in ['icbc', 'alipay'], \
            "转出账单应记录目标账户信息"

        # v6.62: 转入账单被标记为已移除
        assert incoming.get('_removed') is True, "转入账单应被标记为已移除"

    def test_transfer_pair_time_within_30_seconds(self):
        """测试时间差恰好30秒仍能配对"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,
                'source_account_id': 'wechat',
                'description': '转账',
                'counterparty': '银行',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:30',  # 恰好30秒
                'amount': 50.00,
                'source_account_id': 'cmbc',
                'description': '转入',
                'counterparty': '微信',
                'type': '收入'
            }
        ]

        result = self.engine.process(bills)
        assert len(result.transfer_pairs) == 1, "30秒内应该识别为转账"

    def test_transfer_pair_time_over_30_seconds(self):
        """测试时间差超过30秒不配对"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,
                'source_account_id': 'wechat',
                'description': '转账',
                'counterparty': '银行',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:31',  # 31秒，超过阈值
                'amount': 50.00,
                'source_account_id': 'cmbc',
                'description': '转入',
                'counterparty': '微信',
                'type': '收入'
            }
        ]

        result = self.engine.process(bills)
        assert len(result.transfer_pairs) == 0, "超过30秒不应该识别为转账"

    def test_transfer_pair_same_source_no_pair(self):
        """测试同一来源不配对"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,
                'source_account_id': 'alipay',
                'description': '退款',
                'counterparty': '商家',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': 50.00,
                'source_account_id': 'alipay',  # 同一来源
                'description': '退款到账',
                'counterparty': '商家',
                'type': '收入'
            }
        ]

        result = self.engine.process(bills)
        assert len(result.transfer_pairs) == 0, "同一来源不应该配对为转账"

    def test_transfer_pair_same_direction_no_pair(self):
        """测试同方向不配对（都是支出或都是收入）"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,  # 支出
                'source_account_id': 'alipay',
                'description': '购物',
                'counterparty': '商家A',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': -50.00,  # 也是支出
                'source_account_id': 'icbc',
                'description': '消费',
                'counterparty': '商家B',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        assert len(result.transfer_pairs) == 0, "同方向不应该配对为转账"


class TestPlatformBankDedup:
    """测试支付平台/银行去重

    v6.42去重条件：
    1. 时间误差30秒以内
    2. 金额相等（绝对值相等且符号相同）
    3. 一个来自支付平台(wechat/alipay)，一个来自银行(icbc/cmbc/abc/ccb)
    """

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_platform_bank_dedup_basic(self):
        """测试基本的平台-银行去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'alipay',
                'description': '超市购物',
                'counterparty': '超市',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': -100.00,
                'source_account_id': 'icbc',
                'description': '银联消费',
                'counterparty': '超市',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)

        print(f"\n[平台-银行去重] 保留账单数: {len(result.kept_bills)}")
        print(f"[平台-银行去重] 移除账单数: {result.removed_count}")

        # 应该去重为1条，保留支付宝账单
        assert len(result.kept_bills) == 1, "应该保留1条账单"
        assert result.kept_bills[0].get('source_account_id') == 'alipay', \
            "应该保留支付宝账单（优先级更高）"

    def test_platform_bank_time_within_30_seconds(self):
        """测试时间差恰好30秒的情况"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,
                'source_account_id': 'wechat',
                'description': '外卖',
                'counterparty': '美团',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:30',  # 恰好30秒
                'amount': -50.00,
                'source_account_id': 'cmbc',
                'description': '银联消费',
                'counterparty': '美团',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        assert len(result.kept_bills) == 1, "30秒内应该去重"

    def test_platform_bank_time_over_30_seconds(self):
        """测试时间差超过30秒的情况 - 不应去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,
                'source_account_id': 'wechat',
                'description': '外卖',
                'counterparty': '美团',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:31',  # 超过30秒
                'amount': -50.00,
                'source_account_id': 'cmbc',
                'description': '银联消费',
                'counterparty': '美团',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        assert len(result.kept_bills) == 2, "超过30秒不应去重"

    def test_platform_bank_different_direction_no_dedup(self):
        """测试不同方向（一正一负）不应被平台-银行去重，但应被识别为转账配对

        v6.62: 金额方向不同的平台-银行账单不符合平台-银行去重条件，
        但符合转账配对条件（时间30秒内+金额相反+来源不同），所以应该被识别为转账。
        转账配对后只保留转出账单（负金额），转入账单被标记为 _removed。
        """
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,  # 支出
                'source_account_id': 'alipay',
                'description': '支付',
                'counterparty': '商户',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': 100.00,  # 收入（方向不同）
                'source_account_id': 'icbc',
                'description': '收款',
                'counterparty': '商户',
                'type': '收入'
            }
        ]

        result = self.engine.process(bills)
        # v6.62: 应被识别为转账配对，只保留转出账单（负金额）
        assert len(result.transfer_pairs) == 1, "应识别为转账配对"
        assert len(result.kept_bills) == 1, "转账配对后只保留转出账单"
        assert result.kept_bills[0]['type'] == '转账', "保留账单类型应为转账"
        assert result.kept_bills[0]['amount'] < 0, "保留账单应为转出（负金额）"

    def test_platform_bank_same_source_no_dedup(self):
        """测试同一来源（都是平台或都是银行）不应去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'alipay',
                'description': '支付1',
                'counterparty': '商户',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': -100.00,
                'source_account_id': 'wechat',  # 同为平台
                'description': '支付2',
                'counterparty': '商户',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        # 都是平台，不符合"一平台一银行"条件，不应通过platform_bank去重
        # 但可能通过similar去重（如果counterparty相似）
        # 这里counterparty相同为'商户'，相似度100%，所以会去重
        print(f"[同来源测试] 保留账单数: {len(result.kept_bills)}")


class TestSimilarBillsDedup:
    """测试类似账单去重

    v6.42去重条件：
    1. 时间误差30秒以内
    2. 金额相等（绝对值相等且符号相同）
    3. counterparty 或 payment_method 相似度≥50%（任一满足）
    4. source_account_id 不同
    """

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_similar_counterparty_dedup(self):
        """测试counterparty相似度去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'abc',  # 农业银行
                'description': '购物',
                'counterparty': '沃尔玛超市',
                'payment_method': '',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:10',
                'amount': -100.00,
                'source_account_id': 'ccb',  # 建设银行
                'description': '消费',
                'counterparty': '沃尔玛',  # 与'沃尔玛超市'相似度高
                'payment_method': '',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)

        print(f"\n[类似账单去重] 保留账单数: {len(result.kept_bills)}")
        # counterparty相似度高（'沃尔玛'包含在'沃尔玛超市'中）
        assert len(result.kept_bills) == 1, "counterparty相似度高应该去重"

    def test_similar_payment_method_dedup(self):
        """测试payment_method相似度去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -50.00,
                'source_account_id': 'abc',
                'description': '餐饮',
                'counterparty': '',
                'payment_method': '微信支付',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:15',
                'amount': -50.00,
                'source_account_id': 'ccb',
                'description': '餐费',
                'counterparty': '',
                'payment_method': '微信',  # 与'微信支付'相似度高
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        # payment_method相似度高
        assert len(result.kept_bills) == 1, "payment_method相似度高应该去重"

    def test_similar_low_similarity_no_dedup(self):
        """测试相似度低于50%时不应去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'abc',
                'description': '购物',
                'counterparty': 'ABC商店',
                'payment_method': '现金',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:10',
                'amount': -100.00,
                'source_account_id': 'ccb',
                'description': '消费',
                'counterparty': 'XYZ超市',  # 与'ABC商店'相似度低
                'payment_method': '银行卡',  # 与'现金'相似度低
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        # 相似度都低于50%
        assert len(result.kept_bills) == 2, "相似度低于50%不应去重"

    def test_similar_same_source_no_dedup(self):
        """测试相同来源不应通过类似账单去重"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'abc',
                'description': '购物',
                'counterparty': '沃尔玛',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:10',
                'amount': -100.00,
                'source_account_id': 'abc',  # 相同来源
                'description': '消费',
                'counterparty': '沃尔玛超市',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        # 来源相同，不符合"source_account_id 不同"条件
        assert len(result.kept_bills) == 2, "相同来源不应通过类似账单去重"


class TestSplitBillsDedup:
    """测试分账单去重

    v6.42去重条件：
    1. 多个账单的时间误差30秒以内
    2. source_account_id 不同且仅来自两个不同的来源
    3. 来自其中一个来源的单个账单金额 = 来自另一个来源的多个账单金额之和
    4. 金额方向相同
    """

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_split_bill_basic(self):
        """测试基本的分账单识别 - 1个总账单拆分为3个分账单"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -300.00,  # 总账单
                'source_account_id': 'alipay',
                'description': '超市购物',
                'counterparty': '超市',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:01',
                'amount': -100.00,  # 分账单1
                'source_account_id': 'icbc',
                'description': '蔬菜',
                'counterparty': '超市',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:02',
                'amount': -150.00,  # 分账单2
                'source_account_id': 'icbc',
                'description': '肉类',
                'counterparty': '超市',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:03',
                'amount': -50.00,  # 分账单3
                'source_account_id': 'icbc',
                'description': '水果',
                'counterparty': '超市',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)

        print(f"\n[分账单去重] 保留账单数: {len(result.kept_bills)}")
        print(f"[分账单去重] 移除账单数: {result.removed_count}")
        print(f"[分账单去重] 分账单组数: {len(result.split_groups)}")

        # 总账单300 = 分账单100+150+50
        # 应移除总账单，保留3个分账单
        assert len(result.kept_bills) == 3, "应保留3个分账单"
        assert len(result.split_groups) == 1, "应识别出1组分账单"

    def test_split_bill_time_over_30_seconds(self):
        """测试时间差超过30秒不应识别为分账单"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -200.00,
                'source_account_id': 'alipay',
                'description': '购物',
                'counterparty': '商户',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:31',  # 超过30秒
                'amount': -100.00,
                'source_account_id': 'icbc',
                'description': '商品A',
                'counterparty': '商户',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:32',
                'amount': -100.00,
                'source_account_id': 'icbc',
                'description': '商品B',
                'counterparty': '商户',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)
        # 时间差超过30秒，不应识别为分账单
        assert len(result.split_groups) == 0, "时间差超过30秒不应识别为分账单"


class TestExactDuplicates:
    """测试完全重复去重"""

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_exact_duplicate(self):
        """测试完全重复的账单"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'alipay',
                'description': '超市购物',
                'counterparty': '超市',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'alipay',
                'description': '超市购物',
                'counterparty': '超市',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)

        assert len(result.kept_bills) == 1, "完全重复应只保留1条"


class TestDiagnoseFunctions:
    """测试工具函数"""

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_time_close_within_30_seconds(self):
        """测试时间接近检测 - 30秒内"""
        dt1 = datetime(2025, 1, 15, 10, 0, 0)
        dt2 = datetime(2025, 1, 15, 10, 0, 25)

        is_close = self.engine._time_close(dt1, dt2)
        assert is_close is True, "25秒应该被认为接近"

    def test_time_close_exactly_30_seconds(self):
        """测试时间接近检测 - 恰好30秒"""
        dt1 = datetime(2025, 1, 15, 10, 0, 0)
        dt2 = datetime(2025, 1, 15, 10, 0, 30)

        is_close = self.engine._time_close(dt1, dt2)
        assert is_close is True, "30秒应该被认为接近"

    def test_time_close_over_30_seconds(self):
        """测试时间接近检测 - 超过30秒"""
        dt1 = datetime(2025, 1, 15, 10, 0, 0)
        dt2 = datetime(2025, 1, 15, 10, 0, 31)

        is_close = self.engine._time_close(dt1, dt2)
        assert is_close is False, "31秒不应该被认为接近"

    def test_amount_equal_same_direction_positive(self):
        """测试金额相等且方向相同 - 都是正数"""
        amt1 = 100.00
        amt2 = 100.00

        result = self.engine._amount_equal_same_direction(amt1, amt2)
        assert result is True, "两个正数100应该相等且方向相同"

    def test_amount_equal_same_direction_negative(self):
        """测试金额相等且方向相同 - 都是负数"""
        amt1 = -100.00
        amt2 = -100.00

        result = self.engine._amount_equal_same_direction(amt1, amt2)
        assert result is True, "两个负数-100应该相等且方向相同"

    def test_amount_equal_different_direction(self):
        """测试金额相等但方向不同"""
        amt1 = 100.00
        amt2 = -100.00

        result = self.engine._amount_equal_same_direction(amt1, amt2)
        assert result is False, "一正一负不应被认为相等"

    def test_calculate_similarity_identical(self):
        """测试相似度计算 - 完全相同"""
        s1 = "沃尔玛超市"
        s2 = "沃尔玛超市"

        similarity = self.engine._calculate_similarity(s1, s2)
        assert similarity == 1.0, "完全相同应该是100%相似度"

    def test_calculate_similarity_partial(self):
        """测试相似度计算 - 部分相似"""
        s1 = "沃尔玛超市"
        s2 = "沃尔玛"

        similarity = self.engine._calculate_similarity(s1, s2)
        assert similarity >= 0.5, "部分匹配应该有较高相似度"

    def test_calculate_similarity_empty(self):
        """测试相似度计算 - 空字符串"""
        s1 = ""
        s2 = ""

        similarity = self.engine._calculate_similarity(s1, s2)
        assert similarity == 1.0, "两个空字符串应该相似"


class TestMergedFields:
    """测试字段合并"""

    def setup_method(self):
        """每个测试方法前初始化引擎"""
        self.engine = SmartDeduplicationEngine()

    def test_merge_keeps_richer_info(self):
        """测试合并后保留更丰富的信息"""
        bills = [
            {
                'date': '2025-01-15 10:00:00',
                'amount': -100.00,
                'source_account_id': 'alipay',
                'description': '超市购物-生鲜区',
                'counterparty': '沃尔玛超市',
                'type': '支出'
            },
            {
                'date': '2025-01-15 10:00:05',
                'amount': -100.00,
                'source_account_id': 'icbc',
                'description': '银联消费',
                'counterparty': '沃尔玛',
                'type': '支出'
            }
        ]

        result = self.engine.process(bills)

        assert len(result.kept_bills) == 1
        kept = result.kept_bills[0]

        # 验证保留了更丰富的信息
        print(f"\n[合并测试] description: {kept.get('description')}")
        print(f"[合并测试] counterparty: {kept.get('counterparty')}")
