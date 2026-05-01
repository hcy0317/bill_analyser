"""Exact duplicate detection."""
# pylint: disable=line-too-long

from typing import Any

from .models import DeduplicationType, DuplicateGroup


# pylint: disable=too-few-public-methods


class ExactDuplicateMixin:

    """ExactDuplicateMixin implementation shard."""



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
