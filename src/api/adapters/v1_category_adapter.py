"""V1 Category Adapter - 分类数据格式适配器

统一处理分类相关的数据格式转换和层级结构构建。

Author: Bill Analyser Team
Created: 2025-11-21
"""

from typing import Dict, Any, List, Tuple

from src.utils.constants import (
    CategoryType,
    DEFAULT_PARENT_ID
)
from src.utils.logger import get_logger, log_method

logger = get_logger('V1CategoryAdapter')


class V1CategoryAdapter:
    """v1分类数据适配器"""
    
    @staticmethod
    @log_method
    def backend_to_frontend(category: Dict[str, Any], is_parent: bool = False) -> Dict[str, Any]:
        """将后端分类数据转换为前端v1格式
        
        Args:
            category: 后端分类数据
            is_parent: 是否为父分类
        
        Returns:
            Dict: 前端v1格式分类数据
        """
        if not category:
            return category
        
        formatted = {}
        
        # 1. ID字段(确保字符串格式)
        formatted['id'] = str(category.get('id', '0'))
        
        # 2. 名称字段
        if is_parent:
            # 父分类: name = main_category
            formatted['name'] = category.get('main_category', '')
            formatted['parentId'] = DEFAULT_PARENT_ID
        else:
            # 子分类: name = sub_category
            formatted['name'] = category.get('sub_category', '')
            # 需要查找父分类ID(由调用方处理)
            formatted['parentId'] = DEFAULT_PARENT_ID  # 占位
        
        # 3. 类型字段(与TransactionType一致)
        formatted['type'] = category.get('type', CategoryType.EXPENSE)
        
        # 4. 显示相关字段
        formatted['icon'] = category.get('icon', '')
        formatted['color'] = category.get('color', '')
        formatted['comment'] = category.get('description', '')
        formatted['displayOrder'] = category.get('priority', 0)
        formatted['visible'] = not category.get('hidden', False)
        
        # 5. 关键词字段
        formatted['keywords'] = category.get('keywords', '')
        
        # 6. 子分类数组(父分类特有)
        if is_parent:
            formatted['subCategories'] = []
        
        return formatted
    
    @staticmethod
    @log_method
    def build_hierarchy(categories: List[Dict[str, Any]]) -> Dict[int, List[Dict[str, Any]]]:
        """构建分类层级结构，按类型分组
        
        Args:
            categories: 扁平的分类列表
        
        Returns:
            Dict: 按类型分组的层级结构
                {
                    2: [收入分类],
                    3: [支出分类],
                    4: [转账分类],
                    5: [投资分类]
                }
        """
        logger.info(f"开始构建分类层级: total={len(categories)}")
        
        # 按类型分组的结果（使用整数键）
        result_map = {
            CategoryType.INCOME.value: [],      # 2
            CategoryType.EXPENSE.value: [],     # 3
            CategoryType.TRANSFER.value: [],    # 4
            CategoryType.INVESTMENT.value: []   # 5
        }
        
        # 主分类映射: (type, main_category) -> category_dict
        main_category_map: Dict[Tuple[int, str], Dict] = {}
        
        # 第一遍：处理主分类(sub_category为空)
        for cat in categories:
            cat_type = cat.get('type', CategoryType.EXPENSE.value)
            main_cat = cat.get('main_category', '')
            sub_cat = cat.get('sub_category', '')
            
            if not sub_cat:  # 主分类
                formatted = V1CategoryAdapter.backend_to_frontend(cat, is_parent=True)
                result_map[cat_type].append(formatted)
                main_category_map[(cat_type, main_cat)] = formatted
                logger.debug(f"主分类: type={cat_type}, name={main_cat}, id={cat['id']}")
        
        # 第二遍：处理子分类(sub_category不为空)
        for cat in categories:
            cat_type = cat.get('type', CategoryType.EXPENSE.value)
            main_cat = cat.get('main_category', '')
            sub_cat = cat.get('sub_category', '')
            
            if sub_cat:  # 子分类
                # 查找父分类
                parent_key = (cat_type, main_cat)
                if parent_key in main_category_map:
                    parent = main_category_map[parent_key]
                    formatted = V1CategoryAdapter.backend_to_frontend(cat, is_parent=False)
                    formatted['parentId'] = parent['id']
                    parent['subCategories'].append(formatted)
                    logger.debug(f"子分类: type={cat_type}, main={main_cat}, sub={sub_cat}, parent_id={parent['id']}")
                else:
                    logger.warning(f"找不到父分类: type={cat_type}, main={main_cat}")
        
        # 统计信息
        for cat_type, cat_list in result_map.items():
            total_subs = sum(len(c.get('subCategories', [])) for c in cat_list)
            logger.info(f"类型{cat_type}: 主分类={len(cat_list)}, 子分类={total_subs}")
        
        return result_map
    
    @staticmethod
    @log_method
    def format_list_response(categories: List[Dict[str, Any]]) -> Dict[str, Any]:
        """格式化分类列表响应
        
        Args:
            categories: 分类列表
        
        Returns:
            Dict: v1响应格式
                {
                    "success": true,
                    "result": {
                        "2": [收入分类],
                        "3": [支出分类],
                        "4": [转账分类],
                        "5": [投资分类]
                    }
                }
        """
        hierarchy = V1CategoryAdapter.build_hierarchy(categories)
        
        # 将枚举键转换为字符串键(JSON兼容)
        result = {str(k): v for k, v in hierarchy.items()}
        
        return {
            'success': True,
            'result': result
        }
    
    @staticmethod
    @log_method  
    def get_flat_list(categories: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """获取扁平的分类列表(不构建层级)
        
        Args:
            categories: 原始分类列表
        
        Returns:
            List: 扁平分类列表
        """
        formatted = []
        for cat in categories:
            # 判断是否为父分类
            is_parent = not cat.get('sub_category', '')
            formatted_cat = V1CategoryAdapter.backend_to_frontend(cat, is_parent)
            formatted.append(formatted_cat)
        
        return formatted
