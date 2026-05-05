"""backup files route handlers."""

from __future__ import annotations

from .support import *  # noqa: F403


@bp.route("/", methods=["GET"])
@log_method
def get_backups():
    """
    获取备份列表

    Returns:
        JSON响应，包含备份文件列表
    """
    try:
        backup_dir = get_backup_dir()
        backups = []

        for file_path in _iter_local_backup_files(backup_dir):
            backups.append(_build_backup_info(file_path))

        backups = _merge_backup_records(backups)

        # 按创建时间降序排序
        backups.sort(key=lambda x: x["created_at"], reverse=True)

        return jsonify({"success": True, "data": backups})
    except Exception as e:
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/create", methods=["POST"])
@log_method
async def create_backup():
    """
    创建新备份

    Returns:
        JSON响应，包含备份文件信息
    """
    try:
        # pylint: disable=import-outside-toplevel
        from bill_analyser.core.sync import SyncManager

        sync_manager = SyncManager()
        backup_path = await sync_manager.backup_local()

        if not backup_path:
            return jsonify({"success": False, "error": "数据未变化，无需备份"}), 400

        file_path = Path(backup_path)
        encryption_secret = _get_backup_encryption_secret()
        if encryption_secret:
            file_path = _encrypt_backup_file(file_path, encryption_secret)
        backup_info = _build_backup_info(file_path)
        await _upsert_backup_record_async(backup_info)

        await _write_backup_audit_log_async(
            "backup_created",
            details={
                "filename": file_path.name,
                "path": str(file_path),
                "checksum": backup_info["checksum"],
            },
            affected_count=1,
        )

        return jsonify(
            {
                "success": True,
                "data": backup_info,
            }
        )
    except Exception as e:
        await _write_backup_audit_log_async(
            "backup_created",
            details={"filename": "", "path": ""},
            status="failed",
            error_message=str(e),
        )
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/restore/verify", methods=["POST"])
@log_method
def verify_backup_restore():
    """恢复前校验备份文件结构与校验值。"""
    try:
        data = request.get_json(silent=True) or {}
        filename = str(data.get("filename", "") or "").strip()
        if not filename:
            return jsonify({"success": False, "error": "filename is required"}), 400

        backup_dir = get_backup_dir()
        file_path = _resolve_backup_path_in_dir(backup_dir, filename)
        if file_path is None:
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        safe_filename = file_path.name
        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        backup_info = _build_backup_info(file_path)
        _write_backup_audit_log_sync(
            "backup_restore_verified",
            details={
                "filename": safe_filename,
                "checksum": backup_info["checksum"],
                "valid_zip": backup_info["valid_zip"],
                "ready_to_restore": backup_info["ready_to_restore"],
                "metadata_checksum_matched": backup_info["metadata_checksum_matched"],
            },
            status="success" if backup_info["valid_zip"] else "failed",
            error_message=None if backup_info["valid_zip"] else backup_info.get("error") or "invalid backup archive",
        )
        status_code = 200 if backup_info["valid_zip"] else 400
        return jsonify({"success": backup_info["valid_zip"], "data": backup_info}), status_code
    except Exception as e:
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/download/<filename>", methods=["GET"])
@log_method
def download_backup(filename):
    """
    下载备份文件

    Args:
        filename: 备份文件名

    Returns:
        备份文件
    """
    try:
        # 安全文件名检查
        backup_dir = get_backup_dir()
        file_path = _resolve_backup_path_in_dir(backup_dir, filename)
        if file_path is None:
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        safe_filename = file_path.name

        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        return send_file(file_path, as_attachment=True, download_name=safe_filename)
    except Exception as e:
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/delete/<filename>", methods=["DELETE"])
@log_method
def delete_backup(filename):
    """
    删除备份文件

    Args:
        filename: 备份文件名

    Returns:
        JSON响应
    """
    try:
        # 安全文件名检查
        backup_dir = get_backup_dir()
        file_path = _resolve_backup_path_in_dir(backup_dir, filename)
        if file_path is None:
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        safe_filename = file_path.name

        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        file_path.unlink()
        metadata_path = _get_backup_metadata_path(file_path)
        if metadata_path.exists():
            metadata_path.unlink()

        _update_backup_record_sync(
            safe_filename,
            {
                "status": "deleted",
                "metadata": {
                    "deleted_reason": "manual_delete",
                    "deleted_at": datetime.now().isoformat(),
                },
            },
        )

        _write_backup_audit_log_sync(
            "backup_deleted",
            details={"filename": safe_filename},
            affected_count=1,
        )

        return jsonify({"success": True, "data": {"filename": safe_filename}})
    except Exception as e:
        _write_backup_audit_log_sync(
            "backup_deleted",
            details={"filename": str(filename or "")},
            status="failed",
            error_message=str(e),
        )
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/restore/<filename>", methods=["POST"])
@log_method
async def restore_backup(filename):
    """
    恢复备份

    Args:
        filename: 备份文件名

    Returns:
        JSON响应
    """
    try:
        # pylint: disable=import-outside-toplevel
        import shutil
        # 安全文件名检查
        backup_dir = get_backup_dir()
        file_path = _resolve_backup_path_in_dir(backup_dir, filename)
        if file_path is None:
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        safe_filename = file_path.name

        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        restore_source_path = file_path
        temp_decrypted_path: Path | None = None
        if file_path.suffix == ".enc":
            encryption_secret = _get_backup_encryption_secret()
            if not encryption_secret:
                return jsonify({"success": False, "error": "备份加密密钥未配置"}), 400
            try:
                temp_decrypted_path = _decrypt_backup_file_to_temp(file_path, encryption_secret)
            except (InvalidToken, OSError, ValueError):
                await _write_backup_audit_log_async(
                    "backup_restored",
                    details={"filename": safe_filename},
                    status="failed",
                    error_message="backup decrypt failed",
                )
                return jsonify({"success": False, "error": "备份解密失败"}), 400
            restore_source_path = temp_decrypted_path

        # 获取数据目录 - 固定为 data 文件夹
        data_dir = DATA_DIR

        # 创建临时恢复目录
        temp_restore_dir = backup_dir / f"restore_temp_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
        temp_restore_dir.mkdir(parents=True, exist_ok=True)

        try:
            # 解压备份文件
            with ZipFile(restore_source_path, "r") as zipf:
                zipf.extractall(temp_restore_dir)

            # 备份当前数据
            if data_dir.exists():
                backup_current = backup_dir / f"before_restore_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
                shutil.copytree(data_dir, backup_current)

            # 恢复数据
            extracted_data_dir = temp_restore_dir / "data"
            if extracted_data_dir.exists():
                if data_dir.exists():
                    shutil.rmtree(data_dir)
                shutil.copytree(extracted_data_dir, data_dir)
            else:
                # 如果解压后直接是数据文件，移动整个目录
                if data_dir.exists():
                    shutil.rmtree(data_dir)
                shutil.copytree(temp_restore_dir, data_dir)

            # 清理临时目录
            shutil.rmtree(temp_restore_dir)

            restored_at = datetime.now().isoformat()
            update_backup_record = cast("Any", getattr(get_app_context(), "update_backup_record_by_filename", None))
            if callable(update_backup_record):
                await cast(
                    "Any",
                    update_backup_record(
                        safe_filename,
                        {"status": "restored", "metadata": {"restored_at": restored_at}},
                    ),
                )
            await _write_backup_audit_log_async(
                "backup_restored",
                details={"filename": safe_filename, "restored_at": restored_at},
                affected_count=1,
            )

            return jsonify({"success": True, "data": {"filename": safe_filename, "restored_at": restored_at}})

        except Exception as e:
            # 清理临时目录
            if temp_restore_dir.exists():
                shutil.rmtree(temp_restore_dir)
            raise e
        finally:
            if temp_decrypted_path and temp_decrypted_path.exists():
                temp_decrypted_path.unlink()

    except Exception as e:
        await _write_backup_audit_log_async(
            "backup_restored",
            details={"filename": str(filename or "")},
            status="failed",
            error_message=str(e),
        )
        return jsonify({"success": False, "error": str(e)}), 500
