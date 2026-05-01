"""Durable/session import-learning rules, annotation suggestions, and promotion helpers."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    Database,
    log_method,
)

class ImportLearningRulesMixin:
    """Durable/session import-learning rules, annotation suggestions, and promotion helpers."""

    @staticmethod
    def _normalize_learning_text(raw_value: Any) -> str:
        """标准化长期学习匹配文本。"""
        if raw_value is None:
            return ""

        text = str(raw_value).strip().lower()
        if not text:
            return ""

        parts = [part.strip() for part in text.split("|") if part.strip()]
        if parts:
            text = " | ".join(parts)

        return " ".join(text.split())

    async def _apply_import_learning_rules(
        self, bills: list[dict[str, Any]], user_id: int = 1, type_only: bool = False, record_usage: bool = False
    ) -> int:
        """应用长期导入学习规则。

        匹配优先级: 复合匹配 > 交易对方 > 描述 > 支付方式
        """
        if not bills:
            return 0

        user = await self.db.get_user_by_id(user_id)
        if not user or not bool(user.get("import_learning_enabled", 1)):
            self.logger.debug("[长期学习] 已关闭或用户不存在: user_id=%d", user_id)
            return 0

        rules = await self.db.get_import_learning_rules(user_id=user_id, enabled_only=True, limit=None)
        if not rules:
            return 0

        # 新生成的长期学习规则仅保留复合规则；这里兼容历史单字段规则，
        # 避免升级后老用户既有学习规则静默失效。
        composite_lookup: dict[str, dict[str, Any]] = {
            str(rule.get("composite_match_hash") or ""): rule
            for rule in rules
            if rule.get("match_type") == "composite" and str(rule.get("composite_match_hash") or "")
        }
        single_lookup: dict[tuple[str, str], dict[str, Any]] = {
            (str(rule.get("match_type") or ""), str(rule.get("normalized_match_value") or "")): rule
            for rule in rules
            if rule.get("match_type") in {"counterparty", "description", "payment_method"}
            and str(rule.get("normalized_match_value") or "")
        }
        if not composite_lookup and not single_lookup:
            return 0

        category_cache: dict[int, dict[str, Any] | None] = {}
        account_cache: dict[int, dict[str, Any] | None] = {}
        matched_rule_ids: list[int] = []
        applied_count = 0

        for bill in bills:
            matched_rule = None

            # 优先尝试复合匹配
            if composite_lookup:
                composite_hash = self.db.build_composite_match_hash(
                    parser_id=bill.get("_parser_id", ""),
                    counterparty=bill.get("counterparty", ""),
                    description=bill.get("description", ""),
                    payment_method=bill.get("payment_method", ""),
                )
                if composite_hash:
                    matched_rule = composite_lookup.get(composite_hash)
                    if matched_rule:
                        bill["_import_learning_match_type"] = "composite"
                        bill["_import_learning_rule_id"] = matched_rule.get("id")

            # 兼容历史单字段长期学习规则。
            if not matched_rule and single_lookup:
                for match_type, raw_value in [
                    ("counterparty", bill.get("counterparty", "")),
                    ("description", bill.get("description", "")),
                    ("payment_method", bill.get("payment_method", "")),
                ]:
                    normalized_value = self._normalize_learning_text(raw_value)
                    if not normalized_value:
                        continue

                    matched_rule = single_lookup.get((match_type, normalized_value))
                    if matched_rule:
                        bill["_import_learning_match_type"] = match_type
                        bill["_import_learning_rule_id"] = matched_rule.get("id")
                        break

            if not matched_rule:
                continue

            learned_type = matched_rule.get("learned_type")
            if learned_type:
                bill["type"] = learned_type

            if not type_only:
                learned_category_id = matched_rule.get("learned_category_id")
                if learned_category_id:
                    cat_id = int(learned_category_id)
                    if cat_id not in category_cache:
                        category_cache[cat_id] = await self.db.get_category_by_id(cat_id, user_id)
                    category = category_cache.get(cat_id)
                    if category:
                        bill["main_category"] = category.get("main_category", "")
                        bill["sub_category"] = category.get("sub_category", "")

                learned_source_account_id = matched_rule.get("learned_source_account_id")
                if learned_source_account_id not in (None, "", 0, "0"):
                    source_account_id = int(learned_source_account_id)
                    if source_account_id not in account_cache:
                        account_cache[source_account_id] = await self.db.get_account_by_id(
                            source_account_id,
                            user_id=user_id,
                        )
                    source_account = account_cache.get(source_account_id)
                    if source_account and source_account.get("id") not in (None, ""):
                        bill["source_account_id"] = int(source_account.get("id") or 0)

                learned_destination_account_id = matched_rule.get("learned_destination_account_id")
                if learned_destination_account_id not in (None, "", 0, "0"):
                    destination_account_id = int(learned_destination_account_id)
                    if destination_account_id not in account_cache:
                        account_cache[destination_account_id] = await self.db.get_account_by_id(
                            destination_account_id,
                            user_id=user_id,
                        )
                    destination_account = account_cache.get(destination_account_id)
                    if destination_account and destination_account.get("id") not in (None, ""):
                        bill["destination_account_id"] = int(destination_account.get("id") or 0)

            applied_count += 1
            rule_id = matched_rule.get("id")
            if rule_id:
                matched_rule_ids.append(int(rule_id))

        if record_usage and matched_rule_ids:
            await self.db.increment_import_learning_rule_usage(matched_rule_ids, user_id=user_id)
        if applied_count > 0 and hasattr(self.db, "record_import_learning_feedback_event"):
            await self.db.record_import_learning_feedback_event(
                "rule_auto_apply",
                user_id=user_id,
                payload={
                    "applied_count": applied_count,
                    "bill_count": len(bills),
                    "rule_ids": sorted(set(matched_rule_ids)),
                    "type_only": bool(type_only),
                },
            )

        if applied_count > 0:
            self.logger.info(
                (
                    "[长期学习] 应用完成: user_id=%d, bills=%d, applied=%d, type_only=%s, "
                    "composite_rules=%d, legacy_single_rules=%d"
                ),
                user_id,
                len(bills),
                applied_count,
                type_only,
                len(composite_lookup),
                len(single_lookup),
            )

        return applied_count

    async def _build_session_annotation_rule_lookup(
        self, previews: list[dict[str, Any]], annotation_samples: list[dict[str, Any]], user_id: int = 1
    ) -> dict[tuple[str, str], dict[str, Any]]:
        """基于当前会话人工标注构建临时学习规则查找表。

        包含复合匹配规则（match_type='composite'）和单字段规则。
        """
        if not previews or not annotation_samples:
            return {}

        preview_map = {int(preview["id"]): preview for preview in previews if preview.get("id")}
        rule_lookup: dict[tuple[str, str], dict[str, Any]] = {}
        category_cache: dict[int, dict[str, Any] | None] = {}

        for sample in annotation_samples:
            preview_id = int(sample.get("preview_id", 0) or 0)
            source_preview = preview_map.get(preview_id)
            if not source_preview:
                continue

            learned_category_id = sample.get("annotated_category_id")
            learned_main_category = ""
            learned_sub_category = ""
            if learned_category_id:
                category_id = int(learned_category_id)
                if category_id not in category_cache:
                    category_cache[category_id] = await self.db.get_category_by_id(category_id, user_id=user_id)
                category = category_cache.get(category_id) or {}
                learned_main_category = category.get("main_category", "")
                learned_sub_category = category.get("sub_category", "")

            rule_payload = {
                "preview_id": preview_id,
                "annotated_type": sample.get("annotated_type", ""),
                "annotated_category_id": learned_category_id,
                "annotated_main_category": learned_main_category,
                "annotated_sub_category": learned_sub_category,
                "annotated_source_account_id": sample.get("annotated_source_account_id"),
                "annotated_destination_account_id": sample.get("annotated_destination_account_id"),
            }

            # 单字段规则
            for match_type, raw_value in [
                ("counterparty", source_preview.get("preview_counterparty", "")),
                ("description", source_preview.get("preview_description", "")),
                ("payment_method", source_preview.get("preview_payment_method", "")),
            ]:
                normalized_value = self._normalize_learning_text(raw_value)
                if not normalized_value:
                    continue
                rule_lookup[(match_type, normalized_value)] = rule_payload

            # 复合匹配规则
            composite_hash = self.db.build_composite_match_hash(
                parser_id=source_preview.get("preview_parser_id", ""),
                counterparty=source_preview.get("preview_counterparty", ""),
                description=source_preview.get("preview_description", ""),
                payment_method=source_preview.get("preview_payment_method", ""),
            )
            if composite_hash:
                rule_lookup[("composite", composite_hash)] = rule_payload

        if rule_lookup:
            self.logger.info("[会话学习] 构建临时规则 %d 条", len(rule_lookup))

        return rule_lookup

    @staticmethod
    def _apply_session_annotation_learning_rules(
        bills: list[dict[str, Any]],
        rule_lookup: dict[tuple[str, str], dict[str, Any]],
        annotation_map: dict[int, dict[str, Any]],
        type_only: bool = False,
    ) -> int:
        """将当前会话人工标注以临时学习规则形式回放到相似账单。

        匹配优先级: 复合匹配 > 交易对方 > 描述 > 支付方式
        """
        if not bills or not rule_lookup:
            return 0

        applied_count = 0

        for bill in bills:
            bill_id = int(bill.get("id", 0) or 0)
            if bill_id and bill_id in annotation_map:
                continue

            matched_rule = None

            # 优先尝试复合匹配
            composite_hash = Database.build_composite_match_hash(
                parser_id=bill.get("_parser_id", ""),
                counterparty=bill.get("counterparty", ""),
                description=bill.get("description", ""),
                payment_method=bill.get("payment_method", ""),
            )
            if composite_hash:
                matched_rule = rule_lookup.get(("composite", composite_hash))
                if matched_rule:
                    bill["_session_annotation_match_type"] = "composite"
                    bill["_session_annotation_source_preview_id"] = matched_rule.get("preview_id")

            # 回退到单字段匹配
            if not matched_rule:
                for match_type, raw_value in [
                    ("counterparty", bill.get("counterparty", "")),
                    ("description", bill.get("description", "")),
                    ("payment_method", bill.get("payment_method", "")),
                ]:
                    normalized_value = ImportLearningRulesMixin._normalize_learning_text(raw_value)
                    if not normalized_value:
                        continue

                    matched_rule = rule_lookup.get((match_type, normalized_value))
                    if matched_rule:
                        bill["_session_annotation_match_type"] = match_type
                        bill["_session_annotation_source_preview_id"] = matched_rule.get("preview_id")
                        break

            if not matched_rule:
                continue

            if matched_rule.get("annotated_type"):
                bill["type"] = matched_rule.get("annotated_type")

            if not type_only:
                if matched_rule.get("annotated_main_category"):
                    bill["main_category"] = matched_rule.get("annotated_main_category", "")
                    bill["sub_category"] = matched_rule.get("annotated_sub_category", "")

                if matched_rule.get("annotated_source_account_id"):
                    bill["source_account_id"] = matched_rule.get("annotated_source_account_id")

                if matched_rule.get("annotated_destination_account_id"):
                    bill["destination_account_id"] = matched_rule.get("annotated_destination_account_id")

            applied_count += 1

        return applied_count

    @log_method
    async def promote_session_annotations_to_learning(
        self,
        session_id: str,
        preview_updates: list[dict[str, Any]] | None = None,
        preview_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """将当前会话人工标注提升为长期学习规则。"""
        selected_preview_ids: list[int] | None = None
        if preview_updates is not None:
            selected_preview_ids = []
            await self.db.save_import_annotation_samples(
                session_id,
                preview_updates,
                user_id=user_id,
            )
            selected_preview_ids = [int(item["id"]) for item in preview_updates if item.get("id")]
        elif preview_ids is not None:
            selected_preview_ids = [int(preview_id) for preview_id in preview_ids if int(preview_id) > 0]

        promote_result = await self.db.promote_import_annotation_samples_to_learning(
            session_id,
            preview_ids=selected_preview_ids,
            user_id=user_id,
        )
        return {
            "success": True,
            "session_id": session_id,
            **promote_result,
        }

    @log_method
    async def get_import_learning_suggestions(
        self,
        session_id: str,
        preview_updates: list[dict[str, Any]] | None = None,
        preview_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """返回当前导入会话的 dry-run 长期学习建议列表。"""
        selected_preview_ids: set[int] | None = None
        if preview_updates is not None:
            await self.db.save_import_annotation_samples(
                session_id,
                preview_updates,
                user_id=user_id,
            )
            selected_preview_ids = {
                int(item["id"])
                for item in preview_updates
                if item.get("id")
            }
        elif preview_ids is not None:
            selected_preview_ids = {
                int(preview_id)
                for preview_id in preview_ids
                if int(preview_id) > 0
            }

        suggestions = await self.db.list_import_learning_suggestions_for_session(
            session_id,
            user_id=user_id,
        )
        if selected_preview_ids is not None and not selected_preview_ids:
            suggestions = []

        if not suggestions:
            return {
                "sessionId": session_id,
                "totalCount": 0,
                "suggestions": [],
            }

        categories = await self.db.get_all_categories(user_id=user_id)
        accounts = await self.db.get_all_accounts(user_id=user_id)
        categories_by_id = {
            int(category["id"]): category
            for category in categories
            if category.get("id") is not None
        }
        accounts_by_id = {
            int(account["id"]): account
            for account in accounts
            if account.get("id") is not None
        }

        result: list[dict[str, Any]] = []
        for suggestion in suggestions:
            source_preview_ids = [
                int(preview_id)
                for preview_id in list(suggestion.get("source_preview_ids") or [])
                if int(preview_id) > 0
            ]
            if selected_preview_ids is not None:
                source_preview_ids = [
                    preview_id
                    for preview_id in source_preview_ids
                    if preview_id in selected_preview_ids
                ]
                if not source_preview_ids:
                    continue

            learned_category_id = suggestion.get("learned_category_id")
            learned_source_account_id = suggestion.get("learned_source_account_id")
            learned_destination_account_id = suggestion.get("learned_destination_account_id")

            learned_category = (
                categories_by_id.get(int(learned_category_id))
                if learned_category_id not in (None, "", 0, "0")
                else None
            )
            learned_source_account = (
                accounts_by_id.get(int(learned_source_account_id))
                if learned_source_account_id not in (None, "", 0, "0")
                else None
            )
            learned_destination_account = (
                accounts_by_id.get(int(learned_destination_account_id))
                if learned_destination_account_id not in (None, "", 0, "0")
                else None
            )

            learned_category_name = ""
            if learned_category:
                main_category = str(learned_category.get("main_category") or "").strip()
                sub_category = str(learned_category.get("sub_category") or "").strip()
                learned_category_name = (
                    f"{main_category}/{sub_category}"
                    if main_category and sub_category
                    else main_category
                )

            result.append(
                {
                    "matchType": str(suggestion.get("match_type") or ""),
                    "matchValue": str(suggestion.get("match_value") or ""),
                    "matchFeatures": dict(suggestion.get("match_features") or {}),
                    "sampleCount": len(source_preview_ids),
                    "sourcePreviewIds": source_preview_ids,
                    "learnedType": str(suggestion.get("learned_type") or ""),
                    "learnedCategoryId": (
                        int(learned_category_id)
                        if learned_category_id not in (None, "", 0, "0")
                        else None
                    ),
                    "learnedCategoryName": learned_category_name,
                    "learnedSourceAccountId": (
                        int(learned_source_account_id)
                        if learned_source_account_id not in (None, "", 0, "0")
                        else None
                    ),
                    "learnedSourceAccountName": (
                        str(learned_source_account.get("name") or "")
                        if learned_source_account
                        else ""
                    ),
                    "learnedDestinationAccountId": (
                        int(learned_destination_account_id)
                        if learned_destination_account_id not in (None, "", 0, "0")
                        else None
                    ),
                    "learnedDestinationAccountName": (
                        str(learned_destination_account.get("name") or "")
                        if learned_destination_account
                        else ""
                    ),
                    "summary": self._build_learning_rule_result_summary(
                        suggestion,
                        categories_by_id,
                        accounts_by_id,
                    ),
                }
            )

        return {
            "sessionId": session_id,
            "totalCount": len(result),
            "suggestions": result,
        }

    @log_method
    async def _apply_session_annotation_samples(
        self, bills: list[dict[str, Any]], annotation_map: dict[int, dict[str, Any]], user_id: int = 1
    ) -> int:
        """将当前会话中的人工标注样本回放到重新分类结果。"""
        if not bills or not annotation_map:
            return 0

        applied_count = 0
        category_cache: dict[int, dict[str, Any]] = {}

        for bill in bills:
            bill_id = bill.get("id")
            if not bill_id:
                continue

            sample = annotation_map.get(int(bill_id))
            if not sample:
                continue

            if sample.get("annotated_type"):
                bill["type"] = sample.get("annotated_type")

            category_id = sample.get("annotated_category_id")
            if category_id:
                category_id = int(category_id)
                if category_id not in category_cache:
                    category_cache[category_id] = await self.db.get_category_by_id(category_id, user_id=user_id) or {}
                category = category_cache.get(category_id, {})
                bill["main_category"] = category.get("main_category", "")
                bill["sub_category"] = category.get("sub_category", "")

            if sample.get("annotated_source_account_id"):
                bill["source_account_id"] = sample.get("annotated_source_account_id")

            if sample.get("annotated_destination_account_id"):
                bill["destination_account_id"] = sample.get("annotated_destination_account_id")

            applied_count += 1

        self.logger.info("[重新分类] 回放会话标注样本 %d 条", applied_count)
        return applied_count
