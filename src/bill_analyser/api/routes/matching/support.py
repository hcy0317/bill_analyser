"""Matching API Routes - 导入候选与历史账单后配对接口。"""

import asyncio
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("MatchingAPI")

bp = Blueprint("matching", __name__)


def get_app_context() -> tuple[Any, Any]:
    """获取 matching API 所需的应用上下文实例。"""
    return (
        cast("Any", current_app.config.get("DB_INSTANCE")),
        cast("Any", current_app.config.get("BILL_SERVICE_INSTANCE")),
    )


def _get_request_user_id() -> int:
    return int(getattr(request, "user_id", 0) or 0)


def _run_async(coroutine: Any) -> Any:
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return loop.run_until_complete(coroutine)
    finally:
        loop.close()


def _serialize_bill_pair(pair: dict[str, Any] | None) -> dict[str, Any] | None:
    if not isinstance(pair, dict):
        return None

    serialized_pair = {
        "id": int(pair.get("id") or 0),
        "pairType": str(pair.get("pair_type") or "transfer"),
        "source": str(pair.get("source") or "manual"),
        "leftBillId": int(pair.get("left_bill_id") or 0),
        "rightBillId": int(pair.get("right_bill_id") or 0),
    }
    if pair.get("other_bill_id") not in (None, ""):
        serialized_pair["otherBillId"] = int(pair.get("other_bill_id") or 0)
    return serialized_pair


def _serialize_bill_snapshot(snapshot: dict[str, Any] | None) -> dict[str, Any]:
    bill_snapshot = dict(snapshot or {}) if isinstance(snapshot, dict) else {}
    return {
        "id": int(bill_snapshot.get("id") or 0),
        "date": str(bill_snapshot.get("date") or ""),
        "type": str(bill_snapshot.get("type") or ""),
        "amount": float(bill_snapshot.get("amount") or 0.0),
        "counterparty": str(bill_snapshot.get("counterparty") or ""),
        "description": str(bill_snapshot.get("description") or ""),
        "paymentMethod": str(bill_snapshot.get("payment_method") or ""),
        "mainCategory": str(bill_snapshot.get("main_category") or ""),
        "subCategory": str(bill_snapshot.get("sub_category") or ""),
        "sourceAccountId": int(bill_snapshot.get("source_account_id") or 0),
        "destinationAccountId": int(bill_snapshot.get("destination_account_id") or 0),
    }


def _serialize_matching_pair_detail(pair: dict[str, Any]) -> dict[str, Any]:
    serialized_pair = _serialize_bill_pair(pair) or {
        "id": 0,
        "pairType": "transfer",
        "source": "manual",
        "leftBillId": 0,
        "rightBillId": 0,
    }
    serialized_pair["createdAt"] = str(pair.get("created_at") or "")
    serialized_pair["updatedAt"] = str(pair.get("updated_at") or "")
    serialized_pair["leftBill"] = _serialize_bill_snapshot(pair.get("left_bill"))
    serialized_pair["rightBill"] = _serialize_bill_snapshot(pair.get("right_bill"))
    return serialized_pair


def _serialize_matching_bill_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    serialized_candidate = {
        "candidateId": str(candidate.get("candidate_id") or ""),
        "kind": str(candidate.get("kind") or "transfer"),
        "score": float(candidate.get("score") or 0.0),
        "level": str(candidate.get("level") or ""),
        "reason": str(candidate.get("reason") or ""),
    }
    if candidate.get("bill_id") not in (None, ""):
        serialized_candidate["billId"] = int(candidate.get("bill_id") or 0)
    if isinstance(candidate.get("bill"), dict):
        serialized_candidate["bill"] = _serialize_bill_snapshot(candidate.get("bill"))
    if candidate.get("rule_id") not in (None, ""):
        serialized_candidate["ruleId"] = int(candidate.get("rule_id") or 0)
    if candidate.get("recommended_type") not in (None, ""):
        serialized_candidate["recommendedType"] = str(candidate.get("recommended_type") or "")
    if candidate.get("summary") not in (None, ""):
        serialized_candidate["summary"] = str(candidate.get("summary") or "")
    if candidate.get("suppressed") not in (None, ""):
        serialized_candidate["suppressed"] = bool(candidate.get("suppressed"))
    if isinstance(candidate.get("reconciliation"), dict):
        serialized_candidate["reconciliation"] = dict(candidate.get("reconciliation") or {})
    return serialized_candidate


def _build_matching_bill_candidates_payload(
    bill_id: int,
    result: dict[str, Any],
) -> dict[str, Any]:
    payload = {
        "billId": bill_id,
        "linkedPair": _serialize_bill_pair(result.get("linked_pair")),
        "candidates": [
            _serialize_matching_bill_candidate(candidate) for candidate in list(result.get("candidates") or [])
        ],
    }
    if isinstance(result.get("reconciliation"), dict):
        payload["reconciliation"] = dict(result.get("reconciliation") or {})
    return payload


def _serialize_matching_feedback_event(event: dict[str, Any]) -> dict[str, Any]:
    payload = dict(event.get("payload") or {}) if isinstance(event.get("payload"), dict) else {}
    return {
        "id": int(event.get("id") or 0),
        "candidateId": str(event.get("candidate_id") or ""),
        "action": str(event.get("action") or ""),
        "createdAt": str(event.get("created_at") or ""),
        "payload": payload,
    }


def _serialize_reconciliation_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    serialized = {
        "id": int(candidate.get("id") or 0),
        "candidateId": str(candidate.get("candidate_id") or ""),
        "candidateType": str(candidate.get("candidate_type") or ""),
        "status": str(candidate.get("status") or ""),
        "sessionId": str(candidate.get("session_id") or ""),
        "importBillKey": str(candidate.get("import_bill_key") or ""),
        "existingBillId": int(candidate.get("existing_bill_id") or 0),
        "groupKey": str(candidate.get("group_key") or ""),
        "amountAbs": float(candidate.get("amount_abs") or 0.0),
        "score": float(candidate.get("score") or 0.0),
        "level": str(candidate.get("level") or ""),
        "reason": str(candidate.get("reason") or ""),
        "groupId": int(candidate.get("group_id") or 0),
        "groupStatus": str(candidate.get("group_status") or ""),
        "canonicalBillId": int(candidate.get("canonical_bill_id") or 0),
        "signalLabel": str(candidate.get("signal_label") or ""),
        "sourceChain": list(candidate.get("source_chain") or []),
        "seenCount": int(candidate.get("seen_count") or 0),
        "firstSeenAt": str(candidate.get("first_seen_at") or ""),
        "lastSeenAt": str(candidate.get("last_seen_at") or ""),
        "importBill": dict(candidate.get("import_bill_snapshot") or {}),
        "existingBill": dict(candidate.get("existing_bill_snapshot") or {}),
    }
    if candidate.get("preview_id") not in (None, ""):
        serialized["previewId"] = int(candidate.get("preview_id") or 0)
    if candidate.get("time_diff_seconds") not in (None, ""):
        serialized["timeDiffSeconds"] = int(candidate.get("time_diff_seconds") or 0)
    if isinstance(candidate.get("source_payload"), dict):
        serialized["sourcePayload"] = dict(candidate.get("source_payload") or {})
    return serialized


def _parse_matching_candidates_selector() -> tuple[str | None, int | None]:
    session_id = str(request.args.get("sessionId") or "").strip()
    raw_bill_id = request.args.get("billId")
    has_session_id = bool(session_id)
    has_bill_id = raw_bill_id not in (None, "")

    if has_session_id == has_bill_id:
        raise ValueError("Exactly one of sessionId or billId is required")

    if has_session_id:
        return session_id, None

    try:
        normalized_raw_bill_id = "" if raw_bill_id is None else str(raw_bill_id)
        bill_id = int(normalized_raw_bill_id)
    except (TypeError, ValueError) as exc:
        raise ValueError("Invalid billId") from exc

    if bill_id <= 0:
        raise ValueError("Invalid billId")

    return None, bill_id


def _parse_optional_positive_query_int(field_name: str) -> int | None:
    raw_value = request.args.get(field_name)
    if raw_value in (None, ""):
        return None
    try:
        normalized_value = int(str(raw_value).strip())
    except (TypeError, ValueError) as exc:
        raise ValueError(f"Invalid {field_name}") from exc
    if normalized_value <= 0:
        raise ValueError(f"Invalid {field_name}")
    return normalized_value


def _parse_reconciliation_candidates_query() -> dict[str, Any]:
    candidate_type = str(request.args.get("candidateType") or "").strip().lower() or None
    if candidate_type is not None and candidate_type not in {"transfer", "duplicate"}:
        raise ValueError("Invalid candidateType")

    status = str(request.args.get("status") or "").strip().lower() or None
    if status is not None and status not in {
        "pending",
        "accepted",
        "rejected",
        "merged",
        "rolled_back",
        "superseded",
    }:
        raise ValueError("Invalid status")

    limit = _parse_optional_positive_query_int("limit") or 200
    return {
        "session_id": str(request.args.get("sessionId") or "").strip() or None,
        "preview_id": _parse_optional_positive_query_int("previewId"),
        "existing_bill_id": _parse_optional_positive_query_int("billId"),
        "candidate_type": candidate_type,
        "status": status,
        "limit": min(limit, 500),
    }


def _parse_positive_request_int(data: dict[str, Any], field_name: str) -> int:
    raw_value = data.get(field_name)
    if raw_value in (None, ""):
        raise KeyError("billId and candidateBillId are required")

    if isinstance(raw_value, bool):
        raise ValueError("Invalid request")
    if isinstance(raw_value, int):
        normalized_value = raw_value
    elif isinstance(raw_value, str):
        normalized_raw_value = raw_value.strip()
        if not normalized_raw_value.isdigit():
            raise ValueError("Invalid request")
        normalized_value = int(normalized_raw_value)
    else:
        raise ValueError("Invalid request")

    if normalized_value <= 0:
        raise ValueError("Invalid request")

    return normalized_value


def _parse_manual_pair_request(data: Any) -> tuple[int, int, str]:
    if not isinstance(data, dict):
        raise ValueError("Invalid request")

    normalized_bill_id = _parse_positive_request_int(data, "billId")
    normalized_candidate_bill_id = _parse_positive_request_int(data, "candidateBillId")

    raw_pair_type = data.get("pairType", None)
    if raw_pair_type is None:
        pair_type = "transfer"
    else:
        pair_type = str(raw_pair_type).strip().lower()
    if pair_type not in {"transfer", "investment"}:
        raise ValueError("Invalid pairType")

    if normalized_bill_id == normalized_candidate_bill_id:
        raise LookupError("billId and candidateBillId must be different")

    return normalized_bill_id, normalized_candidate_bill_id, pair_type


def _parse_reconcile_history_request(data: Any) -> tuple[list[int], list[str] | None]:
    """Return ``(bill_ids, families | None)`` from the request body."""
    if not isinstance(data, dict):
        raise ValueError("Invalid request")

    raw_bill_ids = data.get("billIds")
    if raw_bill_ids is None:
        raise KeyError("billIds is required")
    if not isinstance(raw_bill_ids, list) or not raw_bill_ids:
        raise ValueError("billIds must be a non-empty list")

    normalized_bill_ids: list[int] = []
    seen_bill_ids: set[int] = set()
    for raw_bill_id in raw_bill_ids:
        if isinstance(raw_bill_id, bool):
            raise ValueError("Invalid billIds")
        if isinstance(raw_bill_id, int):
            normalized_bill_id = raw_bill_id
        elif isinstance(raw_bill_id, str):
            normalized_raw_bill_id = raw_bill_id.strip()
            if not normalized_raw_bill_id.isdigit():
                raise ValueError("Invalid billIds")
            normalized_bill_id = int(normalized_raw_bill_id)
        else:
            raise ValueError("Invalid billIds")
        if normalized_bill_id <= 0:
            raise ValueError("Invalid billIds")
        if normalized_bill_id in seen_bill_ids:
            continue
        seen_bill_ids.add(normalized_bill_id)
        normalized_bill_ids.append(normalized_bill_id)

    raw_families = data.get("families")
    families: list[str] | None = None
    if isinstance(raw_families, list) and raw_families:
        families = [str(f) for f in raw_families if isinstance(f, str)] or None

    return normalized_bill_ids, families


def _build_matching_candidate_action_payload(
    candidate_id: str,
    result: dict[str, Any],
) -> dict[str, Any]:
    response_data: dict[str, Any] = {
        "candidateId": str(result.get("candidate_id") or candidate_id),
        "action": str(result.get("action") or ""),
    }
    if result.get("preview_id") not in (None, ""):
        response_data["previewId"] = int(result.get("preview_id") or 0)
    if result.get("session_id") not in (None, ""):
        response_data["sessionId"] = str(result.get("session_id") or "")
    if result.get("recurring_id") not in (None, ""):
        response_data["recurringId"] = int(result.get("recurring_id") or 0)
    if result.get("review_status") not in (None, ""):
        response_data["reviewStatus"] = str(result.get("review_status") or "")
    if result.get("suppressed") is not None:
        response_data["suppressed"] = bool(result.get("suppressed"))
    if isinstance(result.get("preview_item"), dict):
        response_data["previewItem"] = dict(result.get("preview_item") or {})
    if isinstance(result.get("preview"), list):
        response_data["preview"] = list(result.get("preview") or [])
    if isinstance(result.get("pair"), dict):
        response_data["pair"] = _serialize_bill_pair(result.get("pair"))
    if isinstance(result.get("bill"), dict):
        response_data["bill"] = _serialize_bill_snapshot(result.get("bill"))
    if isinstance(result.get("projection"), dict):
        response_data["projection"] = dict(result.get("projection") or {})
    return response_data


def _build_matching_reconcile_history_payload(result: dict[str, Any]) -> dict[str, Any]:
    summary = dict(result.get("summary") or {})
    results = list(result.get("results") or [])
    serialized_results = [
        _build_matching_bill_candidates_payload(int(item.get("bill_id") or 0), item)
        for item in results
    ]
    return {
        "summary": {
            "billCount": int(summary.get("bill_count") or 0),
            "candidateCount": int(summary.get("candidate_count") or 0),
            "linkedPairCount": int(summary.get("linked_pair_count") or 0),
        },
        "results": serialized_results,
    }

__all__ = [name for name in globals() if not name.startswith("__")]
