"""Database-backed duplicate detection."""
# pylint: disable=broad-exception-caught,duplicate-code,line-too-long,too-many-branches
# pylint: disable=too-many-locals,too-many-statements

from datetime import timedelta
from typing import Any

from ...utils.logger import log_method
from .models import DeduplicationType, DuplicateGroup


# pylint: disable=too-few-public-methods


class DatabaseDuplicateMixin:

    """DatabaseDuplicateMixin implementation shard."""



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
