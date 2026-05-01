"""Smart deduplication public data models."""

from dataclasses import dataclass
from enum import Enum
from typing import Any


class DeduplicationType(Enum):
    """去重类型"""

    EXACT = "exact"  # 完全重复
    TRANSFER = "transfer"  # 转账配对
    PLATFORM_BANK = "platform_bank"  # 支付平台与银行重复
    SIMILAR = "similar"  # 类似账单去重
    SPLIT_MERGE = "split_merge"  # 总账单与分账单
    DATABASE_DUPLICATE = "database_duplicate"  # 与数据库已有账单重复


@dataclass
class DuplicateGroup:
    """重复账单组"""

    type: DeduplicationType
    bills: list[dict[str, Any]]
    keep_bill: dict[str, Any]  # 保留的账单
    remove_bills: list[dict[str, Any]]  # 移除的账单
    reason: str  # 去重原因说明


@dataclass
class DeduplicationResult:
    """去重结果

    v6.42.1修正：恢复transfer_pairs字段用于转账配对
    """

    original_count: int  # 原始账单数
    kept_bills: list[dict[str, Any]]  # 保留的账单
    removed_count: int  # 移除的账单数
    duplicate_groups: list[DuplicateGroup]  # 重复组详情
    transfer_pairs: list[tuple[dict, dict]]  # 识别的转账对
    split_groups: list[dict]  # 识别的分账单组
