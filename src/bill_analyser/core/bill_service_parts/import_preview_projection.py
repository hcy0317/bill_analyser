"""Preview item projection, context loading, and signal-status helpers."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    TransactionType,
    build_preview_matching_payload,
    inspect,
    log_method,
)

class ImportPreviewProjectionMixin:
    """Preview item projection, context loading, and signal-status helpers."""

    @log_method
    async def get_import_preview(
        self,
        session_id: str,
        selected_only: bool = False,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """
        获取导入预览数据

        Args:
            session_id: 导入会话ID
            selected_only: 是否只获取选中的

        Returns:
            List[Dict]: 预览账单列表
        """
        preview_method_params = inspect.signature(self.db.get_preview_by_session).parameters
        if "user_id" in preview_method_params:
            previews = await self.db.get_preview_by_session(
                session_id,
                user_id=user_id,
                selected_only=selected_only,
            )
        else:
            previews = await self.db.get_preview_by_session(session_id, selected_only=selected_only)
        if not previews:
            return []

        preview_user_id = int(previews[0].get("user_id") or 0)
        projection_context = await self._load_import_preview_projection_context(
            session_id,
            user_id=preview_user_id,
        )
        result: list[dict[str, Any]] = []
        for preview in previews:
            result.append(
                await self._build_import_preview_item(
                    preview,
                    user_id=preview_user_id,
                    projection_context=projection_context,
                )
            )

        return result

    async def get_import_preview_item(
        self,
        preview_id: int,
        user_id: int = 1,
    ) -> dict[str, Any] | None:
        """Return one import-preview row projected into the frontend preview-item contract."""
        preview = await self.db.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return None

        preview_user_id = int(preview.get("user_id") or user_id or 1)
        projection_context = await self._load_import_preview_projection_context(
            str(preview.get("session_id") or ""),
            user_id=preview_user_id,
        )
        return await self._build_import_preview_item(
            preview,
            user_id=preview_user_id,
            projection_context=projection_context,
        )

    async def _load_import_preview_projection_context(
        self,
        session_id: str,
        *,
        user_id: int,
    ) -> dict[str, Any]:
        annotation_samples = (
            await self.db.get_import_annotation_samples(session_id, user_id=user_id)
            if session_id and user_id > 0
            else []
        )
        manually_annotated_preview_ids = {
            int(sample["preview_id"]) for sample in annotation_samples if sample.get("preview_id")
        }
        user = await self.db.get_user_by_id(user_id) if user_id > 0 else None
        import_learning_enabled = bool(user.get("import_learning_enabled", 1)) if user else False
        learning_rules = (
            await self.db.get_import_learning_rules(user_id=user_id, enabled_only=True, limit=None)
            if user_id > 0 and import_learning_enabled
            else []
        )
        active_learning_model = (
            await self.db.get_active_import_learning_model(user_id=user_id)
            if user_id > 0 and import_learning_enabled and hasattr(self.db, "get_active_import_learning_model")
            else None
        )
        learning_model_suppressed_preview_ids = (
            await self.db.get_import_learning_model_suppressed_preview_ids(session_id, user_id=user_id)
            if session_id
            and user_id > 0
            and import_learning_enabled
            and hasattr(self.db, "get_import_learning_model_suppressed_preview_ids")
            else set()
        )
        composite_learning_rules = [
            rule for rule in learning_rules if rule.get("match_type") == "composite" and rule.get("match_features_json")
        ]
        learning_categories_by_id: dict[int, dict[str, Any]] = {}
        learning_accounts_by_id: dict[int, dict[str, Any]] = {}
        if (composite_learning_rules or active_learning_model) and user_id > 0:
            learning_categories = await self.db.get_all_categories(user_id=user_id)
            learning_accounts = await self.db.get_all_accounts(user_id=user_id)
            learning_categories_by_id = {
                int(category["id"]): category for category in learning_categories if category.get("id") is not None
            }
            learning_accounts_by_id = {
                int(account["id"]): account for account in learning_accounts if account.get("id") is not None
            }

        reconciliation_candidates = (
            await self.db.list_import_reconciliation_candidates(
                user_id=user_id,
                session_id=session_id,
                limit=500,
            )
            if session_id and user_id > 0 and hasattr(self.db, "list_import_reconciliation_candidates")
            else []
        )
        reconciliation_by_preview_id: dict[int, list[dict[str, Any]]] = {}
        reconciliation_by_template_id: dict[int, list[dict[str, Any]]] = {}
        for candidate in reconciliation_candidates:
            preview_id = self._coerce_optional_positive_int(candidate.get("preview_id"))
            if preview_id is not None:
                reconciliation_by_preview_id.setdefault(preview_id, []).append(candidate)

            import_snapshot = dict(candidate.get("import_bill_snapshot") or {})
            template_id = self._coerce_optional_positive_int(import_snapshot.get("template_id"))
            if template_id is not None:
                reconciliation_by_template_id.setdefault(template_id, []).append(candidate)

        return {
            "session_id": session_id,
            "user_id": user_id,
            "import_learning_enabled": import_learning_enabled,
            "manually_annotated_preview_ids": manually_annotated_preview_ids,
            "composite_learning_rules": composite_learning_rules,
            "active_learning_model": active_learning_model,
            "learning_model_suppressed_preview_ids": learning_model_suppressed_preview_ids,
            "learning_categories_by_id": learning_categories_by_id,
            "learning_accounts_by_id": learning_accounts_by_id,
            "reconciliation_by_preview_id": reconciliation_by_preview_id,
            "reconciliation_by_template_id": reconciliation_by_template_id,
        }

    @staticmethod
    def _coerce_optional_positive_int(raw_value: Any) -> int | None:
        if raw_value in (None, "", 0, "0"):
            return None
        try:
            normalized_value = int(raw_value)
        except (TypeError, ValueError):
            return None
        return normalized_value if normalized_value > 0 else None

    @classmethod
    def _normalize_import_preview_source_ids(cls, raw_value: Any) -> list[int]:
        if raw_value in (None, ""):
            return []
        if isinstance(raw_value, str):
            raw_items = [item.strip() for item in raw_value.split(",") if item.strip()]
        elif isinstance(raw_value, list):
            raw_items = list(raw_value)
        elif isinstance(raw_value, (tuple, set)):
            raw_items = list(raw_value)
        else:
            raw_items = [raw_value]

        source_ids: list[int] = []
        for item in raw_items:
            normalized_id = cls._coerce_optional_positive_int(item)
            if normalized_id is not None and normalized_id not in source_ids:
                source_ids.append(normalized_id)
        return source_ids

    @classmethod
    def _get_reconciliation_candidates_for_preview(
        cls,
        preview: dict[str, Any],
        context: dict[str, Any],
    ) -> list[dict[str, Any]]:
        preview_id = cls._coerce_optional_positive_int(preview.get("id"))
        candidates: list[dict[str, Any]] = []
        seen_candidate_ids: set[str] = set()

        def add_candidates(raw_candidates: list[dict[str, Any]] | None) -> None:
            for candidate in raw_candidates or []:
                candidate_id = str(candidate.get("candidate_id") or "")
                if candidate_id and candidate_id not in seen_candidate_ids:
                    candidates.append(candidate)
                    seen_candidate_ids.add(candidate_id)

        if preview_id is not None:
            add_candidates(
                dict(context.get("reconciliation_by_preview_id") or {}).get(preview_id)
            )

        template_candidates = dict(context.get("reconciliation_by_template_id") or {})
        for source_id in cls._normalize_import_preview_source_ids(preview.get("dedup_source_ids")):
            add_candidates(template_candidates.get(source_id))

        return candidates

    async def _build_import_preview_item(
        self,
        preview: dict[str, Any],
        *,
        user_id: int,
        projection_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        preview_user_id = int(preview.get("user_id") or user_id or 1)
        session_id = str(preview.get("session_id") or "")
        context = projection_context
        context_session_id = (
            str(context.get("session_id") or "") if isinstance(context, dict) else ""
        )
        should_reload_context = not isinstance(context, dict) or int(context.get("user_id") or 0) != preview_user_id
        if not should_reload_context and session_id:
            should_reload_context = context_session_id != session_id
        if should_reload_context:
            context = await self._load_import_preview_projection_context(
                session_id or context_session_id,
                user_id=preview_user_id,
            )

        transfer_suggestion = self._build_transfer_suggestion_from_preview(preview)
        category_id = preview.get("category_id")
        if category_id in (None, "", 0, "0"):
            category_id = await self._get_preview_category_id(preview, user_id=preview_user_id)

        preview_id = int(preview.get("id", 0) or 0)
        is_manually_annotated = preview_id in set(context.get("manually_annotated_preview_ids") or set())
        learning_rules = list(context.get("composite_learning_rules") or [])
        categories_by_id = dict(context.get("learning_categories_by_id") or {})
        accounts_by_id = dict(context.get("learning_accounts_by_id") or {})
        learning_recommendation = self._build_learning_similarity_signal_from_preview(
            preview,
            learning_rules,
            categories_by_id=categories_by_id,
            accounts_by_id=accounts_by_id,
        )
        is_model_suppressed = preview_id in set(context.get("learning_model_suppressed_preview_ids") or set())
        if not learning_recommendation:
            active_model_for_preview = None
            if isinstance(context, dict) and not is_model_suppressed:
                active_model_for_preview = context.get("active_learning_model")
            learning_recommendation = self._build_learning_model_signal_from_preview(
                preview,
                active_model_for_preview,
                learning_rules=learning_rules,
                categories_by_id=categories_by_id,
                accounts_by_id=accounts_by_id,
                is_manually_annotated=is_manually_annotated,
            )
        if learning_recommendation.get("auto_apply") and not preview.get("preview_matching_feedback", {}).get("learning"):
            auto_applied_preview = await self.db.update_preview_learning_decision(
                preview_id,
                "accept",
                user_id=preview_user_id,
                applied_result=self._build_preview_learning_model_apply_payload(learning_recommendation),
            )
            if auto_applied_preview and not auto_applied_preview.get("_state_conflict"):
                preview = auto_applied_preview
                category_id = preview.get("category_id")
                if category_id in (None, "", 0, "0"):
                    category_id = await self._get_preview_category_id(preview, user_id=preview_user_id)
                if hasattr(self.db, "record_import_learning_feedback_event"):
                    await self.db.record_import_learning_feedback_event(
                        "model_blue_auto_apply",
                        user_id=preview_user_id,
                        session_id=session_id,
                        preview_id=preview_id,
                        candidate_id=f"model:{learning_recommendation.get('model_version')}:preview:{preview_id}",
                        payload={
                            "applied_count": 1,
                            "model_version": learning_recommendation.get("model_version"),
                            "confidence": learning_recommendation.get("confidence"),
                            "margin": learning_recommendation.get("margin"),
                            "confirmations": learning_recommendation.get("confirmations"),
                        },
                    )
        reconciliation_candidates = self._get_reconciliation_candidates_for_preview(
            preview,
            context,
        )
        matching_feedback = preview.get("preview_matching_feedback")
        if not bool(context.get("import_learning_enabled", False)) and isinstance(matching_feedback, dict):
            matching_feedback = {key: value for key, value in matching_feedback.items() if key != "learning"}

        return {
            "id": preview.get("id"),
            "preview_date": preview.get("preview_date", ""),
            "preview_type": preview.get("preview_type", ""),
            "preview_amount": preview.get("preview_amount", 0),
            "preview_destination_amount": preview.get("preview_destination_amount", 0),
            "category_id": category_id,
            "preview_main_category": preview.get("preview_main_category", ""),
            "preview_sub_category": preview.get("preview_sub_category", ""),
            "preview_source_account_id": preview.get("preview_source_account_id"),
            "preview_destination_account_id": preview.get("preview_destination_account_id"),
            "preview_counterparty": preview.get("preview_counterparty", ""),
            "preview_payment_method": preview.get("preview_payment_method", ""),
            "preview_description": preview.get("preview_description", ""),
            "preview_parser_id": preview.get("preview_parser_id", ""),
            "preview_parser_tags": list(preview.get("preview_parser_tags") or []),
            "preview_recurring_id": preview.get("preview_recurring_id"),
            "preview_recurring_name": preview.get("preview_recurring_name", ""),
            "preview_recurring_candidate_count": preview.get("preview_recurring_candidate_count", 0),
            "preview_recurring_match_score": preview.get("preview_recurring_match_score", 0),
            "preview_recurring_match_reasons": preview.get("preview_recurring_match_reasons", ""),
            "preview_recurring_matched_date": preview.get("preview_recurring_matched_date", ""),
            "preview_selected": bool(preview.get("preview_selected", 1)),
            "preview_is_manually_annotated": is_manually_annotated,
            "dedup_type": preview.get("dedup_type", ""),
            "dedup_source_ids": preview.get("dedup_source_ids", ""),
            "suggested_preview_type": transfer_suggestion.get("suggested_preview_type", ""),
            "transfer_suggestion_score": transfer_suggestion.get("score", 0.0),
            "transfer_suggestion_level": transfer_suggestion.get("level", ""),
            "transfer_suggestion_reason": transfer_suggestion.get("reason", ""),
            "investment_signal_score": 0.0,
            "investment_signal_level": "",
            "investment_signal_reason": "",
            "investment_platform": "",
            "investment_product": "",
            "learning_recommendation_rule_id": learning_recommendation.get("rule_id"),
            "learning_recommendation_score": learning_recommendation.get("score", 0.0),
            "learning_recommendation_level": learning_recommendation.get("level", ""),
            "learning_recommendation_reason": learning_recommendation.get("reason", ""),
            "learning_recommendation_type": learning_recommendation.get("recommended_type", ""),
            "learning_recommendation_summary": learning_recommendation.get("summary", ""),
            "learning_recommendation_mode": learning_recommendation.get("mode", ""),
            "learning_recommendation_source": learning_recommendation.get("source", ""),
            "learning_recommendation_model_version": learning_recommendation.get("model_version", ""),
            "reconciliation_candidates": reconciliation_candidates,
            "matching": build_preview_matching_payload(
                preview,
                transfer_suggestion=transfer_suggestion,
                learning_recommendation=learning_recommendation,
                matching_feedback=matching_feedback,
                is_manually_annotated=is_manually_annotated,
                reconciliation_candidates=reconciliation_candidates,
            ),
        }

    @staticmethod
    def _map_import_preview_type_to_frontend_value(preview_type: str | None) -> int:
        normalized_preview_type = str(preview_type or "").strip()
        if normalized_preview_type in {"收入", "income"}:
            return int(TransactionType.INCOME)
        if normalized_preview_type in {"支出", "expense"}:
            return int(TransactionType.EXPENSE)
        if normalized_preview_type in {"转账", "transfer"}:
            return int(TransactionType.TRANSFER)
        if normalized_preview_type in {"投资", "investment"}:
            return int(TransactionType.INVESTMENT)
        return 1

    @staticmethod
    def _resolve_import_preview_transfer_signal_status(preview_item: dict[str, Any]) -> str | None:
        matching = dict(preview_item.get("matching") or {})
        transfer_matching = dict(matching.get("transfer") or {})
        review_status = str(transfer_matching.get("review_status") or "").strip().lower()
        if review_status in {"accepted", "rejected"}:
            return review_status

        suggested_preview_type = str(preview_item.get("suggested_preview_type") or "").strip().lower()
        preview_type = str(preview_item.get("preview_type") or "").strip().lower()
        transfer_score = float(preview_item.get("transfer_suggestion_score") or 0)
        transfer_suppressed = bool(transfer_matching.get("suppressed"))
        if (
            suggested_preview_type in {"转账", "transfer"}
            and preview_type not in {"转账", "transfer"}
            and transfer_score > 0
            and not transfer_suppressed
        ):
            return "pending"

        return None

    @staticmethod
    def _resolve_import_preview_learning_signal_status(preview_item: dict[str, Any]) -> str | None:
        matching = dict(preview_item.get("matching") or {})
        learning_matching = dict(matching.get("learning") or {})
        review_status = str(learning_matching.get("review_status") or "").strip().lower()
        if review_status in {"accepted", "rejected"}:
            return review_status

        try:
            learning_rule_id = int(learning_matching.get("rule_id") or 0)
        except (TypeError, ValueError):
            learning_rule_id = 0

        has_pending_learning = (
            float(preview_item.get("learning_recommendation_score") or 0) > 0
            or bool(str(preview_item.get("learning_recommendation_summary") or "").strip())
            or bool(str(preview_item.get("learning_recommendation_reason") or "").strip())
            or bool(str(preview_item.get("learning_recommendation_type") or "").strip())
            or learning_rule_id > 0
        ) and not bool(learning_matching.get("suppressed"))

        return "pending" if has_pending_learning else None
