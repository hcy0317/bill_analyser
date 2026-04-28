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
    return serialized_candidate


def _build_matching_bill_candidates_payload(
    bill_id: int,
    result: dict[str, Any],
) -> dict[str, Any]:
    return {
        "billId": bill_id,
        "linkedPair": _serialize_bill_pair(result.get("linked_pair")),
        "candidates": [
            _serialize_matching_bill_candidate(candidate) for candidate in list(result.get("candidates") or [])
        ],
    }


def _serialize_matching_feedback_event(event: dict[str, Any]) -> dict[str, Any]:
    payload = dict(event.get("payload") or {}) if isinstance(event.get("payload"), dict) else {}
    return {
        "id": int(event.get("id") or 0),
        "candidateId": str(event.get("candidate_id") or ""),
        "action": str(event.get("action") or ""),
        "createdAt": str(event.get("created_at") or ""),
        "payload": payload,
    }


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


@bp.route("/sessions/<session_id>/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_session_candidates(session_id: str):
    """返回某个导入会话的显式 matching 候选列表。"""
    try:
        db, bill_service = get_app_context()
        user_id = _get_request_user_id()

        session = _run_async(db.get_import_session(session_id, user_id=user_id))
        if not session:
            return jsonify({"success": False, "error": "Import session not found"}), 404

        result = _run_async(bill_service.get_matching_session_candidates(session_id, user_id=user_id))
        return jsonify({"success": True, "data": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 matching 会话候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/bills/<int:bill_id>/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_bill_candidates(bill_id: int):
    """返回某条正式账单的历史 matching 候选。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()

        result = _run_async(bill_service.get_matching_bill_candidates(bill_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_bill_candidates_payload(bill_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单 matching 候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/bills/<int:bill_id>/feedback", methods=["GET"])
@log_method
@require_auth
def get_matching_bill_feedback(bill_id: int):
    """返回某条正式账单的 matching feedback 审查事件流。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()

        result = _run_async(bill_service.get_matching_bill_feedback(bill_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": {
                    "billId": bill_id,
                    "events": [
                        _serialize_matching_feedback_event(event) for event in list(result.get("events") or [])
                    ],
                },
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单 matching feedback 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_candidates():
    """按 sessionId 或 billId 统一读取 matching 候选。"""
    try:
        try:
            session_id, bill_id = _parse_matching_candidates_selector()
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        db, bill_service = get_app_context()
        user_id = _get_request_user_id()

        if session_id is not None:
            session = _run_async(db.get_import_session(session_id, user_id=user_id))
            if not session:
                return jsonify({"success": False, "error": "Import session not found"}), 404

            result = _run_async(bill_service.get_matching_session_candidates(session_id, user_id=user_id))
            return jsonify({"success": True, "data": result})

        normalized_bill_id = int(bill_id or 0)
        result = _run_async(bill_service.get_matching_bill_candidates(normalized_bill_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_bill_candidates_payload(normalized_bill_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取统一 matching 候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/reconcile-history", methods=["POST"])
@log_method
@require_auth
def reconcile_matching_history():
    """按显式 billIds 聚合读取历史正式账单 matching 候选（支持可选 families 过滤）。"""
    try:
        try:
            normalized_bill_ids, families = _parse_reconcile_history_request(request.get_json(silent=True))
        except KeyError as exc:
            return jsonify({"success": False, "error": str(exc.args[0])}), 400
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        reconcile_handler = getattr(bill_service, "reconcile_matching_history", None)
        if not callable(reconcile_handler):
            raise AttributeError("Matching reconcile-history handler not available")

        result = _run_async(reconcile_handler(normalized_bill_ids, user_id=user_id, families=families))
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to reconcile matching history")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_reconcile_history_payload(result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("历史 matching 调和失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates/<path:candidate_id>/accept", methods=["POST"])
@log_method
@require_auth
def accept_matching_candidate(candidate_id: str):
    """接受一个当前已支持 family 的 matching candidate。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        accept_handler = getattr(bill_service, "_accept_matching_candidate", None)
        if not callable(accept_handler):
            raise AttributeError("Matching accept handler not available")
        result = _run_async(
            accept_handler(
                candidate_id,
                data,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to accept candidate")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_candidate_action_payload(candidate_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("接受 matching candidate 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates/<path:candidate_id>/reject", methods=["POST"])
@log_method
@require_auth
def reject_matching_candidate(candidate_id: str):
    """拒绝一个当前已支持 family 的 matching candidate。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        reject_handler = getattr(bill_service, "_reject_matching_candidate", None)
        if not callable(reject_handler):
            raise AttributeError("Matching reject handler not available")
        result = _run_async(
            reject_handler(
                candidate_id,
                data,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to reject candidate")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_candidate_action_payload(candidate_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("拒绝 matching candidate 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/candidates/<path:candidate_id>/clear", methods=["POST"])
@log_method
@require_auth
def clear_matching_candidate(candidate_id: str):
    """清除一个当前已支持 family 的 matching candidate 决策状态。"""
    try:
        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        clear_handler = getattr(bill_service, "_clear_matching_candidate", None)
        if not callable(clear_handler):
            raise AttributeError("Matching clear handler not available")
        result = _run_async(
            clear_handler(
                candidate_id,
                data,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Failed to clear candidate")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": _build_matching_candidate_action_payload(candidate_id, result),
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("清除 matching candidate 决策失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/pairs", methods=["GET"])
@log_method
@require_auth
def get_matching_pairs():
    """返回当前用户已持久化的正式账单手工配对列表。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(bill_service.get_matching_pairs(user_id=user_id))
        serialized_pairs = [_serialize_matching_pair_detail(pair) for pair in list(result.get("pairs") or [])]
        return jsonify(
            {
                "success": True,
                "data": {"pairs": serialized_pairs},
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单配对列表失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/manual-pair", methods=["POST"])
@log_method
@require_auth
def create_manual_pair():
    """为两条正式账单创建 1:1 manual pair。默认 transfer，可选 investment。"""
    try:
        try:
            normalized_bill_id, normalized_candidate_bill_id, pair_type = _parse_manual_pair_request(
                request.get_json(silent=True) or {}
            )
        except KeyError as exc:
            return jsonify({"success": False, "error": str(exc.args[0])}), 400
        except LookupError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400
        except ValueError as exc:
            return jsonify({"success": False, "error": str(exc)}), 400

        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        pair_creator = (
            bill_service.create_manual_investment_pair
            if pair_type == "investment"
            else bill_service.create_manual_transfer_pair
        )
        result = _run_async(
            pair_creator(
                normalized_bill_id,
                normalized_candidate_bill_id,
                user_id=user_id,
            )
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 400))
            error_message = result.get("error", "Unable to create bill pair")
            return jsonify({"success": False, "error": error_message}), status_code

        serialized_pair = _serialize_bill_pair(result.get("pair"))
        return jsonify({"success": True, "data": {"pair": serialized_pair}})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("创建历史账单手工配对失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/pairs/<int:pair_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_manual_pair(pair_id: int):
    """删除一条当前用户下的正式账单 manual pair（含 investment/manual）。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(bill_service.delete_manual_transfer_pair(pair_id, user_id=user_id))
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Pair not found")
            return jsonify({"success": False, "error": error_message}), status_code

        serialized_pair = _serialize_bill_pair(result.get("pair"))
        return jsonify({"success": True, "data": {"pair": serialized_pair}})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除历史账单手工配对失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/investment-settings", methods=["GET", "PUT"])
@log_method
@require_auth
def manage_matching_investment_settings():
    """Retired investment-settings endpoint.

    Investment recognition keywords are migrated into ``category_rules`` and are
    edited through the category-rule system.  Keep a deterministic 410 response
    instead of a hidden writable settings path so old clients fail explicitly.
    """
    _get_request_user_id()
    return (
        jsonify(
            {
                "success": False,
                "error": (
                    "Investment recognition settings are managed by category rules"
                ),
            }
        ),
        410,
    )
