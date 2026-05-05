"""backup cleanup route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/cleanup", methods=["POST"])
@log_method
def cleanup_old_backups():
    """
    清理旧备份

    Request JSON:
        {
            "keep_count": 10  # 保留最新的N个备份
        }

    Returns:
        JSON响应，包含删除的备份数量
    """
    try:
        data = request.get_json() or {}
        keep_count = data.get("keep_count", 10)
        try:
            keep_count = int(keep_count)
        except (TypeError, ValueError):
            return jsonify({"success": False, "error": "keep_count must be an integer"}), 400

        if keep_count < 0:
            return jsonify({"success": False, "error": "keep_count must be greater than or equal to 0"}), 400

        backup_dir = get_backup_dir()
        deleted_count = 0
        db = get_app_context()
        get_backup_records = cast("Any", getattr(db, "get_backup_records", None))

        if callable(get_backup_records):
            loop = asyncio.new_event_loop()
            try:
                asyncio.set_event_loop(loop)
                backup_records = loop.run_until_complete(cast("Any", get_backup_records()))
            finally:
                loop.close()

            active_record_entries: list[dict[str, object]] = []
            for record in backup_records:
                if str(record.get("storage_type", "local") or "local") != "local":
                    continue
                if str(record.get("status", "") or "") == "deleted":
                    continue

                backup_name = str(record.get("backup_name") or "")
                file_path = _resolve_backup_path_in_dir(backup_dir, backup_name)
                if file_path is None:
                    continue

                if not file_path.exists():
                    _update_backup_record_sync(
                        backup_name,
                        {
                            "status": "deleted",
                            "metadata": {
                                "deleted_reason": "missing_file",
                                "deleted_at": datetime.now().isoformat(),
                            },
                        },
                    )
                    continue

                active_record_entries.append({**record, "path": file_path})

            def _record_sort_key(item: dict[str, object]) -> tuple[str, int]:
                record_id = item.get("id")
                normalized_id = record_id if isinstance(record_id, int) else 0
                return str(item.get("created_at") or ""), normalized_id

            active_record_entries.sort(key=_record_sort_key, reverse=True)

            kept_record_entries = active_record_entries[:keep_count]
            retention_candidates = active_record_entries[keep_count:]
            kept_record_names = {
                str(cast("Path", item["path"]).name)
                for item in kept_record_entries
                if isinstance(item.get("path"), Path)
            }

            for backup in retention_candidates:
                backup_name = str(backup.get("backup_name") or "")
                file_path = cast("Path", backup["path"])
                if file_path.exists():
                    file_path.unlink()
                metadata_path = _get_backup_metadata_path(file_path)
                if metadata_path.exists():
                    metadata_path.unlink()
                _update_backup_record_sync(
                    backup_name,
                    {
                        "status": "deleted",
                        "metadata": {
                            "deleted_reason": "retention_cleanup",
                            "deleted_at": datetime.now().isoformat(),
                        },
                    },
                )
                deleted_count += 1

            stray_files = [
                file_path
                for file_path in _iter_local_backup_files(backup_dir)
                if file_path.name not in kept_record_names
            ]
            stray_files.sort(key=lambda item: item.stat().st_mtime, reverse=True)

            remaining_slots = max(keep_count - len(kept_record_entries), 0)
            for stray_file in stray_files[remaining_slots:]:
                stray_file.unlink()
                metadata_path = _get_backup_metadata_path(stray_file)
                if metadata_path.exists():
                    metadata_path.unlink()
                deleted_count += 1

            kept_count = min(len(kept_record_entries) + min(len(stray_files), remaining_slots), keep_count)
        else:
            backups = []
            for file_path in _iter_local_backup_files(backup_dir):
                stat = file_path.stat()
                backups.append({"path": file_path, "mtime": stat.st_mtime})

            backups.sort(key=lambda x: x["mtime"], reverse=True)

            for backup in backups[keep_count:]:
                backup["path"].unlink()
                metadata_path = _get_backup_metadata_path(backup["path"])
                if metadata_path.exists():
                    metadata_path.unlink()
                deleted_count += 1
            kept_count = min(len(backups), keep_count)

        _write_backup_audit_log_sync(
            "backup_cleanup",
            details={"keep_count": keep_count, "deleted_count": deleted_count},
            affected_count=deleted_count,
        )

        return jsonify(
            {"success": True, "data": {"deleted_count": deleted_count, "kept_count": kept_count}}
        )
    except Exception as e:
        return jsonify({"success": False, "error": str(e)}), 500
