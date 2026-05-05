"""Formal/reconciliation matching actions, generic dispatch, and manual pair writes."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    ReconciliationProjectionConflictError,
    log_method,
    parse_matching_candidate_id,
)

class MatchingActionsMixin:
    """Formal/reconciliation matching actions, generic dispatch, and manual pair writes."""

    @log_method
    async def _accept_bill_transfer_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        result = await self.create_manual_transfer_pair(
            int(parsed_candidate_id["bill_id"]),
            int(parsed_candidate_id["candidate_bill_id"]),
            user_id=user_id,
            feedback_candidate_id=str(candidate_id),
        )
        if not result.get("success"):
            return result
        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "pair": result.get("pair"),
        }

    @log_method
    async def _accept_bill_investment_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        result = await self.create_manual_investment_pair(
            int(parsed_candidate_id["bill_id"]),
            int(parsed_candidate_id["candidate_bill_id"]),
            user_id=user_id,
            feedback_candidate_id=str(candidate_id),
        )
        if not result.get("success"):
            return result

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "pair": result.get("pair"),
        }

    @log_method
    async def _accept_bill_learning_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        bill_id = int(parsed_candidate_id["bill_id"])
        rule_id = int(parsed_candidate_id["rule_id"])
        bill_candidates = await self.get_matching_bill_candidates(bill_id, user_id=user_id)
        if not bill_candidates.get("success"):
            return bill_candidates

        candidate_ids = {
            str(candidate.get("candidate_id") or "") for candidate in list(bill_candidates.get("candidates") or [])
        }
        if str(candidate_id) not in candidate_ids:
            return {
                "success": False,
                "error": "Learning candidate not available",
                "status_code": 400,
            }

        try:
            result = await self.db.accept_bill_learning_candidate(
                bill_id,
                rule_id,
                user_id=user_id,
                expected_rule_revision=parsed_candidate_id.get("rule_revision"),
            )
        except LookupError:
            return {"success": False, "error": "Bill not found", "status_code": 404}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 400}

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "bill": result.get("bill"),
        }

    @log_method
    async def _accept_reconciliation_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        if str(parsed_candidate_id.get("kind") or "") not in {"transfer", "duplicate"}:
            return {"success": False, "error": "Invalid candidateId", "status_code": 400}
        try:
            result = await self.db.accept_import_reconciliation_candidate(
                candidate_id,
                user_id=user_id,
            )
        except LookupError:
            return {
                "success": False,
                "error": "Reconciliation candidate not found",
                "status_code": 404,
            }
        except ReconciliationProjectionConflictError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 400}

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "accept",
            "bill": result.get("bill"),
            "projection": result.get("projection"),
        }

    @log_method
    async def _reject_bill_transfer_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        try:
            await self.db.reject_bill_transfer_candidate(
                int(parsed_candidate_id["bill_id"]),
                int(parsed_candidate_id["candidate_bill_id"]),
                user_id=user_id,
                feedback_candidate_id=str(candidate_id),
            )
        except LookupError:
            return {"success": False, "error": "Bill not found", "status_code": 404}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
        }

    @log_method
    async def _reject_bill_investment_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        try:
            await self.db.reject_bill_investment_candidate(
                int(parsed_candidate_id["bill_id"]),
                int(parsed_candidate_id["candidate_bill_id"]),
                user_id=user_id,
                feedback_candidate_id=str(candidate_id),
            )
        except LookupError:
            return {"success": False, "error": "Bill not found", "status_code": 404}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
        }

    @log_method
    async def _reject_bill_learning_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        bill_id = int(parsed_candidate_id["bill_id"])
        rule_id = int(parsed_candidate_id["rule_id"])
        bill_candidates = await self.get_matching_bill_candidates(bill_id, user_id=user_id)
        if not bill_candidates.get("success"):
            return bill_candidates

        candidate_ids = {
            str(candidate.get("candidate_id") or "") for candidate in list(bill_candidates.get("candidates") or [])
        }
        if str(candidate_id) not in candidate_ids:
            return {
                "success": False,
                "error": "Learning candidate not available",
                "status_code": 400,
            }

        try:
            await self.db.reject_bill_learning_candidate(
                bill_id,
                rule_id,
                user_id=user_id,
                expected_rule_revision=parsed_candidate_id.get("rule_revision"),
            )
        except LookupError:
            return {
                "success": False,
                "error": "Learning candidate not available",
                "status_code": 400,
            }

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
        }

    @log_method
    async def _reject_reconciliation_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        if str(parsed_candidate_id.get("kind") or "") not in {"transfer", "duplicate"}:
            return {"success": False, "error": "Invalid candidateId", "status_code": 400}
        try:
            result = await self.db.reject_import_reconciliation_candidate(
                candidate_id,
                user_id=user_id,
            )
        except LookupError:
            return {
                "success": False,
                "error": "Reconciliation candidate not found",
                "status_code": 404,
            }
        except ReconciliationProjectionConflictError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 400}

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "reject",
            "projection": result.get("projection"),
        }

    @log_method
    async def _clear_reconciliation_candidate(
        self,
        candidate_id: str,
        parsed_candidate_id: dict[str, Any],
        *,
        user_id: int = 1,
    ) -> dict[str, Any]:
        if str(parsed_candidate_id.get("kind") or "") not in {"transfer", "duplicate"}:
            return {"success": False, "error": "Invalid candidateId", "status_code": 400}
        try:
            result = await self.db.clear_import_reconciliation_candidate(
                candidate_id,
                user_id=user_id,
            )
        except LookupError:
            return {
                "success": False,
                "error": "Reconciliation candidate not found",
                "status_code": 404,
            }
        except ReconciliationProjectionConflictError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 400}

        return {
            "success": True,
            "candidate_id": str(candidate_id),
            "action": "clear",
            "projection": result.get("projection"),
        }

    @log_method
    async def _accept_matching_candidate(
        self,
        candidate_id: str,
        payload: dict[str, Any] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Accept a supported matching candidate by dispatching to existing write paths."""
        if not isinstance(payload, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        parsed_candidate_id = parse_matching_candidate_id(candidate_id)
        if not isinstance(parsed_candidate_id, dict):
            return {"success": False, "error": "Invalid candidateId", "status_code": 400}

        candidate_scope = str(parsed_candidate_id.get("scope") or "")
        candidate_kind = str(parsed_candidate_id.get("kind") or "")

        if candidate_scope == "reconciliation":
            return await self._accept_reconciliation_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "transfer":
            return await self._accept_preview_transfer_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "recurring":
            return await self._accept_preview_recurring_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "learning":
            return await self._accept_preview_learning_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        if candidate_scope == "bill" and candidate_kind == "transfer":
            return await self._accept_bill_transfer_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "bill" and candidate_kind == "investment":
            return await self._accept_bill_investment_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "bill" and candidate_kind == "learning":
            return await self._accept_bill_learning_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        return {
            "success": False,
            "error": "Candidate family not supported",
            "status_code": 400,
        }

    @log_method
    async def _reject_matching_candidate(
        self,
        candidate_id: str,
        payload: dict[str, Any] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Reject a supported matching candidate by dispatching to existing write paths."""
        if not isinstance(payload, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        parsed_candidate_id = parse_matching_candidate_id(candidate_id)
        if not isinstance(parsed_candidate_id, dict):
            return {"success": False, "error": "Invalid candidateId", "status_code": 400}

        candidate_scope = str(parsed_candidate_id.get("scope") or "")
        candidate_kind = str(parsed_candidate_id.get("kind") or "")

        if candidate_scope == "reconciliation":
            return await self._reject_reconciliation_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "transfer":
            return await self._reject_preview_transfer_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "learning":
            return await self._reject_preview_learning_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "recurring":
            return await self._reject_preview_recurring_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        if candidate_scope == "bill" and candidate_kind == "transfer":
            return await self._reject_bill_transfer_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "bill" and candidate_kind == "investment":
            return await self._reject_bill_investment_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "bill" and candidate_kind == "learning":
            return await self._reject_bill_learning_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        return {
            "success": False,
            "error": "Candidate family not supported",
            "status_code": 400,
        }

    @log_method
    async def _clear_matching_candidate(
        self,
        candidate_id: str,
        payload: dict[str, Any] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Clear a supported matching candidate by dispatching to existing write paths."""
        if not isinstance(payload, dict):
            return {"success": False, "error": "Invalid request", "status_code": 400}

        parsed_candidate_id = parse_matching_candidate_id(candidate_id)
        if not isinstance(parsed_candidate_id, dict):
            return {"success": False, "error": "Invalid candidateId", "status_code": 400}

        candidate_scope = str(parsed_candidate_id.get("scope") or "")
        candidate_kind = str(parsed_candidate_id.get("kind") or "")

        if candidate_scope == "reconciliation":
            return await self._clear_reconciliation_candidate(
                candidate_id,
                parsed_candidate_id,
                user_id=user_id,
            )

        if candidate_scope == "preview" and candidate_kind == "learning":
            if not isinstance(payload.get("expectedState"), dict):
                return {"success": False, "error": "Invalid request", "status_code": 400}
            return await self._clear_preview_learning_candidate(
                candidate_id,
                parsed_candidate_id,
                payload,
                user_id=user_id,
            )

        return {
            "success": False,
            "error": "Candidate family not supported",
            "status_code": 400,
        }

    @log_method
    async def create_manual_transfer_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
        *,
        feedback_candidate_id: str | None = None,
    ) -> dict[str, Any]:
        """Persist a manual transfer pair for two historical bills."""
        if int(bill_id) == int(candidate_bill_id):
            return {
                "success": False,
                "error": "billId and candidateBillId must be different",
                "status_code": 400,
            }

        try:
            pair = await self.db.create_manual_transfer_pair(
                bill_id,
                candidate_bill_id,
                user_id=user_id,
                feedback_candidate_id=feedback_candidate_id,
            )
        except LookupError:
            return {"success": False, "error": "Bill not found", "status_code": 404}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}
        return {"success": True, "pair": pair}

    @log_method
    async def create_manual_investment_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
        *,
        feedback_candidate_id: str | None = None,
    ) -> dict[str, Any]:
        """Persist a manual investment pair for two historical bills."""
        if int(bill_id) == int(candidate_bill_id):
            return {
                "success": False,
                "error": "billId and candidateBillId must be different",
                "status_code": 400,
            }

        try:
            pair = await self.db.create_manual_investment_pair(
                bill_id,
                candidate_bill_id,
                user_id=user_id,
                feedback_candidate_id=feedback_candidate_id,
            )
        except LookupError:
            return {"success": False, "error": "Bill not found", "status_code": 404}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}
        return {"success": True, "pair": pair}

    @log_method
    async def delete_manual_transfer_pair(
        self,
        pair_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        """Delete a persisted manual pair for historical bills."""
        try:
            pair = await self.db.delete_manual_pair(pair_id, user_id=user_id)
        except LookupError:
            return {"success": False, "error": "Pair not found", "status_code": 404}
        except ValueError as exc:
            return {"success": False, "error": str(exc), "status_code": 409}

        return {"success": True, "pair": pair}
