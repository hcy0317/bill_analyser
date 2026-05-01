"""Normalization, scoring, and merge helpers for smart deduplication."""
# pylint: disable=duplicate-code,line-too-long,too-many-return-statements

import hashlib
from datetime import datetime
from difflib import SequenceMatcher
from typing import Any

import pandas as pd


# pylint: disable=too-few-public-methods


class NormalizationMixin:

    """NormalizationMixin implementation shard."""



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

    def _merge_parser_tags(self, *bills: dict[str, Any]) -> list[str]:
        """Merge parser tags from multiple bills while preserving order."""
        merged_tags: list[str] = []
        for bill in bills:
            raw_tags = bill.get("_parser_tags") or []
            if isinstance(raw_tags, (list, tuple, set)):
                normalized_tags = [str(tag).strip().lower() for tag in raw_tags if str(tag).strip()]
            elif raw_tags:
                normalized_tags = [str(raw_tags).strip().lower()]
            else:
                source_type = self._get_source_type(bill)
                normalized_tags = [f"parser:{source_type}"] if source_type else []

            for tag in normalized_tags:
                if tag and tag not in merged_tags:
                    merged_tags.append(tag)

        return merged_tags
