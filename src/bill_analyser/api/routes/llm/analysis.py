"""llm analysis route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/analyze-transactions", methods=["POST"])
@log_method
@require_auth
def analyze_transactions():  # pylint: disable=too-many-return-statements
    """使用 LLM 分析未分类交易并生成分类建议"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True)
        if data is None:
            data = {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400
        bill_ids = data.get("bill_ids")
        limit = data.get("limit", 20)
        session_id = data.get("session_id")
        preview_ids = data.get("preview_ids")
        preview_updates = data.get("preview_updates")

        service = _get_llm_service(user_id)
        candidates = _run_async(
            service.analyze_transactions(
                user_id=user_id,
                bill_ids=bill_ids,
                limit=limit,
                session_id=session_id,
                preview_ids=preview_ids,
                preview_updates=preview_updates,
            )
        )

        response_payload = {
            "candidates_created": len(candidates),
            "candidates": candidates,
        }
        if session_id:
            response_payload["session_id"] = session_id
            response_payload["mode"] = "import_session"
        elif bill_ids:
            response_payload["mode"] = "persisted_selection"
        else:
            response_payload["mode"] = "persisted_uncategorized"

        return jsonify({"success": True, "data": response_payload, "total": len(candidates)})
    except LLMImportSessionAnalysisError as exc:
        return _error_response(str(exc), exc.code, exc.status_code)
    except ValueError as exc:
        status_code = 404 if str(exc) == "Import session not found" else 400
        code = "IMPORT_SESSION_NOT_FOUND" if status_code == 404 else "INVALID_REQUEST"
        return _error_response(str(exc), code, status_code)
    except RuntimeError as exc:
        logger.warning("LLM 分析交易失败: %s", exc)
        if str(exc).startswith("Rate limit exceeded"):
            return _error_response(str(exc), "LLM_RATE_LIMITED", 429)
        return _error_response(str(exc), "LLM_PROVIDER_UNAVAILABLE", 503)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM 分析交易失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)


@bp.route("/induce-rules", methods=["POST"])
@log_method
@require_auth
def induce_rules():
    """使用 LLM 从已分类样本中归纳关键词规则"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return jsonify({"success": False, "error": "LLM service is not enabled"}), 400

        data = request.get_json()
        if not data or "category_id" not in data:
            return jsonify({"success": False, "error": "category_id is required"}), 400

        category_id = data["category_id"]
        sample_count = data.get("sample_count", 10)

        service = _get_llm_service(user_id)
        candidates = _run_async(
            service.induce_rules(
                user_id=user_id,
                category_id=category_id,
                sample_count=sample_count,
            )
        )

        return jsonify({"success": True, "data": candidates, "total": len(candidates)})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 404
    except RuntimeError as exc:
        logger.warning("LLM 归纳规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 429
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM 归纳规则失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500


@bp.route("/rule-synthesis", methods=["POST"])
@log_method
@require_auth
def synthesize_rules():
    """使用 LLM 从长期学习知识摘要归纳规则中心候选。"""
    try:
        user_id = _get_request_user_id()
        config = _get_llm_config(user_id)
        if not config.get("enabled", False):
            return _error_response("LLM service is not enabled", "LLM_DISABLED", 400)

        data = request.get_json(silent=True) or {}
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid request"}), 400

        limit = data.get("limit", 8)
        service = _get_llm_service(user_id)
        result = _run_async(service.synthesize_rule_candidates(user_id=user_id, limit=limit))
        return jsonify({"success": True, "data": result, "total": result["candidates_created"]})
    except RuntimeError as exc:
        logger.warning("LLM 规则候选归纳失败: %s", exc)
        if str(exc).startswith("Rate limit exceeded"):
            return _error_response(str(exc), "LLM_RATE_LIMITED", 429)
        return _error_response(str(exc), "LLM_PROVIDER_UNAVAILABLE", 503)
    except ValueError as exc:
        return _error_response(str(exc), "INVALID_REQUEST", 400)
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("LLM 规则候选归纳失败: %s", exc)
        return _error_response(str(exc), "INTERNAL_ERROR", 500)
