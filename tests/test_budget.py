"""
Budget Tests - 预算模块测试
"""

import pytest
import tempfile
from pathlib import Path

from src.core.db import Database
from src.core.budget import BudgetManager, BudgetStatus


@pytest.fixture
async def budget_manager():
    """预算管理器fixture"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        db = Database(str(db_path))
        await db.init_db()
        
        # 插入测试数据
        bills = [
            {
                'date': '2025-01-15',
                'type': '支出',
                'amount': 800.0,
                'counterparty': '商家',
                'description': '测试',
                'channel': '测试',
                'main_category': '餐饮',
                'sub_category': '外卖'
            }
        ]
        await db.insert_bills(bills)
        
        manager = BudgetManager(db)
        manager.budgets = {'餐饮': 1000.0, '交通': 500.0}
        
        yield manager
        
        await db.close()


def test_check_budget_status(budget_manager):
    """测试预算状态检查"""
    # 正常
    status = budget_manager.check_budget_status('餐饮', 500.0)
    assert status == BudgetStatus.NORMAL
    
    # 警告（80%）
    status = budget_manager.check_budget_status('餐饮', 850.0)
    assert status == BudgetStatus.WARNING
    
    # 严重（90%）
    status = budget_manager.check_budget_status('餐饮', 950.0)
    assert status == BudgetStatus.CRITICAL
    
    # 超支
    status = budget_manager.check_budget_status('餐饮', 1100.0)
    assert status == BudgetStatus.EXCEEDED


@pytest.mark.asyncio
async def test_budget_report(budget_manager):
    """测试预算报告"""
    report = await budget_manager.get_budget_report('month')
    
    assert 'categories' in report
    assert 'summary' in report
    
    # 检查餐饮分类
    if '餐饮' in report['categories']:
        cat_info = report['categories']['餐饮']
        assert 'budget' in cat_info
        assert 'spent' in cat_info
        assert 'status' in cat_info


def test_alert_callback(budget_manager):
    """测试警报回调"""
    alert_triggered = []
    
    def callback(category, budget, spent, status):
        """测试回调"""
        alert_triggered.append({
            'category': category,
            'budget': budget,
            'spent': spent,
            'status': status
        })
    
    budget_manager.register_alert_callback(callback)
    
    # 触发警报
    budget_manager.check_budget_status('餐饮', 950.0)
    
    assert len(alert_triggered) > 0
    assert alert_triggered[0]['category'] == '餐饮'
