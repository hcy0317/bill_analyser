"""测试投资类型交易的分类持久化问题

复现场景：
1. 创建投资类型交易并设置分类
2. 刷新交易列表（模拟前往其他页面再返回）
3. 验证分类是否仍然存在

预期结果：
- 分类信息应该在数据库中正确存储
- API返回的数据应该包含正确的categoryId和category对象
- 前端Transaction对象应该正确解析并保留分类信息
"""

import asyncio
import sys
import os
from datetime import datetime

# 添加项目根目录到Python路径
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from src.core.db import Database
from src.api.adapters.transaction_adapter import TransactionAdapter
from src.utils.logger import get_logger

logger = get_logger('TestInvestmentCategoryPersistence')


async def test_investment_category_persistence():
    """测试投资类型交易的分类持久化"""
    
    logger.info("="*60)
    logger.info("测试开始: 投资类型交易分类持久化")
    logger.info("="*60)
    
    # 初始化数据库和适配器
    db = Database()
    adapter = TransactionAdapter(db)
    
    # 1. 查找或创建测试账户
    logger.info("\n[步骤1] 查找测试账户...")
    accounts = await db.get_all_accounts()
    test_account = None
    for acc in accounts:
        if '投资' in acc['name']:
            test_account = acc
            break
    
    if not test_account:
        logger.error("❌ 找不到投资账户，请先创建一个名称包含'投资'的账户")
        return False
    
    logger.info(f"✅ 使用账户: {test_account['name']} (ID={test_account['id']})")
    
    # 2. 查找投资分类
    logger.info("\n[步骤2] 查找投资分类...")
    categories = await db.get_all_categories()
    investment_category = None
    for cat in categories:
        if cat['type'] == 5 and cat['sub_category']:  # 投资类型且有子分类
            investment_category = cat
            break
    
    if not investment_category:
        logger.error("❌ 找不到投资分类，请先创建投资类型的分类")
        return False
    
    logger.info(f"✅ 使用分类: {investment_category['main_category']}/{investment_category['sub_category']} (ID={investment_category['id']})")
    
    # 3. 创建测试交易
    logger.info("\n[步骤3] 创建投资类型交易...")
    now = datetime.now()
    bill_data = {
        'date': now.strftime('%Y-%m-%d %H:%M:%S'),
        'type': '投资',
        'amount': 500.0,
        'counterparty': '测试投资',
        'description': '测试投资分类持久化',
        'main_category': investment_category['main_category'],
        'sub_category': investment_category['sub_category'],
        'source_account_id': test_account['id'],
        'destination_account_id': test_account['id'],
        'created_at': now.isoformat(),
        'updated_at': now.isoformat()
    }
    
    bill_id = await db.create_bill(bill_data)
    if not bill_id:
        logger.error("❌ 创建交易失败")
        return False
    
    logger.info(f"✅ 创建交易成功: ID={bill_id}")
    
    # 4. 立即查询交易（第一次查询）
    logger.info("\n[步骤4] 第一次查询交易（创建后）...")
    bill1 = await db.get_bill_by_id(bill_id)
    
    if not bill1:
        logger.error("❌ 查询交易失败")
        return False
        
    logger.info(f"数据库数据: main_category={bill1.get('main_category')}, sub_category={bill1.get('sub_category')}")
    
    # 使用adapter转换
    tags1 = await db.get_tags_for_bill(bill_id)
    v1_bill1 = await adapter.backend_to_frontend(bill1, tags=tags1)
    
    logger.info(f"适配器转换后: categoryId={v1_bill1.get('categoryId')}, categoryName={v1_bill1.get('categoryName')}")
    logger.info(f"是否有category对象: {'是' if 'category' in v1_bill1 else '否'}")
    
    if 'category' in v1_bill1:
        logger.info(f"category对象内容: id={v1_bill1['category'].get('id')}, name={v1_bill1['category'].get('name')}, type={v1_bill1['category'].get('type')}")
    
    # 5. 模拟前往其他页面（清除缓存）
    logger.info("\n[步骤5] 模拟页面切换（清除adapter缓存）...")
    # 创建新的adapter实例，模拟前端重新加载
    adapter2 = TransactionAdapter(db)
    
    # 6. 再次查询交易（第二次查询）
    logger.info("\n[步骤6] 第二次查询交易（模拟返回交易列表）...")
    bill2 = await db.get_bill_by_id(bill_id)
    
    if not bill2:
        logger.error("❌ 第二次查询交易失败")
        return False
        
    logger.info(f"数据库数据: main_category={bill2.get('main_category')}, sub_category={bill2.get('sub_category')}")
    
    # 使用新的adapter转换
    tags2 = await db.get_tags_for_bill(bill_id)
    v1_bill2 = await adapter2.backend_to_frontend(bill2, tags=tags2)
    
    logger.info(f"适配器转换后: categoryId={v1_bill2.get('categoryId')}, categoryName={v1_bill2.get('categoryName')}")
    logger.info(f"是否有category对象: {'是' if 'category' in v1_bill2 else '否'}")
    
    if 'category' in v1_bill2:
        logger.info(f"category对象内容: id={v1_bill2['category'].get('id')}, name={v1_bill2['category'].get('name')}, type={v1_bill2['category'].get('type')}")
    
    # 7. 验证结果
    logger.info("\n[步骤7] 验证结果...")
    success = True
    
    # 验证第一次查询
    if v1_bill1.get('categoryId') == "0":
        logger.error("❌ 第一次查询: categoryId=0，分类丢失！")
        success = False
    else:
        logger.info(f"✅ 第一次查询: categoryId={v1_bill1.get('categoryId')}")
    
    if 'category' not in v1_bill1:
        logger.error("❌ 第一次查询: 缺少category对象！")
        success = False
    else:
        logger.info(f"✅ 第一次查询: category对象存在")
    
    # 验证第二次查询
    if v1_bill2.get('categoryId') == "0":
        logger.error("❌ 第二次查询: categoryId=0，分类丢失！（重现bug）")
        success = False
    else:
        logger.info(f"✅ 第二次查询: categoryId={v1_bill2.get('categoryId')}")
    
    if 'category' not in v1_bill2:
        logger.error("❌ 第二次查询: 缺少category对象！（重现bug）")
        success = False
    else:
        logger.info(f"✅ 第二次查询: category对象存在")
    
    # 8. 清理测试数据
    logger.info(f"\n[步骤8] 清理测试数据: bill_id={bill_id}")
    await db.delete_bill(bill_id)
    
    logger.info("\n" + "="*60)
    if success:
        logger.info("✅✅✅ 测试通过: 分类信息正确持久化")
    else:
        logger.error("❌❌❌ 测试失败: 分类信息丢失")
    logger.info("="*60)
    
    return success


if __name__ == '__main__':
    asyncio.run(test_investment_category_persistence())
