"""Preview learning recommendation application and training feedback writes."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    log_method,
)

class PreviewLearningDecisionsMixin:
    """Preview learning recommendation application and training feedback writes."""

    async def _build_learning_recommendation_from_preview(
        self,
        preview: dict[str, Any],
        *,
        user_id: int = 1,
        projection_context: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        preview_user_id = int(preview.get("user_id") or user_id or 1)
        if preview_user_id <= 0:
            return {}

        session_id = str(preview.get("session_id") or "")
        context = projection_context
        context_session_id = (
            str(context.get("session_id") or "") if isinstance(context, dict) else ""
        )
        context_user_id = int(context.get("user_id") or 0) if isinstance(context, dict) else 0
        if not isinstance(context, dict) or context_user_id != preview_user_id or (
            session_id and context_session_id != session_id
        ):
            context = await self._load_import_preview_projection_context(
                session_id,
                user_id=preview_user_id,
            )

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
        if learning_recommendation:
            return learning_recommendation

        is_model_suppressed = preview_id in set(context.get("learning_model_suppressed_preview_ids") or set())
        return self._build_learning_model_signal_from_preview(
            preview,
            None if is_model_suppressed else context.get("active_learning_model"),
            learning_rules=learning_rules,
            categories_by_id=categories_by_id,
            accounts_by_id=accounts_by_id,
            is_manually_annotated=is_manually_annotated,
        )

    async def _build_preview_learning_apply_payload(
        self,
        rule: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        applied_result: dict[str, Any] = {"rule_id": int(rule.get("id") or 0)}

        learned_type = str(rule.get("learned_type") or "").strip()
        if learned_type:
            applied_result["preview_type"] = learned_type

        learned_category_id = rule.get("learned_category_id")
        if learned_category_id not in (None, "", 0, "0"):
            category = await self.db.get_category_by_id(
                int(learned_category_id),
                user_id=user_id,
            )
            if category:
                applied_result["preview_main_category"] = str(category.get("main_category") or "")
                applied_result["preview_sub_category"] = str(category.get("sub_category") or "")

        learned_source_account_id = rule.get("learned_source_account_id")
        if learned_source_account_id not in (None, "", 0, "0"):
            source_account = await self.db.get_account_by_id(
                int(learned_source_account_id),
                user_id=user_id,
            )
            if source_account and source_account.get("id") not in (None, ""):
                applied_result["preview_source_account_id"] = int(source_account.get("id") or 0)

        learned_destination_account_id = rule.get("learned_destination_account_id")
        if learned_destination_account_id not in (None, "", 0, "0"):
            destination_account = await self.db.get_account_by_id(
                int(learned_destination_account_id),
                user_id=user_id,
            )
            if destination_account and destination_account.get("id") not in (None, ""):
                applied_result["preview_destination_account_id"] = int(
                    destination_account.get("id") or 0
                )

        return applied_result

    async def _record_preview_learning_decision_training_sample(
        self,
        preview: dict[str, Any],
        *,
        decision: str,
        user_id: int,
        model_version: str = "",
    ) -> None:
        session_id = str(preview.get("session_id") or "")
        preview_id = self._coerce_optional_positive_int(preview.get("id"))
        if not session_id or preview_id is None:
            return

        category_id = await self._get_preview_category_id(preview, user_id=user_id)
        await self.db.save_import_annotation_samples(
            session_id,
            [
                {
                    "id": preview_id,
                    "preview_type": preview.get("preview_type"),
                    "category_id": category_id,
                    "preview_source_account_id": preview.get("preview_source_account_id"),
                    "preview_destination_account_id": preview.get("preview_destination_account_id"),
                }
            ],
            user_id=user_id,
        )
        if hasattr(self.db, "record_import_learning_feedback_event"):
            await self.db.record_import_learning_feedback_event(
                f"model_preview_{decision}",
                user_id=user_id,
                session_id=session_id,
                preview_id=preview_id,
                candidate_id=f"model:{model_version}:preview:{preview_id}" if model_version else None,
                payload={
                    "decision": decision,
                    "model_version": model_version,
                    "corrective_sample": decision == "reject",
                },
            )

    @log_method
    async def apply_preview_learning_decision(  # pylint: disable=too-many-locals,too-many-return-statements,too-many-boolean-expressions
        self,
        preview_id: int,
        decision: str,
        expected_state: dict[str, Any] | None = None,
        response_mode: str | None = None,
        rule_id: Any | None = None,
        model_version: Any | None = None,
        dataset_snapshot_id: Any | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Persist a preview-scoped learning decision and return refreshed preview data."""
        normalized_decision = str(decision or "").strip().lower()
        if normalized_decision not in {"accept", "reject", "clear"}:
            return {"success": False, "error": "Invalid decision", "status_code": 400}

        if not isinstance(expected_state, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        preview = await self.db.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        preview_user_id = int(preview.get("user_id") or user_id or 1)
        projection_context = await self._load_import_preview_projection_context(
            str(preview.get("session_id") or ""),
            user_id=preview_user_id,
        )
        learning_recommendation = await self._build_learning_recommendation_from_preview(
            preview,
            user_id=preview_user_id,
            projection_context=projection_context,
        )
        current_learning_review_status = await self._get_preview_learning_review_status(
            preview,
            user_id=preview_user_id,
            projection_context=projection_context,
            learning_recommendation=learning_recommendation,
        )
        current_preview_category_id = await self._get_preview_category_id(preview, user_id=user_id)
        current_preview_recurring_id = preview.get("preview_recurring_id")
        current_preview_recurring_id = (
            None if current_preview_recurring_id in (None, "") else int(current_preview_recurring_id)
        )
        current_preview_source_account_id = self._normalize_preview_learning_account_id(
            preview.get("preview_source_account_id")
        )
        current_preview_destination_account_id = self._normalize_preview_learning_account_id(
            preview.get("preview_destination_account_id")
        )

        expected_session_id = str(expected_state.get("sessionId") or "")
        expected_review_status = str(expected_state.get("reviewStatus") or "").strip().lower()
        expected_preview_type = str(expected_state.get("previewType") or "")
        try:
            expected_category_id_raw = expected_state.get("categoryId")
            expected_category_id = (
                None if expected_category_id_raw in (None, "", 0, "0") else int(expected_category_id_raw)
            )
            expected_recurring_id_raw = expected_state.get("recurringId")
            expected_recurring_id = (
                None if expected_recurring_id_raw in (None, "", 0, "0") else int(expected_recurring_id_raw)
            )
            source_account_state_required = (
                normalized_decision == "accept"
                or current_learning_review_status == "accepted"
                or "sourceAccountId" in expected_state
            )
            destination_account_state_required = (
                normalized_decision == "accept"
                or current_learning_review_status == "accepted"
                or "destinationAccountId" in expected_state
            )
            expected_source_account_id = (
                self._normalize_preview_learning_account_id(expected_state.get("sourceAccountId"))
                if source_account_state_required
                else current_preview_source_account_id
            )
            expected_destination_account_id = (
                self._normalize_preview_learning_account_id(expected_state.get("destinationAccountId"))
                if destination_account_state_required
                else current_preview_destination_account_id
            )
        except (TypeError, ValueError):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        if (
            str(preview.get("session_id") or "") != expected_session_id
            or current_learning_review_status != expected_review_status
            or str(preview.get("preview_type") or "") != expected_preview_type
            or current_preview_category_id != expected_category_id
            or current_preview_recurring_id != expected_recurring_id
            or current_preview_source_account_id != expected_source_account_id
            or current_preview_destination_account_id != expected_destination_account_id
        ):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}

        model_candidate_conflict = self._validate_preview_learning_model_candidate(
            learning_recommendation,
            model_version=model_version,
            dataset_snapshot_id=dataset_snapshot_id,
            require_model_version=normalized_decision in {"accept", "reject"},
        )
        if model_candidate_conflict:
            return model_candidate_conflict

        if str(learning_recommendation.get("source") or "").strip().lower() == "model" and not (
            self._learning_model_recommendation_references_available(
                learning_recommendation,
                categories_by_id=dict(projection_context.get("learning_categories_by_id") or {}),
                accounts_by_id=dict(projection_context.get("learning_accounts_by_id") or {}),
            )
        ):
            return {
                "success": False,
                "error": "Learning candidate not available",
                "status_code": 409,
            }

        learning_feedback = preview.get("preview_matching_feedback", {}).get("learning")
        has_existing_learning_review = current_learning_review_status in {"accepted", "rejected"}
        retained_learning_rule_id = None
        if isinstance(learning_feedback, dict) and learning_feedback.get("rule_id") not in (None, ""):
            try:
                retained_learning_rule_id = self._normalize_preview_learning_rule_id(
                    learning_feedback.get("rule_id")
                )
            except ValueError:
                retained_learning_rule_id = None

        applied_result: dict[str, Any] | None = None
        if normalized_decision == "accept":
            live_learning_rule_id = int(learning_recommendation.get("rule_id") or 0)
            if str(learning_recommendation.get("source") or "") == "model":
                if rule_id not in (None, ""):
                    return {
                        "success": False,
                        "error": "Learning candidate not available",
                        "status_code": 400,
                    }
                applied_result = self._build_preview_learning_model_apply_payload(learning_recommendation)
            else:
                if rule_id in (None, ""):
                    return {"success": False, "error": "Missing ruleId", "status_code": 400}

                try:
                    normalized_rule_id = self._normalize_preview_learning_rule_id(rule_id)
                except ValueError:
                    return {"success": False, "error": "Invalid request", "status_code": 400}

                if live_learning_rule_id > 0:
                    if live_learning_rule_id != normalized_rule_id:
                        return {
                            "success": False,
                            "error": "Learning candidate not available",
                            "status_code": 400,
                        }
                elif retained_learning_rule_id != normalized_rule_id:
                    return {
                        "success": False,
                        "error": "Learning candidate not available",
                        "status_code": 400,
                    }

                learning_rule = await self.db.get_import_learning_rule_by_id(
                    normalized_rule_id,
                    user_id=user_id,
                )
                if not learning_rule:
                    return {
                        "success": False,
                        "error": "Learning candidate not available",
                        "status_code": 400,
                    }

                applied_result = await self._build_preview_learning_apply_payload(
                    learning_rule,
                    user_id=user_id,
                )
        elif normalized_decision != "clear" and not has_existing_learning_review:
            if not learning_recommendation:
                return {
                    "success": False,
                    "error": "Learning candidate not available",
                    "status_code": 400,
                }

            live_learning_rule_id = int(learning_recommendation.get("rule_id") or 0)
            if live_learning_rule_id > 0:
                applied_result = {"rule_id": live_learning_rule_id}
        elif retained_learning_rule_id is not None:
            applied_result = {"rule_id": retained_learning_rule_id}

        updated_preview = await self.db.update_preview_learning_decision(
            preview_id,
            normalized_decision,
            user_id=user_id,
            expected_state={
                "session_id": str(preview.get("session_id") or ""),
                "preview_type": str(preview.get("preview_type") or ""),
                "preview_main_category": str(preview.get("preview_main_category") or ""),
                "preview_sub_category": str(preview.get("preview_sub_category") or ""),
                "preview_recurring_id": current_preview_recurring_id,
                "preview_source_account_id": current_preview_source_account_id,
                "preview_destination_account_id": current_preview_destination_account_id,
                "preview_matching_feedback_json": str(preview.get("preview_matching_feedback_json") or ""),
            },
            applied_result=applied_result,
        )
        if updated_preview and updated_preview.get("_state_conflict"):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}
        if not updated_preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        session_id = str(updated_preview.get("session_id") or "")
        if normalized_decision in {"accept", "reject"}:
            await self._record_preview_learning_decision_training_sample(
                updated_preview,
                decision=normalized_decision,
                user_id=int(updated_preview.get("user_id") or user_id or 1),
                model_version=str(learning_recommendation.get("model_version") or ""),
            )
        elif normalized_decision == "clear" and isinstance(projection_context, dict):
            suppressed_preview_ids = set(projection_context.get("learning_model_suppressed_preview_ids") or set())
            suppressed_preview_ids.add(preview_id)
            projection_context = {
                **projection_context,
                "learning_model_suppressed_preview_ids": suppressed_preview_ids,
            }

        if str(response_mode or "").strip().lower() == "preview-item":
            return {
                "success": True,
                "preview_id": preview_id,
                "session_id": session_id,
                "decision": normalized_decision,
                "preview_item": await self._build_import_preview_item(
                    updated_preview,
                    user_id=int(updated_preview.get("user_id") or user_id or 1),
                    projection_context=projection_context,
                ),
            }

        refreshed_preview = await self.get_import_preview(session_id, selected_only=False, user_id=user_id)
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": session_id,
            "decision": normalized_decision,
            "preview": refreshed_preview,
        }
