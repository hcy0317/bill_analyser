"""Category maintenance and simple bill CRUD/statistics delegates."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    log_method,
    log_step,
)

class CategoryMaintenanceMixin:
    """Category maintenance and simple bill CRUD/statistics delegates."""

    @log_method
    async def batch_update_category(
        self, bill_ids: list[int], main_category: str, sub_category: str | None = None, user_id: int = 1
    ) -> dict[str, Any]:
        """
        批量更新账单分类

        Args:
            bill_ids: 账单ID列表
            main_category: 主分类
            sub_category: 子分类（可选）
            user_id: 用户ID

        Returns:
            Dict: 更新结果
        """
        if not self._initialized:
            await self.initialize()

        result = {"success": False, "total": len(bill_ids), "updated": 0, "errors": []}

        updated = 0
        for bill_id in bill_ids:
            try:
                success = await self.db.update_bill(
                    bill_id, {"main_category": main_category, "sub_category": sub_category}, user_id=user_id
                )
                if success:
                    updated += 1
            except Exception as e:  # pylint: disable=broad-except
                result["errors"].append(f"Bill {bill_id}: {e!s}")

        result["updated"] = updated
        result["success"] = True

        self.logger.info("批量分类更新完成: 总计 %d 条, 更新 %d 条", result["total"], result["updated"])

        return result

    @log_method
    async def add_category_keyword(
        self, main_category: str, sub_category: str | None, keyword: str, user_id: int = 1
    ) -> bool:
        """
        为分类添加关键词

        Args:
            main_category: 主分类
            sub_category: 子分类
            keyword: 要添加的关键词
            user_id: 用户ID

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        # 获取现有分类规则
        category = await self.db.get_category_by_name(main_category, sub_category or "", user_id=user_id)

        if not category:
            self.logger.warning("分类不存在: %s/%s", main_category, sub_category)
            return False

        # 添加关键词
        current_keywords = category.get("keywords", "") or ""
        keyword_list = [k.strip() for k in current_keywords.split(",") if k.strip()]

        if keyword not in keyword_list:
            keyword_list.append(keyword)
            new_keywords = ",".join(keyword_list)

            success = await self.db.update_category(category["id"], {"keywords": new_keywords}, user_id=user_id)

            if success:
                # 重新加载分类规则（使用当前用户ID）
                await self.category_engine.load_rules_from_db(self.db, user_id=user_id)
                self.logger.info(
                    "添加关键词成功: %s/%s <- '%s' (user_id=%d)", main_category, sub_category, keyword, user_id
                )
                return True

        return False

    @log_method
    async def refresh_category_for_bills(self, bill_ids: list[int] | None = None, user_id: int = 1) -> dict[str, Any]:
        """
        刷新账单分类（使用最新的分类规则重新匹配）

        Args:
            bill_ids: 要刷新的账单ID列表，如果为None则刷新所有未分类账单
            user_id: 用户ID

        Returns:
            Dict: 刷新结果
        """
        if not self._initialized:
            await self.initialize()

        # 重新加载分类规则（使用当前用户ID）
        self.logger.info("刷新分类规则 (user_id=%d)", user_id)
        await self.category_engine.load_rules_from_db(self.db, user_id=user_id)

        result = {"success": False, "total": 0, "categorized": 0, "still_uncategorized": 0}

        # 获取需要分类的账单
        if bill_ids:
            bills = []
            for bid in bill_ids:
                bill = await self.db.get_bill_by_id(bid, user_id=user_id)
                if bill:
                    bills.append(bill)
        else:
            # 获取所有未分类账单
            bills = await self.db.get_bills(filters={"main_category": None}, user_id=user_id)

        result["total"] = len(bills)

        # 重新分类
        for bill in bills:
            main_cat, sub_cat = self.category_engine.match_category(bill)

            if main_cat:
                await self.db.update_bill(
                    bill["id"], {"main_category": main_cat, "sub_category": sub_cat}, user_id=user_id
                )
                result["categorized"] += 1
            else:
                result["still_uncategorized"] += 1

        result["success"] = True

        self.logger.info(
            "分类刷新完成: 总计 %d 条, 成功分类 %d 条, 仍未分类 %d 条",
            result["total"],
            result["categorized"],
            result["still_uncategorized"],
        )

        return result

    @log_method
    @log_step("清理无效账单")
    async def clean_invalid(self) -> int:
        """
        清理无效的账单数据

        Returns:
            int: 清理的数量
        """
        if not self._initialized:
            await self.initialize()

        # 获取所有账单
        bills = await self.db.get_bills()

        invalid_ids = []
        for bill in bills:
            is_valid, _ = self.validator.validate_bill(bill)
            if not is_valid:
                invalid_ids.append(bill["id"])

        # 删除无效账单
        deleted = 0
        for bill_id in invalid_ids:
            if await self.db.delete_bill(bill_id):
                deleted += 1

        self.logger.info("清理完成: 删除了 %d 条无效账单", deleted)
        return deleted

    @log_method
    @log_step("更新账单分类")
    async def update_categories(self, force: bool = False) -> dict[str, int]:
        """
        重新分类所有账单

        Args:
            force: 是否强制重新分类（包括已有分类的账单）

        Returns:
            Dict: {'total': 总数, 'updated': 更新数}
        """
        if not self._initialized:
            await self.initialize()

        # 获取需要分类的账单
        if force:
            bills = await self.db.get_bills()
        else:
            bills = await self.db.get_bills({"main_category": None})

        self.logger.info("开始重新分类 %d 条账单", len(bills))

        updated = 0
        for bill in bills:
            main_cat, sub_cat = self.category_engine.match_category(bill)

            if main_cat:
                success = await self.db.update_bill(bill["id"], {"main_category": main_cat, "sub_category": sub_cat})

                if success:
                    updated += 1

        self.logger.info("分类更新完成: 总计 %d 条，更新 %d 条", len(bills), updated)

        return {"total": len(bills), "updated": updated}

    @log_method
    async def get_bills(
        self, filters: dict[str, Any] | None = None, limit: int | None = None, offset: int = 0
    ) -> list[dict[str, Any]]:
        """
        查询账单

        Args:
            filters: 过滤条件
            limit: 限制数量
            offset: 偏移量

        Returns:
            List[Dict]: 账单列表
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.get_bills(filters, limit, offset)

    @log_method
    async def delete_bill(self, bill_id: int) -> bool:
        """
        删除账单

        Args:
            bill_id: 账单ID

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.delete_bill(bill_id)

    @log_method
    async def update_bill(self, bill_id: int, updates: dict[str, Any]) -> bool:
        """
        更新账单

        Args:
            bill_id: 账单ID
            updates: 更新内容

        Returns:
            bool: 是否成功
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.update_bill(bill_id, updates)

    @log_method
    async def deduplicate(self) -> int:
        """
        去除重复账单

        Returns:
            int: 删除的数量
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.deduplicate()

    @log_method
    async def get_statistics(self) -> dict[str, Any]:
        """
        获取账单统计信息

        Returns:
            Dict: 统计信息
        """
        if not self._initialized:
            await self.initialize()

        return await self.db.get_statistics()
