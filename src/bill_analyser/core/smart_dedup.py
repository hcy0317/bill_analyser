"""智能去重引擎

v6.57 版本 (2025-12-01):
处理多来源账单的智能去重，包含以下5种机制：

1. **转账配对** (transfer):
   - 时间误差30秒以内
   - 金额绝对值相等，符号相反（一正一负，即一收一支）
   - 来自不同账户
   - 将两条账单配对为一笔转账交易
   - 金额为负的是转出账户(source_account)，为正的是转入账户(destination_account)

2. **支付平台/银行去重** (platform_bank):
   - 时间误差30秒以内
   - 金额绝对值相等；同号直接视为重复，异号需排除明确转账意图并具备文本重复证据
   - 一个来自支付平台(wechat/alipay)，一个来自银行(icbc/cmbc/abc/ccb)
   - 优先保留支付平台账单（信息更丰富）

3. **类似账单去重** (similar):
   - 时间误差30秒以内
   - 金额相等（绝对值相等且符号相同，都是支出或都是收入）
   - counterparty 或 payment_method 相似度≥50%（任一满足）
   - source_account_id 不同

4. **分账单去重** (split_merge):
   - 多个账单的时间误差30秒以内
   - source_account_id 不同且仅来自两个不同的来源
   - 来自其中一个来源的单个账单金额 = 来自另一个来源的多个账单金额之和
   - 金额方向相同（都是支出或都是收入）
   - 保留分账单（信息更详细），移除总账单

5. **同源关联账单去重** (v6.43新增):
   - 时间误差5秒以内（更严格）
   - 来源相同（source_account_id相同）
   - 金额完全相等，方向相同
   - 至少一条描述包含第三方支付关键词（支付宝、微信等）
   - 用于处理银行账单中同时出现原始流水和第三方支付代扣的情况

去重优先级：完全重复 → 支付平台/银行去重 → 转账配对 → 类似账单去重 → 分账单去重 → 同源关联去重 → 数据库重复

**v6.57关键变更**:
- 使用 pandas 向量化操作优化时间比对性能
- 批量解析日期时间，替代逐条 datetime.strptime
- O(n²) 双重循环优化为 pandas merge + 布尔索引
- 大幅减少 DEBUG 日志输出，只在匹配成功时记录
"""

import hashlib
from collections import defaultdict
from dataclasses import dataclass
from datetime import datetime, timedelta
from difflib import SequenceMatcher
from enum import Enum
from typing import Any

import numpy as np
import pandas as pd

from ..utils.logger import get_logger, log_method


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


class SmartDeduplicationEngine:
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

    # ==================== v6.57: Pandas 向量化辅助方法 ====================

    def _bills_to_dataframe(self, bills: list[dict[str, Any]]) -> pd.DataFrame:
        """将账单列表转换为 pandas DataFrame，批量解析日期

        参数：
            bills: 账单字典列表

        返回：
            pd.DataFrame: 包含解析后日期的 DataFrame
        """
        if not bills:
            return pd.DataFrame()

        # 构建基础数据
        data = {
            "_idx": list(range(len(bills))),
            "date_str": [b.get("date", "") for b in bills],
            "amount": [float(b.get("amount", 0)) for b in bills],
            "source_account_id": [str(b.get("source_account_id", "")) for b in bills],
            "counterparty": [str(b.get("counterparty", "")) for b in bills],
            "payment_method": [str(b.get("payment_method", "")) for b in bills],
            "description": [str(b.get("description", "")) for b in bills],
            "_removed": [b.get("_removed", False) for b in bills],
            "_parser_id": [b.get("_parser_id", "") or b.get("source", "") for b in bills],
            "_template_id": [b.get("_template_id") for b in bills],
        }
        df = pd.DataFrame(data)

        # v6.60: 添加来源标识字段，优先使用 _parser_id，否则回退到 source_account_id
        # 这确保转账配对能够正确识别不同来源的账单
        df["_source_identifier"] = df.apply(
            lambda row: row["_parser_id"] if row["_parser_id"] else row["source_account_id"], axis=1
        )

        # v6.57: 批量解析日期时间
        df["datetime"] = pd.to_datetime(df["date_str"], format="%Y-%m-%d %H:%M:%S", errors="coerce")

        # 对于解析失败的，尝试其他格式
        mask = df["datetime"].isna()
        if mask.any():
            for fmt in self._date_formats[1:]:
                if not mask.any():
                    break
                df.loc[mask, "datetime"] = pd.to_datetime(df.loc[mask, "date_str"], format=fmt, errors="coerce")
                mask = df["datetime"].isna()

        # 添加辅助列
        df["abs_amount"] = df["amount"].abs()
        df["is_positive"] = df["amount"] >= 0

        return df

    def _find_time_close_pairs_vectorized(
        self, df1: pd.DataFrame, df2: pd.DataFrame, tolerance_seconds: int = 30
    ) -> pd.DataFrame:
        """使用向量化操作查找时间接近的账单对

        参数：
            df1: 第一组账单 DataFrame
            df2: 第二组账单 DataFrame
            tolerance_seconds: 时间容差（秒）

        返回：
            pd.DataFrame: 匹配的账单对，包含 _idx_1 和 _idx_2 列
        """
        if df1.empty or df2.empty:
            return pd.DataFrame(columns=["_idx_1", "_idx_2"])

        # 过滤有效日期的账单
        df1_valid = df1[df1["datetime"].notna() & ~df1["_removed"]].copy()
        df2_valid = df2[df2["datetime"].notna() & ~df2["_removed"]].copy()

        if df1_valid.empty or df2_valid.empty:
            return pd.DataFrame(columns=["_idx_1", "_idx_2"])

        # 使用金额绝对值进行初步分组以减少比较次数
        # 按金额分桶（精确到0.01元）
        df1_valid["amount_bucket"] = (df1_valid["abs_amount"] * 100).round().astype(int)
        df2_valid["amount_bucket"] = (df2_valid["abs_amount"] * 100).round().astype(int)

        # 按日期分桶（精确到分钟）以减少比较范围
        df1_valid["date_bucket"] = df1_valid["datetime"].dt.floor("1min")
        df2_valid["date_bucket"] = df2_valid["datetime"].dt.floor("1min")

        # 在相同金额桶内进行合并
        merged = df1_valid.merge(df2_valid, on="amount_bucket", suffixes=("_1", "_2"), how="inner")

        if merged.empty:
            return pd.DataFrame(columns=["_idx_1", "_idx_2"])

        # v6.57: 向量化计算时间差
        time_diff = (merged["datetime_1"] - merged["datetime_2"]).abs()
        tolerance = pd.Timedelta(seconds=tolerance_seconds)

        # 筛选时间接近的配对
        close_pairs = merged[time_diff <= tolerance][["_idx_1", "_idx_2"]].copy()

        return close_pairs

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

    # ==================== 工具方法 ====================

    def _generate_bill_hash(self, bill: dict[str, Any]) -> str:
        """生成账单的唯一哈希值

        Args:
            bill: 账单数据

        Returns:
            str: 12位哈希值
        """
        key_fields = [
            str(bill.get("date", "")),
            str(bill.get("amount", 0)),
            str(bill.get("counterparty", "")),
            str(bill.get("description", "")),
        ]
        key_str = "|".join(key_fields)
        return hashlib.md5(key_str.encode("utf-8")).hexdigest()[:12]

    def _parse_datetime(self, date_str: str) -> datetime | None:
        """解析日期时间字符串

        Args:
            date_str: 日期字符串

        Returns:
            Optional[datetime]: 解析后的日期时间，失败返回None
        """
        formats = [
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%d %H:%M",
            "%Y-%m-%d",
            "%Y/%m/%d %H:%M:%S",
            "%Y/%m/%d",
        ]
        for fmt in formats:
            try:
                return datetime.strptime(str(date_str).strip(), fmt)
            except ValueError:
                continue
        return None

    def _time_close(self, dt1: datetime, dt2: datetime, log_always: bool = False) -> bool:
        """判断两个时间是否在30秒内

        v6.57优化: 默认不记录DEBUG日志，仅在匹配成功或 log_always=True 时记录

        Args:
            dt1: 时间1
            dt2: 时间2
            log_always: 是否总是记录日志（用于调试）

        Returns:
            bool: 时间差是否≤30秒
        """
        diff = abs((dt1 - dt2).total_seconds())
        is_close = diff <= self.TIME_TOLERANCE
        # v6.57: 只在匹配成功或明确要求时记录日志，减少大量无效日志
        if is_close or log_always:
            self.logger.debug(
                "[时间比对] %s vs %s, 差=%.1f秒, 结果=%s",
                dt1.strftime("%H:%M:%S"),
                dt2.strftime("%H:%M:%S"),
                diff,
                is_close,
            )
        return is_close

    def _amount_equal_same_direction(self, amt1: float, amt2: float, log_always: bool = False) -> bool:
        """判断两个金额是否相等且方向相同

        条件：绝对值相等（容差0.01）且符号相同（都是正数或都是负数）

        v6.57优化: 默认不记录DEBUG日志

        Args:
            amt1: 金额1
            amt2: 金额2
            log_always: 是否总是记录日志

        Returns:
            bool: 金额相等且方向相同
        """
        # 绝对值相等
        abs_equal = abs(abs(amt1) - abs(amt2)) <= self.AMOUNT_TOLERANCE
        # 符号相同（同为正或同为负）
        same_sign = (amt1 >= 0 and amt2 >= 0) or (amt1 < 0 and amt2 < 0)
        result = abs_equal and same_sign
        # v6.57: 只在匹配成功时记录日志
        if result or log_always:
            self.logger.debug(
                "[金额比对] %.2f vs %.2f, 绝对值相等=%s, 符号相同=%s, 结果=%s", amt1, amt2, abs_equal, same_sign, result
            )
        return result

    def _amount_opposite(self, amt1: float, amt2: float, log_always: bool = False) -> bool:
        """判断两个金额是否绝对值相等但方向相反

        用于转账配对：一个账户转出（负数），另一个账户转入（正数）

        条件：绝对值相等（容差0.01）且符号相反（一正一负）

        v6.57优化: 默认不记录DEBUG日志

        Args:
            amt1: 金额1
            amt2: 金额2
            log_always: 是否总是记录日志

        Returns:
            bool: 金额绝对值相等且方向相反
        """
        # 绝对值相等
        abs_equal = abs(abs(amt1) - abs(amt2)) <= self.AMOUNT_TOLERANCE
        # 符号相反（一正一负）- 使用乘积判断更简洁
        opposite_sign = amt1 * amt2 < 0
        result = abs_equal and opposite_sign
        # v6.57: 只在匹配成功时记录日志
        if result or log_always:
            self.logger.debug(
                "[金额相反比对] %.2f vs %.2f, 绝对值相等=%s, 符号相反=%s, 结果=%s",
                amt1,
                amt2,
                abs_equal,
                opposite_sign,
                result,
            )
        return result

    def _calculate_similarity(self, s1: str, s2: str) -> float:
        """计算两个字符串的相似度

        使用SequenceMatcher计算基于最长公共子序列的相似度。

        Args:
            s1: 字符串1
            s2: 字符串2

        Returns:
            float: 相似度 0.0-1.0
        """
        # 标准化：去除空白、转小写
        s1 = str(s1 or "").strip().lower()
        s2 = str(s2 or "").strip().lower()

        # 如果都为空，认为相似
        if not s1 and not s2:
            return 1.0
        # 如果只有一个为空，相似度为0
        if not s1 or not s2:
            return 0.0

        return SequenceMatcher(None, s1, s2).ratio()

    def _get_source_priority(self, source_id: str) -> int:
        """获取来源优先级

        Args:
            source_id: 来源ID

        Returns:
            int: 优先级数值（越小越优先）
        """
        return self.SOURCE_PRIORITY.get(source_id, 100)

    def _merge_bill_fields(self, primary_bill: dict[str, Any], secondary_bill: dict[str, Any]) -> dict[str, Any]:
        """合并两个账单的字段

        v6.47更新：
        - 优先保留primary_bill的时间、类型、金额
        - 合并两者不同的counterparty、payment_method、description
        - 使用"|"作为分隔符
        - 去除重复的片段

        Args:
            primary_bill: 主账单（优先保留）
            secondary_bill: 次账单

        Returns:
            Dict: 合并后的账单
        """
        merged = primary_bill.copy()
        merge_fields = ["counterparty", "payment_method", "description"]

        for field in merge_fields:
            merged[field] = self._merge_field_values(primary_bill.get(field, ""), secondary_bill.get(field, ""))

        # 记录合并来源
        if "_merged_from" not in merged:
            merged["_merged_from"] = []
        merged["_merged_from"].append(
            {
                "source": secondary_bill.get("source_account_id"),
                "date": secondary_bill.get("date"),
                "amount": secondary_bill.get("amount"),
                "_template_id": secondary_bill.get("_template_id"),
            }
        )

        # v6.47: 记录合并的模板ID
        if "_merged_template_ids" not in merged:
            merged["_merged_template_ids"] = []
        if secondary_bill.get("_template_id"):
            merged["_merged_template_ids"].append(secondary_bill.get("_template_id"))

        # v6.69: 使用更有意义的日志标识，避免显示空值
        primary_id = (
            primary_bill.get("_parser_id") or primary_bill.get("source_account_id") or primary_bill.get("date", "")[:10]
        )
        secondary_id = (
            secondary_bill.get("_parser_id")
            or secondary_bill.get("source_account_id")
            or secondary_bill.get("date", "")[:10]
        )
        self.logger.debug(
            "[字段合并] 主=%s (金额=%.2f), 次=%s (金额=%.2f)",
            primary_id,
            float(primary_bill.get("amount", 0)),
            secondary_id,
            float(secondary_bill.get("amount", 0)),
        )

        return merged

    def _merge_field_values(self, value1: Any, value2: Any) -> str:
        """合并两个字段值，使用"|"分隔，去除重复片段

        v6.47新增：
        1. 将两个值用"|"分隔
        2. 如果一个值包含另一个，只保留较长的
        3. 如果值相同，只保留一个
        4. 对合并后的内容按"|"拆分，去重

        Args:
            value1: 第一个值（优先）
            value2: 第二个值

        Returns:
            str: 合并后的字符串
        """
        val1 = str(value1 or "").strip()
        val2 = str(value2 or "").strip()

        # 空值处理
        if not val1 and not val2:
            return ""
        if not val1:
            return val2
        if not val2:
            return val1

        # 相同值
        if val1 == val2:
            return val1

        # 包含关系
        if val1 in val2:
            return val2
        if val2 in val1:
            return val1

        # 合并并去重
        # 将已有的和新的都按"|"拆分
        parts1 = [p.strip() for p in val1.split("|") if p.strip()]
        parts2 = [p.strip() for p in val2.split("|") if p.strip()]

        # 使用有序去重
        seen = set()
        unique_parts = []
        for part in parts1 + parts2:
            # 检查是否已有包含此部分的内容
            is_duplicate = False
            for existing in unique_parts:
                if part in existing or existing in part:
                    # 保留较长的
                    if len(part) > len(existing):
                        unique_parts[unique_parts.index(existing)] = part
                        seen.discard(existing)
                        seen.add(part)
                    is_duplicate = True
                    break
            if not is_duplicate and part not in seen:
                unique_parts.append(part)
                seen.add(part)

        return " | ".join(unique_parts)

    # ==================== 去重方法1：完全重复 ====================

    def _find_exact_duplicates(self, bills: list[dict[str, Any]]) -> list[DuplicateGroup]:
        """查找完全重复的账单

        基于账单哈希值识别完全相同的账单。

        Args:
            bills: 账单列表

        Returns:
            List[DuplicateGroup]: 重复组列表
        """
        groups = []
        hash_map: dict[str, list[dict[str, Any]]] = {}

        for bill in bills:
            if bill.get("_removed"):
                continue
            hash_val = bill["_dedup_id"]
            if hash_val not in hash_map:
                hash_map[hash_val] = []
            hash_map[hash_val].append(bill)

        for dup_bills in hash_map.values():
            if len(dup_bills) > 1:
                # 按来源优先级排序，保留优先级最高的
                dup_bills.sort(key=lambda b: self._get_source_priority(b.get("source_account_id", "")))
                keep_bill = dup_bills[0]
                remove_bills = dup_bills[1:]

                for bill in remove_bills:
                    bill["_removed"] = True

                # v6.69: 使用更有意义的日志标识
                keep_id = (
                    keep_bill.get("_parser_id") or keep_bill.get("source_account_id") or keep_bill.get("date", "")[:10]
                )
                groups.append(
                    DuplicateGroup(
                        type=DeduplicationType.EXACT,
                        bills=dup_bills,
                        keep_bill=keep_bill,
                        remove_bills=remove_bills,
                        reason=f"完全重复，保留 {keep_id} 来源",
                    )
                )

                self.logger.debug(
                    "[完全重复] 保留=%s (金额=%.2f), 移除%d条",
                    keep_id,
                    float(keep_bill.get("amount", 0)),
                    len(remove_bills),
                )

        return groups

    # ==================== 去重方法2：支付平台/银行去重 ====================

    def _get_source_type(self, bill: dict[str, Any]) -> str:
        """获取账单来源类型（解析器标识）

        v6.48修复: 优先使用 _parser_id 字段（解析器标识如 alipay, abc），
        而非 source_account_id（已匹配的账户ID如 1, 2, 3...）
        """
        # 优先使用 _parser_id（v6.48新增字段）
        parser_id = bill.get("_parser_id", "")
        if parser_id:
            return parser_id.lower()

        # 兼容旧字段名
        source = bill.get("source", "")
        if source:
            return source.lower()

        # 如果source_account_id是字符串形式的解析器标识，也支持
        source_id = str(bill.get("source_account_id", ""))
        if source_id.lower() in self.PLATFORM_SOURCES or source_id.lower() in self.BANK_SOURCES:
            return source_id.lower()

        return ""

    def _bill_text_for_intent(self, bill: dict[str, Any]) -> str:
        """Return searchable text for transfer-intent and duplicate-evidence checks."""
        return " ".join(
            str(bill.get(field, "") or "").strip()
            for field in [
                "counterparty",
                "payment_method",
                "description",
                "original_category",
                "main_category",
                "sub_category",
            ]
            if str(bill.get(field, "") or "").strip()
        )

    def _has_transfer_intent_keywords(self, bill: dict[str, Any]) -> bool:
        """Whether a bill explicitly looks like a transfer."""
        text_lower = self._bill_text_for_intent(bill).lower()
        return any(keyword.lower() in text_lower for keyword in self.TRANSFER_INTENT_KEYWORDS)

    def _has_platform_bank_duplicate_text_evidence(
        self,
        platform_bill: dict[str, Any],
        bank_bill: dict[str, Any],
    ) -> bool:
        """Check whether an opposite-sign platform/bank pair looks duplicated."""
        comparable_fields = [
            "counterparty",
            "description",
            "payment_method",
            "original_category",
        ]
        for field in comparable_fields:
            left = str(platform_bill.get(field, "") or "").strip()
            right = str(bank_bill.get(field, "") or "").strip()
            if not left or not right:
                continue
            if left in right or right in left:
                return True
            if self._calculate_similarity(left, right) >= self.SIMILARITY_THRESHOLD:
                return True

        platform_text = self._bill_text_for_intent(platform_bill)
        bank_text = self._bill_text_for_intent(bank_bill)
        return bool(
            platform_text
            and bank_text
            and self._calculate_similarity(platform_text, bank_text) >= 0.62
        )

    def _is_platform_bank_duplicate_candidate(
        self,
        platform_bill: dict[str, Any],
        bank_bill: dict[str, Any],
    ) -> bool:
        """Return True when a platform/bank pair should win before transfer pairing."""
        platform_amount = float(platform_bill.get("amount", 0) or 0)
        bank_amount = float(bank_bill.get("amount", 0) or 0)
        if (platform_amount >= 0) == (bank_amount >= 0):
            return True

        if self._has_transfer_intent_keywords(platform_bill) or self._has_transfer_intent_keywords(
            bank_bill
        ):
            return False

        return self._has_platform_bank_duplicate_text_evidence(platform_bill, bank_bill)

    def _find_platform_bank_duplicates(self, bills: list[dict[str, Any]]) -> list[DuplicateGroup]:
        """查找支付平台与银行的重复账单

        v6.57优化: 使用pandas向量化操作替代双重循环
        v6.42去重条件（全部满足）：
        1. 时间误差30秒以内
        2. 金额绝对值相等；同号直接通过，异号需排除转账意图且文本证据相似
        3. 一个来自支付平台(wechat/alipay)，一个来自银行(icbc/cmbc/abc/ccb)

        优先保留支付平台账单（信息更丰富）。

        Args:
            bills: 账单列表

        Returns:
            List[DuplicateGroup]: 重复组列表
        """
        groups = []

        # 分离平台账单和银行账单
        platform_bills = []
        bank_bills = []

        for i, b in enumerate(bills):
            if b.get("_removed"):
                continue
            source_type = self._get_source_type(b)
            b["_original_idx"] = i  # 保存原始索引
            if source_type in self.PLATFORM_SOURCES:
                platform_bills.append(b)
            elif source_type in self.BANK_SOURCES:
                bank_bills.append(b)

        self.logger.debug("[平台-银行去重] 平台账单=%d, 银行账单=%d", len(platform_bills), len(bank_bills))

        if not platform_bills or not bank_bills:
            return groups

        # v6.57: 使用pandas进行向量化匹配
        # 构建DataFrame
        df_platform = self._bills_to_dataframe(platform_bills)
        df_bank = self._bills_to_dataframe(bank_bills)

        if df_platform.empty or df_bank.empty:
            return groups

        # 筛选有效日期
        df_platform = df_platform[df_platform["datetime"].notna()].copy()
        df_bank = df_bank[df_bank["datetime"].notna()].copy()

        if df_platform.empty or df_bank.empty:
            return groups

        # v6.57: 使用向量化查找时间接近的配对
        time_close_pairs = self._find_time_close_pairs_vectorized(
            df_platform,
            df_bank,
            self.TIME_TOLERANCE,
        )

        if time_close_pairs.empty:
            self.logger.debug("[平台-银行去重] 无时间接近的配对")
            return groups

        # 合并原始数据以进行金额比较
        time_close_pairs = time_close_pairs.merge(
            df_platform[["_idx", "amount", "is_positive"]].rename(
                columns={"_idx": "_idx_1", "amount": "amt_p", "is_positive": "pos_p"}
            ),
            on="_idx_1",
        ).merge(
            df_bank[["_idx", "amount", "is_positive"]].rename(
                columns={"_idx": "_idx_2", "amount": "amt_b", "is_positive": "pos_b"}
            ),
            on="_idx_2",
        )

        # v6.57: 向量化金额比较
        # 条件2：金额绝对值相等；符号方向由逐条候选 guard 决定
        time_close_pairs["abs_amt_p"] = time_close_pairs["amt_p"].abs()
        time_close_pairs["abs_amt_b"] = time_close_pairs["amt_b"].abs()
        time_close_pairs["amount_match"] = (
            np.abs(time_close_pairs["abs_amt_p"] - time_close_pairs["abs_amt_b"])
            <= self.AMOUNT_TOLERANCE
        )

        matched_pairs = time_close_pairs[time_close_pairs["amount_match"]]

        if matched_pairs.empty:
            self.logger.debug("[平台-银行去重] 无金额匹配的配对")
            return groups

        self.logger.debug("[平台-银行去重] 候选匹配数: %d", len(matched_pairs))

        # 处理匹配结果
        matched_bank_indices: set[int] = set()

        for _, row in matched_pairs.iterrows():
            p_idx = int(row["_idx_1"])
            b_idx = int(row["_idx_2"])

            if b_idx in matched_bank_indices:
                continue

            p_bill = platform_bills[p_idx]
            b_bill = bank_bills[b_idx]

            if p_bill.get("_removed") or b_bill.get("_removed"):
                continue

            if not self._is_platform_bank_duplicate_candidate(p_bill, b_bill):
                continue

            matched_bank_indices.add(b_idx)
            b_bill["_removed"] = True

            # 合并字段
            merged = self._merge_bill_fields(p_bill, b_bill)
            for key, value in merged.items():
                if not key.startswith("_"):
                    p_bill[key] = value

            # v6.48修复: 设置去重类型和合并的模板ID列表
            p_bill["_dedup_type"] = "platform_bank"
            merged_ids = []
            if b_bill.get("_template_id"):
                merged_ids.append(b_bill.get("_template_id"))
            if p_bill.get("_merged_template_ids"):
                merged_ids.extend(list(p_bill.get("_merged_template_ids", [])))
            if b_bill.get("_merged_template_ids"):
                merged_ids.extend(list(b_bill.get("_merged_template_ids", [])))
            if merged_ids:
                p_bill["_merged_template_ids"] = merged_ids

            p_amt = float(p_bill.get("amount", 0))
            groups.append(
                DuplicateGroup(
                    type=DeduplicationType.PLATFORM_BANK,
                    bills=[p_bill, b_bill],
                    keep_bill=p_bill,
                    remove_bills=[b_bill],
                    reason=(
                        f"支付平台({self._get_source_type(p_bill)})与"
                        f"银行({self._get_source_type(b_bill)})重复，"
                        f"金额={p_amt:.2f}，保留平台账单"
                    ),
                )
            )

            self.logger.debug(
                "[平台-银行去重] 匹配: %s ↔ %s, 金额=%.2f",
                self._get_source_type(p_bill),
                self._get_source_type(b_bill),
                p_amt,
            )

        return groups

    # ==================== 去重方法3：类似账单去重 ====================

    def _find_similar_duplicates(self, bills: list[dict[str, Any]]) -> list[DuplicateGroup]:
        """基于相似度查找重复账单

        v6.72优化: 使用金额+时间分桶替代全量笛卡尔积，内存占用从O(n²)降到O(n)
        v6.57优化: 使用pandas进行初步筛选，再对候选配对进行相似度计算
        v6.42去重条件（全部满足）：
        1. 时间误差30秒以内
        2. 金额相等（绝对值相等且符号相同）
        3. counterparty 或 payment_method 相似度≥50%（任一满足）
        4. source_account_id 不同

        Args:
            bills: 账单列表

        Returns:
            List[DuplicateGroup]: 重复组列表
        """
        groups: list[DuplicateGroup] = []

        # 只处理未被移除的账单
        active_bills = [b for b in bills if not b.get("_removed")]
        for i, b in enumerate(active_bills):
            b["_temp_idx"] = i

        n = len(active_bills)
        self.logger.debug("[类似账单去重] 活跃账单数: %d", n)

        if n < 2:
            return groups

        # v6.72: 使用金额+时间分桶策略替代全量笛卡尔积
        # 分桶策略：按 (金额桶, 方向, 时间桶) 分组，只在同组内比较
        # 这将复杂度从 O(n²) 降低到 O(n * k)，其中 k 是桶内平均元素数
        buckets: dict[tuple, list[int]] = defaultdict(list)

        # 时间桶大小：30秒（与TIME_TOLERANCE一致），相邻桶需要交叉检查
        TIME_BUCKET_SECONDS = self.TIME_TOLERANCE

        for i, bill in enumerate(active_bills):
            dt = self._parse_datetime(bill.get("date", ""))
            if not dt:
                continue

            amt = float(bill.get("amount", 0))
            abs_amt = abs(amt)
            is_positive = amt >= 0

            # 金额桶：精确到分（0.01）
            amt_bucket = round(abs_amt, 2)

            # 时间桶：按30秒分桶
            timestamp = int(dt.timestamp())
            time_bucket = timestamp // TIME_BUCKET_SECONDS

            # 添加到当前桶
            key = (amt_bucket, is_positive, time_bucket)
            buckets[key].append(i)

            # 同时添加到相邻时间桶（处理边界情况）
            key_prev = (amt_bucket, is_positive, time_bucket - 1)
            buckets[key_prev].append(i)

        # 收集候选配对（只比较同桶内的账单）
        candidate_pairs: list[tuple[int, int]] = []
        seen_pairs: set[tuple[int, int]] = set()

        for bucket_indices in buckets.values():
            if len(bucket_indices) < 2:
                continue

            # 桶内两两配对
            for i, idx1 in enumerate(bucket_indices):
                for idx2 in bucket_indices[i + 1 :]:
                    # 确保 idx1 < idx2 避免重复
                    pair = (min(idx1, idx2), max(idx1, idx2))
                    if pair in seen_pairs:
                        continue
                    seen_pairs.add(pair)

                    bill1 = active_bills[pair[0]]
                    bill2 = active_bills[pair[1]]

                    # 条件4: 来源不同
                    source1 = self._get_source_type(bill1) or str(bill1.get("source_account_id", ""))
                    source2 = self._get_source_type(bill2) or str(bill2.get("source_account_id", ""))
                    if source1 == source2 or not source1 or not source2:
                        continue

                    # 条件1: 时间30秒内（精确验证）
                    dt1 = self._parse_datetime(bill1.get("date", ""))
                    dt2 = self._parse_datetime(bill2.get("date", ""))
                    if not dt1 or not dt2:
                        continue
                    if abs((dt1 - dt2).total_seconds()) > self.TIME_TOLERANCE:
                        continue

                    # 条件2: 金额绝对值相等且方向相同（精确验证）
                    amt1 = float(bill1.get("amount", 0))
                    amt2 = float(bill2.get("amount", 0))
                    if abs(abs(amt1) - abs(amt2)) > self.AMOUNT_TOLERANCE:
                        continue
                    if (amt1 >= 0) != (amt2 >= 0):
                        continue

                    candidate_pairs.append(pair)

        self.logger.debug("[类似账单去重] 时间+金额候选数: %d (分桶数=%d)", len(candidate_pairs), len(buckets))

        # 条件3: 相似度计算（需要逐对计算）
        matched: set[int] = set()

        for idx1, idx2 in candidate_pairs:
            if idx1 in matched or idx2 in matched:
                continue

            bill1 = active_bills[idx1]
            bill2 = active_bills[idx2]

            if bill1.get("_removed") or bill2.get("_removed"):
                continue

            # 计算相似度
            cp1 = str(bill1.get("counterparty", ""))
            pm1 = str(bill1.get("payment_method", ""))
            cp2 = str(bill2.get("counterparty", ""))
            pm2 = str(bill2.get("payment_method", ""))

            cp_similarity = self._calculate_similarity(cp1, cp2)
            pm_similarity = self._calculate_similarity(pm1, pm2)

            if cp_similarity < self.SIMILARITY_THRESHOLD and pm_similarity < self.SIMILARITY_THRESHOLD:
                continue

            # 找到重复！
            matched.add(idx1)
            matched.add(idx2)

            source1 = str(bill1.get("source_account_id", ""))
            source2 = str(bill2.get("source_account_id", ""))

            # 确定主账单和次账单（按优先级）
            if self._get_source_priority(source1) <= self._get_source_priority(source2):
                primary_bill, secondary_bill = bill1, bill2
            else:
                primary_bill, secondary_bill = bill2, bill1

            # 合并字段
            merged = self._merge_bill_fields(primary_bill, secondary_bill)
            for key, value in merged.items():
                if not key.startswith("_"):
                    primary_bill[key] = value

            secondary_bill["_removed"] = True

            # v6.48修复: 设置去重类型和合并的模板ID列表
            primary_bill["_dedup_type"] = "similar"
            merged_ids = []
            if secondary_bill.get("_template_id"):
                merged_ids.append(secondary_bill.get("_template_id"))
            if primary_bill.get("_merged_template_ids"):
                merged_ids.extend(list(primary_bill.get("_merged_template_ids", [])))
            if secondary_bill.get("_merged_template_ids"):
                merged_ids.extend(list(secondary_bill.get("_merged_template_ids", [])))
            if merged_ids:
                primary_bill["_merged_template_ids"] = merged_ids

            similarity_used = max(cp_similarity, pm_similarity)
            similarity_type = "counterparty" if cp_similarity >= pm_similarity else "payment_method"

            groups.append(
                DuplicateGroup(
                    type=DeduplicationType.SIMILAR,
                    bills=[primary_bill, secondary_bill],
                    keep_bill=primary_bill,
                    remove_bills=[secondary_bill],
                    reason=(
                        f"类似账单去重: {similarity_type}相似度={similarity_used:.0%}, "
                        f"保留{primary_bill.get('source_account_id')}账单"
                    ),
                )
            )

            amt1 = float(bill1.get("amount", 0))
            self.logger.debug(
                "[类似账单去重] 匹配: %s ↔ %s, 金额=%.2f, %s相似度=%.0f%%",
                source1,
                source2,
                amt1,
                similarity_type,
                similarity_used * 100,
            )

        # 清理临时索引
        for b in active_bills:
            if "_temp_idx" in b:
                del b["_temp_idx"]

        return groups

    # ==================== 去重方法4：分账单去重 ====================

    def _find_split_bills(self, bills: list[dict[str, Any]]) -> list[dict]:
        """识别分账单

        v6.42去重条件：
        1. 多个账单的时间误差30秒以内
        2. source_account_id 不同且仅来自两个不同的来源
        3. 来自一个来源的单个账单金额 = 来自另一个来源的多个账单金额之和
        4. 金额方向相同（都是支出或都是收入）

        处理方式：保留分账单（信息更详细），移除总账单

        Args:
            bills: 账单列表

        Returns:
            List[Dict]: 分账单组列表
        """
        groups = []
        matched: set[int] = set()

        # 只处理未被移除的账单
        active_bills = [
            (i, b) for i, b in enumerate(bills) if not b.get("_removed") and abs(float(b.get("amount", 0))) > 0
        ]

        # 按时间排序
        active_bills.sort(key=lambda x: x[1].get("date", ""))

        n = len(active_bills)
        self.logger.debug("[分账单去重] 活跃账单数: %d", n)

        for i, (idx1, bill1) in enumerate(active_bills):
            if idx1 in matched:
                continue

            dt1 = self._parse_datetime(bill1.get("date", ""))
            amt1 = float(bill1.get("amount", 0))
            source1 = str(bill1.get("source_account_id", ""))

            if not dt1 or abs(amt1) < 10:  # 忽略小额账单
                continue

            # 收集时间接近的账单
            candidate_bills: list[tuple] = []
            for j in range(i + 1, min(i + 30, n)):  # 最多检查后续30条
                idx2, bill2 = active_bills[j]

                if idx2 in matched:
                    continue

                dt2 = self._parse_datetime(bill2.get("date", ""))
                amt2 = float(bill2.get("amount", 0))
                source2 = str(bill2.get("source_account_id", ""))

                if not dt2:
                    continue

                # 时间差太大则停止
                if abs((dt2 - dt1).total_seconds()) > self.TIME_TOLERANCE:
                    break

                # 来源不同
                if source1 == source2:
                    continue

                # 方向相同（符号相同）
                if amt1 * amt2 <= 0:
                    continue

                candidate_bills.append((idx2, bill2, amt2, source2))

            if len(candidate_bills) < 2:
                continue

            # 检查是否只来自两个不同来源
            sources_in_candidates = set(c[3] for c in candidate_bills)
            if len(sources_in_candidates) > 1:
                # 候选账单来自多个来源，不符合"仅两个来源"条件
                continue

            # 现在 source1 和 candidates 的来源构成两个不同来源
            candidate_source = list(sources_in_candidates)[0]

            # 检查候选账单之和是否等于当前账单
            total_candidates = sum(c[2] for c in candidate_bills)
            if self._amount_equal_same_direction(total_candidates, amt1):
                # 找到分账单组
                matched.add(idx1)
                for idx2, _, _, _ in candidate_bills:
                    matched.add(idx2)

                # 移除总账单，保留分账单
                bill1["_removed"] = True

                # v6.48修复: 为每个保留的分账单设置去重类型
                split_bill_list = [c[1] for c in candidate_bills]
                total_bill_template_id = bill1.get("_template_id")
                for split_bill in split_bill_list:
                    split_bill["_dedup_type"] = "split"
                    if total_bill_template_id:
                        merged_ids = split_bill.get("_merged_template_ids", [])
                        if not merged_ids:
                            merged_ids = []
                        merged_ids.append(total_bill_template_id)
                        split_bill["_merged_template_ids"] = merged_ids

                groups.append(
                    {
                        "total_bill": bill1,
                        "split_bills": split_bill_list,
                        "total_amount": amt1,
                        "source_total": source1,
                        "source_splits": candidate_source,
                        "reason": (
                            f"总账单({source1}, {amt1:.2f})拆分为{len(candidate_bills)}笔分账单({candidate_source})"
                        ),
                    }
                )

                self.logger.debug(
                    "[分账单去重] 总额=%.2f (%s) -> %d笔分账单 (%s)",
                    amt1,
                    source1,
                    len(candidate_bills),
                    candidate_source,
                )

        return groups

    # ==================== 转账配对方法 ====================

    def _find_transfer_pairs(self, bills: list[dict[str, Any]]) -> list[tuple[dict, dict]]:
        """识别账户间转账

        v6.57优化: 使用pandas进行初步筛选，减少双重循环开销

        转账配对条件：
        - 时间接近（30秒内）
        - 金额绝对值相等，符号相反（一正一负）
        - 来自不同账户

        处理结果：
        - 金额为负的是转出账户(source_account)
        - 金额为正的是转入账户(destination_account)
        - 两条账单都设置type为'转账'
        - 转出账单的destination_account_id设为转入账户
        - 转入账单的source_account_id设为转出账户

        Args:
            bills: 账单列表

        Returns:
            List[Tuple[Dict, Dict]]: 转账配对列表，每对为(转出账单, 转入账单)
        """
        pairs: list[tuple[dict, dict]] = []

        # 过滤活跃账单（未被移除且金额不为0）
        active_bills = [b for b in bills if not b.get("_removed") and abs(float(b.get("amount", 0))) > 0.001]
        # 添加临时索引
        for i, b in enumerate(active_bills):
            b["_temp_idx"] = i

        n = len(active_bills)
        self.logger.debug("[转账配对] 活跃账单数: %d", n)

        if n < 2:
            return pairs

        # v6.57: 使用pandas构建DataFrame
        df = self._bills_to_dataframe(active_bills)
        df = df[df["datetime"].notna()].copy()

        if len(df) < 2:
            return pairs

        # 分离正负金额账单（转账需要一正一负）
        df_positive = df[df["amount"] >= 0].copy()
        df_negative = df[df["amount"] < 0].copy()

        if df_positive.empty or df_negative.empty:
            self.logger.debug("[转账配对] 无正/负金额配对候选")
            return pairs

        # v6.57: 使用向量化查找时间接近的配对
        time_close_pairs = self._find_time_close_pairs_vectorized(df_positive, df_negative, self.TIME_TOLERANCE)

        if time_close_pairs.empty:
            self.logger.debug("[转账配对] 无时间接近的配对")
            return pairs

        # 合并原始数据
        # v6.60: 使用 _source_identifier 判断来源（优先使用 _parser_id，否则用 source_account_id）
        # 这确保无论是阶段2导入场景（有 _parser_id）还是测试场景（只有 source_account_id）
        # 都能正确识别不同来源的账单
        time_close_pairs = time_close_pairs.merge(
            df_positive[["_idx", "amount", "_source_identifier"]].rename(
                columns={"_idx": "_idx_1", "amount": "amt_pos", "_source_identifier": "src_pos"}
            ),
            on="_idx_1",
        ).merge(
            df_negative[["_idx", "amount", "_source_identifier"]].rename(
                columns={"_idx": "_idx_2", "amount": "amt_neg", "_source_identifier": "src_neg"}
            ),
            on="_idx_2",
        )

        if time_close_pairs.empty:
            return pairs

        # 条件2: 来源不同（不同银行/平台的账单）
        # v6.60: 当两个账单的来源标识都为空时，不认为它们来源不同
        # 只有当 src_pos != src_neg 且两者都非空时才配对
        time_close_pairs = time_close_pairs[
            (time_close_pairs["src_pos"] != time_close_pairs["src_neg"])
            & (time_close_pairs["src_pos"] != "")
            & (time_close_pairs["src_neg"] != "")
        ]

        if time_close_pairs.empty:
            return pairs

        # 条件3: 金额绝对值相等
        time_close_pairs["abs_amt_pos"] = time_close_pairs["amt_pos"].abs()
        time_close_pairs["abs_amt_neg"] = time_close_pairs["amt_neg"].abs()
        time_close_pairs["amount_match"] = (
            time_close_pairs["abs_amt_pos"] - time_close_pairs["abs_amt_neg"]
        ).abs() <= self.AMOUNT_TOLERANCE
        matched_pairs = time_close_pairs[time_close_pairs["amount_match"]]

        if matched_pairs.empty:
            return pairs

        self.logger.debug("[转账配对] 候选匹配数: %d", len(matched_pairs))

        # 处理匹配结果
        matched_indices: set[int] = set()

        for _, row in matched_pairs.iterrows():
            pos_idx = int(row["_idx_1"])
            neg_idx = int(row["_idx_2"])

            if pos_idx in matched_indices or neg_idx in matched_indices:
                continue

            incoming_bill = active_bills[pos_idx]  # 正金额 = 转入
            outgoing_bill = active_bills[neg_idx]  # 负金额 = 转出

            if incoming_bill.get("_removed") or outgoing_bill.get("_removed"):
                continue

            matched_indices.add(pos_idx)
            matched_indices.add(neg_idx)

            # v6.62: 转账配对只保留一条记录（转出账单）
            # 规格：
            # - preview_source_account_id = 负金额账单的 parser_account_id（账户匹配阶段设置）
            # - preview_destination_account_id = 正金额账单的 parser_account_id（账户匹配阶段设置）
            # 转入账单（正金额）标记为已移除，不写入预览表

            # 获取模板ID用于合并
            outgoing_template_id = outgoing_bill.get("_template_id")
            incoming_template_id = incoming_bill.get("_template_id")

            # 设置转出账单为转账类型
            outgoing_bill["type"] = "转账"
            outgoing_bill["_dedup_type"] = "transfer"

            # 合并模板ID：转出账单包含转入账单的模板ID
            outgoing_merged = outgoing_bill.get("_merged_template_ids", [])
            if incoming_template_id:
                outgoing_merged.append(incoming_template_id)
            outgoing_bill["_merged_template_ids"] = outgoing_merged

            # v6.62: 记录转入账单的解析器信息，用于账户匹配阶段设置目标账户
            # 因为账户匹配是在去重之后执行，此时 source_account_id 可能为空
            # 所以记录 parser_id 和 payment_method，让账户匹配阶段能够找到正确的目标账户
            outgoing_bill["_destination_parser_id"] = incoming_bill.get("_parser_id", "")
            outgoing_bill["_destination_payment_method"] = incoming_bill.get("payment_method", "")
            outgoing_bill["_destination_counterparty"] = incoming_bill.get("counterparty", "")

            # v6.62: 合并描述信息
            outgoing_desc = outgoing_bill.get("description", "") or ""
            incoming_desc = incoming_bill.get("description", "") or ""
            if incoming_desc and incoming_desc not in outgoing_desc:
                merged_desc = f"{outgoing_desc} | {incoming_desc}".strip(" |")
                outgoing_bill["description"] = merged_desc

            # v6.62: 转入账单也设置类型为转账（测试需要验证 transfer_pairs 中两个账单的类型）
            incoming_bill["type"] = "转账"

            # v6.62: 将转入账单标记为已移除（避免重复写入预览表）
            incoming_bill["_removed"] = True
            incoming_bill["_merged_into"] = outgoing_template_id

            amt = abs(float(outgoing_bill.get("amount", 0)))
            # v6.62: 简化日志输出
            self.logger.debug(
                "[转账配对] 金额=%.2f, 转出=%s -> 转入=%s",
                amt,
                outgoing_bill.get("_parser_id", "") or outgoing_bill.get("source_account_id", ""),
                incoming_bill.get("_parser_id", "") or incoming_bill.get("source_account_id", ""),
            )

            pairs.append((outgoing_bill, incoming_bill))

        # 清理临时索引
        for b in active_bills:
            if "_temp_idx" in b:
                del b["_temp_idx"]

        return pairs

    # ==================== v6.88: 跨批次转账配对 ====================

    @log_method
    async def _find_cross_batch_transfer_pairs(
        self, bills: list[dict[str, Any]], db, user_id: int = 1, time_tolerance_seconds: int = 300
    ) -> list[tuple[dict, dict]]:
        """检测新导入账单与数据库已有账单之间的转账关系

        当前批次有一笔-100元(支出)的账单，数据库中已有一笔+100元(收入)
        的账单且时间接近来源不同，则识别为跨批次转账配对。

        配对成功后会更新数据库中已有账单的type为'转账'并设置目标/来源账户。

        Args:
            bills: 待导入的账单列表
            db: 数据库实例
            user_id: 用户ID
            time_tolerance_seconds: 时间容差（秒），默认5分钟

        Returns:
            List[Tuple[Dict, Dict]]: 转账对列表 [(新账单, 已有账单), ...]
        """
        pairs: list[tuple[dict, dict]] = []

        active_bills = [b for b in bills if not b.get("_removed", False)]
        if not active_bills:
            return pairs

        # 获取日期范围
        dates = []
        for bill in active_bills:
            dt = self._parse_datetime(bill.get("date", ""))
            if dt:
                dates.append(dt)
        if not dates:
            return pairs

        min_date = min(dates)
        max_date = max(dates)
        start_date = (min_date - timedelta(days=1)).strftime("%Y-%m-%d")
        end_date = (max_date + timedelta(days=1)).strftime("%Y-%m-%d")

        try:
            existing_bills = await db.get_bills_by_date_range(start_date, end_date, user_id=user_id)
        except Exception as e:
            self.logger.error("[跨批次转账] 查询数据库失败: %s", e)
            return pairs

        if not existing_bills:
            return pairs

        # 构建已有账单按 (日期, 金额绝对值) 索引
        existing_index: dict[str, list[dict]] = {}
        for eb in existing_bills:
            abs_amt = abs(float(eb.get("amount", 0)))
            key = f"{eb.get('date', '')[:10]}_{abs_amt:.2f}"
            if key not in existing_index:
                existing_index[key] = []
            existing_index[key].append(eb)

        matched_db_ids = set()

        for bill in active_bills:
            # 跳过已经被标记为转账的账单
            if bill.get("_dedup_type") == "transfer":
                continue

            bill_amt = float(bill.get("amount", 0))
            bill_abs_amt = abs(bill_amt)
            bill_dt = self._parse_datetime(bill.get("date", ""))
            if not bill_dt:
                continue

            bill_source = self._get_source_type(bill)
            key = f"{bill.get('date', '')[:10]}_{bill_abs_amt:.2f}"

            candidates = existing_index.get(key, [])
            for eb in candidates:
                if eb.get("id") in matched_db_ids:
                    continue

                eb_amt = float(eb.get("amount", 0))

                # 金额必须相反（一正一负）
                if not self._amount_opposite(bill_amt, eb_amt):
                    continue

                eb_dt = self._parse_datetime(eb.get("date", ""))
                if not eb_dt:
                    continue

                # 时间必须接近
                if abs((bill_dt - eb_dt).total_seconds()) > time_tolerance_seconds:
                    continue

                eb_source = str(eb.get("source_account_id", "")).lower()
                # 来源必须不同
                if bill_source and eb_source and bill_source == eb_source:
                    continue

                # 配对成功！
                matched_db_ids.add(eb.get("id"))

                # 确定转出/转入方
                if bill_amt < 0:
                    outgoing, incoming_db = bill, eb
                else:
                    outgoing, incoming_db = eb, bill
                    # 新账单是正金额 → 把新账单当做收入方
                    outgoing, incoming_db = eb, bill

                # 更新新账单的转账标记
                bill["type"] = "转账"
                bill["_dedup_type"] = "transfer_cross_batch"
                bill["_cross_batch_db_id"] = eb.get("id")

                # 更新数据库中已有账单（异步更新其type）
                try:
                    await db.update_bill(eb.get("id"), {"type": "转账"}, user_id=user_id)
                except Exception as update_err:
                    self.logger.warning("[跨批次转账] 更新已有账单 ID=%s 失败: %s", eb.get("id"), update_err)

                self.logger.debug(
                    "[跨批次转账] 金额=%.2f, 新账单(%s) ↔ 已有ID=%s", bill_abs_amt, bill_source, eb.get("id")
                )
                pairs.append((bill, eb))
                break  # 每条新账单最多配对一条已有账单

        if pairs:
            self.logger.info("[跨批次转账] 共发现 %d 对跨批次转账", len(pairs))

        return pairs

    # ==================== 去重方法5：数据库重复检测 ====================

    @log_method
    async def _find_database_duplicates(
        self, bills: list[dict[str, Any]], db, user_id: int = 1, time_tolerance_seconds: int = 300
    ) -> list[DuplicateGroup]:
        """与数据库已有账单对比，查找重复

        Args:
            bills: 待导入的账单列表
            db: 数据库实例
            user_id: 用户ID
            time_tolerance_seconds: 时间容差（秒），默认5分钟

        Returns:
            List[DuplicateGroup]: 重复组列表
        """
        groups: list[DuplicateGroup] = []

        if not bills:
            return groups

        # 获取待导入账单的日期范围
        dates = []
        for bill in bills:
            dt = self._parse_datetime(bill.get("date", ""))
            if dt:
                dates.append(dt)

        if not dates:
            return groups

        min_date = min(dates)
        max_date = max(dates)

        # 扩展日期范围（前后各1天）
        start_date = (min_date - timedelta(days=1)).strftime("%Y-%m-%d")
        end_date = (max_date + timedelta(days=1)).strftime("%Y-%m-%d")

        # 从数据库获取范围内的账单
        try:
            existing_bills = await db.get_bills_by_date_range(start_date, end_date, user_id=user_id)
        except Exception as e:
            self.logger.error("[数据库去重] 查询失败: %s", e)
            return groups

        if not existing_bills:
            return groups

        self.logger.debug("[数据库去重] 日期范围 %s~%s, 已有账单 %d 条", start_date, end_date, len(existing_bills))

        # 构建已有账单的索引（按日期+金额绝对值分组）
        # v6.46: 使用金额绝对值，以便匹配可能符号不同的账单（如平台-银行重复）
        existing_index: dict[str, list[dict]] = {}
        for eb in existing_bills:
            abs_amt = abs(float(eb.get("amount", 0)))
            key = f"{eb.get('date', '')[:10]}_{abs_amt:.2f}"
            if key not in existing_index:
                existing_index[key] = []
            existing_index[key].append(eb)

        # 检测重复
        for bill in bills:
            if bill.get("_removed"):
                continue

            bill_date = bill.get("date", "")
            bill_amt = float(bill.get("amount", 0))
            bill_abs_amt = abs(bill_amt)
            # v6.46: 使用金额绝对值查找候选，以便匹配可能符号不同的账单
            key = f"{bill_date[:10]}_{bill_abs_amt:.2f}"

            candidates = existing_index.get(key, [])
            if not candidates:
                continue

            bill_dt = self._parse_datetime(bill_date)
            if not bill_dt:
                continue

            bill_desc = str(bill.get("description", "")).lower()
            bill_counterparty = str(bill.get("counterparty", "")).lower()
            bill_source = str(bill.get("source_account_id", "")).lower()
            bill_is_platform = bill_source in self.PLATFORM_SOURCES

            for eb in candidates:
                eb_dt = self._parse_datetime(eb.get("date", ""))
                if not eb_dt:
                    continue

                # 时间是否接近
                if abs((bill_dt - eb_dt).total_seconds()) > time_tolerance_seconds:
                    continue

                eb_amt = float(eb.get("amount", 0))
                eb_source = str(eb.get("source_account_id", "")).lower()
                eb_is_platform = eb_source in self.PLATFORM_SOURCES

                # v6.46: 判断是否为平台-银行对
                is_platform_bank_pair = bill_is_platform != eb_is_platform

                # 金额是否匹配
                # 对于平台-银行对，允许金额绝对值相等即可（可能符号不同）
                # 对于其他情况，要求金额相等且方向相同
                if is_platform_bank_pair:
                    # 平台-银行对：只要金额绝对值相等就行
                    amount_match = abs(bill_abs_amt - abs(eb_amt)) <= self.AMOUNT_TOLERANCE
                else:
                    # 非平台-银行对：要求金额相等且方向相同
                    amount_match = self._amount_equal_same_direction(bill_amt, eb_amt)

                if not amount_match:
                    continue

                # 描述或交易对手是否相似
                eb_desc = str(eb.get("description", "")).lower()
                eb_counterparty = str(eb.get("counterparty", "")).lower()

                # v6.46: 使用相似度判断（更宽松，能匹配部分相似的counterparty）
                desc_similar = (
                    bill_desc
                    and eb_desc
                    and (
                        bill_desc in eb_desc
                        or eb_desc in bill_desc
                        or self._calculate_similarity(bill_desc, eb_desc) >= 0.5
                    )
                )
                counterparty_similar = (
                    bill_counterparty
                    and eb_counterparty
                    and (
                        bill_counterparty in eb_counterparty
                        or eb_counterparty in bill_counterparty
                        or self._calculate_similarity(bill_counterparty, eb_counterparty) >= 0.5
                    )
                )

                # 对于平台-银行对，判断条件更宽松
                if is_platform_bank_pair:
                    # 平台-银行对：counterparty有一定相似度即可
                    match_condition = counterparty_similar or desc_similar
                else:
                    # 其他情况：保持原来的逻辑
                    match_condition = desc_similar or counterparty_similar or (not bill_desc and not eb_desc)

                if match_condition:
                    # 找到重复
                    bill["_removed"] = True
                    bill["_duplicate_of_db_id"] = eb.get("id")

                    reason_type = "平台-银行跨文件重复" if is_platform_bank_pair else "与数据库已有账单重复"
                    groups.append(
                        DuplicateGroup(
                            type=DeduplicationType.DATABASE_DUPLICATE,
                            bills=[bill, eb],
                            keep_bill=eb,
                            remove_bills=[bill],
                            reason=(
                                f"{reason_type} (ID={eb.get('id')}, 日期={eb.get('date')}, 金额={eb.get('amount')})"
                            ),
                        )
                    )

                    # v6.62: 改为 DEBUG 级别，减少日志冗余
                    self.logger.debug(
                        "[数据库重复] %s: 新账单 %s/%.2f (%s) 与已有账单 ID=%s (%s)",
                        reason_type,
                        bill_date,
                        bill_amt,
                        bill_source,
                        eb.get("id"),
                        eb_source,
                    )
                    break

        return groups


# ==================== 便捷函数 ====================

_engine: SmartDeduplicationEngine | None = None


def get_dedup_engine() -> SmartDeduplicationEngine:
    """获取去重引擎单例

    Returns:
        SmartDeduplicationEngine: 去重引擎实例
    """
    global _engine  # pylint: disable=global-statement
    if _engine is None:
        _engine = SmartDeduplicationEngine()
    return _engine


def smart_deduplicate(bills: list[dict[str, Any]]) -> DeduplicationResult:
    """智能去重

    Args:
        bills: 账单列表

    Returns:
        DeduplicationResult: 去重结果
    """
    return get_dedup_engine().process(bills)
