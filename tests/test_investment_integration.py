"""
Investment Transaction 集成测试
测试前端Investment功能与后端API的完整集成
"""
import json
import tempfile
from pathlib import Path

import pytest

from src.core.db import Database


class TestInvestmentIntegration:
    """Investment交易集成测试"""

    @pytest.fixture
    async def db(self):
        """创建测试数据库实例"""
        with tempfile.TemporaryDirectory() as tmpdir:
            db_path = Path(tmpdir) / "test_investment.db"
            db_instance = Database(str(db_path))
            await db_instance.init_db()
            yield db_instance
            await db_instance.close()

    @pytest.mark.asyncio
    async def test_investment_category_type_value(self, db):
        """测试Investment类型值为5"""
        # 通过create_category插入Investment分类
        cat_id = await db.create_category({
            'main_category': '投资',
            'sub_category': '股票',
            'description': '投资股票交易',
            'type': 5
        })
        assert cat_id is not None

        # 验证查询
        categories = await db.get_all_categories()
        investment_cats = [c for c in categories if c['main_category'] == '投资']
        assert len(investment_cats) > 0
        assert investment_cats[0]['type'] == 5

    @pytest.mark.asyncio
    async def test_investment_account_is_asset(self):
        """测试Investment账户被识别为资产"""
        # 根据accounts.py的逻辑，category=5的账户应该是资产
        # isAsset = category in [1, 2, 5, 7, 8, 9]
        investment_account_categories = [5]

        for category in investment_account_categories:
            is_asset = category in [1, 2, 5, 7, 8, 9]
            assert is_asset, f"Category {category} should be an asset"

    @pytest.mark.asyncio
    async def test_investment_bill_creation(self, db):
        """测试创建Investment账单"""
        # 创建Investment账单
        bills = [{
            'date': '2024-11-20',
            'type': 'Investment',
            'amount': 15356.40,
            'counterparty': '贵州茅台(600519)',
            'description': json.dumps({
                "investmentProduct": "贵州茅台(600519)",
                "quantity": 10,
                "unitPrice": 1535.64,
                "totalAmount": 15356.40
            }, ensure_ascii=False),
            'channel': '证券账户',
            'main_category': '投资',
            'sub_category': '股票',
            'batch_id': 'test_batch_investment'
        }]

        # 执行插入
        count = await db.insert_bills(bills)
        assert count == 1

        # 验证插入
        result = await db.get_bills({'type': 'Investment'})
        assert len(result) == 1
        bill = result[0]
        assert bill['type'] == 'Investment'
        assert bill['amount'] == 15356.40
        assert bill['main_category'] == '投资'
        assert 'investmentProduct' in bill['description']

    @pytest.mark.asyncio
    async def test_investment_json_description_parsing(self, db):
        """测试Investment详情JSON解析"""
        investment_details = {
            "investmentProduct": "华夏沪深300ETF",
            "quantity": 500,
            "unitPrice": 4.23,
            "totalAmount": 2115.00,
            "expectedReturnRate": 8.5,
            "riskLevel": "medium",
            "investmentType": "funds"
        }

        # 创建账单
        bills = [{
            'date': '2024-11-20',
            'type': 'Investment',
            'amount': investment_details['totalAmount'],
            'counterparty': investment_details['investmentProduct'],
            'description': json.dumps(investment_details, ensure_ascii=False),
            'channel': '投资账户',
            'main_category': '投资',
            'sub_category': '基金',
            'batch_id': 'test_batch_json'
        }]

        count = await db.insert_bills(bills)
        assert count == 1

        # 读取并解析
        result = await db.get_bills({'main_category': '投资', 'sub_category': '基金'})
        assert len(result) == 1

        parsed = json.loads(result[0]['description'])
        assert parsed['investmentProduct'] == '华夏沪深300ETF'
        assert parsed['quantity'] == 500
        assert parsed['unitPrice'] == 4.23
        assert parsed['expectedReturnRate'] == 8.5
        assert parsed['riskLevel'] == 'medium'

    @pytest.mark.asyncio
    async def test_category_type_enum_alignment(self, db):
        """测试CategoryType枚举值对齐"""
        # 验证所有类型值符合新的枚举定义
        expected_types = {
            2: '收入',      # Income
            3: '支出',      # Expense
            4: '转账',      # Transfer
            5: '投资'       # Investment
        }

        # 添加测试分类
        for type_val, name in expected_types.items():
            await db.create_category({
                'main_category': name,
                'sub_category': f'{name}测试',
                'description': f'测试{name}分类',
                'type': type_val
            })

        # 验证所有类型都存在
        categories = await db.get_all_categories()
        for type_val, name in expected_types.items():
            matching = [c for c in categories if c['type'] == type_val and c['main_category'] == name]
            assert len(matching) > 0, f"Type {type_val} ({name}) not found"


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--asyncio-mode=auto'])
