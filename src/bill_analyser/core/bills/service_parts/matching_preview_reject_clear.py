"""Preview-family matching reject/clear handlers and review-state readers."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    log_method,
)

class MatchingPreviewRejectClearMixin:
    """Preview-family matching reject/clear handlers and review-state readers."""

    @log_method
    async def _reject_preview_transfer_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        transfer_kwargs: dict[str, Any] = {
            "expected_state": payload.get("expectedState"),
            "user_id": user_id,
        }
        if payload.get("responseMode") is not None:
            transfer_kwargs["response_mode"] = payload.get("responseMode")
        result = await self.apply_preview_transfer_decision(
            int(parsed_candidate_id["preview_id"]),
            "reject",
            **transfer_kwargs,
        )
        if not result.get("success"):
            return result
        action_result = {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
        }
        if isinstance(result.get("preview"), list):
            action_result["preview"] = list(result.get("preview") or [])
        if isinstance(result.get("preview_item"), dict):
            action_result["preview_item"] = dict(result.get("preview_item") or {})
        return action_result

    @log_method
    async def _reject_preview_investment_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        result = await self.apply_preview_investment_decision(
            int(parsed_candidate_id["preview_id"]),
            "reject",
            expected_state=payload.get("expectedState"),
            user_id=user_id,
        )
        if not result.get("success"):
            return result
        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
            "review_status": result.get("review_status"),
            "suppressed": result.get("suppressed"),
        }

    @log_method
    async def _reject_preview_learning_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        learning_kwargs: dict[str, Any] = {
            "expected_state": payload.get("expectedState"),
            "user_id": user_id,
        }
        learning_kwargs.update(self._extract_preview_learning_model_metadata(payload))
        if payload.get("responseMode") is not None:
            learning_kwargs["response_mode"] = payload.get("responseMode")
        result = await self.apply_preview_learning_decision(
            int(parsed_candidate_id["preview_id"]),
            "reject",
            **learning_kwargs,
        )
        if not result.get("success"):
            return result
        action_result = {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
        }
        if isinstance(result.get("preview"), list):
            action_result["preview"] = list(result.get("preview") or [])
        if isinstance(result.get("preview_item"), dict):
            action_result["preview_item"] = dict(result.get("preview_item") or {})
        return action_result

    @log_method
    async def _clear_preview_learning_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        learning_kwargs: dict[str, Any] = {
            "expected_state": payload.get("expectedState"),
            "user_id": user_id,
        }
        learning_kwargs.update(self._extract_preview_learning_model_metadata(payload))
        if payload.get("responseMode") is not None:
            learning_kwargs["response_mode"] = payload.get("responseMode")
        result = await self.apply_preview_learning_decision(
            int(parsed_candidate_id["preview_id"]),
            "clear",
            **learning_kwargs,
        )
        if not result.get("success"):
            return result
        action_result = {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "clear",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
        }
        if isinstance(result.get("preview"), list):
            action_result["preview"] = list(result.get("preview") or [])
        if isinstance(result.get("preview_item"), dict):
            action_result["preview_item"] = dict(result.get("preview_item") or {})
        return action_result

    @log_method
    async def _clear_preview_investment_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        result = await self.apply_preview_investment_decision(
            int(parsed_candidate_id["preview_id"]),
            "clear",
            expected_state=payload.get("expectedState"),
            user_id=user_id,
        )
        if not result.get("success"):
            return result
        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "clear",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
            "review_status": result.get("review_status"),
            "suppressed": result.get("suppressed"),
        }

    @log_method
    async def _reject_preview_recurring_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        result = await self.update_preview_recurring_match(
            int(parsed_candidate_id["preview_id"]),
            None,
            expected_state=payload.get("expectedState"),
            user_id=user_id,
        )
        if not result.get("success"):
            return result
        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
            "preview": result.get("preview", []),
        }

    async def _get_preview_category_id(self, preview: dict[str, Any], user_id: int = 1) -> int | None:
        main_category = str(preview.get("preview_main_category") or "")
        sub_category = str(preview.get("preview_sub_category") or "")
        if not main_category and not sub_category:
            return None

        category = await self.db.get_category_by_name(main_category, sub_category, user_id=user_id)
        if not category or category.get("id") in (None, ""):
            return None

        return int(category["id"])

    async def _get_preview_transfer_review_status(self, preview: dict[str, Any]) -> str:
        transfer_feedback = preview.get("preview_matching_feedback", {}).get("transfer")
        review_status = (
            str(transfer_feedback.get("review_status") or "").strip().lower()
            if isinstance(transfer_feedback, dict)
            else ""
        )
        if review_status in {"accepted", "rejected"}:
            return review_status

        transfer_suggestion = self._build_transfer_suggestion_from_preview(preview)
        return "pending" if transfer_suggestion else ""

    async def _get_preview_investment_review_status(
        self,
        preview: dict[str, Any],
        user_id: int = 1,
    ) -> str:
        _ = user_id
        return self._resolve_preview_investment_review_status(preview)

    def _resolve_preview_investment_review_status(
        self,
        preview: dict[str, Any],
    ) -> str:
        investment_feedback = preview.get("preview_matching_feedback", {}).get("investment")
        review_status = (
            str(investment_feedback.get("review_status") or "").strip().lower()
            if isinstance(investment_feedback, dict)
            else ""
        )
        investment_signal = self._build_investment_signal_from_preview(preview)
        if not investment_signal:
            return ""

        if review_status in {"accepted", "rejected"}:
            return review_status
        return "pending"

    async def _get_preview_learning_review_status(
        self,
        preview: dict[str, Any],
        user_id: int = 1,
        projection_context: dict[str, Any] | None = None,
        learning_recommendation: dict[str, Any] | None = None,
    ) -> str:
        learning_feedback = preview.get("preview_matching_feedback", {}).get("learning")
        review_status = (
            str(learning_feedback.get("review_status") or "").strip().lower()
            if isinstance(learning_feedback, dict)
            else ""
        )
        resolved_learning_recommendation = (
            dict(learning_recommendation)
            if isinstance(learning_recommendation, dict)
            else await self._build_learning_recommendation_from_preview(
                preview,
                user_id=user_id,
                projection_context=projection_context,
            )
        )
        feedback_rule_id = None
        if isinstance(learning_feedback, dict) and learning_feedback.get("rule_id") not in (None, ""):
            try:
                feedback_rule_id = self._normalize_preview_learning_rule_id(learning_feedback.get("rule_id"))
            except ValueError:
                feedback_rule_id = None

        live_rule_id = int(resolved_learning_recommendation.get("rule_id") or 0)
        if review_status in {"accepted", "rejected"}:
            if live_rule_id == 0:
                return review_status
            if feedback_rule_id in (None, live_rule_id):
                return review_status

        if not resolved_learning_recommendation:
            return ""

        return "pending"
