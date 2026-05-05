"""Preview reclassification orchestration."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    cast,
    inspect,
    log_method,
)

class ImportReclassifyMixin:
    """Preview reclassification orchestration."""

    @log_method
    async def reclassify_preview_bills(
        self, session_id: str, preview_updates: list[dict[str, Any]] | None = None, user_id: int = 1
    ) -> dict[str, Any]:
        """
        v6.55: 重新分类预览账单

        功能：
        1. 强制刷新分类规则
        2. 从 bills_preview 表读取所有账单
        3. 根据 dedup_type 使用不同类型的分类规则：
           - transfer: 使用 TRANSFER 类型规则
           - 其他: 使用 EXPENSE/INCOME/INVESTMENT 规则
        4. 重新执行账户匹配
        5. 更新 bills_preview 表

        Args:
            session_id: 导入会话ID
            user_id: 用户ID

        Returns:
            Dict: 重新分类结果
        """
        self.logger.info("[重新分类] 开始 session=%s, user_id=%d", session_id, user_id)

        result = {
            "success": False,
            "session_id": session_id,
            "total": 0,
            "categorized": 0,
            "account_matched": 0,
            "session_samples_saved": 0,
            "session_suggestion_applied": 0,
            "annotation_applied": 0,
            "errors": [],
        }

        try:
            if preview_updates:
                self.logger.info("[重新分类] 步骤0: 同步当前预览草稿 (%d 条)", len(preview_updates))
                update_preview_batch = getattr(self.db, "update_preview_bills_batch", None)
                if callable(update_preview_batch):
                    update_preview_batch_fn = cast("Any", update_preview_batch)
                    update_preview_batch_params = inspect.signature(update_preview_batch).parameters
                    if "user_id" in update_preview_batch_params:
                        await update_preview_batch_fn(session_id, preview_updates, user_id=user_id)
                    else:
                        await update_preview_batch_fn(session_id, preview_updates)

                self.logger.info("[重新分类] 步骤0: 保存当前会话人工标注样本 (%d 条)", len(preview_updates))
                result["session_samples_saved"] = await self.db.save_import_annotation_samples(
                    session_id, preview_updates, user_id=user_id
                )

            # 1. 强制刷新分类规则
            self.logger.info("[重新分类] 步骤1: 刷新分类规则")
            await self.category_engine.load_rules_from_db(self.db, user_id=user_id)
            self.logger.info("[重新分类] 分类规则加载完成，规则数量: %d", len(self.category_engine.rules))

            # 2. 获取所有预览账单
            self.logger.info("[重新分类] 步骤2: 读取预览账单")
            preview_method_params = inspect.signature(self.db.get_preview_by_session).parameters
            if "user_id" in preview_method_params:
                previews = await self.db.get_preview_by_session(session_id, user_id=user_id)
            else:
                previews = await self.db.get_preview_by_session(session_id)
            result["total"] = len(previews)
            self.logger.info("[重新分类] 读取到 %d 条预览账单", len(previews))

            if not previews:
                result["success"] = True
                return result

            # 3. 将预览数据转换为账单格式，用于分类匹配
            bills_for_category = []
            for preview in previews:
                bill = {
                    "id": preview.get("id"),
                    "date": preview.get("preview_date", ""),
                    "type": preview.get("preview_type", ""),
                    "amount": float(preview.get("preview_amount", 0)),
                    "counterparty": preview.get("preview_counterparty", ""),
                    "payment_method": preview.get("preview_payment_method", ""),
                    "description": preview.get("preview_description", ""),
                    "_parser_id": preview.get("preview_parser_id", ""),
                    "_dedup_type": preview.get("dedup_type", "remaining"),
                    # 保留原有账户ID用于后续更新
                    "source_account_id": preview.get("preview_source_account_id"),
                    "destination_account_id": preview.get("preview_destination_account_id"),
                }
                bills_for_category.append(bill)

            annotation_samples = await self.db.get_import_annotation_samples(session_id, user_id=user_id)
            annotation_map = {
                int(sample["preview_id"]): sample for sample in annotation_samples if sample.get("preview_id")
            }
            session_rule_lookup = await self._build_session_annotation_rule_lookup(
                previews, annotation_samples, user_id=user_id
            )

            for bill in bills_for_category:
                sample = annotation_map.get(int(bill.get("id", 0))) if bill.get("id") else None
                if sample and sample.get("annotated_type"):
                    bill["type"] = sample.get("annotated_type")

            result["session_suggestion_applied"] = self._apply_session_annotation_learning_rules(
                bills_for_category, session_rule_lookup, annotation_map, type_only=True
            )
            if result["session_suggestion_applied"] > 0:
                self.logger.info("[重新分类] 会话临时学习类型预填充 %d 条", result["session_suggestion_applied"])

            learned_seed_count = await self._apply_import_learning_rules(
                bills_for_category, user_id=user_id, type_only=True, record_usage=False
            )
            if learned_seed_count > 0:
                self.logger.info("[重新分类] 长期学习类型预填充 %d 条", learned_seed_count)

            for bill in bills_for_category:
                sample = annotation_map.get(int(bill.get("id", 0))) if bill.get("id") else None
                if sample and sample.get("annotated_type"):
                    bill["type"] = sample.get("annotated_type")

            await self._normalize_non_pair_investment_balance_changes(
                bills_for_category,
                user_id=user_id,
            )

            # 4. 分类匹配
            # v6.72: 简化分类逻辑 - 让 match_category() 自动根据账单金额正负选择分类类型
            # - 转账配对账单：使用转账类关键词（通过 _dedup_type='transfer' 识别）
            # - 收入账单（amount > 0）：只使用收入类和投资类关键词
            # - 支出账单（amount < 0）：只使用支出类和投资类关键词
            self.logger.info("[重新分类] 步骤3: 分类匹配 (%d 条账单)", len(bills_for_category))

            # 批量分类 - 不传递 types 参数，让 match_category() 自动根据账单特征选择
            categorized_bills = await self.category_engine.batch_match_categories(bills_for_category, types=None)

            # 5. 账户匹配
            self.logger.info("[重新分类] 步骤4: 账户匹配")
            matched_bills = await self._match_accounts(categorized_bills, user_id)

            matched_bills = await self._detect_cash_transfers(matched_bills, user_id)

            learned_replay_count = await self._apply_import_learning_rules(
                matched_bills, user_id=user_id, type_only=False, record_usage=True
            )
            if learned_replay_count > 0:
                self.logger.info("[重新分类] 长期学习结果回放 %d 条", learned_replay_count)

            result["session_suggestion_applied"] += self._apply_session_annotation_learning_rules(
                matched_bills, session_rule_lookup, annotation_map, type_only=False
            )
            if result["session_suggestion_applied"] > 0:
                self.logger.info("[重新分类] 会话临时学习结果回放累计 %d 条", result["session_suggestion_applied"])

            result["annotation_applied"] = await self._apply_session_annotation_samples(
                matched_bills, annotation_map, user_id=user_id
            )

            # 6. 统计并更新预览表
            self.logger.info("[重新分类] 步骤5: 更新预览表")
            updates = []
            categorized_count = 0
            account_matched_count = 0

            for bill in matched_bills:
                preview_id = bill.get("id")
                if not preview_id:
                    continue

                update_data = {
                    "id": preview_id,
                    "preview_type": bill.get("type", ""),
                    "preview_main_category": bill.get("main_category", ""),
                    "preview_sub_category": bill.get("sub_category", ""),
                    "preview_source_account_id": bill.get("source_account_id"),
                    "preview_destination_account_id": bill.get("destination_account_id"),
                }
                updates.append(update_data)

                if bill.get("main_category"):
                    categorized_count += 1
                if bill.get("source_account_id") and (
                    isinstance(bill.get("source_account_id"), int)
                    or (
                        isinstance(bill.get("source_account_id"), str)
                        and str(bill.get("source_account_id", "")).isdigit()
                    )
                ):
                    account_matched_count += 1

            # 批量更新
            if updates:
                updated = await self.db.batch_update_preview_classification(updates)
                self.logger.info("[重新分类] 更新了 %d 条预览记录", updated)

            result["success"] = True
            result["categorized"] = categorized_count
            result["account_matched"] = account_matched_count

            self.logger.info(
                "[重新分类] 完成: 总计 %d, 分类匹配 %d, 账户匹配 %d, 会话样本 %d, 临时学习 %d, 精确回放 %d",
                result["total"],
                result["categorized"],
                result["account_matched"],
                result["session_samples_saved"],
                result["session_suggestion_applied"],
                result["annotation_applied"],
            )

        except Exception as e:
            self.logger.error("[重新分类] 失败: %s", e, exc_info=True)
            result["errors"].append(str(e))

        return result
