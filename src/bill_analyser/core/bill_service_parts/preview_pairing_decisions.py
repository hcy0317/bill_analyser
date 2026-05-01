"""Preview transfer, investment, and recurring decision write paths."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    log_method,
)

class PreviewPairingDecisionsMixin:
    """Preview transfer, investment, and recurring decision write paths."""

    @log_method
    async def apply_preview_transfer_decision(
        self,
        preview_id: int,
        decision: str,
        expected_state: dict[str, Any] | None = None,
        response_mode: str | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Persist a preview-scoped transfer suggestion decision and return refreshed preview data."""
        normalized_decision = str(decision or "").strip().lower()
        if normalized_decision not in {"accept", "reject", "clear"}:
            return {"success": False, "error": "Invalid decision", "status_code": 400}

        if not isinstance(expected_state, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        preview = await self.db.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        current_transfer_review_status = await self._get_preview_transfer_review_status(preview)
        current_preview_category_id = await self._get_preview_category_id(preview, user_id=user_id)
        current_preview_recurring_id = preview.get("preview_recurring_id")
        current_preview_recurring_id = (
            None if current_preview_recurring_id in (None, "") else int(current_preview_recurring_id)
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
        except (TypeError, ValueError):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        if (
            str(preview.get("session_id") or "") != expected_session_id
            or current_transfer_review_status != expected_review_status
            or str(preview.get("preview_type") or "") != expected_preview_type
            or current_preview_category_id != expected_category_id
            or current_preview_recurring_id != expected_recurring_id
        ):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}

        transfer_feedback = preview.get("preview_matching_feedback", {}).get("transfer")
        has_existing_transfer_review = isinstance(transfer_feedback, dict) and str(
            transfer_feedback.get("review_status") or ""
        ).strip().lower() in {"accepted", "rejected"}
        if normalized_decision != "clear" and not has_existing_transfer_review:
            transfer_suggestion = self._build_transfer_suggestion_from_preview(preview)
            if not transfer_suggestion:
                return {"success": False, "error": "Transfer suggestion not available", "status_code": 400}

        updated_preview = await self.db.update_preview_transfer_decision(
            preview_id,
            normalized_decision,
            user_id=user_id,
            expected_state={
                "session_id": str(preview.get("session_id") or ""),
                "preview_type": str(preview.get("preview_type") or ""),
                "preview_main_category": str(preview.get("preview_main_category") or ""),
                "preview_sub_category": str(preview.get("preview_sub_category") or ""),
                "preview_recurring_id": current_preview_recurring_id,
                "preview_matching_feedback_json": str(preview.get("preview_matching_feedback_json") or ""),
            },
        )
        if updated_preview and updated_preview.get("_state_conflict"):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}
        if not updated_preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        session_id = str(updated_preview.get("session_id") or "")
        if str(response_mode or "").strip().lower() == "preview-item":
            return {
                "success": True,
                "preview_id": preview_id,
                "session_id": session_id,
                "decision": normalized_decision,
                "preview_item": await self._build_import_preview_item(
                    updated_preview,
                    user_id=int(updated_preview.get("user_id") or user_id or 1),
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

    @log_method
    async def apply_preview_investment_decision(  # pylint: disable=too-many-locals,too-many-return-statements,too-many-boolean-expressions
        self,
        preview_id: int,
        decision: str,
        expected_state: dict[str, Any] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Persist a preview-scoped investment decision and return row-level review state."""
        normalized_decision = str(decision or "").strip().lower()
        if normalized_decision not in {"accept", "reject", "clear"}:
            return {"success": False, "error": "Invalid decision", "status_code": 400}

        if not isinstance(expected_state, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        preview = await self.db.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        current_investment_review_status = self._resolve_preview_investment_review_status(preview)
        current_preview_category_id = await self._get_preview_category_id(preview, user_id=user_id)
        current_preview_recurring_id = preview.get("preview_recurring_id")
        current_preview_recurring_id = (
            None if current_preview_recurring_id in (None, "") else int(current_preview_recurring_id)
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
        except (TypeError, ValueError):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        if (
            str(preview.get("session_id") or "") != expected_session_id
            or current_investment_review_status != expected_review_status
            or str(preview.get("preview_type") or "") != expected_preview_type
            or current_preview_category_id != expected_category_id
            or current_preview_recurring_id != expected_recurring_id
        ):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}

        has_existing_investment_review = current_investment_review_status in {"accepted", "rejected"}
        if normalized_decision != "clear" and not has_existing_investment_review:
            investment_signal = self._build_investment_signal_from_preview(preview)
            if not investment_signal:
                return {
                    "success": False,
                    "error": "Investment candidate not available",
                    "status_code": 400,
                }

        updated_preview = await self.db.update_preview_investment_decision(
            preview_id,
            normalized_decision,
            user_id=user_id,
            expected_state={
                "session_id": str(preview.get("session_id") or ""),
                "preview_type": str(preview.get("preview_type") or ""),
                "preview_main_category": str(preview.get("preview_main_category") or ""),
                "preview_sub_category": str(preview.get("preview_sub_category") or ""),
                "preview_recurring_id": current_preview_recurring_id,
                "preview_matching_feedback_json": str(preview.get("preview_matching_feedback_json") or ""),
            },
        )
        if updated_preview and updated_preview.get("_state_conflict"):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}
        if not updated_preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        session_id = str(updated_preview.get("session_id") or "")
        next_review_status = self._resolve_preview_investment_review_status(updated_preview)
        investment_feedback = updated_preview.get("preview_matching_feedback", {}).get("investment")
        suppressed = (
            bool(investment_feedback.get("suppressed"))
            if isinstance(investment_feedback, dict)
            else False
        )
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": session_id,
            "decision": normalized_decision,
            "review_status": next_review_status,
            "suppressed": suppressed,
        }

    @log_method
    async def update_preview_recurring_match(
        self,
        preview_id: int,
        recurring_id: int | None,
        *,
        expected_state: dict[str, Any] | None = None,
        response_mode: str | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Persist a preview-scoped recurring choice and return refreshed preview data."""
        if not isinstance(expected_state, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        preview = await self.db.get_preview_bill_by_id(preview_id, user_id=user_id)
        if not preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        current_preview_category_id = await self._get_preview_category_id(preview, user_id=user_id)
        current_transfer_review_status = await self._get_preview_transfer_review_status(preview)
        current_preview_recurring_id = preview.get("preview_recurring_id")
        current_preview_recurring_id = (
            None if current_preview_recurring_id in (None, "") else int(current_preview_recurring_id)
        )

        if "reviewStatus" not in expected_state:
            return {"success": False, "error": "Invalid request", "status_code": 400}

        expected_session_id = str(expected_state.get("sessionId") or "")
        expected_preview_type = str(expected_state.get("previewType") or "")
        expected_review_status = str(expected_state.get("reviewStatus") or "").strip().lower()
        try:
            expected_category_id_raw = expected_state.get("categoryId")
            expected_category_id = (
                None if expected_category_id_raw in (None, "", 0, "0") else int(expected_category_id_raw)
            )
            expected_recurring_id_raw = expected_state.get("recurringId")
            expected_recurring_id = (
                None if expected_recurring_id_raw in (None, "", 0, "0") else int(expected_recurring_id_raw)
            )
        except (TypeError, ValueError):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        if (
            str(preview.get("session_id") or "") != expected_session_id
            or str(preview.get("preview_type") or "") != expected_preview_type
            or current_transfer_review_status != expected_review_status
            or current_preview_category_id != expected_category_id
            or current_preview_recurring_id != expected_recurring_id
        ):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}

        updated_preview = await self.db.update_preview_recurring_match(
            preview_id,
            recurring_id,
            user_id=user_id,
            expected_state={
                "session_id": str(preview.get("session_id") or ""),
                "preview_type": str(preview.get("preview_type") or ""),
                "preview_main_category": str(preview.get("preview_main_category") or ""),
                "preview_sub_category": str(preview.get("preview_sub_category") or ""),
                "preview_recurring_id": current_preview_recurring_id,
                "preview_matching_feedback_json": str(preview.get("preview_matching_feedback_json") or ""),
            },
        )
        if updated_preview and updated_preview.get("_state_conflict"):
            return {"success": False, "error": "Preview state changed, please refresh", "status_code": 409}
        if updated_preview and updated_preview.get("_invalid_recurring_id"):
            return {"success": False, "error": "Recurring candidate not available", "status_code": 404}
        if not updated_preview:
            return {"success": False, "error": "Preview bill not found", "status_code": 404}

        session_id = str(updated_preview.get("session_id") or "")
        normalized_recurring_id = None if recurring_id in (None, "") else int(recurring_id)
        if str(response_mode or "").strip().lower() == "preview-item":
            return {
                "success": True,
                "preview_id": preview_id,
                "session_id": session_id,
                "recurring_id": normalized_recurring_id,
                "preview_item": await self._build_import_preview_item(
                    updated_preview,
                    user_id=int(updated_preview.get("user_id") or user_id or 1),
                ),
            }

        refreshed_preview = await self.get_import_preview(session_id, selected_only=False, user_id=user_id)
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": session_id,
            "recurring_id": normalized_recurring_id,
            "preview": refreshed_preview,
        }
