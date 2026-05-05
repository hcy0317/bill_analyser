"""backup jobs route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/jobs", methods=["GET"])
@log_method
def list_backup_jobs():
    """获取备份任务配置列表。"""
    try:
        db = get_app_context()
        get_backup_jobs = cast("Any", getattr(db, "get_backup_jobs", None))
        if not callable(get_backup_jobs):
            return jsonify({"success": True, "data": []})

        loop = asyncio.new_event_loop()
        try:
            asyncio.set_event_loop(loop)
            jobs = loop.run_until_complete(cast("Any", get_backup_jobs)())
        finally:
            loop.close()

        return jsonify({"success": True, "data": jobs})
    except Exception as e:
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/jobs", methods=["POST"])
@log_method
def save_backup_job():
    """创建或更新备份任务配置。"""
    try:
        data = request.get_json(silent=True) or {}
        job_type = str(data.get("job_type", "") or "").strip()
        if not job_type:
            return jsonify({"success": False, "error": "job_type is required"}), 400

        try:
            retention_days = int(data.get("retention_days", 30))
        except (TypeError, ValueError):
            return jsonify({"success": False, "error": "retention_days must be an integer"}), 400

        try:
            retention_count = int(data.get("retention_count", 10))
        except (TypeError, ValueError):
            return jsonify({"success": False, "error": "retention_count must be an integer"}), 400

        db = get_app_context()
        save_job_method = cast("Any", getattr(db, "create_or_update_backup_job", None))
        if not callable(save_job_method):
            return jsonify({"success": False, "error": "backup job storage is unavailable"}), 503

        payload = {
            "id": data.get("id"),
            "job_type": job_type,
            "schedule_expr": str(data.get("schedule_expr", "") or "").strip(),
            "retention_days": retention_days,
            "retention_count": retention_count,
            "enabled": bool(data.get("enabled", True)),
            "last_status": data.get("last_status"),
        }

        loop = asyncio.new_event_loop()
        try:
            asyncio.set_event_loop(loop)
            job_id = loop.run_until_complete(cast("Any", save_job_method)(payload))
        finally:
            loop.close()

        _write_backup_audit_log_sync(
            "backup_job_saved",
            details={
                "job_id": job_id,
                "job_type": job_type,
                "retention_days": retention_days,
                "retention_count": retention_count,
            },
            affected_count=1,
        )

        response_payload = dict(payload)
        response_payload["id"] = job_id
        return jsonify({"success": True, "data": response_payload})
    except Exception as e:
        _write_backup_audit_log_sync(
            "backup_job_saved",
            details={"job_type": str((request.get_json(silent=True) or {}).get("job_type", "") or "")},
            status="failed",
            error_message=str(e),
        )
        return jsonify({"success": False, "error": str(e)}), 500
