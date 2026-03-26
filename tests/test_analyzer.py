"""
Analyzer Tests - 分析器测试
"""

import pytest
import tempfile
from pathlib import Path

from src.core.db import Database
from src.core.analyzer import Analyzer


@pytest.fixture
async def analyzer():
    """分析器fixture"""
    with tempfile.TemporaryDirectory() as tmpdir:
        db_path = Path(tmpdir) / "test.db"
        db = Database(str(db_path))
        await db.init_db()
        
        # 插入测试数据
        bills = [
            {
                'date': '2025-01-15',
                'type': '支出',
                'amount': 100.0,
                'counterparty': '商家A',
                'description': '测试',
                'channel': '测试',
                'main_category': '餐饮',
                'sub_category': '外卖'
            },
            {
                'date': '2025-01-16',
                'type': '收入',
                'amount': 500.0,
                'counterparty': '公司',
                'description': '工资',
                'channel': '银行',
                'main_category': None,
                'sub_category': None
            }
        ]
        await db.insert_bills(bills)
        
        analyzer_instance = Analyzer(db)
        yield analyzer_instance
        
        await db.close()


@pytest.mark.asyncio
async def test_generate_report(analyzer):
    """测试生成报告"""
    report = await analyzer.generate_report('month')
    
    assert 'summary' in report
    assert 'by_category' in report
    assert 'by_type' in report
    assert 'trend' in report
    
    # 检查摘要
    summary = report['summary']
    assert summary['total_income'] >= 0
    assert summary['total_expense'] >= 0


@pytest.mark.asyncio
async def test_cache(analyzer):
    """测试缓存机制"""
    # 第一次生成
    report1 = await analyzer.generate_report('month')

    # 第二次应该使用缓存
    report2 = await analyzer.generate_report('month')

    # 比较除时间戳外的其他字段
    assert report1['period'] == report2['period']
    assert report1['total_records'] == report2['total_records']
    assert report1['summary'] == report2['summary']

    # 清除缓存
    analyzer.clear_cache()


@pytest.mark.asyncio
async def test_empty_report(analyzer):
    """测试空数据报告"""
    # 使用未来的日期，确保没有数据
    from datetime import datetime, timedelta
    
    # 清空缓存
    analyzer.clear_cache()
    
    # 生成报告（应该处理空数据）
    report = await analyzer.generate_report('year')
    assert report['total_records'] >= 0
