# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403

@bp.route("/import/v2/session/<session_id>", methods=["GET"])
@log_method
@require_auth
def get_import_session(session_id: str):
    """
    获取导入会话状态

    Response:
        {
            'success': true,
            'data': {
                'session_id': 'uuid',
                'status': 'parsed|deduped|confirmed|expired',
                'created_at': '2025-11-30 10:00:00',
                'parsed_count': 100,
                'preview_count': 80
            }
        }
    """
    try:
        db, _, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            session = loop.run_until_complete(db.get_import_session(session_id, user_id))

            if not session:
                return jsonify({"success": False, "error": "Session not found or expired"}), 404

            return jsonify(
                {
                    "success": True,
                    "data": {
                        "session_id": session["session_id"],
                        "status": session["status"],
                        "created_at": session["created_at"],
                        "parsed_count": session.get("parsed_count", 0),
                        "preview_count": session.get("preview_count", 0),
                        "file_paths": session.get("file_paths", ""),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error("[获取会话] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/session/<session_id>", methods=["DELETE"])
@log_method
@require_auth
def cancel_import_session(session_id: str):
    """
    取消/清理导入会话

    清理 bills_parser_template 和 bills_preview 中的临时数据。
    """
    try:
        db, _, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            session = loop.run_until_complete(db.get_import_session(session_id, user_id))
            if not session:
                return jsonify({"success": False, "message": "Session not found"})

            loop.run_until_complete(db.clear_session_data(session_id, user_id))
            return jsonify({"success": True, "message": "Session cleared"})

        finally:
            loop.close()

    except Exception as e:
        logger.error("[取消会话] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview/<session_id>", methods=["GET"])
@log_method
@require_auth
def get_import_preview(session_id: str):
    """
    获取预览数据（分页）

    Query Parameters:
        - page: 页码（默认1）
        - page_size: 每页数量（默认50）

    Response:
        {
            'success': true,
            'data': {
                'preview': [...],
                'total': 100,
                'page': 1,
                'page_size': 50
            }
        }
    """
    try:
        page = max(request.args.get("page", 1, type=int) or 1, 1)
        page_size = request.args.get("page_size", 50, type=int) or 50
        page_size = max(min(page_size, 200), 1)
        sort_by = request.args.get("sort_by", default="", type=str) or ""
        sort_direction = request.args.get("sort_direction", default="asc", type=str) or "asc"
        raw_preview_ids = request.args.get("preview_ids", default="", type=str) or ""
        preview_ids = _parse_int_list(raw_preview_ids) if raw_preview_ids else None

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            result = loop.run_until_complete(
                bill_service.get_import_preview_page(
                    session_id,
                    page=page,
                    page_size=page_size,
                    sort_by=sort_by,
                    sort_direction=sort_direction,
                    preview_ids=preview_ids,
                    user_id=user_id,
                )
            )

            return jsonify(
                {
                    "success": True,
                    "data": {
                        "preview": result.get("preview", []),
                        "total": int(result.get("total", 0) or 0),
                        "page": int(result.get("page", page) or page),
                        "page_size": int(result.get("page_size", page_size) or page_size),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error("[获取预览] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/preview/<session_id>/index", methods=["GET"])
@log_method
@require_auth
def get_import_preview_index(session_id: str):
    """获取导入预览全局轻量索引，用于 server-paged 模式下的全局筛选/统计/排序。"""
    try:
        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            items = loop.run_until_complete(
                bill_service.get_import_preview_filter_index(
                    session_id,
                    user_id=user_id,
                )
            )
            return jsonify(
                {
                    "success": True,
                    "data": {
                        "items": items,
                        "total": len(items),
                    },
                }
            )
        finally:
            loop.close()
    except Exception as e:
        logger.error("[获取预览索引] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
