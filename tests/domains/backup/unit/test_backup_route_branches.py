from __future__ import annotations

import asyncio
import json
import os
from collections.abc import Callable
from pathlib import Path
from typing import Any, cast
from zipfile import ZipFile

import pytest
from flask import Flask

from bill_analyser.api.routes import backup as backup_module
from bill_analyser.core import sync as sync_module


@pytest.fixture(name="backup_route_app")
def backup_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct backup route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    wrapped = getattr(current, "__wrapped__", None)
    return cast("Callable[..., Any]", wrapped or current)


def _unwrap_response(result: Any) -> tuple[Any, int]:
    if isinstance(result, tuple):
        response, status = result
        return response, status
    return result, result.status_code


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)


class FakeBackupDB:
    def __init__(self) -> None:
        self.audit_logs: list[dict[str, Any]] = []

    async def create_audit_log(self, **payload: Any) -> None:
        self.audit_logs.append(payload)


def test_backup_listing_download_delete_and_cleanup_branches(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """备份路由应覆盖列出、下载、删除和清理旧备份的主要分支。"""
    backup_dir = tmp_path / "backups"
    backup_dir.mkdir(parents=True, exist_ok=True)

    older = backup_dir / "backup_older.zip"
    newer = backup_dir / "backup_newer.zip"
    older.write_bytes(b"older")
    newer.write_bytes(b"newer")
    older_stat = older.stat().st_mtime - 10
    newer_stat = newer.stat().st_mtime
    os.utime(older, (older_stat, older_stat))
    os.utime(newer, (newer_stat, newer_stat))

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)

    get_backups = _unwrap(backup_module.get_backups)
    download_backup = _unwrap(backup_module.download_backup)
    delete_backup = _unwrap(backup_module.delete_backup)
    cleanup_backups = _unwrap(backup_module.cleanup_old_backups)

    with backup_route_app.test_request_context("/api/backup/"):
        payload = get_backups().get_json() or {}
        assert payload["success"] is True
        assert [item["filename"] for item in payload["data"]] == ["backup_newer.zip", "backup_older.zip"]

    with backup_route_app.test_request_context("/api/backup/download/not-valid.txt"):
        response, status = download_backup("not-valid.txt")
        assert status == 400
        assert response.get_json()["error"] == "无效的文件名"

    with backup_route_app.test_request_context("/api/backup/download/backup_missing.zip"):
        response, status = download_backup("backup_missing.zip")
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"

    with backup_route_app.test_request_context("/api/backup/download/backup_newer.zip"):
        response = download_backup("backup_newer.zip")
        assert response.status_code == 200

    with backup_route_app.test_request_context("/api/backup/delete/not-valid.txt", method="DELETE"):
        response, status = delete_backup("not-valid.txt")
        assert status == 400
        assert response.get_json()["error"] == "无效的文件名"

    with backup_route_app.test_request_context("/api/backup/delete/backup_missing.zip", method="DELETE"):
        response, status = delete_backup("backup_missing.zip")
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"

    with backup_route_app.test_request_context("/api/backup/delete/backup_older.zip", method="DELETE"):
        payload = delete_backup("backup_older.zip").get_json() or {}
        assert payload["success"] is True
        assert older.exists() is False

    extra_a = backup_dir / "backup_a.zip"
    extra_b = backup_dir / "backup_b.zip"
    extra_a.write_bytes(b"a")
    extra_b.write_bytes(b"b")

    with backup_route_app.test_request_context("/api/backup/cleanup", method="POST", json={"keep_count": 1}):
        payload = cleanup_backups().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["deleted_count"] >= 1
        assert payload["data"]["kept_count"] == 1


def test_backup_create_and_restore_cover_success_empty_and_missing_cases(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """创建/恢复备份应覆盖无变化、成功恢复、非法文件名和缺失文件。"""
    backup_dir = tmp_path / "backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "data"
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "old.txt").write_text("old", encoding="utf-8")

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    created_backup = backup_dir / "backup_created.zip"
    created_backup.write_bytes(b"created")

    class FakeSyncManager:
        return_none = True

        async def backup_local(self) -> str | None:
            return None if self.return_none else str(created_backup)

    monkeypatch.setattr(sync_module, "SyncManager", FakeSyncManager)

    create_backup = _unwrap(backup_module.create_backup)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context("/api/backup/create", method="POST"):
        response, status = asyncio.run(create_backup())
        assert status == 400
        assert response.get_json()["error"] == "数据未变化，无需备份"

    FakeSyncManager.return_none = False
    with backup_route_app.test_request_context("/api/backup/create", method="POST"):
        payload = asyncio.run(create_backup()).get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["filename"] == "backup_created.zip"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_created"

    with backup_route_app.test_request_context("/api/backup/restore/not-valid.txt", method="POST"):
        response, status = asyncio.run(restore_backup("not-valid.txt"))
        assert status == 400
        assert response.get_json()["error"] == "无效的文件名"

    with backup_route_app.test_request_context("/api/backup/restore/backup_missing.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_missing.zip"))
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"

    valid_backup = backup_dir / "backup_valid.zip"
    with ZipFile(valid_backup, "w") as zip_file:
        zip_file.writestr("data/restored.txt", "restored")

    with backup_route_app.test_request_context("/api/backup/restore/backup_valid.zip", method="POST"):
        payload = asyncio.run(restore_backup("backup_valid.zip")).get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["filename"] == "backup_valid.zip"
        assert (data_dir / "restored.txt").read_text(encoding="utf-8") == "restored"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"


def test_backup_routes_cover_directory_creation_restore_fallback_and_error_paths(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """备份路由应覆盖目录创建、restore fallback 分支和异常兜底。"""
    nested_backup_dir = tmp_path / "runtime" / "backups"
    monkeypatch.setattr(backup_module, "BACKUP_DIR", nested_backup_dir)
    created_dir = backup_module.get_backup_dir()
    assert created_dir == nested_backup_dir
    assert created_dir.exists() is True

    get_backups = _unwrap(backup_module.get_backups)
    create_backup = _unwrap(backup_module.create_backup)
    download_backup = _unwrap(backup_module.download_backup)
    delete_backup = _unwrap(backup_module.delete_backup)
    restore_backup = _unwrap(backup_module.restore_backup)
    cleanup_backups = _unwrap(backup_module.cleanup_old_backups)

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: _raise_runtime_error("backup dir boom"))

    with backup_route_app.test_request_context("/api/backup/"):
        response, status = get_backups()
        assert status == 500
        assert response.get_json()["error"] == "backup dir boom"

    class ExplodingSyncManager:
        async def backup_local(self) -> str | None:
            raise RuntimeError("create boom")

    monkeypatch.setattr(sync_module, "SyncManager", ExplodingSyncManager)
    with backup_route_app.test_request_context("/api/backup/create", method="POST"):
        response, status = asyncio.run(create_backup())
        assert status == 500
        assert response.get_json()["error"] == "create boom"

    with backup_route_app.test_request_context("/api/backup/download/backup_any.zip"):
        response, status = download_backup("backup_any.zip")
        assert status == 500
        assert response.get_json()["error"] == "backup dir boom"

    with backup_route_app.test_request_context("/api/backup/delete/backup_any.zip", method="DELETE"):
        response, status = delete_backup("backup_any.zip")
        assert status == 500
        assert response.get_json()["error"] == "backup dir boom"

    with backup_route_app.test_request_context("/api/backup/cleanup", method="POST", json={"keep_count": 2}):
        response, status = cleanup_backups()
        assert status == 500
        assert response.get_json()["error"] == "backup dir boom"

    backup_dir = tmp_path / "restore_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "restore_data"
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "old.txt").write_text("old", encoding="utf-8")

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)

    flat_backup = backup_dir / "backup_flat.zip"
    with ZipFile(flat_backup, "w") as zip_file:
        zip_file.writestr("flat.txt", "flat-restored")

    with backup_route_app.test_request_context("/api/backup/restore/backup_flat.zip", method="POST"):
        payload = asyncio.run(restore_backup("backup_flat.zip")).get_json() or {}
        assert payload["success"] is True
        assert (data_dir / "flat.txt").read_text(encoding="utf-8") == "flat-restored"

    broken_backup = backup_dir / "backup_broken.zip"
    broken_backup.write_bytes(b"not-a-zip")

    with backup_route_app.test_request_context("/api/backup/restore/backup_broken.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_broken.zip"))
        assert status == 500
        assert response.get_json()["success"] is False


def test_backup_metadata_and_restore_verify_cover_checksum_and_validation(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """备份列表与恢复预验证应暴露校验信息，并识别元信息不匹配与坏包。"""
    backup_dir = tmp_path / "verify_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)

    valid_backup = backup_dir / "backup_valid.zip"
    with ZipFile(valid_backup, "w") as zip_file:
        zip_file.writestr("data/restored.txt", "restored")

    broken_backup = backup_dir / "backup_broken.zip"
    broken_backup.write_bytes(b"not-a-zip")

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    get_backups = _unwrap(backup_module.get_backups)
    verify_backup = _unwrap(backup_module.verify_backup_restore)

    metadata = backup_module._persist_backup_metadata(valid_backup)
    assert metadata["checksum"]
    assert metadata["ready_to_restore"] is True

    with backup_route_app.test_request_context("/api/backup/"):
        payload = get_backups().get_json() or {}
        assert payload["success"] is True
        listed_valid = next(item for item in payload["data"] if item["filename"] == "backup_valid.zip")
        assert listed_valid["checksum"] == metadata["checksum"]
        assert listed_valid["valid_zip"] is True
        assert listed_valid["ready_to_restore"] is True

    with backup_route_app.test_request_context("/api/backup/restore/verify", method="POST", json={}):
        response, status = verify_backup()
        assert status == 400
        assert response.get_json()["error"] == "filename is required"

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "not-valid.txt"},
    ):
        response, status = verify_backup()
        assert status == 400
        assert response.get_json()["error"] == "无效的文件名"

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_missing.zip"},
    ):
        response, status = verify_backup()
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"

    metadata_path = backup_module._get_backup_metadata_path(valid_backup)
    metadata_payload = json.loads(metadata_path.read_text(encoding="utf-8"))
    metadata_payload["checksum"] = "mismatch"
    metadata_path.write_text(json.dumps(metadata_payload, ensure_ascii=False, indent=2), encoding="utf-8")

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_valid.zip"},
    ):
        response, status = _unwrap_response(verify_backup())
        assert status == 200
        payload = response.get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["valid_zip"] is True
        assert payload["data"]["ready_to_restore"] is True
        assert payload["data"]["metadata_checksum_matched"] is False
        assert payload["data"]["contains_data_dir"] is True
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restore_verified"

    with backup_route_app.test_request_context("/api/backup/delete/backup_valid.zip", method="DELETE"):
        payload = backup_module.delete_backup("backup_valid.zip").get_json() or {}
        assert payload["success"] is True
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_deleted"

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_broken.zip"},
    ):
        response, status = verify_backup()
        assert status == 400
        payload = response.get_json() or {}
        assert payload["success"] is False
        assert payload["data"]["valid_zip"] is False
        assert payload["data"]["ready_to_restore"] is False


def test_backup_helper_and_verify_error_branches_cover_audit_sidecar_and_outer_failure(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """备份 helper 应静默吞掉审计异常、容忍损坏 sidecar，并覆盖 verify 外层异常。"""

    class ExplodingAuditDB:
        async def create_audit_log(self, **_payload: Any) -> None:
            raise RuntimeError("audit boom")

    backup_dir = tmp_path / "error_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    valid_backup = backup_dir / "backup_valid.zip"
    with ZipFile(valid_backup, "w") as zip_file:
        zip_file.writestr("data/restored.txt", "restored")

    metadata_path = backup_module._get_backup_metadata_path(valid_backup)
    metadata_path.write_text("{bad-json", encoding="utf-8")

    with backup_route_app.test_request_context("/api/backup/"):
        monkeypatch.setattr(backup_module, "get_app_context", lambda: ExplodingAuditDB())
        asyncio.run(backup_module._write_backup_audit_log_async("backup_created"))

    assert backup_module._read_backup_metadata(valid_backup) == {}

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    verify_backup = _unwrap(backup_module.verify_backup_restore)
    monkeypatch.setattr(backup_module, "_build_backup_info", lambda _path: _raise_runtime_error("verify boom"))

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_valid.zip"},
    ):
        response, status = _unwrap_response(verify_backup())
        assert status == 500
        assert response.get_json()["error"] == "verify boom"
