"""
Backup Routes - 备份和恢复API路由
"""

# pylint: disable=line-too-long,broad-exception-caught,too-many-locals,too-many-branches,too-many-statements

import asyncio
import hashlib
import json
import os
import tempfile
from base64 import urlsafe_b64encode
from datetime import datetime
from pathlib import Path
from typing import Any, cast
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
    create_audit_log = cast("Any", getattr(db, "create_audit_log", None))
    if not callable(create_audit_log):
        return

    try:
        await cast(
            "Any",
            create_audit_log(
                operation_type=operation_type,
                operation_target="backup",
                details=details,
                affected_count=affected_count,
                ip_address=request.remote_addr,
                user_agent=request.headers.get("User-Agent", ""),
                status=status,
                error_message=error_message,
            ),
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
    create_backup_record = cast("Any", getattr(db, "create_backup_record", None))
    if not callable(create_backup_record):
        return

    await cast(
        "Any",
        create_backup_record(
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
        ),
    )


def _merge_backup_records(backup_infos: list[dict]) -> list[dict]:
    """将 backup_records 中的稳定状态合并到备份列表响应。"""
    db = get_app_context()
    get_backup_records = cast("Any", getattr(db, "get_backup_records", None))
    if not callable(get_backup_records):
        return backup_infos

    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        records = loop.run_until_complete(cast("Any", get_backup_records()))
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
    update_backup_record = cast("Any", getattr(db, "update_backup_record_by_filename", None))
    if not callable(update_backup_record):
        return False

    loop = asyncio.new_event_loop()
    try:
        asyncio.set_event_loop(loop)
        return bool(loop.run_until_complete(cast("Any", update_backup_record(filename, updates))))
    finally:
        loop.close()


def get_backup_dir() -> Path:
    """获取备份目录"""
    backup_dir = BACKUP_DIR
    backup_dir.mkdir(parents=True, exist_ok=True)
    return backup_dir

__all__ = [name for name in globals() if not name.startswith("__")]
