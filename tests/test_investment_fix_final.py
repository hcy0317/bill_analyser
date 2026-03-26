"""测试投资账单和时间筛选功能修复

此测试验证：
1. 投资账单创建时destination_account_id正确保存
2. 时间筛选器（交易日历、对账单、时间控件）正常工作
3. 总收入总支出正确计算

运行方式:
    .venv/Scripts/python tests/test_investment_fix_final.py

作者: Bill Analyser Team
日期: 2025-11-21
"""

import sys
import os
from pathlib import Path

# 添加项目根目录到Python路径
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

# 设置环境变量
os.environ['PYTHONPATH'] = str(project_root)

import asyncio  # noqa: E402
from src.core.db import Database  # noqa: E402
from src.utils.logger import get_logger  # noqa: E402

logger = get_logger('TestInvestmentFix')


async def test_investment_account_persistence():
    """测试投资账单的destination_account_id持久化"""
    logger.info("=" * 60)
    logger.info("测试1: 投资账单destination_account_id持久化")
    logger.info("=" * 60)
    
    db = Database(':memory:')  # 使用内存数据库测试
    await db.init_db()  # 修正：使用正确的方法名
    
    # 创建测试账户
    account1_data = {
        'name': '农业银行-储蓄卡',
        'type': 1,  # 储蓄卡
        'category': 2,  # 银行卡
        'currency': 'CNY',
        'balance': 10000.0,
        'created_at': '2025-11-21 00:00:00',
        'updated_at': '2025-11-21 00:00:00'
    }
    account1_id = await db.create_account(account1_data)
    logger.info(f"✓ 创建源账户: {account1_data['name']} (ID={account1_id})")
    
    account2_data = {
        'name': '中信建投证券',
        'type': 5,  # 投资账户
        'category': 5,  # 证券
        'currency': 'CNY',
        'balance': 0.0,
        'created_at': '2025-11-21 00:00:00',
        'updated_at': '2025-11-21 00:00:00'
    }
    account2_id = await db.create_account(account2_data)
    logger.info(f"✓ 创建目标账户: {account2_data['name']} (ID={account2_id})")
    
    # 创建投资账单
    bill_data = {
        'date': '2025-11-21 15:00:00',
        'type': '投资',
        'amount': 1000.0,
        'counterparty': '',  # 必填字段
        'source_account_id': account1_id,
        'destination_account_id': account2_id,  # 关键字段
        'destination_amount': 1000.0,
        'description': '购买股票测试',
        'channel': account1_data['name'],
        'main_category': '投资理财',
        'sub_category': '证券投资',
        'created_at': '2025-11-21 15:00:00',
        'updated_at': '2025-11-21 15:00:00'
    }
    
    bill_id = await db.create_bill(bill_data)
    logger.info(f"✓ 创建投资账单 ID={bill_id}")
    logger.info(f"  - source_account_id: {account1_id}")
    logger.info(f"  - destination_account_id: {account2_id} (关键验证点)")
    logger.info(f"  - destination_amount: {bill_data['destination_amount']}")
    
    # 从数据库查询验证
    bills = await db.get_bills()
    created_bill = bills[0] if bills else None
    
    assert created_bill is not None, "❌ 账单未创建成功"
    assert created_bill['id'] == bill_id, f"❌ 账单ID不匹配: {created_bill['id']} != {bill_id}"
    assert created_bill['destination_account_id'] == account2_id, \
        f"❌ destination_account_id丢失或错误: {created_bill['destination_account_id']} != {account2_id}"
    assert created_bill['destination_amount'] == 1000.0, \
        f"❌ destination_amount错误: {created_bill['destination_amount']} != 1000.0"
    
    logger.info("✅✅✅ 测试1通过: destination_account_id正确保存到数据库")
    logger.info(f"验证结果: 数据库中的destination_account_id = {created_bill['destination_account_id']}")
    logger.info("=" * 60)
    
    return True


async def test_time_filter_query():
    """测试时间筛选查询功能"""
    logger.info("=" * 60)
    logger.info("测试2: 时间筛选查询功能")
    logger.info("=" * 60)
    
    db = Database(':memory:')
    await db.init_db()  # 修正：使用正确的方法名
    
    # 创建测试账户
    account_data = {
        'name': '测试账户',
        'type': 1,
        'category': 2,
        'currency': 'CNY',
        'balance': 0.0,
        'created_at': '2025-11-21 00:00:00',
        'updated_at': '2025-11-21 00:00:00'
    }
    account_id = await db.create_account(account_data)
    
    # 创建不同月份的测试账单
    test_bills = [
        {
            'date': '2025-10-15 12:00:00',
            'type': '支出',
            'amount': 100.0,
            'counterparty': '',
            'source_account_id': account_id,
            'description': '10月支出',
            'channel': '测试账户',
            'main_category': '餐饮',
            'sub_category': '午餐',
            'created_at': '2025-10-15 12:00:00',
            'updated_at': '2025-10-15 12:00:00'
        },
        {
            'date': '2025-11-10 14:00:00',
            'type': '支出',
            'amount': 200.0,
            'counterparty': '',
            'source_account_id': account_id,
            'description': '11月支出1',
            'channel': '测试账户',
            'main_category': '餐饮',
            'sub_category': '晚餐',
            'created_at': '2025-11-10 14:00:00',
            'updated_at': '2025-11-10 14:00:00'
        },
        {
            'date': '2025-11-20 16:00:00',
            'type': '收入',
            'amount': 5000.0,
            'counterparty': '',
            'source_account_id': account_id,
            'description': '11月收入',
            'channel': '测试账户',
            'main_category': '工资',
            'sub_category': '',
            'created_at': '2025-11-20 16:00:00',
            'updated_at': '2025-11-20 16:00:00'
        },
        {
            'date': '2025-12-05 10:00:00',
            'type': '支出',
            'amount': 300.0,
            'counterparty': '',
            'source_account_id': account_id,
            'description': '12月支出',
            'channel': '测试账户',
            'main_category': '购物',
            'sub_category': '日用品',
            'created_at': '2025-12-05 10:00:00',
            'updated_at': '2025-12-05 10:00:00'
        }
    ]
    
    for bill in test_bills:
        bill_id = await db.create_bill(bill)
        logger.info(f"✓ 创建测试账单: {bill['date'][:10]} - {bill['type']} - ¥{bill['amount']}")
    
    # 测试按月筛选（2025年11月）
    filters = {
        'start_date': '2025-11-01',
        'end_date': '2025-12-01'
    }
    
    bills, total = await db.query_bills(page=1, page_size=100, filters=filters)
    
    logger.info(f"\n📅 查询2025年11月的账单:")
    logger.info(f"  - 筛选条件: {filters}")
    logger.info(f"  - 查询结果: 找到 {total} 条记录")
    
    assert total == 2, f"❌ 时间筛选错误: 应该找到2条11月记录，实际找到{total}条"
    
    # 验证返回的记录都是11月的
    for bill in bills:
        bill_month = bill['date'][5:7]  # 提取月份
        assert bill_month == '11', f"❌ 筛选结果包含非11月记录: {bill['date']}"
        logger.info(f"  ✓ {bill['date']} - {bill['type']} - ¥{bill['amount']} - {bill['description']}")
    
    # 计算总收入总支出
    total_income = sum(b['amount'] for b in bills if b['type'] == '收入')
    total_expense = sum(b['amount'] for b in bills if b['type'] == '支出')
    
    logger.info(f"\n💰 11月统计:")
    logger.info(f"  - 总收入: ¥{total_income:.2f}")
    logger.info(f"  - 总支出: ¥{total_expense:.2f}")
    
    assert total_income == 5000.0, f"❌ 总收入计算错误: {total_income} != 5000.0"
    assert total_expense == 200.0, f"❌ 总支出计算错误: {total_expense} != 200.0"
    
    logger.info("✅✅✅ 测试2通过: 时间筛选查询功能正常")
    logger.info("=" * 60)
    
    return True


async def main():
    """运行所有测试"""
    logger.info("\n" + "=" * 60)
    logger.info("🚀 开始测试投资账单和时间筛选功能修复")
    logger.info("=" * 60 + "\n")
    
    test_results = []
    
    try:
        # 测试1: 投资账单持久化
        result1 = await test_investment_account_persistence()
        test_results.append(("投资账单destination_account_id持久化", result1))
    except Exception as e:
        logger.error(f"❌ 测试1失败: {e}", exc_info=True)
        test_results.append(("投资账单destination_account_id持久化", False))
    
    try:
        # 测试2: 时间筛选查询
        result2 = await test_time_filter_query()
        test_results.append(("时间筛选查询功能", result2))
    except Exception as e:
        logger.error(f"❌ 测试2失败: {e}", exc_info=True)
        test_results.append(("时间筛选查询功能", False))
    
    # 汇总测试结果
    logger.info("\n" + "=" * 60)
    logger.info("📊 测试结果汇总")
    logger.info("=" * 60)
    
    passed = sum(1 for _, result in test_results if result)
    total = len(test_results)
    
    for test_name, result in test_results:
        status = "✅ 通过" if result else "❌ 失败"
        logger.info(f"{status} - {test_name}")
    
    logger.info("=" * 60)
    logger.info(f"总计: {passed}/{total} 通过 ({passed/total*100:.1f}%)")
    logger.info("=" * 60)
    
    if passed == total:
        logger.info("🎉 所有测试通过！修复成功！")
        return 0
    else:
        logger.error("⚠️ 部分测试失败，请检查日志")
        return 1


if __name__ == '__main__':
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
