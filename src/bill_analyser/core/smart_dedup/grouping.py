"""Similar-bill and split-bill grouping logic."""
# pylint: disable=invalid-name,line-too-long,too-many-branches,too-many-locals
# pylint: disable=too-many-statements

from collections import defaultdict
from typing import Any

from .models import DeduplicationType, DuplicateGroup


# pylint: disable=too-few-public-methods


class BillGroupingMixin:

    """BillGroupingMixin implementation shard."""



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
