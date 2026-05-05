"""categories analytics route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/statistics", methods=["GET"])
@log_method
@require_auth
def get_category_statistics():
    """获取分类统计"""
    try:
        # 获取查询参数
        period = request.args.get("period", "month")
        start_date = request.args.get("start_date")
        end_date = request.args.get("end_date")

        db, _, _ = get_app_context()
        stats_list = _run_async(
            db.get_category_statistics(
                period=period,
                start_date=start_date,
                end_date=end_date,
                user_id=_get_request_user_id(),
            )
        )

        # 转换为树形字典结构
        stats_dict = {}
        for stat in stats_list:
            main_cat = stat.get("main_category", "未分类")
            sub_cat = stat.get("sub_category", "")

            if main_cat not in stats_dict:
                stats_dict[main_cat] = {"total_amount": 0, "count": 0, "sub_categories": {}}

            stats_dict[main_cat]["total_amount"] += abs(float(stat.get("total_amount", 0)))
            stats_dict[main_cat]["count"] += stat.get("count", 0)

            if sub_cat:
                stats_dict[main_cat]["sub_categories"][sub_cat] = {
                    "total_amount": abs(float(stat.get("total_amount", 0))),
                    "count": stat.get("count", 0),
                }

        return jsonify({"success": True, "result": stats_dict})

    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("获取分类统计失败: %s", exc)
        return jsonify({"success": False, "error": str(exc)}), 500
