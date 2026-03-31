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
        self.backup_records: list[dict[str, Any]] = []
        self.backup_jobs: list[dict[str, Any]] = []

    async def create_audit_log(self, **payload: Any) -> None:
        self.audit_logs.append(payload)

    async def create_backup_record(self, payload: dict[str, Any]) -> int:
        record = dict(payload)
        record["id"] = len(self.backup_records) + 1
        self.backup_records.append(record)
        return record["id"]

    async def get_backup_records(self) -> list[dict[str, Any]]:
        return [dict(record) for record in self.backup_records]

    async def update_backup_record_by_filename(self, filename: str, updates: dict[str, Any]) -> bool:
        for record in self.backup_records:
            if record.get("backup_name") == filename:
                merged = dict(record)
                if "metadata" in updates:
                    merged_metadata = dict(record.get("metadata", {}) or {})
                    merged_metadata.update(dict(updates.get("metadata") or {}))
                    merged["metadata"] = merged_metadata
                merged.update({key: value for key, value in updates.items() if key != "metadata"})
                record.clear()
                record.update(merged)
                return True
        return False

    async def get_backup_jobs(self) -> list[dict[str, Any]]:
        return [dict(job) for job in self.backup_jobs]

    async def create_or_update_backup_job(self, payload: dict[str, Any]) -> int:
        job_id = int(payload.get("id") or 0)
        if job_id:
            for job in self.backup_jobs:
                if int(job.get("id") or 0) == job_id:
                    job.update(payload)
                    return job_id

        created = dict(payload)
        created["id"] = len(self.backup_jobs) + 1
        self.backup_jobs.append(created)
        return int(created["id"])

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

    sidecarless = backup_dir / "backup_sidecarless.zip"
    sidecarless.write_bytes(b"sidecarless")

    with backup_route_app.test_request_context("/api/backup/delete/backup_sidecarless.zip", method="DELETE"):
        payload = delete_backup("backup_sidecarless.zip").get_json() or {}
        assert payload["success"] is True
        assert sidecarless.exists() is False

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
        assert fake_db.audit_logs[-1]["details"]["filename"] == "backup_created.zip"

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
        assert fake_db.audit_logs[-1]["details"]["filename"] == "backup_valid.zip"


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


def test_backup_jobs_routes_cover_list_validation_create_and_update(
    backup_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """备份任务接口应支持列表、新建与更新 retention 配置。"""
    fake_db = FakeBackupDB()
    fake_db.backup_jobs = [
        {
            "id": 1,
            "job_type": "daily",
            "schedule_expr": "0 2 * * *",
            "retention_days": 30,
            "retention_count": 10,
            "enabled": True,
            "last_status": "success",
        }
    ]
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    list_jobs = _unwrap(backup_module.list_backup_jobs)
    save_job = _unwrap(backup_module.save_backup_job)

    with backup_route_app.test_request_context("/api/backup/jobs", method="GET"):
        payload = list_jobs().get_json() or {}
        assert payload["success"] is True
        assert payload["data"][0]["job_type"] == "daily"

    with backup_route_app.test_request_context("/api/backup/jobs", method="POST", json={}):
        response, status = save_job()
        assert status == 400
        assert response.get_json()["error"] == "job_type is required"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={"job_type": "manual", "retention_days": "bad"},
    ):
        response, status = save_job()
        assert status == 400
        assert response.get_json()["error"] == "retention_days must be an integer"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={
            "job_type": "weekly",
            "schedule_expr": "0 3 * * 0",
            "retention_days": 14,
            "retention_count": 4,
            "enabled": False,
        },
    ):
        payload = save_job().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["id"] == 2
        assert fake_db.backup_jobs[-1]["job_type"] == "weekly"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={
            "id": 1,
            "job_type": "daily",
            "schedule_expr": "0 1 * * *",
            "retention_days": 60,
            "retention_count": 20,
            "enabled": True,
        },
    ):
        payload = save_job().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["id"] == 1
        assert fake_db.backup_jobs[0]["retention_days"] == 60


def test_cleanup_old_backups_uses_backup_records_and_marks_deleted(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """清理旧备份应支持 .zip.enc，优先按 backup_records 决定淘汰项，并回写删除状态。"""
    backup_dir = tmp_path / "cleanup_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)

    fake_db = FakeBackupDB()
    fake_db.backup_records = [
        {
            "id": 1,
            "backup_name": "backup_old.zip",
            "storage_type": "local",
            "file_path": str(backup_dir / "backup_old.zip"),
            "checksum": "old",
            "encrypted": False,
            "status": "created",
            "metadata": {"valid_zip": True},
        },
        {
            "id": 2,
            "backup_name": "backup_new.zip.enc",
            "storage_type": "local",
            "file_path": str(backup_dir / "backup_new.zip.enc"),
            "checksum": "new",
            "encrypted": True,
            "status": "created",
            "metadata": {"valid_zip": True},
        },
    ]
    (backup_dir / "backup_old.zip").write_bytes(b"old")
    (backup_dir / "backup_new.zip.enc").write_bytes(b"new")
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    cleanup_backups = _unwrap(backup_module.cleanup_old_backups)

    with backup_route_app.test_request_context("/api/backup/cleanup", method="POST", json={"keep_count": 1}):
        payload = _unwrap_response(cleanup_backups())[0].get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["deleted_count"] == 1
        assert payload["data"]["kept_count"] == 1
        assert (backup_dir / "backup_old.zip").exists() is False
        assert (backup_dir / "backup_new.zip.enc").exists() is True
        assert fake_db.backup_records[0]["status"] == "deleted"
        assert fake_db.backup_records[0]["metadata"]["deleted_reason"] == "retention_cleanup"
        assert fake_db.backup_records[0]["metadata"]["valid_zip"] is True


def test_delete_backup_marks_record_deleted_and_cleanup_reconciles_missing_files(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """手动删除应回写 deleted 状态，cleanup 还应收敛已缺失文件的旧记录。"""
    backup_dir = tmp_path / "delete_cleanup_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)

    fake_db = FakeBackupDB()
    fake_db.backup_records = [
        {
            "id": 1,
            "backup_name": "backup_manual.zip",
            "storage_type": "local",
            "file_path": str(backup_dir / "backup_manual.zip"),
            "checksum": "manual",
            "encrypted": False,
            "status": "created",
            "metadata": {"valid_zip": True},
        },
        {
            "id": 2,
            "backup_name": "backup_missing.zip",
            "storage_type": "local",
            "file_path": str(backup_dir / "backup_missing.zip"),
            "checksum": "missing",
            "encrypted": False,
            "status": "created",
            "metadata": {"valid_zip": True},
        },
        {
            "id": 3,
            "backup_name": "backup_keep.zip",
            "storage_type": "local",
            "file_path": str(backup_dir / "backup_keep.zip"),
            "checksum": "keep",
            "encrypted": False,
            "status": "created",
            "metadata": {"valid_zip": True},
        },
    ]
    (backup_dir / "backup_manual.zip").write_bytes(b"manual")
    (backup_dir / "backup_keep.zip").write_bytes(b"keep")
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    delete_backup = _unwrap(backup_module.delete_backup)
    cleanup_backups = _unwrap(backup_module.cleanup_old_backups)

    with backup_route_app.test_request_context("/api/backup/delete/backup_manual.zip", method="DELETE"):
        payload = delete_backup("backup_manual.zip").get_json() or {}
        assert payload["success"] is True
        assert fake_db.backup_records[0]["status"] == "deleted"
        assert fake_db.backup_records[0]["metadata"]["deleted_reason"] == "manual_delete"

    with backup_route_app.test_request_context("/api/backup/cleanup", method="POST", json={"keep_count": 1}):
        payload = _unwrap_response(cleanup_backups())[0].get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["deleted_count"] == 0
        assert fake_db.backup_records[1]["status"] == "deleted"
        assert fake_db.backup_records[1]["metadata"]["deleted_reason"] == "missing_file"
        assert fake_db.backup_records[2]["status"] == "created"


def test_encrypted_backup_create_verify_and_restore_flow(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """配置加密密钥后，备份应加密落盘并可预验证/恢复。"""
    backup_dir = tmp_path / "encrypted_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "encrypted_data"
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "bill.txt").write_text("secret-content", encoding="utf-8")

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.setenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "backup-test-key")

    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    source_zip = backup_dir / "backup_plain_source.zip"
    with ZipFile(source_zip, "w") as zip_file:
        zip_file.writestr("data/bill.txt", "secret-content")

    class FakeSyncManager:
        async def backup_local(self) -> str | None:
            return str(source_zip)

    monkeypatch.setattr(sync_module, "SyncManager", FakeSyncManager)

    create_backup = _unwrap(backup_module.create_backup)
    verify_backup = _unwrap(backup_module.verify_backup_restore)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context("/api/backup/create", method="POST"):
        payload = asyncio.run(create_backup()).get_json() or {}
        assert payload["success"] is True
        encrypted_filename = payload["data"]["filename"]
        assert payload["data"]["encrypted"] is True
        assert encrypted_filename.endswith(".zip.enc")
        assert fake_db.backup_records[-1]["encrypted"] is True

    encrypted_path = backup_dir / encrypted_filename
    assert encrypted_path.exists() is True
    assert source_zip.exists() is False

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": encrypted_filename},
    ):
        response, status = _unwrap_response(verify_backup())
        assert status == 200
        payload = response.get_json() or {}
        assert payload["data"]["encrypted"] is True
        assert payload["data"]["ready_to_restore"] is True

    restored_target = tmp_path / "restore_target"
    monkeypatch.setattr(backup_module, "DATA_DIR", restored_target)
    with backup_route_app.test_request_context(f"/api/backup/restore/{encrypted_filename}", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup(encrypted_filename)))
        assert status == 200
        payload = response.get_json() or {}
        assert payload["success"] is True

    assert (restored_target / "bill.txt").read_text(encoding="utf-8") == "secret-content"
    assert fake_db.backup_records[-1]["status"] == "restored"


def test_backup_listing_with_encrypted_file_does_not_clobber_same_stem_plain_backup(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """扫描 `.zip.enc` 时，不应覆盖或删除同名明文 `.zip` 备份。"""
    backup_dir = tmp_path / "collision_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    staging_dir = tmp_path / "collision_staging"
    staging_dir.mkdir(parents=True, exist_ok=True)

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "collision-test-key")
    monkeypatch.setattr(backup_module, "get_app_context", lambda: FakeBackupDB())

    plain_backup = backup_dir / "backup_collision.zip"
    with ZipFile(plain_backup, "w") as zip_file:
        zip_file.writestr("data/plain.txt", "plain-original")

    encrypted_source = staging_dir / "backup_collision.zip"
    with ZipFile(encrypted_source, "w") as zip_file:
        zip_file.writestr("data/encrypted.txt", "encrypted-only")

    encrypted_source_path = backup_module._encrypt_backup_file(
        encrypted_source,
        "collision-test-key",
    )
    encrypted_backup = backup_dir / "backup_collision.zip.enc"
    encrypted_backup.write_bytes(encrypted_source_path.read_bytes())

    get_backups = _unwrap(backup_module.get_backups)

    with backup_route_app.test_request_context("/api/backup/"):
        payload = get_backups().get_json() or {}
        assert payload["success"] is True
        filenames = {item["filename"] for item in payload["data"]}
        assert "backup_collision.zip" in filenames
        assert "backup_collision.zip.enc" in filenames

    assert plain_backup.exists() is True
    assert encrypted_backup.exists() is True
    with ZipFile(plain_backup, "r") as zip_file:
        assert zip_file.read("data/plain.txt").decode("utf-8") == "plain-original"


def test_restore_backup_covers_missing_target_data_dir_and_inner_cleanup_false_branch(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """恢复备份应覆盖目标 data 目录不存在时的两条复制路径，以及异常时临时目录已不存在分支。"""
    import shutil

    backup_dir = tmp_path / "restore_missing_data_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "restore_missing_data_target"

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)

    restore_backup = _unwrap(backup_module.restore_backup)

    zipped_backup = backup_dir / "backup_nested.zip"
    with ZipFile(zipped_backup, "w") as zip_file:
        zip_file.writestr("data/from_nested.txt", "nested")

    with backup_route_app.test_request_context("/api/backup/restore/backup_nested.zip", method="POST"):
        payload = asyncio.run(restore_backup("backup_nested.zip")).get_json() or {}
        assert payload["success"] is True
        assert (data_dir / "from_nested.txt").read_text(encoding="utf-8") == "nested"

    shutil.rmtree(data_dir)

    flat_backup = backup_dir / "backup_flat_missing_data.zip"
    with ZipFile(flat_backup, "w") as zip_file:
        zip_file.writestr("flat.txt", "flat-no-existing-data")

    with backup_route_app.test_request_context("/api/backup/restore/backup_flat_missing_data.zip", method="POST"):
        payload = asyncio.run(restore_backup("backup_flat_missing_data.zip")).get_json() or {}
        assert payload["success"] is True
        assert (data_dir / "flat.txt").read_text(encoding="utf-8") == "flat-no-existing-data"

    shutil.rmtree(data_dir)

    broken_after_cleanup = backup_dir / "backup_fail_after_temp_removed.zip"
    with ZipFile(broken_after_cleanup, "w") as zip_file:
        zip_file.writestr("data/will_fail.txt", "x")

    original_copytree = shutil.copytree

    def exploding_copytree(src: str | Path, dst: str | Path, *args: Any, **kwargs: Any) -> Any:
        for restore_temp_dir in backup_dir.glob("restore_temp_*"):
            shutil.rmtree(restore_temp_dir, ignore_errors=True)
        raise RuntimeError("copy after cleanup boom")

    monkeypatch.setattr(shutil, "copytree", exploding_copytree)

    with backup_route_app.test_request_context("/api/backup/restore/backup_fail_after_temp_removed.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_fail_after_temp_removed.zip"))
        assert status == 500
        assert response.get_json()["error"] == "copy after cleanup boom"

    monkeypatch.setattr(shutil, "copytree", original_copytree)


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
        assert listed_valid["metadata_checksum_matched"] is True

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
        payload = _unwrap_response(backup_module.delete_backup("backup_valid.zip"))[0].get_json() or {}
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
