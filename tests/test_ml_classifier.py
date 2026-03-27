"""ML 分类器单元测试

v6.88: 覆盖 MLBillClassifier 的 train / predict / load_model 逻辑
以及跨批次转账配对。
"""
import os
import tempfile
import asyncio
from typing import List, Dict, Any

import pytest


def _import_ml_classifier_module():
    """Import the optional ML classifier module or skip related tests."""
    return pytest.importorskip(
        'bill_analyser.core.ml_classifier',
        reason='可选 ML 分类器模块当前未随默认测试配置提供',
    )

# ==================== ML Classifier Tests ====================


def _make_training_data(n_per_class: int = 10) -> List[Dict[str, Any]]:
    """生成模拟训练数据"""
    data: List[Dict[str, Any]] = []
    classes = [
        ('餐饮', '外卖', '美团外卖，饿了么，肯德基，必胜客，麦当劳'),
        ('交通', '打车', '滴滴出行，高德打车，T3出行，曹操出行，花小猪'),
        ('购物', '日用品', '京东，淘宝，拼多多，天猫，苏宁易购'),
        ('住房', '水电费', '国家电网，水务集团，燃气公司，物业费，暖气费'),
    ]
    for main_cat, sub_cat, samples_str in classes:
        samples = samples_str.split('，')
        for i in range(n_per_class):
            cp = samples[i % len(samples)]
            data.append({
                'counterparty': cp,
                'description': f'{cp} 消费 {i}',
                'main_category': main_cat,
                'sub_category': sub_cat,
            })
    return data


class TestMLBillClassifier:
    """ML 分类器核心功能测试"""

    def test_train_success(self):
        """训练成功并生成模型文件"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf = MLBillClassifier(user_id=99, data_dir=tmpdir)
            result = clf.train(_make_training_data(n_per_class=15))

            assert result['success'] is True
            assert result['sample_count'] >= 40
            assert result['class_count'] == 4
            assert os.path.exists(clf.model_path)

    def test_train_insufficient_data(self):
        """训练数据不足时返回失败"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf = MLBillClassifier(user_id=99, data_dir=tmpdir)
            result = clf.train(_make_training_data(n_per_class=2))

            assert result['success'] is False
            assert '不足' in result.get('error', '')

    def test_predict_after_train(self):
        """训练后预测返回正确分类"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf = MLBillClassifier(user_id=99, data_dir=tmpdir, confidence_threshold=0.3)
            clf.train(_make_training_data(n_per_class=15))

            main_cat, sub_cat, conf = clf.predict(counterparty='美团外卖', description='午餐')
            assert main_cat == '餐饮'
            assert sub_cat == '外卖'
            assert conf > 0.3

    def test_predict_unknown_returns_none(self):
        """未知文本置信度低时返回 None"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf = MLBillClassifier(user_id=99, data_dir=tmpdir, confidence_threshold=0.99)
            clf.train(_make_training_data(n_per_class=15))

            # 设置极高阈值，任何预测都不会通过
            main_cat, _, conf = clf.predict(counterparty='随机文本xyz', description='abc')
            assert main_cat is None

    def test_load_model_roundtrip(self):
        """模型保存后可正确加载"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf1 = MLBillClassifier(user_id=99, data_dir=tmpdir, confidence_threshold=0.3)
            clf1.train(_make_training_data(n_per_class=15))

            # 新实例加载模型
            clf2 = MLBillClassifier(user_id=99, data_dir=tmpdir, confidence_threshold=0.3)
            loaded = clf2.load_model()
            assert loaded is True
            assert clf2.is_ready

            main_cat, _, _ = clf2.predict(counterparty='滴滴出行', description='打车')
            assert main_cat == '交通'

    def test_model_info(self):
        """模型元信息正确"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf = MLBillClassifier(user_id=99, data_dir=tmpdir)
            info = clf.model_info
            assert info['is_ready'] is False
            assert info['user_id'] == 99

            clf.train(_make_training_data(n_per_class=15))
            info = clf.model_info
            assert info['is_ready'] is True
            assert info['sample_count'] >= 40

    def test_predict_before_train_returns_none(self):
        """未训练时预测返回 None"""
        MLBillClassifier = _import_ml_classifier_module().MLBillClassifier

        with tempfile.TemporaryDirectory() as tmpdir:
            clf = MLBillClassifier(user_id=99, data_dir=tmpdir)
            main_cat, sub_cat, conf = clf.predict(counterparty='美团', description='外卖')
            assert main_cat is None
            assert conf == 0.0


class TestGlobalClassifierManagement:
    """全局实例管理测试"""

    def test_get_ml_classifier_singleton(self):
        """同一 user_id 返回同一实例"""
        module = _import_ml_classifier_module()
        get_ml_classifier = module.get_ml_classifier
        _classifiers = module._classifiers

        _classifiers.clear()
        c1 = get_ml_classifier(user_id=777, data_dir=tempfile.gettempdir())
        c2 = get_ml_classifier(user_id=777, data_dir=tempfile.gettempdir())
        assert c1 is c2
        _classifiers.clear()

    def test_invalidate_ml_classifier(self):
        """invalidate 后重新创建实例"""
        module = _import_ml_classifier_module()
        get_ml_classifier = module.get_ml_classifier
        invalidate_ml_classifier = module.invalidate_ml_classifier
        _classifiers = module._classifiers

        _classifiers.clear()
        c1 = get_ml_classifier(user_id=888, data_dir=tempfile.gettempdir())
        invalidate_ml_classifier(888)
        c2 = get_ml_classifier(user_id=888, data_dir=tempfile.gettempdir())
        assert c1 is not c2
        _classifiers.clear()


# ==================== Cross-Batch Transfer Tests ====================


class TestCrossBatchTransferPairing:
    """跨批次转账配对测试"""

    def _run_async(self, coro):
        """辅助方法：在新事件循环中运行异步函数"""
        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(coro)
        finally:
            loop.close()

    def test_cross_batch_transfer_pair_detected(self):
        """新导入负金额账单能与数据库正金额账单配对为转账"""
        from src.core.smart_dedup import SmartDeduplicationEngine

        engine = SmartDeduplicationEngine()

        # 新导入的账单（一条支出）
        new_bills = [
            {
                'date': '2025-08-09 12:15:21',
                'amount': -3261.52,
                '_parser_id': 'abc',
                'source_account_id': '',
                'counterparty': '支付宝',
                'description': '转账',
                '_removed': False,
                '_dedup_id': 'hash1',
                '_merged_from': [],
            }
        ]

        # 模拟数据库实例
        class MockDB:
            """模拟数据库"""
            async def get_bills_by_date_range(self, start, end, user_id=1):
                """返回已有账单"""
                return [
                    {
                        'id': 42,
                        'date': '2025-08-09 12:15:21',
                        'amount': 3261.52,
                        'source_account_id': 'alipay',
                        'counterparty': '农业银行',
                        'description': '转入',
                    }
                ]

            async def update_bill(self, bill_id, updates, user_id=1):
                """记录更新"""
                self.last_update = (bill_id, updates)
                return True

        mock_db = MockDB()
        pairs = self._run_async(
            engine._find_cross_batch_transfer_pairs(new_bills, mock_db, user_id=1)
        )

        assert len(pairs) == 1
        assert new_bills[0]['type'] == '转账'
        assert new_bills[0].get('_dedup_type') == 'transfer_cross_batch'
        assert mock_db.last_update == (42, {'type': '转账'})

    def test_cross_batch_no_match_same_direction(self):
        """同方向金额不配对"""
        from src.core.smart_dedup import SmartDeduplicationEngine

        engine = SmartDeduplicationEngine()

        new_bills = [
            {
                'date': '2025-08-09 12:00:00',
                'amount': -100.0,
                '_parser_id': 'abc',
                'source_account_id': '',
                'counterparty': 'test',
                'description': '',
                '_removed': False,
                '_dedup_id': 'h1',
                '_merged_from': [],
            }
        ]

        class MockDB:
            """模拟数据库"""
            async def get_bills_by_date_range(self, start, end, user_id=1):
                """返回同方向的已有账单"""
                return [{
                    'id': 1,
                    'date': '2025-08-09 12:00:05',
                    'amount': -100.0,
                    'source_account_id': 'icbc',
                    'counterparty': 'test',
                    'description': '',
                }]

            async def update_bill(self, bill_id, updates, user_id=1):
                """更新"""
                return True

        pairs = self._run_async(
            engine._find_cross_batch_transfer_pairs(new_bills, MockDB(), user_id=1)
        )
        assert len(pairs) == 0
