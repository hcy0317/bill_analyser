"""Payment-platform to bank duplicate detection."""
# pylint: disable=duplicate-code,too-many-branches,too-many-locals,too-many-statements

from typing import Any

import numpy as np

from .models import DeduplicationType, DuplicateGroup


# pylint: disable=too-few-public-methods


class PlatformBankDuplicateMixin:

    """PlatformBankDuplicateMixin implementation shard."""



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
            p_bill["_parser_tags"] = self._merge_parser_tags(p_bill, b_bill)
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
