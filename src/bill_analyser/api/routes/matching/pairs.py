"""matching pairs route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


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
