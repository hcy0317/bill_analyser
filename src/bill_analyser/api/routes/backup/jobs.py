"""backup jobs route handlers."""

# pylint: disable=wildcard-import,undefined-variable,unused-wildcard-import,broad-exception-caught,line-too-long,too-many-locals,too-many-return-statements,too-many-branches,too-many-statements

from __future__ import annotations

from .support import *  # noqa: F403


def _backup_job_validation_audit(data: dict[str, object], error_message: str) -> None:
    """Write failed audit for invalid backup job payloads without logging secrets."""
    _write_backup_audit_log_sync(
        "backup_job_saved",
        details={
            "job_type": _audit_safe_input(data.get("job_type", "")),
            "id": _audit_safe_input(data.get("id", "")),
            "retention_days": _audit_safe_input(data.get("retention_days", "")),
            "retention_count": _audit_safe_input(data.get("retention_count", "")),
        },
        status="failed",
        error_message=error_message,
    )


def _parse_backup_job_id(raw_job_id) -> int | None:
    """Return a positive integer job id, or raise ValueError for invalid input."""
    if raw_job_id in (None, ""):
        return None
    if isinstance(raw_job_id, (bool, float)):
        raise ValueError("id must be a positive integer")
    if isinstance(raw_job_id, int):
        if raw_job_id > 0:
            return raw_job_id
        raise ValueError("id must be a positive integer")
    if isinstance(raw_job_id, str):
        stripped_job_id = raw_job_id.strip()
        if stripped_job_id.isdecimal():
            job_id = int(stripped_job_id)
            if job_id > 0:
                return job_id
    raise ValueError("id must be a positive integer")


@bp.route("/jobs", methods=["GET"])
@log_method
@require_backup_auth
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
@require_backup_auth
def save_backup_job():
    """创建或更新备份任务配置。"""
    try:
        data = request.get_json(silent=True) or {}
        job_type = str(data.get("job_type", "") or "").strip()
        if not job_type:
            _backup_job_validation_audit(data, "job_type is required")
            return jsonify({"success": False, "error": "job_type is required"}), 400

        retention_days_raw = data.get("retention_days", 30)
        try:
            retention_days = 30 if retention_days_raw is None else int(retention_days_raw)
        except (TypeError, ValueError):
            _backup_job_validation_audit(data, "retention_days must be an integer")
            return jsonify({"success": False, "error": "retention_days must be an integer"}), 400

        retention_count_raw = data.get("retention_count", 10)
        try:
            retention_count = 10 if retention_count_raw is None else int(retention_count_raw)
        except (TypeError, ValueError):
            _backup_job_validation_audit(data, "retention_count must be an integer")
            return jsonify({"success": False, "error": "retention_count must be an integer"}), 400

        if retention_count < 0:
            _backup_job_validation_audit(data, "retention_count must be greater than or equal to 0")
            return jsonify({"success": False, "error": "retention_count must be greater than or equal to 0"}), 400

        enabled_raw = data.get("enabled", True)
        enabled = enabled_raw if isinstance(enabled_raw, bool) else True
        raw_job_id = data.get("id")
        try:
            job_id = _parse_backup_job_id(raw_job_id)
        except ValueError:
            _backup_job_validation_audit(data, "id must be a positive integer")
            return jsonify({"success": False, "error": "id must be a positive integer"}), 400

        db = get_app_context()
        save_job_method = cast("Any", getattr(db, "create_or_update_backup_job", None))
        if not callable(save_job_method):
            return jsonify({"success": False, "error": "backup job storage is unavailable"}), 503

        payload = {
            "id": job_id,
            "job_type": job_type,
            "schedule_expr": str(data.get("schedule_expr", "") or "").strip(),
            "retention_days": retention_days,
            "retention_count": retention_count,
            "enabled": enabled,
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
