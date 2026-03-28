"""跨批次转账配对回归测试。"""

import asyncio

from bill_analyser.core.smart_dedup import SmartDeduplicationEngine


class TestCrossBatchTransferPairing:
    """跨批次转账配对测试。"""

    def _run_async(self, coro):
        """辅助方法：在新事件循环中运行异步函数。"""
        loop = asyncio.new_event_loop()
        try:
            return loop.run_until_complete(coro)
        finally:
            loop.close()

    def test_cross_batch_transfer_pair_detected(self):
        """新导入负金额账单能与数据库正金额账单配对为转账。"""
        engine = SmartDeduplicationEngine()

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

        class MockDB:
            """模拟数据库。"""

            async def get_bills_by_date_range(self, start, end, user_id=1):
                """返回已有账单。"""
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
                """记录更新。"""
                self.last_update = (bill_id, updates)
                return True

        mock_db = MockDB()
        pairs = self._run_async(engine._find_cross_batch_transfer_pairs(new_bills, mock_db, user_id=1))

        assert len(pairs) == 1
        assert new_bills[0]['type'] == '转账'
        assert new_bills[0].get('_dedup_type') == 'transfer_cross_batch'
        assert mock_db.last_update == (42, {'type': '转账'})

    def test_cross_batch_no_match_same_direction(self):
        """同方向金额不配对。"""
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
            """模拟数据库。"""

            async def get_bills_by_date_range(self, start, end, user_id=1):
                """返回同方向的已有账单。"""
                return [
                    {
                        'id': 1,
                        'date': '2025-08-09 12:00:05',
                        'amount': -100.0,
                        'source_account_id': 'icbc',
                        'counterparty': 'test',
                        'description': '',
                    }
                ]

            async def update_bill(self, bill_id, updates, user_id=1):
                """更新。"""
                return True

        pairs = self._run_async(engine._find_cross_batch_transfer_pairs(new_bills, MockDB(), user_id=1))
        assert len(pairs) == 0
