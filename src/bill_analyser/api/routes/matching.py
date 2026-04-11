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
        "destinationAccountId": int(
            bill_snapshot.get("destination_account_id") or 0
        ),
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


def _serialize_transfer_candidate(candidate: dict[str, Any]) -> dict[str, Any]:
    return {
        "billId": int(candidate.get("bill_id") or 0),
        "score": float(candidate.get("score") or 0.0),
        "level": str(candidate.get("level") or ""),
        "reason": str(candidate.get("reason") or ""),
        "bill": _serialize_bill_snapshot(candidate.get("bill")),
    }


def _parse_manual_pair_request(data: Any) -> tuple[int, int]:
    if not isinstance(data, dict):
        raise ValueError("Invalid request")

    bill_id = data.get("billId")
    candidate_bill_id = data.get("candidateBillId")
    if bill_id in (None, "") or candidate_bill_id in (None, ""):
        raise KeyError("billId and candidateBillId are required")

    try:
        normalized_bill_id = int(bill_id)
        normalized_candidate_bill_id = int(candidate_bill_id)
    except (TypeError, ValueError) as exc:
        raise ValueError("Invalid request") from exc

    if normalized_bill_id == normalized_candidate_bill_id:
        raise LookupError("billId and candidateBillId must be different")

    return normalized_bill_id, normalized_candidate_bill_id


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

        result = _run_async(
            bill_service.get_matching_session_candidates(session_id, user_id=user_id)
        )
        return jsonify({"success": True, "data": result})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取 matching 会话候选失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/bills/<int:bill_id>/candidates", methods=["GET"])
@log_method
@require_auth
def get_matching_bill_candidates(bill_id: int):
    """返回某条正式账单的 transfer-only 历史后配对候选。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()

        result = _run_async(
            bill_service.get_matching_bill_candidates(bill_id, user_id=user_id)
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Bill not found")
            return jsonify({"success": False, "error": error_message}), status_code

        return jsonify(
            {
                "success": True,
                "data": {
                    "billId": bill_id,
                    "linkedPair": _serialize_bill_pair(result.get("linked_pair")),
                    "candidates": [
                        _serialize_transfer_candidate(candidate)
                        for candidate in list(result.get("candidates") or [])
                    ],
                },
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单 matching 候选失败: %s", exc, exc_info=True)
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
        return jsonify(
            {
                "success": True,
                "data": {
                    "pairs": [
                        _serialize_matching_pair_detail(pair)
                        for pair in list(result.get("pairs") or [])
                    ]
                },
            }
        )
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取历史账单配对列表失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500


@bp.route("/manual-pair", methods=["POST"])
@log_method
@require_auth
def create_manual_pair():
    """为两条正式账单创建 1:1 transfer-only 手工配对。"""
    try:
        try:
            normalized_bill_id, normalized_candidate_bill_id = _parse_manual_pair_request(
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
        result = _run_async(
            bill_service.create_manual_transfer_pair(
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
    """删除一条正式账单 transfer-only 手工配对。"""
    try:
        _, bill_service = get_app_context()
        user_id = _get_request_user_id()
        result = _run_async(
            bill_service.delete_manual_transfer_pair(pair_id, user_id=user_id)
        )
        if not result.get("success"):
            status_code = int(result.get("status_code", 404))
            error_message = result.get("error", "Pair not found")
            return jsonify({"success": False, "error": error_message}), status_code

        serialized_pair = _serialize_bill_pair(result.get("pair"))
        return jsonify({"success": True, "data": {"pair": serialized_pair}})
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("删除历史账单手工配对失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error"}), 500
