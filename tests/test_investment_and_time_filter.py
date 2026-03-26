"""
Bill Analyser - 投资账单和时间筛选功能测试用例

测试目标: 验证投资账单destination_account_id保存和时间筛选功能修复
测试时间: 2025-11-21
测试环境: 本地开发环境（端口5000后端，8081前端）
"""

import sys
import json
import asyncio
from pathlib import Path

# 添加项目路径
project_root = Path(__file__).parent.parent
sys.path.insert(0, str(project_root))

from src.core.db import Database
from src.utils.logger import get_logger

logger = get_logger('TestInvestmentAndTimeFilter')


async def test_investment_bill_destination_account():
    """测试投资账单的destination_account_id是否正确保存"""
    logger.info("=" * 60)
    logger.info("测试1: 投资账单destination_account_id保存")
    logger.info("=" * 60)
    
    db = Database()
    await db.init_db()
    
    # 获取所有账户
    accounts = await db.get_all_accounts()
    if len(accounts) < 2:
        logger.error("❌ 测试失败：账户数量不足（需要至少2个账户）")
        return False
    
    source_account = accounts[0]
    dest_account = accounts[1]
    
    logger.info(f"使用账户: 源={source_account['name']}(ID={source_account['id']}), "
               f"目标={dest_account['name']}(ID={dest_account['id']})")
    
    # 创建测试投资账单
    bill_data = {
        'date': '2025-11-21 10:00:00',
        'type': '投资',
        'amount': 1000.0,
        'source_account_id': source_account['id'],
        'destination_account_id': dest_account['id'],
        'destination_amount': 1000.0,
        'counterparty': '测试投资',
        'description': '投资账单destination_account_id保存测试',
        'channel': source_account['name'],
        'main_category': '投资理财',
        'sub_category': '证券投资',
        'created_at': '2025-11-21 10:00:00',
        'updated_at': '2025-11-21 10:00:00'
    }
    
    logger.info(f"📝 创建测试账单，数据: {json.dumps(bill_data, ensure_ascii=False, indent=2)}")
    
    bill_id = await db.create_bill(bill_data)
    
    if not bill_id:
        logger.error("❌ 测试失败：账单创建失败")
        return False
    
    logger.info(f"✅ 账单创建成功，ID={bill_id}")
    
    # 查询账单验证
    saved_bill = await db.get_bill_by_id(bill_id)
    
    if not saved_bill:
        logger.error(f"❌ 测试失败：无法查询到账单ID={bill_id}")
        await db.delete_bill(bill_id)
        return False
    
    logger.info("🔍 查询到的账单数据:")
    logger.info(f"  - id: {saved_bill['id']}")
    logger.info(f"  - type: {saved_bill['type']}")
    logger.info(f"  - source_account_id: {saved_bill.get('source_account_id')}")
    logger.info(f"  - destination_account_id: {saved_bill.get('destination_account_id')}")
    logger.info(f"  - destination_amount: {saved_bill.get('destination_amount')}")
    
    # 验证关键字段
    success = True
    
    if saved_bill.get('destination_account_id') != dest_account['id']:
        logger.error(f"❌ 测试失败：destination_account_id不匹配")
        logger.error(f"   期望: {dest_account['id']}, 实际: {saved_bill.get('destination_account_id')}")
        success = False
    else:
        logger.info(f"✅ destination_account_id正确: {saved_bill.get('destination_account_id')}")
    
    if saved_bill.get('destination_amount') != 1000.0:
        logger.error(f"❌ 测试失败：destination_amount不匹配")
        logger.error(f"   期望: 1000.0, 实际: {saved_bill.get('destination_amount')}")
        success = False
    else:
        logger.info(f"✅ destination_amount正确: {saved_bill.get('destination_amount')}")
    
    # 清理测试数据
    await db.delete_bill(bill_id)
    logger.info(f"🧹 已删除测试账单ID={bill_id}")
    
    if success:
        logger.info("✅✅✅ 测试1通过：投资账单destination_account_id保存正确")
    else:
        logger.error("❌❌❌ 测试1失败：投资账单destination_account_id保存错误")
    
    logger.info("=" * 60)
    return success


async def test_time_filter_query():
    """测试时间筛选查询功能"""
    logger.info("=" * 60)
    logger.info("测试2: 时间筛选查询功能")
    logger.info("=" * 60)
    
    db = Database()
    await db.init_db()
    
    # 创建测试账单（不同月份）
    test_bills = [
        {
            'date': '2025-10-15 10:00:00',
            'type': '支出',
            'amount': 100.0,
            'source_account_id': 1,
            'counterparty': '测试商户1',
            'description': '10月测试账单',
            'channel': '测试账户',
            'main_category': '食品酒水',
            'sub_category': '餐饮',
            'created_at': '2025-10-15 10:00:00',
            'updated_at': '2025-10-15 10:00:00'
        },
        {
            'date': '2025-11-15 10:00:00',
            'type': '支出',
            'amount': 200.0,
            'source_account_id': 1,
            'counterparty': '测试商户2',
            'description': '11月测试账单',
            'channel': '测试账户',
            'main_category': '食品酒水',
            'sub_category': '餐饮',
            'created_at': '2025-11-15 10:00:00',
            'updated_at': '2025-11-15 10:00:00'
        }
    ]
    
    created_ids = []
    for bill in test_bills:
        bill_id = await db.create_bill(bill)
        if bill_id:
            created_ids.append(bill_id)
            logger.info(f"✅ 创建测试账单ID={bill_id}, 日期={bill['date']}")
        else:
            logger.error(f"❌ 创建测试账单失败: {bill['date']}")
    
    # 测试按月查询
    filters_november = {
        'start_date': '2025-11-01',
        'end_date': '2025-12-01'
    }
    
    logger.info(f"📅 查询11月账单，过滤条件: {filters_november}")
    bills_nov, total_nov = await db.query_bills(page=1, page_size=100, filters=filters_november)
    
    logger.info(f"🔍 11月查询结果: 找到{total_nov}条记录")
    
    november_test_bills = [b for b in bills_nov if b['description'] == '11月测试账单']
    
    success = True
    
    if len(november_test_bills) == 0:
        logger.error("❌ 测试失败：未找到11月测试账单")
        success = False
    else:
        logger.info(f"✅ 找到11月测试账单: {len(november_test_bills)}条")
        for bill in november_test_bills:
            logger.info(f"  - ID={bill['id']}, 日期={bill['date']}, 金额={bill['amount']}")
    
    # 验证10月账单不在结果中
    october_test_bills = [b for b in bills_nov if b['description'] == '10月测试账单']
    if len(october_test_bills) > 0:
        logger.error("❌ 测试失败：11月查询中包含了10月账单")
        success = False
    else:
        logger.info("✅ 11月查询正确：不包含10月账单")
    
    # 清理测试数据
    for bill_id in created_ids:
        await db.delete_bill(bill_id)
        logger.info(f"🧹 已删除测试账单ID={bill_id}")
    
    if success:
        logger.info("✅✅✅ 测试2通过：时间筛选查询功能正常")
    else:
        logger.error("❌❌❌ 测试2失败：时间筛选查询功能异常")
    
    logger.info("=" * 60)
    return success


async def run_all_tests():
    """运行所有测试"""
    logger.info("\n" + "=" * 60)
    logger.info("Bill Analyser - 功能修复验证测试套件")
    logger.info("=" * 60)
    
    results = []
    
    # 测试1: 投资账单
    try:
        result1 = await test_investment_bill_destination_account()
        results.append(("投资账单destination_account_id保存", result1))
    except Exception as e:
        logger.error(f"❌ 测试1异常: {e}", exc_info=True)
        results.append(("投资账单destination_account_id保存", False))
    
    # 测试2: 时间筛选
    try:
        result2 = await test_time_filter_query()
        results.append(("时间筛选查询功能", result2))
    except Exception as e:
        logger.error(f"❌ 测试2异常: {e}", exc_info=True)
        results.append(("时间筛选查询功能", False))
    
    # 汇总结果
    logger.info("\n" + "=" * 60)
    logger.info("测试结果汇总")
    logger.info("=" * 60)
    
    passed = sum(1 for _, result in results if result)
    total = len(results)
    
    for name, result in results:
        status = "✅ 通过" if result else "❌ 失败"
        logger.info(f"{status}: {name}")
    
    logger.info("=" * 60)
    logger.info(f"总计: {passed}/{total} 通过 ({passed/total*100:.1f}%)")
    logger.info("=" * 60)
    
    return passed == total


if __name__ == "__main__":
    success = asyncio.run(run_all_tests())
    sys.exit(0 if success else 1)
