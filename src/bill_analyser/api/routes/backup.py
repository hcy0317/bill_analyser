"""
Backup Routes - 备份和恢复API路由
"""

import asyncio
import hashlib
import json
import os
import tempfile
from base64 import urlsafe_b64encode
from datetime import datetime
from pathlib import Path
from typing import cast
from zipfile import BadZipFile, ZipFile

from cryptography.fernet import Fernet, InvalidToken
from flask import Blueprint, current_app, jsonify, request, send_file
from werkzeug.utils import secure_filename

# pylint: disable=import-error
from bill_analyser.constants import BACKUP_DIR, DATA_DIR
from bill_analyser.utils.logger import log_method

bp = Blueprint("backup", __name__)


def get_app_context():
    """获取应用上下文中的数据库实例。"""
    return current_app.config.get("DB_INSTANCE")


async def _write_backup_audit_log_async(
    operation_type: str,
    *,
    details: dict | None = None,
    affected_count: int = 0,
    status: str = "success",
    error_message: str | None = None,
) -> None:
    """异步写入备份相关审计日志；缺失数据库时静默跳过。"""
    db = get_app_context()
    create_audit_log = getattr(db, "create_audit_log", None)
    if not callable(create_audit_log):
        return

    try:
        await create_audit_log(
            operation_type=operation_type,
            operation_target="backup",
            details=details,
            affected_count=affected_count,
            ip_address=request.remote_addr,
            user_agent=request.headers.get("User-Agent", ""),
            status=status,
            error_message=error_message,
        )
    except Exception:  # pylint: disable=broad-except
        return


def _write_backup_audit_log_sync(
    operation_type: str,
    *,
    details: dict | None = None,
    affected_count: int = 0,
    status: str = "success",
    error_message: str | None = None,
) -> None:
    """同步路由写备份审计日志的包装。"""
    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        loop.run_until_complete(
            _write_backup_audit_log_async(
                operation_type,
                details=details,
                affected_count=affected_count,
                status=status,
                error_message=error_message,
            )
        )
    finally:
        loop.close()


def _get_backup_metadata_path(backup_path: Path) -> Path:
    """返回备份 sidecar 元信息文件路径。"""
    return backup_path.with_suffix(f"{backup_path.suffix}.meta.json")


def _get_backup_encryption_secret() -> str:
    """读取本地备份加密密钥。"""
    return str(os.getenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "") or "").strip()


def _build_backup_fernet(secret: str) -> Fernet:
    """使用环境密钥构造稳定 Fernet key。"""
    digest = hashlib.sha256(secret.encode("utf-8")).digest()
    return Fernet(urlsafe_b64encode(digest))


def _encrypt_backup_file(file_path: Path, secret: str) -> Path:
    """加密备份文件并返回新路径。"""
    encrypted_path = Path(f"{file_path}.enc")
    fernet = _build_backup_fernet(secret)
    encrypted_path.write_bytes(fernet.encrypt(file_path.read_bytes()))
    file_path.unlink()
    return encrypted_path


def _decrypt_backup_file_to_temp(file_path: Path, secret: str) -> Path:
    """将加密备份解密到临时 zip 文件。"""
    fernet = _build_backup_fernet(secret)
    temp_fd, temp_path = tempfile.mkstemp(prefix="bill-analyser-backup-", suffix=".zip")
    os.close(temp_fd)
    decrypted_path = Path(temp_path)
    try:
        decrypted_path.write_bytes(fernet.decrypt(file_path.read_bytes()))
        return decrypted_path
    except Exception:
        if decrypted_path.exists():
            decrypted_path.unlink()
        raise


def _calculate_file_checksum(file_path: Path) -> str:
    """计算文件 SHA-256 校验值。"""
    hasher = hashlib.sha256()
    with open(file_path, "rb") as file_obj:
        for chunk in iter(lambda: file_obj.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def _inspect_backup_archive(backup_path: Path) -> dict:
    """检查备份压缩包结构并返回预验证摘要。"""
    summary = {
        "valid_zip": False,
        "contains_data_dir": False,
        "entry_count": 0,
        "top_level_entries": [],
        "error": "",
        "ready_to_restore": False,
    }

    try:
        with ZipFile(backup_path, "r") as zip_file:
            names = zip_file.namelist()
            summary["valid_zip"] = True
            summary["entry_count"] = len(names)
            summary["contains_data_dir"] = any(name.startswith("data/") for name in names)
            summary["top_level_entries"] = sorted({name.split("/", 1)[0] for name in names if name})
            summary["ready_to_restore"] = summary["contains_data_dir"] or bool(names)
            return summary
    except (BadZipFile, OSError) as exc:
        summary["error"] = str(exc)
        return summary


def _read_backup_metadata(backup_path: Path) -> dict:
    """读取 sidecar 元信息，不存在或损坏时返回空字典。"""
    metadata_path = _get_backup_metadata_path(backup_path)
    if not metadata_path.exists():
        return {}

    try:
        return json.loads(metadata_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def _persist_backup_metadata(backup_path: Path) -> dict:
    """生成并写入备份元信息 sidecar。"""
    stat = backup_path.stat()
    archive_summary = _inspect_backup_archive(backup_path)
    checksum = _calculate_file_checksum(backup_path)
    metadata = {
        "filename": backup_path.name,
        "checksum": checksum,
        "size": stat.st_size,
        "created_at": datetime.fromtimestamp(stat.st_mtime).isoformat(),
        "valid_zip": archive_summary["valid_zip"],
        "contains_data_dir": archive_summary["contains_data_dir"],
        "entry_count": archive_summary["entry_count"],
        "top_level_entries": archive_summary["top_level_entries"],
        "ready_to_restore": archive_summary["ready_to_restore"],
    }
    metadata_path = _get_backup_metadata_path(backup_path)
    metadata_path.write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding="utf-8")
    return metadata


def _build_backup_info(file_path: Path) -> dict:
    """构建统一的备份列表/创建响应。"""
    stat = file_path.stat()
    is_encrypted = file_path.suffix == ".enc"
    inspection_target = file_path
    temp_decrypted_path: Path | None = None

    if is_encrypted:
        secret = _get_backup_encryption_secret()
        if secret:
            try:
                temp_decrypted_path = _decrypt_backup_file_to_temp(file_path, secret)
                inspection_target = temp_decrypted_path
            except (InvalidToken, OSError, ValueError) as exc:
                archive_summary = {
                    "valid_zip": False,
                    "contains_data_dir": False,
                    "entry_count": 0,
                    "top_level_entries": [],
                    "error": str(exc),
                    "ready_to_restore": False,
                }
            else:
                archive_summary = _inspect_backup_archive(inspection_target)
        else:
            archive_summary = {
                "valid_zip": False,
                "contains_data_dir": False,
                "entry_count": 0,
                "top_level_entries": [],
                "error": "backup encryption key is not configured",
                "ready_to_restore": False,
            }
    else:
        archive_summary = _inspect_backup_archive(inspection_target)

    checksum = _calculate_file_checksum(file_path)
    metadata = _read_backup_metadata(file_path)
    metadata_checksum = str(metadata.get("checksum", "") or "")

    if not metadata or metadata.get("size") != stat.st_size:
        metadata = _persist_backup_metadata(file_path)
        metadata_checksum = metadata["checksum"]

    result = {
        "filename": file_path.name,
        "size": stat.st_size,
        "created_at": datetime.fromtimestamp(stat.st_mtime).isoformat(),
        "path": str(file_path),
        "checksum": checksum,
        "encrypted": is_encrypted,
        "valid_zip": archive_summary["valid_zip"],
        "contains_data_dir": archive_summary["contains_data_dir"],
        "entry_count": archive_summary["entry_count"],
        "top_level_entries": archive_summary["top_level_entries"],
        "ready_to_restore": archive_summary["ready_to_restore"],
        "metadata_checksum_matched": bool(metadata_checksum) and metadata_checksum == checksum,
    }
    if temp_decrypted_path and temp_decrypted_path.exists():
        temp_decrypted_path.unlink()
    return result


async def _upsert_backup_record_async(backup_info: dict) -> None:
    """将备份信息写入 backup_records。"""
    db = get_app_context()
    create_backup_record = getattr(db, "create_backup_record", None)
    if not callable(create_backup_record):
        return

    await create_backup_record(
        {
            "backup_name": backup_info["filename"],
            "storage_type": "local",
            "file_path": backup_info["path"],
            "checksum": backup_info["checksum"],
            "encrypted": backup_info["encrypted"],
            "status": "created",
            "metadata": {
                "valid_zip": backup_info["valid_zip"],
                "contains_data_dir": backup_info["contains_data_dir"],
                "entry_count": backup_info["entry_count"],
                "top_level_entries": backup_info["top_level_entries"],
                "ready_to_restore": backup_info["ready_to_restore"],
            },
        }
    )


def _merge_backup_records(backup_infos: list[dict]) -> list[dict]:
    """将 backup_records 中的稳定状态合并到备份列表响应。"""
    db = get_app_context()
    get_backup_records = getattr(db, "get_backup_records", None)
    if not callable(get_backup_records):
        return backup_infos

    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        records = loop.run_until_complete(get_backup_records())
    finally:
        loop.close()

    record_map = {str(record.get("backup_name") or ""): record for record in records}
    merged = []
    for backup_info in backup_infos:
        record = record_map.get(backup_info["filename"])
        merged_info = dict(backup_info)
        if record:
            merged_info["recordId"] = record.get("id")
            merged_info["recordStatus"] = record.get("status")
            merged_info["recordCreatedAt"] = record.get("created_at")
        merged.append(merged_info)
    return merged


def _iter_local_backup_files(backup_dir: Path) -> list[Path]:
    """返回本地备份文件列表，包含明文与加密备份。"""
    backup_files = list(backup_dir.glob("backup_*.zip"))
    backup_files.extend(backup_dir.glob("backup_*.zip.enc"))
    unique_files = {str(file_path.resolve()): file_path for file_path in backup_files}
    return list(unique_files.values())


def _resolve_backup_path_in_dir(backup_dir: Path, filename: str) -> Path | None:
    """基于备份目录安全解析备份文件路径。"""
    safe_filename = secure_filename(filename)
    if not safe_filename.startswith("backup_") or not (
        safe_filename.endswith(".zip") or safe_filename.endswith(".zip.enc")
    ):
        return None

    backup_dir_resolved = backup_dir.resolve()
    resolved_path = (backup_dir / safe_filename).resolve()
    try:
        resolved_path.relative_to(backup_dir_resolved)
    except ValueError:
        return None

    return resolved_path


def _update_backup_record_sync(filename: str, updates: dict[str, object]) -> bool:
    """同步包装 backup_records 更新。"""
    db = get_app_context()
    update_backup_record = getattr(db, "update_backup_record_by_filename", None)
    if not callable(update_backup_record):
        return False

    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return bool(loop.run_until_complete(update_backup_record(filename, updates)))
    finally:
        loop.close()


def get_backup_dir() -> Path:
    """获取备份目录"""
    backup_dir = BACKUP_DIR
    backup_dir.mkdir(parents=True, exist_ok=True)
    return backup_dir


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

        safe_filename = secure_filename(filename)
        if not safe_filename.startswith("backup_") or not (
            safe_filename.endswith(".zip") or safe_filename.endswith(".zip.enc")
        ):
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        backup_dir = get_backup_dir()
        file_path = backup_dir / safe_filename
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
        safe_filename = secure_filename(filename)
        if not safe_filename.startswith("backup_") or not (
            safe_filename.endswith(".zip") or safe_filename.endswith(".zip.enc")
        ):
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        backup_dir = get_backup_dir()
        file_path = backup_dir / safe_filename

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
        safe_filename = secure_filename(filename)
        if not safe_filename.startswith("backup_") or not (
            safe_filename.endswith(".zip") or safe_filename.endswith(".zip.enc")
        ):
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        backup_dir = get_backup_dir()
        file_path = backup_dir / safe_filename

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
        safe_filename = secure_filename(filename)
        if not safe_filename.startswith("backup_") or not (
            safe_filename.endswith(".zip") or safe_filename.endswith(".zip.enc")
        ):
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        backup_dir = get_backup_dir()
        file_path = backup_dir / safe_filename

        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        restore_source_path = file_path
        temp_decrypted_path: Path | None = None
        if file_path.suffix == ".enc":
            encryption_secret = _get_backup_encryption_secret()
            if not encryption_secret:
                return jsonify({"success": False, "error": "备份加密密钥未配置"}), 400
            temp_decrypted_path = _decrypt_backup_file_to_temp(file_path, encryption_secret)
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
            update_backup_record = getattr(get_app_context(), "update_backup_record_by_filename", None)
            if callable(update_backup_record):
                await update_backup_record(
                    safe_filename,
                    {"status": "restored", "metadata": {"restored_at": restored_at}},
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
        get_backup_records = getattr(db, "get_backup_records", None)

        if callable(get_backup_records):
            loop = asyncio.new_event_loop()
            try:
                asyncio.set_event_loop(loop)
                backup_records = loop.run_until_complete(get_backup_records())
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

            active_record_entries.sort(
                key=lambda item: (str(item.get("created_at") or ""), int(item.get("id") or 0)),
                reverse=True,
            )

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


@bp.route("/jobs", methods=["GET"])
@log_method
def list_backup_jobs():
    """获取备份任务配置列表。"""
    try:
        db = get_app_context()
        get_backup_jobs = getattr(db, "get_backup_jobs", None)
        if not callable(get_backup_jobs):
            return jsonify({"success": True, "data": []})

        loop = asyncio.new_event_loop()
        try:
            asyncio.set_event_loop(loop)
            jobs = loop.run_until_complete(get_backup_jobs())
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
        save_job_method = getattr(db, "create_or_update_backup_job", None)
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
            job_id = loop.run_until_complete(save_job_method(payload))
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
