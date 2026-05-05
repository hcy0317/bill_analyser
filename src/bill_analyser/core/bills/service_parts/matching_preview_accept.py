"""Preview-family matching accept handlers and candidate validation helpers."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    log_method,
)

class MatchingPreviewAcceptMixin:
    """Preview-family matching accept handlers and candidate validation helpers."""

    @log_method
    async def _accept_preview_transfer_candidate(
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
            "accept",
            **transfer_kwargs,
        )
        if not result.get("success"):
            return result
        action_result = {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
        }
        if isinstance(result.get("preview"), list):
            action_result["preview"] = list(result.get("preview") or [])
        if isinstance(result.get("preview_item"), dict):
            action_result["preview_item"] = dict(result.get("preview_item") or {})
        return action_result

    @staticmethod
    def _normalize_preview_recurring_id(raw_recurring_id: Any) -> int:
        if isinstance(raw_recurring_id, bool):
            raise ValueError("Invalid recurringId")

        if isinstance(raw_recurring_id, int):
            normalized_recurring_id = raw_recurring_id
        elif isinstance(raw_recurring_id, str):
            stripped_recurring_id = raw_recurring_id.strip()
            if not stripped_recurring_id or not stripped_recurring_id.isdigit():
                raise ValueError("Invalid recurringId")
            normalized_recurring_id = int(stripped_recurring_id)
        else:
            raise ValueError("Invalid recurringId")

        if normalized_recurring_id <= 0:
            raise ValueError("Invalid recurringId")

        return normalized_recurring_id

    @staticmethod
    def _normalize_preview_learning_rule_id(raw_rule_id: Any) -> int:
        if isinstance(raw_rule_id, bool):
            raise ValueError("Invalid ruleId")

        if isinstance(raw_rule_id, int):
            normalized_rule_id = raw_rule_id
        elif isinstance(raw_rule_id, str):
            stripped_rule_id = raw_rule_id.strip()
            if not stripped_rule_id or not stripped_rule_id.isdigit():
                raise ValueError("Invalid ruleId")
            normalized_rule_id = int(stripped_rule_id)
        else:
            raise ValueError("Invalid ruleId")

        if normalized_rule_id <= 0:
            raise ValueError("Invalid ruleId")

        return normalized_rule_id

    @staticmethod
    def _normalize_preview_learning_account_id(raw_account_id: Any) -> int | None:
        if raw_account_id in (None, "", 0, "0"):
            return None
        if isinstance(raw_account_id, bool):
            raise ValueError("Invalid accountId")

        if isinstance(raw_account_id, int):
            normalized_account_id = raw_account_id
        elif isinstance(raw_account_id, str):
            stripped_account_id = raw_account_id.strip()
            if not stripped_account_id or not stripped_account_id.isdigit():
                raise ValueError("Invalid accountId")
            normalized_account_id = int(stripped_account_id)
        else:
            raise ValueError("Invalid accountId")

        if normalized_account_id <= 0:
            raise ValueError("Invalid accountId")

        return normalized_account_id

    @staticmethod
    def _extract_preview_learning_model_metadata(payload: dict[str, Any]) -> dict[str, Any]:
        metadata: dict[str, Any] = {}
        model_version = payload.get("modelVersion")
        if model_version is None:
            model_version = payload.get("model_version")
        if model_version is not None:
            metadata["model_version"] = model_version

        dataset_snapshot_id = payload.get("datasetSnapshotId")
        if dataset_snapshot_id is None:
            dataset_snapshot_id = payload.get("dataset_snapshot_id")
        if dataset_snapshot_id is not None:
            metadata["dataset_snapshot_id"] = dataset_snapshot_id
        return metadata

    @classmethod
    def _learning_model_recommendation_references_available(
        cls,
        recommendation: dict[str, Any],
        *,
        categories_by_id: dict[int, dict[str, Any]],
        accounts_by_id: dict[int, dict[str, Any]],
    ) -> bool:
        for key, lookup in (
            ("category_id", categories_by_id),
            ("source_account_id", accounts_by_id),
            ("destination_account_id", accounts_by_id),
        ):
            raw_value = recommendation.get(key)
            if raw_value in (None, "", 0, "0"):
                continue
            normalized_id = cls._coerce_optional_positive_int(raw_value)
            if normalized_id is None or normalized_id not in lookup:
                return False
        return True

    @classmethod
    def _validate_preview_learning_model_candidate(
        cls,
        recommendation: dict[str, Any],
        *,
        model_version: Any | None,
        dataset_snapshot_id: Any | None,
        require_model_version: bool,
    ) -> dict[str, Any] | None:
        expected_model_version = str(model_version or "").strip()
        live_source = str(recommendation.get("source") or "").strip().lower()
        if expected_model_version and live_source != "model":
            return {
                "success": False,
                "error": "Learning candidate changed, please refresh",
                "status_code": 409,
            }
        if live_source != "model":
            return None

        live_model_version = str(recommendation.get("model_version") or "").strip()
        if require_model_version and not expected_model_version:
            return {
                "success": False,
                "error": "Learning candidate changed, please refresh",
                "status_code": 409,
            }
        if expected_model_version and expected_model_version != live_model_version:
            return {
                "success": False,
                "error": "Learning candidate changed, please refresh",
                "status_code": 409,
            }

        if dataset_snapshot_id not in (None, ""):
            expected_snapshot_id = cls._coerce_optional_positive_int(dataset_snapshot_id)
            live_snapshot_id = cls._coerce_optional_positive_int(
                recommendation.get("dataset_snapshot_id")
            )
            if expected_snapshot_id is None or expected_snapshot_id != live_snapshot_id:
                return {
                    "success": False,
                    "error": "Learning candidate changed, please refresh",
                    "status_code": 409,
                }
        return None

    @log_method
    async def _accept_preview_recurring_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        raw_recurring_id = payload.get("recurringId")
        if raw_recurring_id in (None, ""):
            return {"success": False, "error": "Missing recurringId", "status_code": 400}

        try:
            recurring_id = self._normalize_preview_recurring_id(raw_recurring_id)
        except ValueError:
            return {"success": False, "error": "Invalid request", "status_code": 400}

        recurring_kwargs: dict[str, Any] = {
            "expected_state": payload.get("expectedState"),
            "user_id": user_id,
        }
        if payload.get("responseMode") is not None:
            recurring_kwargs["response_mode"] = payload.get("responseMode")
        result = await self.update_preview_recurring_match(
            int(parsed_candidate_id["preview_id"]),
            recurring_id,
            **recurring_kwargs,
        )
        if not result.get("success"):
            return result
        action_result = {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
            "recurring_id": result.get("recurring_id"),
        }
        if isinstance(result.get("preview"), list):
            action_result["preview"] = list(result.get("preview") or [])
        if isinstance(result.get("preview_item"), dict):
            action_result["preview_item"] = dict(result.get("preview_item") or {})
        return action_result

    @log_method
    async def _accept_preview_investment_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        result = await self.apply_preview_investment_decision(
            int(parsed_candidate_id["preview_id"]),
            "accept",
            expected_state=payload.get("expectedState"),
            user_id=user_id,
        )
        if not result.get("success"):
            return result
        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
            "review_status": result.get("review_status"),
            "suppressed": result.get("suppressed"),
        }

    @log_method
    async def _accept_preview_learning_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        payload: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        learning_kwargs: dict[str, Any] = {
            "expected_state": payload.get("expectedState"),
            "rule_id": payload.get("ruleId"),
            "user_id": user_id,
        }
        learning_kwargs.update(self._extract_preview_learning_model_metadata(payload))
        if payload.get("responseMode") is not None:
            learning_kwargs["response_mode"] = payload.get("responseMode")
        result = await self.apply_preview_learning_decision(
            int(parsed_candidate_id["preview_id"]),
            "accept",
            **learning_kwargs,
        )
        if not result.get("success"):
            return result
        action_result = {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "preview_id": result.get("preview_id"),
            "session_id": result.get("session_id"),
        }
        if isinstance(result.get("preview"), list):
            action_result["preview"] = list(result.get("preview") or [])
        if isinstance(result.get("preview_item"), dict):
            action_result["preview_item"] = dict(result.get("preview_item") or {})
        return action_result
