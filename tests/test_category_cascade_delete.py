"""
测试分类级联删除功能
"""
import pytest
import asyncio
from datetime import datetime

from src.core.db import Database
from src.utils.logger import get_logger

logger = get_logger('TestCategoryCascadeDelete')


@pytest.mark.asyncio
async def test_delete_parent_category_cascades_to_children():
    """测试删除父级分类时级联删除所有子分类"""
    db = Database('src/data/bills.db')
    
    # 创建测试数据：一个父级分类和3个子分类
    test_main = f"测试主分类_{datetime.now().strftime('%Y%m%d%H%M%S')}"
    
    # 创建父级分类
    parent_data = {
        'type': 3,  # 支出
        'main_category': test_main,
        'sub_category': '',
        'priority': 0,
        'keywords': '',
        'description': '测试父级分类',
        'icon': 'test-parent',
        'color': 'ff0000',
        'hidden': False
    }
    parent_id = await db.create_category(parent_data)
    assert parent_id is not None, "创建父级分类失败"
    logger.info(f"创建测试父级分类 ID: {parent_id}, 名称: {test_main}")
    
    # 创建3个子分类
    child_ids = []
    for i in range(3):
        child_data = {
            'type': 3,
            'main_category': test_main,
            'sub_category': f'子分类{i+1}',
            'priority': i,
            'keywords': f'test{i}',
            'description': f'测试子分类{i+1}',
            'icon': f'test-child-{i}',
            'color': f'00ff0{i}',
            'hidden': False
        }
        child_id = await db.create_category(child_data)
        assert child_id is not None, f"创建子分类{i+1}失败"
        child_ids.append(child_id)
        logger.info(f"创建测试子分类{i+1} ID: {child_id}")
    
    # 验证数据已创建
    all_cats = await db.get_all_categories()
    test_cats = [c for c in all_cats if c['main_category'] == test_main]
    assert len(test_cats) == 4, f"应该有4条记录（1父+3子），实际有{len(test_cats)}条"
    
    # 删除父级分类
    logger.info(f"执行级联删除父级分类 ID: {parent_id}")
    success = await db.delete_category(parent_id)
    assert success, "删除父级分类失败"
    
    # 验证父级和所有子分类都已删除
    all_cats_after = await db.get_all_categories()
    remaining_cats = [c for c in all_cats_after if c['main_category'] == test_main]
    assert len(remaining_cats) == 0, f"删除后不应有任何记录，但还有{len(remaining_cats)}条"
    
    # 验证每个子分类都已删除
    for child_id in child_ids:
        child = await db.get_category_by_id(child_id)
        assert child is None, f"子分类 ID {child_id} 应该已删除但仍然存在"
    
    logger.info("✅ 测试通过：删除父级分类成功级联删除所有子分类")


@pytest.mark.asyncio
async def test_delete_child_category_does_not_affect_siblings():
    """测试删除子分类不影响其他子分类和父分类"""
    db = Database('src/data/bills.db')
    
    # 创建测试数据
    test_main = f"测试主分类_{datetime.now().strftime('%Y%m%d%H%M%S')}"
    
    # 创建父级分类
    parent_data = {
        'type': 3,
        'main_category': test_main,
        'sub_category': '',
        'priority': 0,
        'keywords': '',
        'description': '测试父级分类',
        'icon': 'test-parent',
        'color': 'ff0000',
        'hidden': False
    }
    parent_id = await db.create_category(parent_data)
    assert parent_id is not None
    logger.info(f"创建测试父级分类 ID: {parent_id}")
    
    # 创建3个子分类
    child_ids = []
    for i in range(3):
        child_data = {
            'type': 3,
            'main_category': test_main,
            'sub_category': f'子分类{i+1}',
            'priority': i,
            'keywords': '',
            'description': '',
            'icon': '',
            'color': '',
            'hidden': False
        }
        child_id = await db.create_category(child_data)
        child_ids.append(child_id)
        logger.info(f"创建测试子分类{i+1} ID: {child_id}")
    
    # 删除第2个子分类
    target_child_id = child_ids[1]
    logger.info(f"删除子分类 ID: {target_child_id}")
    success = await db.delete_category(target_child_id)
    assert success, "删除子分类失败"
    
    # 验证只有被删除的子分类不存在
    deleted_child = await db.get_category_by_id(target_child_id)
    assert deleted_child is None, "被删除的子分类仍然存在"
    
    # 验证父级分类仍然存在
    parent = await db.get_category_by_id(parent_id)
    assert parent is not None, "父级分类不应该被删除"
    assert parent['main_category'] == test_main
    
    # 验证其他子分类仍然存在
    for i, child_id in enumerate(child_ids):
        if i == 1:  # 跳过被删除的
            continue
        child = await db.get_category_by_id(child_id)
        assert child is not None, f"子分类{i+1} (ID: {child_id}) 不应该被删除"
    
    # 清理测试数据
    await db.delete_category(parent_id)
    
    logger.info("✅ 测试通过：删除子分类不影响其他子分类和父分类")


@pytest.mark.asyncio
async def test_delete_nonexistent_category():
    """测试删除不存在的分类"""
    db = Database('src/data/bills.db')
    
    # 使用一个不存在的ID（假设999999不存在）
    nonexistent_id = 999999
    
    logger.info(f"尝试删除不存在的分类 ID: {nonexistent_id}")
    success = await db.delete_category(nonexistent_id)
    
    # 应该返回False（分类不存在）
    assert success is False, "删除不存在的分类应该返回False"
    
    logger.info("✅ 测试通过：删除不存在的分类正确返回False")


@pytest.mark.asyncio
async def test_delete_category_with_empty_subcategory():
    """测试删除sub_category为空字符串的父级分类"""
    db = Database('src/data/bills.db')
    
    # 创建测试数据
    test_main = f"测试空子类_{datetime.now().strftime('%Y%m%d%H%M%S')}"
    
    # 创建父级分类（sub_category为空字符串）
    parent_data = {
        'type': 3,
        'main_category': test_main,
        'sub_category': '',  # 空字符串
        'priority': 0,
        'keywords': '',
        'description': '测试空字符串子分类',
        'icon': '',
        'color': '',
        'hidden': False
    }
    parent_id = await db.create_category(parent_data)
    logger.info(f"创建sub_category=''的父级分类 ID: {parent_id}")
    
    # 创建2个子分类
    child_ids = []
    for i in range(2):
        child_data = {
            'type': 3,
            'main_category': test_main,
            'sub_category': f'子分类{i+1}',
            'priority': i,
            'keywords': '',
            'description': '',
            'icon': '',
            'color': '',
            'hidden': False
        }
        child_id = await db.create_category(child_data)
        child_ids.append(child_id)
    
    # 删除父级分类（sub_category为空字符串）
    success = await db.delete_category(parent_id)
    assert success, "删除父级分类失败"
    
    # 验证所有相关分类都已删除
    all_cats = await db.get_all_categories()
    remaining = [c for c in all_cats if c['main_category'] == test_main]
    assert len(remaining) == 0, f"应该删除所有记录，但还有{len(remaining)}条"
    
    logger.info("✅ 测试通过：正确处理sub_category为空字符串的父级分类")


if __name__ == '__main__':
    pytest.main([__file__, '-v', '--tb=short'])
