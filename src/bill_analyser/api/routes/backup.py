"""
Backup Routes - 备份和恢复API路由
"""

from datetime import datetime
from pathlib import Path

from flask import Blueprint, jsonify, request, send_file
from werkzeug.utils import secure_filename

# pylint: disable=import-error
from bill_analyser.constants import BACKUP_DIR, DATA_DIR
from bill_analyser.utils.logger import log_method

bp = Blueprint("backup", __name__)


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

        for file_path in backup_dir.glob("backup_*.zip"):
            stat = file_path.stat()
            backups.append(
                {
                    "filename": file_path.name,
                    "size": stat.st_size,
                    "created_at": datetime.fromtimestamp(stat.st_mtime).isoformat(),
                    "path": str(file_path),
                }
            )

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

        # 获取文件信息
        file_path = Path(backup_path)
        stat = file_path.stat()

        return jsonify(
            {
                "success": True,
                "data": {
                    "filename": file_path.name,
                    "size": stat.st_size,
                    "created_at": datetime.fromtimestamp(stat.st_mtime).isoformat(),
                    "path": str(file_path),
                },
            }
        )
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
        if not safe_filename.startswith("backup_") or not safe_filename.endswith(".zip"):
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
        if not safe_filename.startswith("backup_") or not safe_filename.endswith(".zip"):
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        backup_dir = get_backup_dir()
        file_path = backup_dir / safe_filename

        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        file_path.unlink()

        return jsonify({"success": True, "data": {"filename": safe_filename}})
    except Exception as e:
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
        import zipfile

        # 安全文件名检查
        safe_filename = secure_filename(filename)
        if not safe_filename.startswith("backup_") or not safe_filename.endswith(".zip"):
            return jsonify({"success": False, "error": "无效的文件名"}), 400

        backup_dir = get_backup_dir()
        file_path = backup_dir / safe_filename

        if not file_path.exists():
            return jsonify({"success": False, "error": "文件不存在"}), 404

        # 获取数据目录 - 固定为 data 文件夹
        data_dir = DATA_DIR

        # 创建临时恢复目录
        temp_restore_dir = backup_dir / f"restore_temp_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
        temp_restore_dir.mkdir(parents=True, exist_ok=True)

        try:
            # 解压备份文件
            with zipfile.ZipFile(file_path, "r") as zipf:
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

            return jsonify(
                {"success": True, "data": {"filename": safe_filename, "restored_at": datetime.now().isoformat()}}
            )

        except Exception as e:
            # 清理临时目录
            if temp_restore_dir.exists():
                shutil.rmtree(temp_restore_dir)
            raise e

    except Exception as e:
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

        backup_dir = get_backup_dir()
        backups = []

        for file_path in backup_dir.glob("backup_*.zip"):
            stat = file_path.stat()
            backups.append({"path": file_path, "mtime": stat.st_mtime})

        # 按时间排序
        backups.sort(key=lambda x: x["mtime"], reverse=True)

        # 删除超出保留数量的备份
        deleted_count = 0
        for backup in backups[keep_count:]:
            backup["path"].unlink()
            deleted_count += 1

        return jsonify(
            {"success": True, "data": {"deleted_count": deleted_count, "kept_count": min(len(backups), keep_count)}}
        )
    except Exception as e:
        return jsonify({"success": False, "error": str(e)}), 500
