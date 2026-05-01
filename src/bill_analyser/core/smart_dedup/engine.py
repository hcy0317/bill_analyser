"""Smart deduplication engine orchestration."""
# pylint: disable=broad-exception-caught,duplicate-code,line-too-long,too-many-locals

from typing import Any

from ...utils.logger import get_logger, log_method
from .database import DatabaseDuplicateMixin
from .exact import ExactDuplicateMixin
from .grouping import BillGroupingMixin
from .models import DeduplicationResult, DuplicateGroup
from .normalization import NormalizationMixin
from .platform_bank import PlatformBankDuplicateMixin
from .reconciliation import ReconciliationCandidateMixin
from .transfers import TransferPairingMixin


class SmartDeduplicationEngine(
    NormalizationMixin,
    PlatformBankDuplicateMixin,
    ReconciliationCandidateMixin,
    ExactDuplicateMixin,
    BillGroupingMixin,
    TransferPairingMixin,
    DatabaseDuplicateMixin,
):
    """智能去重引擎 v6.42.1

    实现多来源账单的智能去重，包含4种机制：
    1. 转账配对
    2. 支付平台/银行去重
    3. 类似账单去重
    4. 分账单去重
    """

    # 支付平台来源
    PLATFORM_SOURCES = {"wechat", "alipay"}
    # 银行来源
    BANK_SOURCES = {"icbc", "cmbc", "abc", "ccb"}

    # 支付平台优先级（数值越小优先级越高）
    SOURCE_PRIORITY = {
        "wechat": 1,
        "alipay": 2,
        "icbc": 10,
        "cmbc": 10,
        "abc": 10,
        "ccb": 10,
    }

    # 时间容差（秒）- 所有去重机制统一使用30秒
    TIME_TOLERANCE = 30

    # 金额容差（元）
    AMOUNT_TOLERANCE = 0.01

    # 相似度阈值
    SIMILARITY_THRESHOLD = 0.5

    # 异号平台/银行候选里出现这些关键词时，优先保留为真实转账配对
    TRANSFER_INTENT_KEYWORDS = {
        "转账",
        "转入",
        "转出",
        "提现",
        "充值",
        "还款",
        "划转",
        "内部转",
        "存入",
        "取出",
    }

    def __init__(self):
        """初始化去重引擎"""
        self.logger = get_logger("SmartDedup")
        # v6.57: 预编译日期格式列表
        self._date_formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%d",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d",
        ]
        self.logger.info("[智能去重引擎] v6.57 初始化完成 - pandas向量化优化")

    @log_method
    def process(self, bills: list[dict[str, Any]]) -> DeduplicationResult:
        """
        处理账单去重（不包含数据库对比）

        v6.42.1处理流程：
        1. 完全重复检测
        2. 支付平台/银行去重
        3. 转账配对（时间30秒+金额相反+来源不同）
        4. 类似账单去重
        5. 分账单去重

        Args:
            bills: 标准格式账单列表

        Returns:
            DeduplicationResult: 去重结果
        """
        original_count = len(bills)
        duplicate_groups: list[DuplicateGroup] = []
        split_groups: list[dict] = []
        transfer_pairs: list[tuple[dict, dict]] = []

        # 为每条账单生成唯一标识和初始化标记
        for bill in bills:
            bill["_dedup_id"] = self._generate_bill_hash(bill)
            bill["_removed"] = False
            bill["_merged_from"] = []

        # 执行5种去重机制
        exact_groups = self._find_exact_duplicates(bills)
        duplicate_groups.extend(exact_groups)

        platform_bank_groups = self._find_platform_bank_duplicates(bills)
        duplicate_groups.extend(platform_bank_groups)

        transfer_pairs = self._find_transfer_pairs(bills)

        similar_groups = self._find_similar_duplicates(bills)
        duplicate_groups.extend(similar_groups)

        split_groups = self._find_split_bills(bills)

        # 收集保留的账单
        kept_bills = [b for b in bills if not b.get("_removed", False)]

        # 清理临时字段
        for bill in kept_bills:
            bill.pop("_dedup_id", None)
            bill.pop("_removed", None)
            bill.pop("_merged_from", None)

        result = DeduplicationResult(
            original_count=original_count,
            kept_bills=kept_bills,
            removed_count=original_count - len(kept_bills),
            duplicate_groups=duplicate_groups,
            transfer_pairs=transfer_pairs,
            split_groups=split_groups,
        )

        # v6.62: 合并日志 - 一行汇总所有去重结果
        self.logger.info(
            "[去重] 原始=%d, 保留=%d | 完全=%d, 转账=%d, 平台银行=%d, 相似=%d, 分账=%d",
            original_count,
            len(kept_bills),
            len(exact_groups),
            len(transfer_pairs),
            len(platform_bank_groups),
            len(similar_groups),
            len(split_groups),
        )

        return result

    @log_method
    async def process_with_db(self, bills: list[dict[str, Any]], db, user_id: int = 1) -> DeduplicationResult:
        """
        处理账单去重（包含数据库对比）

        v6.42.1处理流程：
        1. 完全重复检测
        2. 支付平台/银行去重
        3. 转账配对（时间30秒+金额相反+来源不同）
        4. 类似账单去重
        5. 分账单去重
        6. 数据库重复检测

        Args:
            bills: 标准格式账单列表
            db: 数据库实例
            user_id: 用户ID

        Returns:
            DeduplicationResult: 去重结果
        """
        original_count = len(bills)
        duplicate_groups: list[DuplicateGroup] = []
        split_groups: list[dict] = []
        transfer_pairs: list[tuple[dict, dict]] = []

        # 为每条账单生成唯一标识和初始化标记
        for bill in bills:
            bill["_dedup_id"] = self._generate_bill_hash(bill)
            bill["_removed"] = False
            bill["_merged_from"] = []

        try:
            await self.find_import_reconciliation_candidates(bills, db, user_id=user_id)
        except Exception as exc:  # pragma: no cover - candidate persistence must not block import
            self.logger.warning("[导入后匹配] 持久化候选失败: %s", exc)

        # 执行6种去重机制
        exact_groups = self._find_exact_duplicates(bills)
        duplicate_groups.extend(exact_groups)

        platform_bank_groups = self._find_platform_bank_duplicates(bills)
        duplicate_groups.extend(platform_bank_groups)

        transfer_pairs = self._find_transfer_pairs(bills)

        similar_groups = self._find_similar_duplicates(bills)
        duplicate_groups.extend(similar_groups)

        split_groups = self._find_split_bills(bills)

        db_duplicate_groups = await self._find_database_duplicates(bills, db, user_id)
        duplicate_groups.extend(db_duplicate_groups)

        # v6.88: 跨批次转账配对（在数据库中查找金额相反的已有账单）
        cross_transfer_pairs = await self._find_cross_batch_transfer_pairs(bills, db, user_id)
        transfer_pairs.extend(cross_transfer_pairs)

        # 收集保留的账单
        kept_bills = [b for b in bills if not b.get("_removed", False)]

        # 清理临时字段
        for bill in kept_bills:
            bill.pop("_dedup_id", None)
            bill.pop("_removed", None)
            bill.pop("_merged_from", None)
            bill.pop("_duplicate_of_db_id", None)

        result = DeduplicationResult(
            original_count=original_count,
            kept_bills=kept_bills,
            removed_count=original_count - len(kept_bills),
            duplicate_groups=duplicate_groups,
            transfer_pairs=transfer_pairs,
            split_groups=split_groups,
        )

        # v6.62: 合并日志 - 一行汇总所有去重结果
        self.logger.info(
            "[去重+DB] 原始=%d, 保留=%d | 完全=%d, 转账=%d(+跨批%d), 平台银行=%d, 相似=%d, 分账=%d, DB重复=%d",
            original_count,
            len(kept_bills),
            len(exact_groups),
            len(transfer_pairs) - len(cross_transfer_pairs),
            len(cross_transfer_pairs),
            len(platform_bank_groups),
            len(similar_groups),
            len(split_groups),
            len(db_duplicate_groups),
        )

        return result
