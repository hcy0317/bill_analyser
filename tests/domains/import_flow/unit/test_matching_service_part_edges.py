from __future__ import annotations

from typing import Any

import pytest

from bill_analyser.core.bills.service_parts.matching_actions import MatchingActionsMixin
from bill_analyser.core.bills.service_parts.matching_preview_accept import MatchingPreviewAcceptMixin
from bill_analyser.core.bills.service_parts.matching_preview_reject_clear import (
    MatchingPreviewRejectClearMixin,
)
from bill_analyser.core.bills.service_parts.matching_reads import MatchingReadsMixin


class FakeMatchingDB:
    def __init__(self) -> None:
        self.reject_calls: list[tuple[str, int, int]] = []

    async def get_bill_transfer_candidates(self, bill_id: int, user_id: int = 1) -> dict[str, Any]:
        if bill_id == 404:
            return {"bill": None}
        if bill_id == 10:
            return {
                "bill": {"id": 10},
                "linked_pair": {"id": 8, "pair_type": "transfer"},
                "candidates": [],
            }
        return {
            "bill": {
                "id": bill_id,
                "date": "2026-05-01 10:00:00",
                "type": "expense",
                "amount": -100.0,
                "counterparty": "Fund",
                "description": "fund subscription",
                "payment_method": "card",
                "source_account_id": 1,
            },
            "linked_pair": None,
            "candidates": [{"candidate_id": f"bill:{bill_id}:transfer:13", "kind": "transfer"}],
        }

    async def get_bill_reconciliation_projection(self, bill_id: int, user_id: int = 1) -> dict[str, Any]:
        return {"bill_id": bill_id, "status": "merged", "user_id": user_id}

    async def list_import_reconciliation_candidates(self, **kwargs: Any) -> list[dict[str, Any]]:
        return [
            {
                "candidate_id": "reconcile:import:duplicate:bill:12:abc",
                "candidate_type": "duplicate",
                "score": 0.91,
                "level": "high",
                "reason": "same_amount",
                "status": "pending",
                "group_id": 4,
                "signal_label": "Duplicate import",
                "source_chain": [{"role": "import"}],
                "import_bill_snapshot": {
                    "date": "2026-05-01",
                    "type": "expense",
                    "amount": -100,
                    "description": "imported",
                    "source_account_id": 9,
                },
            }
        ]

    async def get_bill_investment_candidate_bills(self, bill_id: int, user_id: int = 1) -> dict[str, Any]:
        return {
            "candidates": [
                {
                    "id": 13,
                    "date": "2026-05-01 10:30:00",
                    "type": "income",
                    "amount": 100.0,
                    "counterparty": "Fund",
                    "description": "fund redemption",
                    "payment_method": "card",
                    "main_category": "Investment",
                    "source_account_id": 2,
                },
                {"id": 12, "date": "bad-date", "amount": 100.0, "source_account_id": 2},
                {"id": bill_id, "date": "2026-05-01", "amount": 100.0, "source_account_id": 2},
            ]
        }

    async def get_user_by_id(self, user_id: int) -> dict[str, Any]:
        return {"id": user_id, "import_learning_enabled": 1}

    async def get_import_learning_rules(self, **kwargs: Any) -> list[dict[str, Any]]:
        return [
            {
                "id": 21,
                "match_type": "composite",
                "match_value": "Fund",
                "normalized_match_value": "fund",
                "parser_id": "",
                "composite_match_hash": "hash",
                "match_features_json": '{"counterparty":"Fund"}',
                "features": {"score": 0.93},
                "learned_type": "expense",
                "learned_category_id": 5,
                "learned_source_account_id": 7,
                "learned_destination_account_id": 0,
            },
            {"id": 0, "match_type": "composite", "match_features_json": "{}"},
            {"id": 99, "match_type": "keyword"},
        ]

    async def _get_bill_learning_rule_suppression_created_at_map(
        self,
        bill_id: int,
        user_id: int = 1,
    ) -> dict[int, str]:
        return {}

    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
        return [{"id": 5, "main_category": "Invest", "sub_category": "Fund"}]

    async def get_all_accounts(self, user_id: int = 1) -> list[dict[str, Any]]:
        return [{"id": 7, "name": "Card"}]

    def build_composite_match_features(self, **kwargs: Any) -> dict[str, Any]:
        return {"counterparty": kwargs.get("counterparty"), "description": kwargs.get("description")}

    async def list_manual_transfer_pairs(self, user_id: int = 1) -> list[dict[str, Any]]:
        return [{"id": 1, "user_id": user_id}]

    async def get_bill_by_id(self, bill_id: int, user_id: int = 1) -> dict[str, Any] | None:
        return None if bill_id == 404 else {"id": bill_id}

    async def list_bill_matching_feedback_for_bill(
        self,
        bill_id: int,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        return [{"bill_id": bill_id, "action": "reject"}]

    async def create_manual_transfer_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        **kwargs: Any,
    ) -> dict[str, Any]:
        if candidate_bill_id == 404:
            raise LookupError
        if candidate_bill_id == 409:
            raise ValueError("already paired")
        return {"id": 101, "left_bill_id": bill_id, "right_bill_id": candidate_bill_id}

    async def create_manual_investment_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        **kwargs: Any,
    ) -> dict[str, Any]:
        return {"id": 202, "left_bill_id": bill_id, "right_bill_id": candidate_bill_id}

    async def delete_manual_pair(self, pair_id: int, user_id: int = 1) -> dict[str, Any]:
        if pair_id == 404:
            raise LookupError
        return {"id": pair_id, "deleted": True}

    async def reject_bill_transfer_candidate(self, bill_id: int, candidate_bill_id: int, **kwargs: Any) -> None:
        self.reject_calls.append(("transfer", bill_id, candidate_bill_id))

    async def reject_bill_investment_candidate(self, bill_id: int, candidate_bill_id: int, **kwargs: Any) -> None:
        self.reject_calls.append(("investment", bill_id, candidate_bill_id))

    async def accept_bill_learning_candidate(self, bill_id: int, rule_id: int, **kwargs: Any) -> dict[str, Any]:
        return {"bill": {"id": bill_id, "rule_id": rule_id}}

    async def reject_bill_learning_candidate(self, bill_id: int, rule_id: int, **kwargs: Any) -> None:
        self.reject_calls.append(("learning", bill_id, rule_id))

    async def accept_import_reconciliation_candidate(self, candidate_id: str, user_id: int = 1) -> dict[str, Any]:
        return {"bill": {"id": 12}, "projection": {"candidate_id": candidate_id}}

    async def reject_import_reconciliation_candidate(self, candidate_id: str, user_id: int = 1) -> dict[str, Any]:
        return {"projection": {"candidate_id": candidate_id, "status": "rejected"}}

    async def clear_import_reconciliation_candidate(self, candidate_id: str, user_id: int = 1) -> dict[str, Any]:
        return {"projection": {"candidate_id": candidate_id, "status": "pending"}}


class FakeMatchingService(
    MatchingActionsMixin,
    MatchingPreviewAcceptMixin,
    MatchingPreviewRejectClearMixin,
    MatchingReadsMixin,
):
    def __init__(self) -> None:
        self.db = FakeMatchingDB()
        self.calls: list[tuple[Any, ...]] = []

    async def get_import_preview(self, session_id: str, selected_only: bool = False, user_id: int = 1) -> list[dict[str, Any]]:
        return [{"id": 1, "session_id": session_id, "matching": {"transfer": {"score": 0.8}}}]

    async def _get_investment_keyword_config(self, user_id: int) -> dict[str, Any]:
        return {}

    def _classify_investment_pnl_change(self, bill: dict[str, Any], **kwargs: Any) -> bool:
        return False

    def _score_investment_candidate(self, bill: dict[str, Any], **kwargs: Any) -> bool:
        return bool(bill.get("description"))

    def _deserialize_learning_match_features(self, rule: dict[str, Any]) -> dict[str, Any]:
        return dict(rule.get("features") or {})

    def _score_learning_rule_similarity(
        self,
        bill_features: dict[str, Any],
        rule_features: dict[str, Any],
    ) -> dict[str, Any]:
        return {"score": float(rule_features.get("score") or 0), "reason_parts": ["counterparty"]}

    def _build_learning_rule_result_summary(
        self,
        rule: dict[str, Any],
        categories_by_id: dict[int, dict[str, Any]],
        accounts_by_id: dict[int, dict[str, Any]],
    ) -> str:
        return f"{categories_by_id[5]['main_category']} via {accounts_by_id[7]['name']}"

    @staticmethod
    def _coerce_optional_positive_int(raw_value: Any) -> int | None:
        if raw_value in (None, "", 0, "0"):
            return None
        try:
            normalized = int(raw_value)
        except (TypeError, ValueError):
            return None
        return normalized if normalized > 0 else None

    def _build_transfer_suggestion_from_preview(self, preview: dict[str, Any]) -> dict[str, Any]:
        return dict(preview.get("transfer_suggestion") or {})

    def _build_investment_signal_from_preview(self, preview: dict[str, Any]) -> dict[str, Any]:
        return dict(preview.get("investment_signal") or {})

    async def apply_preview_transfer_decision(self, preview_id: int, decision: str, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("transfer", preview_id, decision, kwargs))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-1",
            "preview": [{"id": preview_id}],
            "preview_item": {"id": preview_id, "decision": decision},
        }

    async def update_preview_recurring_match(self, preview_id: int, recurring_id: int | None, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("recurring", preview_id, recurring_id, kwargs))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-1",
            "recurring_id": recurring_id,
            "preview": [{"id": preview_id}],
            "preview_item": {"id": preview_id, "recurring_id": recurring_id},
        }

    async def apply_preview_investment_decision(self, preview_id: int, decision: str, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("investment", preview_id, decision, kwargs))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-1",
            "review_status": decision,
            "suppressed": decision == "reject",
        }

    async def apply_preview_learning_decision(self, preview_id: int, decision: str, **kwargs: Any) -> dict[str, Any]:
        self.calls.append(("learning", preview_id, decision, kwargs))
        return {
            "success": True,
            "preview_id": preview_id,
            "session_id": "session-1",
            "preview": [{"id": preview_id}],
            "preview_item": {"id": preview_id, "decision": decision},
        }


def test_matching_read_helpers_normalize_history_and_reconcile_metadata() -> None:
    service = FakeMatchingService()

    assert service._derive_matching_candidate_level(0.9) == "high"
    assert service._derive_matching_candidate_level(0.7) == "medium"
    assert service._derive_matching_candidate_level(0.1) == "low"
    assert service._derive_matching_candidate_level(0) == ""
    assert service._normalize_matching_history_bill_ids([1, "2", 2]) == [1, 2]
    assert service._build_matching_history_pair_key({"id": "8"}) == ("id", 8)
    assert service._build_matching_history_pair_key({"left_bill_id": 5, "right_bill_id": 3}) == ("bills", 3, 5)
    assert service._build_matching_history_pair_key({}) is None
    assert service._normalize_reconcile_families(["transfer", "bad", "learning"]) == {"transfer", "learning"}
    assert service._normalize_reconcile_families("transfer") == {"transfer"}

    for raw_bill_id in (True, "x", 0, object()):
        with pytest.raises(ValueError, match="Invalid billIds"):
            service._coerce_matching_history_bill_id(raw_bill_id)


@pytest.mark.asyncio
async def test_matching_reads_build_formal_candidates_and_feedback() -> None:
    service = FakeMatchingService()

    session_candidates = await service.get_matching_session_candidates("session-1", user_id=7)
    assert session_candidates["session_id"] == "session-1"

    not_found = await service.get_matching_bill_candidates(404, user_id=7)
    assert not_found == {"success": False, "error": "Bill not found", "status_code": 404}

    linked = await service.get_matching_bill_candidates(10, user_id=7)
    assert linked["linked_pair"]["id"] == 8
    assert linked["candidates"] == []

    result = await service.get_matching_bill_candidates(12, user_id=7)
    candidate_ids = [candidate["candidate_id"] for candidate in result["candidates"]]
    assert "reconcile:import:duplicate:bill:12:abc" in candidate_ids
    assert "bill:12:investment:13" in candidate_ids
    assert any(candidate["kind"] == "learning" for candidate in result["candidates"])
    assert result["reconciliation"]["status"] == "merged"

    pairs = await service.get_matching_pairs(user_id=7)
    assert pairs["pairs"] == [{"id": 1, "user_id": 7}]
    assert (await service.get_matching_bill_feedback(404, user_id=7))["status_code"] == 404
    feedback = await service.get_matching_bill_feedback(12, user_id=7)
    assert feedback["events"] == [{"bill_id": 12, "action": "reject"}]


@pytest.mark.asyncio
async def test_reconcile_history_filters_families_and_counts_linked_pairs() -> None:
    service = FakeMatchingService()

    invalid = await service.reconcile_matching_history([False], user_id=7)
    assert invalid["status_code"] == 400
    missing = await service.reconcile_matching_history([404], user_id=7)
    assert missing == {"success": False, "error": "Bill not found", "status_code": 404, "bill_id": 404}

    result = await service.reconcile_matching_history([10, 12, 12], user_id=7, families=["transfer", "learning"])
    assert result["success"] is True
    assert result["summary"]["bill_count"] == 2
    assert result["summary"]["linked_pair_count"] == 1
    assert result["summary"]["candidate_count"] >= 2


@pytest.mark.asyncio
async def test_matching_actions_dispatch_manual_pairs_and_bill_candidates() -> None:
    service = FakeMatchingService()

    assert (await service._accept_matching_candidate("bad", {}, user_id=7))["status_code"] == 400
    assert (await service._accept_matching_candidate("preview:1:unknown", {}, user_id=7))["status_code"] == 400
    assert (await service._accept_matching_candidate("bill:12:transfer:13", {}, user_id=7))["pair"]["id"] == 101
    assert (await service._accept_matching_candidate("bill:12:investment:13", {}, user_id=7))["pair"]["id"] == 202

    learning_id = (await service.get_matching_bill_candidates(12, user_id=7))["candidates"][-1]["candidate_id"]
    assert (await service._accept_matching_candidate(learning_id, {}, user_id=7))["bill"]["rule_id"] == 21
    assert (await service._reject_matching_candidate(learning_id, {}, user_id=7))["action"] == "reject"

    assert (await service._reject_matching_candidate("bill:12:transfer:13", {}, user_id=7))["action"] == "reject"
    assert (await service._reject_matching_candidate("bill:12:investment:13", {}, user_id=7))["action"] == "reject"
    assert service.db.reject_calls[:2] == [("learning", 12, 21), ("transfer", 12, 13)]

    assert (await service.create_manual_transfer_pair(1, 1, user_id=7))["status_code"] == 400
    assert (await service.create_manual_transfer_pair(1, 404, user_id=7))["status_code"] == 404
    assert (await service.create_manual_transfer_pair(1, 409, user_id=7))["status_code"] == 409
    assert (await service.create_manual_investment_pair(1, 1, user_id=7))["status_code"] == 400
    assert (await service.delete_manual_transfer_pair(404, user_id=7))["status_code"] == 404
    assert (await service.delete_manual_transfer_pair(9, user_id=7))["pair"]["deleted"] is True


@pytest.mark.asyncio
async def test_matching_actions_dispatch_preview_and_reconciliation_families() -> None:
    service = FakeMatchingService()
    payload = {
        "expectedState": {"sessionId": "session-1"},
        "responseMode": "preview-item",
        "recurringId": "88",
        "ruleId": "21",
        "modelVersion": "v1",
        "datasetSnapshotId": 5,
    }

    assert (await service._accept_matching_candidate("preview:1:transfer", payload, user_id=7))["preview_item"]["id"] == 1
    assert (await service._reject_matching_candidate("preview:1:transfer", payload, user_id=7))["preview_item"]["decision"] == "reject"
    assert (await service._accept_matching_candidate("preview:1:recurring", payload, user_id=7))["recurring_id"] == 88
    assert (await service._reject_matching_candidate("preview:1:recurring", payload, user_id=7))["action"] == "reject"
    assert (await service._accept_matching_candidate("preview:1:learning", payload, user_id=7))["preview_item"]["decision"] == "accept"
    assert (await service._reject_matching_candidate("preview:1:learning", payload, user_id=7))["preview_item"]["decision"] == "reject"
    assert (await service._clear_matching_candidate("preview:1:learning", payload, user_id=7))["preview_item"]["decision"] == "clear"
    assert (await service._accept_matching_candidate("preview:1:recurring", {"recurringId": ""}, user_id=7))["status_code"] == 400
    assert (await service._clear_matching_candidate("preview:1:learning", {}, user_id=7))["status_code"] == 400

    reconciliation_id = "reconcile:import:duplicate:bill:12:abc"
    assert (await service._accept_matching_candidate(reconciliation_id, {}, user_id=7))["projection"]["candidate_id"] == reconciliation_id
    assert (await service._reject_matching_candidate(reconciliation_id, {}, user_id=7))["projection"]["status"] == "rejected"
    assert (await service._clear_matching_candidate(reconciliation_id, {}, user_id=7))["projection"]["status"] == "pending"


@pytest.mark.asyncio
async def test_preview_wrapper_helpers_cover_validation_and_review_statuses() -> None:
    service = FakeMatchingService()

    assert service._normalize_preview_recurring_id(" 9 ") == 9
    assert service._normalize_preview_learning_rule_id("21") == 21
    assert service._normalize_preview_learning_account_id("7") == 7
    assert service._normalize_preview_learning_account_id("") is None
    for helper, raw_value in (
        (service._normalize_preview_recurring_id, True),
        (service._normalize_preview_recurring_id, "x"),
        (service._normalize_preview_learning_rule_id, 0),
        (service._normalize_preview_learning_account_id, object()),
    ):
        with pytest.raises(ValueError):
            helper(raw_value)

    assert service._extract_preview_learning_model_metadata({"model_version": "v2", "dataset_snapshot_id": 4}) == {
        "model_version": "v2",
        "dataset_snapshot_id": 4,
    }
    assert service._learning_model_recommendation_references_available(
        {"category_id": 5, "source_account_id": 7, "destination_account_id": ""},
        categories_by_id={5: {}},
        accounts_by_id={7: {}},
    ) is True
    assert service._learning_model_recommendation_references_available(
        {"category_id": 99},
        categories_by_id={5: {}},
        accounts_by_id={},
    ) is False
    assert service._validate_preview_learning_model_candidate(
        {"source": "rule"},
        model_version="v1",
        dataset_snapshot_id=None,
        require_model_version=False,
    )["status_code"] == 409
    assert service._validate_preview_learning_model_candidate(
        {"source": "model", "model_version": "v1", "dataset_snapshot_id": 5},
        model_version="v1",
        dataset_snapshot_id=5,
        require_model_version=True,
    ) is None
    assert service._validate_preview_learning_model_candidate(
        {"source": "model", "model_version": "v1", "dataset_snapshot_id": 5},
        model_version="v2",
        dataset_snapshot_id=5,
        require_model_version=True,
    )["status_code"] == 409

    assert await service._get_preview_category_id({}, user_id=7) is None
    assert await service._get_preview_transfer_review_status(
        {"preview_matching_feedback": {"transfer": {"review_status": "accepted"}}}
    ) == "accepted"
    assert await service._get_preview_transfer_review_status(
        {"transfer_suggestion": {"score": 0.8}}
    ) == "pending"
    assert service._resolve_preview_investment_review_status({}) == ""
    assert service._resolve_preview_investment_review_status(
        {"investment_signal": {"score": 0.9}}
    ) == "pending"
    assert service._resolve_preview_investment_review_status(
        {
            "investment_signal": {"score": 0.9},
            "preview_matching_feedback": {"investment": {"review_status": "rejected"}},
        }
    ) == "rejected"
    assert await service._get_preview_learning_review_status(
        {"preview_matching_feedback": {"learning": {"review_status": "accepted", "rule_id": "21"}}},
        learning_recommendation={"rule_id": 21},
    ) == "accepted"
    assert await service._get_preview_learning_review_status({}, learning_recommendation={}) == ""
    assert await service._get_preview_learning_review_status({}, learning_recommendation={"rule_id": 3}) == "pending"
