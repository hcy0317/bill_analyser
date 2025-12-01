"""V1 Account Adapter - 账户数据格式适配器

统一处理账户相关的数据格式转换。

Author: Bill Analyser Team
Created: 2025-11-21
"""

import json
from typing import Dict, Any, List

from src.utils.currency import cents_to_yuan, yuan_to_cents
from src.utils.constants import (
    ASSET_ACCOUNT_CATEGORIES,
    LIABILITY_ACCOUNT_CATEGORIES,
    DEFAULT_PARENT_ID
)
from src.utils.logger import get_logger, log_method

logger = get_logger('V1AccountAdapter')


class V1AccountAdapter:
    """v1账户数据适配器"""

    @staticmethod
    @log_method
    def frontend_to_backend(frontend_data: Dict[str, Any]) -> Dict[str, Any]:
        """将前端账户数据转换为后端格式

        Args:
            frontend_data: 前端账户数据(金额为分)

        Returns:
            Dict: 后端账户数据(金额为元)
        """
        backend_data = frontend_data.copy()

        # 转换金额: 分 → 元
        if 'balance' in backend_data and backend_data['balance'] is not None:
            backend_data['balance'] = cents_to_yuan(backend_data['balance'])

        if 'initial_balance' in backend_data and backend_data['initial_balance'] is not None:
            backend_data['initial_balance'] = cents_to_yuan(backend_data['initial_balance'])

        # 转换parent_id格式
        if 'parentId' in backend_data:
            parent_id_str = backend_data.pop('parentId')
            backend_data['parent_id'] = int(parent_id_str) if parent_id_str != "0" else 0

        # 转换aliases: 前端数组 → 后端JSON字符串
        if 'aliases' in backend_data:
            aliases = backend_data['aliases']
            if isinstance(aliases, list):
                backend_data['aliases'] = json.dumps(aliases, ensure_ascii=False)
            elif aliases is None:
                backend_data['aliases'] = None
            # 如果已经是字符串，保持不变

        logger.debug(f"账户数据转换: balance={backend_data.get('balance')}元")

        return backend_data

    @staticmethod
    @log_method
    def backend_to_frontend(account: Dict[str, Any]) -> Dict[str, Any]:
        """将后端账户数据转换为前端v1格式

        Args:
            account: 后端账户数据(金额为元)

        Returns:
            Dict: 前端v1格式账户数据(金额为分)
        """
        if not account:
            return account

        # 创建副本避免修改原数据
        formatted = account.copy()

        # 1. 映射 parent_id -> parentId
        if 'parent_id' in formatted:
            formatted['parentId'] = str(formatted['parent_id'])
            del formatted['parent_id']

        # 确保parentId存在（顶级账户为"0"）
        if 'parentId' not in formatted or formatted.get('parentId') is None:
            formatted['parentId'] = DEFAULT_PARENT_ID

        # 2. 转换金额: 元 → 分
        if 'balance' in formatted and formatted['balance'] is not None:
            formatted['balance'] = yuan_to_cents(formatted['balance'])

        if 'initial_balance' in formatted and formatted['initial_balance'] is not None:
            formatted['initial_balance'] = yuan_to_cents(formatted['initial_balance'])

        # 3. 确保ID是字符串格式
        if 'id' in formatted and isinstance(formatted['id'], int):
            formatted['id'] = str(formatted['id'])

        # 4. 根据category判断资产/负债类型
        category = formatted.get('category', 1)
        formatted['isAsset'] = category in ASSET_ACCOUNT_CATEGORIES
        formatted['isLiability'] = category in LIABILITY_ACCOUNT_CATEGORIES

        # 5. 转换aliases: 后端JSON字符串 → 前端数组
        if 'aliases' in formatted:
            aliases_str = formatted['aliases']
            if aliases_str:
                try:
                    aliases = json.loads(aliases_str)
                    formatted['aliases'] = aliases if isinstance(aliases, list) else []
                except json.JSONDecodeError:
                    formatted['aliases'] = []
            else:
                formatted['aliases'] = []
        else:
            formatted['aliases'] = []

        # 6. 递归格式化子账户
        if 'subAccounts' in formatted and isinstance(formatted['subAccounts'], list):
            formatted['subAccounts'] = [
                V1AccountAdapter.backend_to_frontend(sub)
                for sub in formatted['subAccounts']
            ]

        return formatted

    @staticmethod
    @log_method
    def build_hierarchy(accounts: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
        """构建账户层级结构

        Args:
            accounts: 扁平的账户列表

        Returns:
            List: 层级结构账户列表(顶级账户包含subAccounts)
        """
        if not accounts:
            return []

        logger.info(f"开始构建账户层级: total={len(accounts)}")

        # 分离顶级账户和子账户
        top_level_accounts = []
        sub_accounts_map = {}  # parent_id -> [子账户列表]

        for acc in accounts:
            parent_id = acc.get('parent_id', 0)
            if parent_id == 0 or parent_id is None:
                top_level_accounts.append(acc)
            else:
                if parent_id not in sub_accounts_map:
                    sub_accounts_map[parent_id] = []
                sub_accounts_map[parent_id].append(acc)

        # 将子账户附加到父账户
        for parent in top_level_accounts:
            parent_id = parent['id']
            if parent_id in sub_accounts_map:
                parent['subAccounts'] = sub_accounts_map[parent_id]

        logger.info(f"层级构建完成: 顶级={len(top_level_accounts)}, "
                   f"子账户={len(accounts) - len(top_level_accounts)}")

        return top_level_accounts

    @staticmethod
    @log_method
    def format_list_response(accounts: List[Dict[str, Any]],
                            build_hierarchy_flag: bool = True) -> Dict[str, Any]:
        """格式化账户列表响应

        Args:
            accounts: 账户列表
            build_hierarchy_flag: 是否构建层级结构

        Returns:
            Dict: v1响应格式
        """
        if build_hierarchy_flag:
            accounts = V1AccountAdapter.build_hierarchy(accounts)

        # 格式化每个账户
        formatted_accounts = [
            V1AccountAdapter.backend_to_frontend(acc)
            for acc in accounts
        ]

        return {
            'success': True,
            'result': formatted_accounts
        }
