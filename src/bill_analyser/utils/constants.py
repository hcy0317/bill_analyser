"""
API Constants and Mappings - API常量和映射定义

统一管理前后端之间的类型映射和常量定义，遵循ezBookkeeping标准。

Author: Bill Analyser Team
Created: 2025-11-21
"""

from enum import IntEnum
from typing import Dict, List


# ==================== 交易类型定义 ====================

class TransactionType(IntEnum):
    """交易类型枚举 (ezBookkeeping标准)"""
    INCOME = 2      # 收入
    EXPENSE = 3     # 支出
    TRANSFER = 4    # 转账
    INVESTMENT = 5  # 投资


# 前端整数类型 → 后端中文类型
FRONTEND_TO_BACKEND_TYPE: Dict[int, str] = {
    TransactionType.INCOME: '收入',
    TransactionType.EXPENSE: '支出',
    TransactionType.TRANSFER: '转账',
    TransactionType.INVESTMENT: '投资',
}

# 后端中文类型 → 前端整数类型
BACKEND_TO_FRONTEND_TYPE: Dict[str, int] = {
    '收入': TransactionType.INCOME,
    '支出': TransactionType.EXPENSE,
    '转账': TransactionType.TRANSFER,
    '投资': TransactionType.INVESTMENT,
}

# 类型显示名称(多语言支持)
TRANSACTION_TYPE_NAMES: Dict[int, Dict[str, str]] = {
    TransactionType.INCOME: {
        'zh-CN': '收入',
        'zh-TW': '收入',
        'en-US': 'Income'
    },
    TransactionType.EXPENSE: {
        'zh-CN': '支出',
        'zh-TW': '支出',
        'en-US': 'Expense'
    },
    TransactionType.TRANSFER: {
        'zh-CN': '转账',
        'zh-TW': '轉賬',
        'en-US': 'Transfer'
    },
    TransactionType.INVESTMENT: {
        'zh-CN': '投资',
        'zh-TW': '投資',
        'en-US': 'Investment'
    }
}


# ==================== 账户类型定义 ====================

class AccountCategory(IntEnum):
    """账户分类 (ezBookkeeping标准)"""
    CASH = 1                # 现金
    CHECKING = 2            # 储蓄账户
    CREDIT_CARD = 3         # 信用卡
    DEBIT_CARD = 4          # 借记卡
    VIRTUAL_ACCOUNT = 5     # 虚拟账户(支付宝/微信)
    INVESTMENT = 6          # 投资账户
    LOAN = 7               # 贷款
    RECEIVABLE = 8         # 应收账款
    PAYABLE = 9            # 应付账款
    OTHER = 10             # 其他


class AccountType(IntEnum):
    """账户类型"""
    SINGLE_ACCOUNT = 0      # 单账户
    MULTI_SUB_ACCOUNTS = 1  # 多子账户


# 账户分类显示名称
ACCOUNT_CATEGORY_NAMES: Dict[int, Dict[str, str]] = {
    AccountCategory.CASH: {'zh-CN': '现金', 'en-US': 'Cash'},
    AccountCategory.CHECKING: {'zh-CN': '储蓄账户', 'en-US': 'Checking'},
    AccountCategory.CREDIT_CARD: {'zh-CN': '信用卡', 'en-US': 'Credit Card'},
    AccountCategory.DEBIT_CARD: {'zh-CN': '借记卡', 'en-US': 'Debit Card'},
    AccountCategory.VIRTUAL_ACCOUNT: {'zh-CN': '虚拟账户', 'en-US': 'Virtual Account'},
    AccountCategory.INVESTMENT: {'zh-CN': '投资账户', 'en-US': 'Investment'},
    AccountCategory.LOAN: {'zh-CN': '贷款', 'en-US': 'Loan'},
    AccountCategory.RECEIVABLE: {'zh-CN': '应收账款', 'en-US': 'Receivable'},
    AccountCategory.PAYABLE: {'zh-CN': '应付账款', 'en-US': 'Payable'},
    AccountCategory.OTHER: {'zh-CN': '其他', 'en-US': 'Other'},
}

# 资产类账户(正余额为资产)
ASSET_ACCOUNT_CATEGORIES: List[int] = [
    AccountCategory.CASH,
    AccountCategory.CHECKING,
    AccountCategory.DEBIT_CARD,
    AccountCategory.VIRTUAL_ACCOUNT,
    AccountCategory.INVESTMENT,
    AccountCategory.RECEIVABLE
]

# 负债类账户(正余额为负债)
LIABILITY_ACCOUNT_CATEGORIES: List[int] = [
    AccountCategory.CREDIT_CARD,
    AccountCategory.LOAN,
    AccountCategory.PAYABLE
]


# ==================== 分类类型定义 ====================

class CategoryType(IntEnum):
    """分类类型 (与TransactionType一致)"""
    INCOME = 2
    EXPENSE = 3
    TRANSFER = 4
    INVESTMENT = 5


# ==================== API响应常量 ====================

# 默认父级ID(顶级节点)
DEFAULT_PARENT_ID = "0"

# 默认时区偏移(UTC+8, 中国标准时间)
DEFAULT_UTC_OFFSET = 480  # 分钟

# 默认货币
DEFAULT_CURRENCY = "CNY"

# 默认分页大小
DEFAULT_PAGE_SIZE = 50
MAX_PAGE_SIZE = 500

# 时间戳格式
TIMESTAMP_FORMAT_DB = '%Y-%m-%d %H:%M:%S'      # 数据库存储格式
TIMESTAMP_FORMAT_DATE = '%Y-%m-%d'             # 仅日期格式
TIMESTAMP_FORMAT_DISPLAY = '%Y年%m月%d日 %H:%M'  # 显示格式


# ==================== 标签筛选类型 ====================

class TagFilterType(IntEnum):
    """标签筛选类型"""
    ANY = 0     # 包含任意一个标签
    ALL = 1     # 包含所有标签


# ==================== 金额筛选操作符 ====================

AMOUNT_FILTER_OPERATORS = {
    'eq': '等于',       # ==
    'ne': '不等于',     # !=
    'gt': '大于',       # >
    'lt': '小于',       # <
    'gte': '大于等于',  # >=
    'lte': '小于等于',  # <=
    'between': '范围'   # BETWEEN ... AND ...
}


# ==================== 排序字段 ====================

VALID_SORT_FIELDS = [
    'date',
    'amount',
    'type',
    'category',
    'account',
    'created_at',
    'updated_at'
]

VALID_SORT_ORDERS = ['asc', 'desc']


# ==================== 去重模式 ====================

DEDUPLICATION_MODES = {
    'simple': '简单去重(完全匹配)',
    'advanced': '高级去重(模糊匹配+时间窗口)',
    'aggressive': '激进去重(最宽松)'
}


# ==================== 错误代码 ====================

class ErrorCode(IntEnum):
    """API错误代码"""
    SUCCESS = 0
    INVALID_REQUEST = 400
    UNAUTHORIZED = 401
    FORBIDDEN = 403
    NOT_FOUND = 404
    CONFLICT = 409
    VALIDATION_ERROR = 422
    INTERNAL_ERROR = 500
    DATABASE_ERROR = 501
    EXTERNAL_SERVICE_ERROR = 502


ERROR_MESSAGES: Dict[int, Dict[str, str]] = {
    ErrorCode.INVALID_REQUEST: {
        'zh-CN': '请求参数错误',
        'en-US': 'Invalid request parameters'
    },
    ErrorCode.UNAUTHORIZED: {
        'zh-CN': '未授权，请先登录',
        'en-US': 'Unauthorized, please login first'
    },
    ErrorCode.FORBIDDEN: {
        'zh-CN': '无权访问此资源',
        'en-US': 'Forbidden to access this resource'
    },
    ErrorCode.NOT_FOUND: {
        'zh-CN': '资源不存在',
        'en-US': 'Resource not found'
    },
    ErrorCode.CONFLICT: {
        'zh-CN': '资源冲突',
        'en-US': 'Resource conflict'
    },
    ErrorCode.VALIDATION_ERROR: {
        'zh-CN': '数据验证失败',
        'en-US': 'Validation failed'
    },
    ErrorCode.INTERNAL_ERROR: {
        'zh-CN': '服务器内部错误',
        'en-US': 'Internal server error'
    },
    ErrorCode.DATABASE_ERROR: {
        'zh-CN': '数据库操作失败',
        'en-US': 'Database operation failed'
    }
}


# ==================== 辅助函数 ====================

def get_transaction_type_name(type_value: int, locale: str = 'zh-CN') -> str:
    """获取交易类型显示名称"""
    return TRANSACTION_TYPE_NAMES.get(type_value, {}).get(locale, '未知')


def get_account_category_name(category: int, locale: str = 'zh-CN') -> str:
    """获取账户分类显示名称"""
    return ACCOUNT_CATEGORY_NAMES.get(category, {}).get(locale, '未知')


def is_asset_account(category: int) -> bool:
    """判断是否为资产类账户"""
    return category in ASSET_ACCOUNT_CATEGORIES


def is_liability_account(category: int) -> bool:
    """判断是否为负债类账户"""
    return category in LIABILITY_ACCOUNT_CATEGORIES


def get_error_message(error_code: int, locale: str = 'zh-CN') -> str:
    """获取错误消息"""
    return ERROR_MESSAGES.get(error_code, {}).get(locale, '未知错误')
