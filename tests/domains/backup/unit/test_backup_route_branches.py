from __future__ import annotations

import asyncio
import json
import os
from collections.abc import Callable
from datetime import datetime, timedelta
from pathlib import Path
from typing import Any, cast
from zipfile import ZipFile

import jwt
import pytest
from flask import Flask

from bill_analyser.api.middleware import auth as auth_middleware
from bill_analyser.api.routes import backup as backup_module
from bill_analyser.api.routes.backup import support as backup_support
from bill_analyser.core import sync as sync_module


@pytest.fixture(name="backup_route_app")
def backup_route_app_fixture() -> Flask:
    """Create a tiny Flask app for direct backup route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


def _unwrap(func: Callable[..., Any]) -> Callable[..., Any]:
    current = cast("Any", func)
    while getattr(current, "__wrapped__", None) is not None:
        current = current.__wrapped__
    return cast("Callable[..., Any]", current)


def _unwrap_response(result: Any) -> tuple[Any, int]:
    if isinstance(result, tuple):
        response, status = result
        return response, status
    return result, result.status_code


def _raise_runtime_error(message: str) -> Any:
    raise RuntimeError(message)


def test_backup_archive_member_path_safety_contract() -> None:
    """备份归档成员路径必须限制为恢复根目录内的相对路径。"""
    assert backup_support._is_safe_backup_archive_member("data/restored.txt") is True
    assert backup_support._is_safe_backup_archive_member("") is False
    assert backup_support._is_safe_backup_archive_member("/escape.txt") is False
    assert backup_support._is_safe_backup_archive_member("C:/escape.txt") is False
    assert backup_support._is_safe_backup_archive_member("safe/../escape.txt") is False


def test_backup_filename_resolution_rejects_traversal_shaped_raw_input(tmp_path: Path) -> None:
    """备份文件名解析必须拒绝 traversal 形态，而不是清理成合法备份名。"""
    backup_dir = tmp_path / "backups"
    backup_dir.mkdir()
    assert backup_support._resolve_backup_path_in_dir(backup_dir, " backup_20260505.zip ") == (
        backup_dir / "backup_20260505.zip"
    ).resolve()

    for unsafe_filename in [
        "../backup_20260505.zip",
        "..\\backup_20260505.zip",
        "backup_20260505.zip/..",
        "backup_20260505.zip:ads",
        "C:/backup_20260505.zip",
        "backup_20260505.zip\u0007",
    ]:
        assert backup_support._resolve_backup_path_in_dir(backup_dir, unsafe_filename) is None


class FakeBackupDB:
    def __init__(self) -> None:
        self.audit_logs: list[dict[str, Any]] = []
        self.backup_records: list[dict[str, Any]] = []
        self.backup_jobs: list[dict[str, Any]] = []
        self.session_activity_updates: list[int] = []

    async def get_session_by_token_hash(self, _token_hash: str) -> dict[str, Any]:
        return {
            "id": 100,
            "user_id": 42,
            "username": "security-user",
            "email": "security@example.test",
            "expires_at": (datetime.now() + timedelta(hours=1)).isoformat(),
            "user_is_active": True,
        }

    async def update_session_activity(self, session_id: int) -> None:
        self.session_activity_updates.append(session_id)

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


def _auth_header() -> dict[str, str]:
    token = jwt.encode(
        {"sub": "42", "exp": datetime.now() + timedelta(hours=1)},
        "backup-route-test-secret",
        algorithm="HS256",
    )
    return {"Authorization": f"Bearer {token}"}


def _register_backup_blueprint(app: Flask, fake_db: FakeBackupDB) -> None:
    app.config["DB_INSTANCE"] = fake_db
    app.register_blueprint(backup_module.bp, url_prefix="/api/backup")


def test_backup_routes_reject_unauthenticated_requests(backup_route_app: Flask) -> None:
    """所有备份入口都必须先过认证门禁。"""
    _register_backup_blueprint(backup_route_app, FakeBackupDB())
    client = backup_route_app.test_client()

    requests = [
        ("GET", "/api/backup/", None),
        ("POST", "/api/backup/create", {}),
        ("POST", "/api/backup/restore/verify", {"filename": "backup_valid.zip"}),
        ("GET", "/api/backup/download/backup_valid.zip", None),
        ("DELETE", "/api/backup/delete/backup_valid.zip", None),
        ("POST", "/api/backup/restore/backup_valid.zip", None),
        ("POST", "/api/backup/cleanup", {"keep_count": 1}),
        ("GET", "/api/backup/jobs", None),
        ("POST", "/api/backup/jobs", {"job_type": "manual"}),
    ]

    for method, path, payload in requests:
        response = client.open(path, method=method, json=payload)
        body = response.get_json() or {}
        assert response.status_code == 401
        assert body["error"] == "Unauthorized"


def test_backup_routes_accept_authenticated_requests(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """认证通过后备份主路由仍应按原合同执行。"""
    backup_dir = tmp_path / "auth_backups"
    data_dir = tmp_path / "auth_data"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "old.txt").write_text("old", encoding="utf-8")

    created_backup = backup_dir / "backup_created.zip"
    valid_backup = backup_dir / "backup_valid.zip"
    delete_backup_path = backup_dir / "backup_delete.zip"
    for path, member in [
        (created_backup, "data/created.txt"),
        (valid_backup, "data/restored.txt"),
        (delete_backup_path, "data/delete.txt"),
    ]:
        with ZipFile(path, "w") as zip_file:
            zip_file.writestr(member, path.stem)

    fake_db = FakeBackupDB()
    fake_db.backup_jobs = [{"id": 1, "job_type": "daily", "enabled": True}]
    monkeypatch.setattr(auth_middleware, "load_auth_settings", lambda: {"jwt_secret": "backup-route-test-secret"})
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.delenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", raising=False)

    class FakeSyncManager:
        async def backup_local(self) -> str:
            return str(created_backup)

    monkeypatch.setattr(sync_module, "SyncManager", FakeSyncManager)
    _register_backup_blueprint(backup_route_app, fake_db)
    client = backup_route_app.test_client()
    headers = _auth_header()

    assert client.get("/api/backup/", headers=headers).status_code == 200
    assert client.post("/api/backup/create", headers=headers, json={}).status_code == 200
    assert client.post(
        "/api/backup/restore/verify",
        headers=headers,
        json={"filename": "backup_valid.zip"},
    ).status_code == 200
    assert client.get("/api/backup/download/backup_valid.zip", headers=headers).status_code == 200
    assert client.delete("/api/backup/delete/backup_delete.zip", headers=headers).status_code == 200
    assert client.post("/api/backup/restore/backup_valid.zip", headers=headers).status_code == 200
    assert (data_dir / "restored.txt").read_text(encoding="utf-8") == "backup_valid"
    assert client.post("/api/backup/cleanup", headers=headers, json={"keep_count": 2}).status_code == 200
    assert client.get("/api/backup/jobs", headers=headers).status_code == 200
    job_response = client.post(
        "/api/backup/jobs",
        headers=headers,
        json={"job_type": "manual", "retention_count": None, "enabled": None},
    )
    assert job_response.status_code == 200
    assert (job_response.get_json() or {})["data"]["enabled"] is True
    assert fake_db.session_activity_updates


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
    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

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
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_downloaded"
        assert fake_db.audit_logs[-1]["details"]["filename"] == "backup_newer.zip"

    with backup_route_app.test_request_context("/api/backup/delete/not-valid.txt", method="DELETE"):
        response, status = delete_backup("not-valid.txt")
        assert status == 400
        assert response.get_json()["error"] == "无效的文件名"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_deleted"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context("/api/backup/delete/backup_missing.zip", method="DELETE"):
        response, status = delete_backup("backup_missing.zip")
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_deleted"
        assert fake_db.audit_logs[-1]["status"] == "failed"

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

    with backup_route_app.test_request_context("/api/backup/cleanup", method="POST", json={"keep_count": "bad"}):
        response, status = cleanup_backups()
        assert status == 400
        assert response.get_json()["error"] == "keep_count must be an integer"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_cleanup"
        assert fake_db.audit_logs[-1]["status"] == "failed"


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
    with ZipFile(created_backup, "w") as zip_file:
        zip_file.writestr("data/created.txt", "created")

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
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context("/api/backup/restore/backup_missing.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_missing.zip"))
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
        assert fake_db.audit_logs[-1]["status"] == "failed"

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

    unsafe_backup = backup_dir / "backup_unsafe.zip"
    with ZipFile(unsafe_backup, "w") as zip_file:
        zip_file.writestr("../escape.txt", "escape")
        zip_file.writestr("C:/escape.txt", "drive-escape")

    verify_backup = _unwrap(backup_module.verify_backup_restore)
    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_unsafe.zip"},
    ):
        response, status = verify_backup()
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["data"]["valid_zip"] is False
        assert "备份文件包含不安全路径" in payload["data"]["error"]

    with backup_route_app.test_request_context("/api/backup/restore/backup_unsafe.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_unsafe.zip"))
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert "备份文件包含不安全路径" in payload["error"]
        assert (tmp_path / "escape.txt").exists() is False
        assert (tmp_path / "C:" / "escape.txt").exists() is False

    empty_backup = backup_dir / "backup_empty.zip"
    with ZipFile(empty_backup, "w"):
        pass

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_empty.zip"},
    ):
        response, status = verify_backup()
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["data"]["valid_zip"] is True
        assert payload["data"]["ready_to_restore"] is False

    with backup_route_app.test_request_context("/api/backup/restore/backup_empty.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_empty.zip"))
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["error"] == "backup archive is not ready to restore"
        assert (data_dir / "restored.txt").read_text(encoding="utf-8") == "restored"

    data_dir_only_backup = backup_dir / "backup_data_dir_only.zip"
    with ZipFile(data_dir_only_backup, "w") as zip_file:
        zip_file.writestr("data/", "")

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_data_dir_only.zip"},
    ):
        response, status = verify_backup()
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["data"]["valid_zip"] is True
        assert payload["data"]["contains_data_dir"] is True
        assert payload["data"]["ready_to_restore"] is False

    with backup_route_app.test_request_context("/api/backup/restore/backup_data_dir_only.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_data_dir_only.zip"))
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["error"] == "backup archive is not ready to restore"
        assert (data_dir / "restored.txt").read_text(encoding="utf-8") == "restored"


def test_backup_routes_cover_directory_creation_restore_rejection_and_error_paths(
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
        response, status = asyncio.run(restore_backup("backup_flat.zip"))
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["error"] == "backup archive is not ready to restore"
        assert (data_dir / "old.txt").read_text(encoding="utf-8") == "old"
        assert (data_dir / "flat.txt").exists() is False

    broken_backup = backup_dir / "backup_broken.zip"
    broken_backup.write_bytes(b"not-a-zip")

    with backup_route_app.test_request_context("/api/backup/restore/backup_broken.zip", method="POST"):
        response, status = asyncio.run(restore_backup("backup_broken.zip"))
        assert status == 400
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
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_job_saved"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={"job_type": "manual", "retention_days": "bad"},
    ):
        response, status = save_job()
        assert status == 400
        assert response.get_json()["error"] == "retention_days must be an integer"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_job_saved"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={"job_type": "manual", "retention_count": -1},
    ):
        response, status = save_job()
        assert status == 400
        assert response.get_json()["error"] == "retention_count must be greater than or equal to 0"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_job_saved"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={"job_type": "manual", "retention_count": "bad"},
    ):
        response, status = save_job()
        assert status == 400
        assert response.get_json()["error"] == "retention_count must be an integer"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_job_saved"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    for invalid_job_id in ["abc", "1.5", 0, -1, 1.5, True, False]:
        with backup_route_app.test_request_context(
            "/api/backup/jobs",
            method="POST",
            json={"id": invalid_job_id, "job_type": "manual"},
        ):
            response, status = save_job()
            assert status == 400
            assert response.get_json()["error"] == "id must be a positive integer"
            assert fake_db.audit_logs[-1]["operation_type"] == "backup_job_saved"
            assert fake_db.audit_logs[-1]["status"] == "failed"
    assert len(fake_db.backup_jobs) == 1

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={
            "job_type": "weekly",
            "schedule_expr": "0 3 * * 0",
            "retention_days": 14,
            "retention_count": None,
            "enabled": None,
        },
    ):
        payload = save_job().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["id"] == 2
        assert payload["data"]["retention_count"] == 10
        assert payload["data"]["enabled"] is True
        assert fake_db.backup_jobs[-1]["job_type"] == "weekly"

    with backup_route_app.test_request_context(
        "/api/backup/jobs",
        method="POST",
        json={
            "id": "1",
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


def test_cleanup_old_backups_audits_failure_after_partial_deletion(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """清理中途失败时，应记录已删除数量的失败审计。"""
    backup_dir = tmp_path / "cleanup_partial_failure_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)

    now = datetime.now()
    fake_db = FakeBackupDB()
    fake_db.backup_records = [
        {
            "id": 1,
            "backup_name": "backup_keep.zip",
            "storage_type": "local",
            "checksum": "keep",
            "encrypted": False,
            "status": "created",
            "created_at": now.isoformat(),
            "metadata": {},
        },
        {
            "id": 2,
            "backup_name": "backup_delete_first.zip",
            "storage_type": "local",
            "checksum": "first",
            "encrypted": False,
            "status": "created",
            "created_at": (now - timedelta(minutes=1)).isoformat(),
            "metadata": {},
        },
        {
            "id": 3,
            "backup_name": "backup_delete_second.zip",
            "storage_type": "local",
            "checksum": "second",
            "encrypted": False,
            "status": "created",
            "created_at": (now - timedelta(minutes=2)).isoformat(),
            "metadata": {},
        },
    ]
    first_delete = backup_dir / "backup_delete_first.zip"
    second_delete = backup_dir / "backup_delete_second.zip"
    (backup_dir / "backup_keep.zip").write_bytes(b"keep")
    first_delete.write_bytes(b"first")
    second_delete.write_bytes(b"second")
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    original_unlink = Path.unlink

    def failing_second_unlink(path: Path, *args: Any, **kwargs: Any) -> None:
        if path.name == "backup_delete_second.zip":
            raise RuntimeError("unlink boom")
        original_unlink(path, *args, **kwargs)

    monkeypatch.setattr(Path, "unlink", failing_second_unlink)
    cleanup_backups = _unwrap(backup_module.cleanup_old_backups)

    with backup_route_app.test_request_context("/api/backup/cleanup", method="POST", json={"keep_count": 1}):
        response, status = _unwrap_response(cleanup_backups())
        payload = response.get_json() or {}

    assert status == 500
    assert payload["error"] == "unlink boom"
    assert first_delete.exists() is False
    assert second_delete.exists() is True
    assert fake_db.audit_logs[-1]["operation_type"] == "backup_cleanup"
    assert fake_db.audit_logs[-1]["status"] == "failed"
    assert fake_db.audit_logs[-1]["details"]["deleted_count"] == 1


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


def test_encrypted_backup_restore_returns_400_when_key_is_wrong(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """加密备份在密钥错误时应返回明确的 400，而不是通用 500。"""
    backup_dir = tmp_path / "wrong_key_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "wrong_key_restore_target"

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)

    plain_backup = backup_dir / "backup_wrong_key_source.zip"
    with ZipFile(plain_backup, "w") as zip_file:
        zip_file.writestr("data/secret.txt", "classified")

    encrypted_backup = backup_module._encrypt_backup_file(plain_backup, "correct-backup-key")
    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)
    monkeypatch.setenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "wrong-backup-key")

    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context(f"/api/backup/restore/{encrypted_backup.name}", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup(encrypted_backup.name)))
        assert status == 400
        payload = response.get_json() or {}
        assert payload["success"] is False
        assert payload["error"] == "备份解密失败"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
        assert fake_db.audit_logs[-1]["status"] == "failed"


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
    """恢复备份应覆盖目标 data 目录不存在、缺失 data/ 目录与异常清理分支。"""
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
        response, status = asyncio.run(restore_backup("backup_flat_missing_data.zip"))
        payload = response.get_json() or {}
        assert status == 400
        assert payload["success"] is False
        assert payload["error"] == "backup archive is not ready to restore"
        assert data_dir.exists() is False

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


def test_restore_backup_uses_unique_temp_dir_without_reusing_stale_contents(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """恢复临时目录必须每次唯一创建，避免复用旧 restore_temp 内容。"""
    backup_dir = tmp_path / "unique_restore_temp_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "unique_restore_temp_data"
    data_dir.mkdir(parents=True, exist_ok=True)
    stale_temp = backup_dir / "restore_temp_stale"
    stale_temp.mkdir()
    (stale_temp / "stale.txt").write_text("stale", encoding="utf-8")

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)

    valid_backup = backup_dir / "backup_unique_temp.zip"
    with ZipFile(valid_backup, "w") as zip_file:
        zip_file.writestr("data/restored.txt", "restored")

    created_temp_dirs: list[Path] = []
    original_mkdtemp = backup_module.tempfile.mkdtemp

    def capturing_mkdtemp(*args: Any, **kwargs: Any) -> str:
        temp_dir = Path(original_mkdtemp(*args, **kwargs))
        created_temp_dirs.append(temp_dir)
        return str(temp_dir)

    monkeypatch.setattr(backup_module.tempfile, "mkdtemp", capturing_mkdtemp)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context("/api/backup/restore/backup_unique_temp.zip", method="POST"):
        payload = _unwrap_response(asyncio.run(restore_backup("backup_unique_temp.zip")))[0].get_json() or {}

    assert payload["success"] is True
    assert created_temp_dirs
    assert all(not path.exists() for path in created_temp_dirs)
    assert stale_temp.exists() is True
    assert (stale_temp / "stale.txt").read_text(encoding="utf-8") == "stale"
    assert (data_dir / "restored.txt").read_text(encoding="utf-8") == "restored"


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
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restore_verified"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "not-valid.txt"},
    ):
        response, status = verify_backup()
        assert status == 400
        assert response.get_json()["error"] == "无效的文件名"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restore_verified"
        assert fake_db.audit_logs[-1]["status"] == "failed"

    with backup_route_app.test_request_context(
        "/api/backup/restore/verify",
        method="POST",
        json={"filename": "backup_missing.zip"},
    ):
        response, status = verify_backup()
        assert status == 404
        assert response.get_json()["error"] == "文件不存在"
        assert fake_db.audit_logs[-1]["operation_type"] == "backup_restore_verified"
        assert fake_db.audit_logs[-1]["status"] == "failed"

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
        payload = _unwrap_response(_unwrap(backup_module.delete_backup)("backup_valid.zip"))[0].get_json() or {}
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


def test_build_backup_info_removes_decrypted_temp_file_when_later_step_fails(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """加密备份列表检查在后续异常时，也必须清理解密临时 zip。"""
    backup_dir = tmp_path / "build_info_cleanup_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    plain_backup = backup_dir / "backup_info_source.zip"
    with ZipFile(plain_backup, "w") as zip_file:
        zip_file.writestr("data/secret.txt", "secret")
    encrypted_backup = backup_module._encrypt_backup_file(plain_backup, "info-cleanup-key")

    monkeypatch.setenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "info-cleanup-key")
    decrypted_paths: list[Path] = []
    original_decrypt = backup_module._decrypt_backup_file_to_temp

    def capturing_decrypt(file_path: Path, secret: str) -> Path:
        decrypted_path = original_decrypt(file_path, secret)
        decrypted_paths.append(decrypted_path)
        return decrypted_path

    monkeypatch.setattr(backup_module, "_decrypt_backup_file_to_temp", capturing_decrypt)
    monkeypatch.setattr(backup_module, "_calculate_file_checksum", lambda _path: _raise_runtime_error("checksum boom"))

    with pytest.raises(RuntimeError, match="checksum boom"):
        backup_module._build_backup_info(encrypted_backup)

    assert decrypted_paths
    assert all(not path.exists() for path in decrypted_paths)


def test_create_backup_rejects_valid_zip_without_restorable_data_dir(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """创建备份后必须验证归档可恢复，否则写失败审计并返回 500。"""
    backup_dir = tmp_path / "create_invalid_ready_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    created_backup = backup_dir / "backup_flat_created.zip"
    with ZipFile(created_backup, "w") as zip_file:
        zip_file.writestr("flat.txt", "not-restorable")

    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)
    monkeypatch.delenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", raising=False)

    class FakeSyncManager:
        async def backup_local(self) -> str:
            return str(created_backup)

    monkeypatch.setattr(sync_module, "SyncManager", FakeSyncManager)
    create_backup = _unwrap(backup_module.create_backup)

    with backup_route_app.test_request_context("/api/backup/create", method="POST"):
        response, status = _unwrap_response(asyncio.run(create_backup()))
        payload = response.get_json() or {}

    assert status == 500
    assert payload["success"] is False
    assert payload["error"] == "backup archive is not ready to restore"
    assert fake_db.backup_records == []
    assert fake_db.audit_logs[-1]["operation_type"] == "backup_created"
    assert fake_db.audit_logs[-1]["status"] == "failed"


def test_restore_encrypted_backup_without_key_returns_400(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """加密备份恢复缺少环境密钥时应返回明确 400。"""
    backup_dir = tmp_path / "missing_key_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "missing_key_data"
    data_dir.mkdir(parents=True, exist_ok=True)

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.delenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", raising=False)
    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    plain_backup = backup_dir / "backup_missing_key_source.zip"
    with ZipFile(plain_backup, "w") as zip_file:
        zip_file.writestr("data/secret.txt", "secret")
    encrypted_backup = backup_module._encrypt_backup_file(plain_backup, "configured-elsewhere")

    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context(f"/api/backup/restore/{encrypted_backup.name}", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup(encrypted_backup.name)))
        payload = response.get_json() or {}

    assert status == 400
    assert payload["success"] is False
    assert payload["error"] == "备份加密密钥未配置"
    assert data_dir.exists() is True
    assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
    assert fake_db.audit_logs[-1]["status"] == "failed"


def test_restore_encrypted_backup_precheck_failure_removes_temp_decrypted_file(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """加密备份解密成功但预检查失败时，应清理临时解密 zip。"""
    backup_dir = tmp_path / "encrypted_precheck_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "encrypted_precheck_data"
    data_dir.mkdir(parents=True, exist_ok=True)

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.setenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "flat-backup-key")
    monkeypatch.setattr(backup_module, "get_app_context", lambda: FakeBackupDB())

    flat_backup = backup_dir / "backup_flat_encrypted_source.zip"
    with ZipFile(flat_backup, "w") as zip_file:
        zip_file.writestr("flat.txt", "flat")
    encrypted_backup = backup_module._encrypt_backup_file(flat_backup, "flat-backup-key")

    decrypted_paths: list[Path] = []
    original_decrypt = backup_module._decrypt_backup_file_to_temp

    def capturing_decrypt(file_path: Path, secret: str) -> Path:
        decrypted_path = original_decrypt(file_path, secret)
        decrypted_paths.append(decrypted_path)
        return decrypted_path

    monkeypatch.setattr(backup_module, "_decrypt_backup_file_to_temp", capturing_decrypt)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context(f"/api/backup/restore/{encrypted_backup.name}", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup(encrypted_backup.name)))
        payload = response.get_json() or {}

    assert status == 400
    assert payload["success"] is False
    assert payload["error"] == "backup archive is not ready to restore"
    assert decrypted_paths
    assert all(not path.exists() for path in decrypted_paths)


def test_restore_encrypted_backup_mkdtemp_failure_removes_temp_decrypted_file(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """加密备份解密成功后，即使创建恢复临时目录失败，也必须清理解密 zip。"""
    backup_dir = tmp_path / "encrypted_mkdtemp_failure_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "encrypted_mkdtemp_failure_data"
    data_dir.mkdir(parents=True, exist_ok=True)
    fake_db = FakeBackupDB()

    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.setenv("BILL_ANALYSER_BACKUP_ENCRYPTION_KEY", "mkdtemp-failure-key")
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    plain_backup = backup_dir / "backup_mkdtemp_failure_source.zip"
    with ZipFile(plain_backup, "w") as zip_file:
        zip_file.writestr("data/secret.txt", "secret")
    encrypted_backup = backup_module._encrypt_backup_file(plain_backup, "mkdtemp-failure-key")

    decrypted_paths: list[Path] = []
    original_decrypt = backup_module._decrypt_backup_file_to_temp

    def capturing_decrypt(file_path: Path, secret: str) -> Path:
        decrypted_path = original_decrypt(file_path, secret)
        decrypted_paths.append(decrypted_path)
        return decrypted_path

    def failing_mkdtemp(*_args: Any, **_kwargs: Any) -> str:
        raise RuntimeError("mkdtemp boom")

    monkeypatch.setattr(backup_module, "_decrypt_backup_file_to_temp", capturing_decrypt)
    monkeypatch.setattr(backup_module.tempfile, "mkdtemp", failing_mkdtemp)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context(f"/api/backup/restore/{encrypted_backup.name}", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup(encrypted_backup.name)))
        payload = response.get_json() or {}

    assert status == 500
    assert payload["success"] is False
    assert payload["error"] == "mkdtemp boom"
    assert decrypted_paths
    assert all(not path.exists() for path in decrypted_paths)
    assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
    assert fake_db.audit_logs[-1]["status"] == "failed"


def test_restore_backup_inner_missing_data_dir_branch_after_forced_precheck_pass(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """预检查边界被放行后，恢复内层仍必须拒绝缺少 data/ 的归档并清理临时目录。"""
    backup_dir = tmp_path / "forced_precheck_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "forced_precheck_data"
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "existing.txt").write_text("existing", encoding="utf-8")

    flat_backup = backup_dir / "backup_forced_flat.zip"
    with ZipFile(flat_backup, "w") as zip_file:
        zip_file.writestr("flat.txt", "flat")

    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    def forced_ready(_path: Path) -> dict[str, Any]:
        return {
            "valid_zip": True,
            "contains_data_dir": True,
            "entry_count": 1,
            "top_level_entries": ["flat.txt"],
            "error": "",
            "ready_to_restore": True,
        }

    monkeypatch.setattr(backup_module, "_inspect_backup_archive", forced_ready)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context("/api/backup/restore/backup_forced_flat.zip", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup("backup_forced_flat.zip")))
        payload = response.get_json() or {}

    assert status == 400
    assert payload["success"] is False
    assert payload["error"] == "备份文件缺少 data/ 目录"
    assert (data_dir / "existing.txt").read_text(encoding="utf-8") == "existing"
    assert list(backup_dir.glob("restore_temp_*")) == []
    assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
    assert fake_db.audit_logs[-1]["status"] == "failed"


def test_restore_backup_inner_rejects_data_dir_only_after_forced_precheck_pass(
    backup_route_app: Flask,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """即使预检查边界误放行，恢复替换前仍必须拒绝 data/ 空目录归档。"""
    backup_dir = tmp_path / "forced_data_dir_only_backups"
    backup_dir.mkdir(parents=True, exist_ok=True)
    data_dir = tmp_path / "forced_data_dir_only_data"
    data_dir.mkdir(parents=True, exist_ok=True)
    (data_dir / "existing.txt").write_text("existing", encoding="utf-8")

    data_dir_only_backup = backup_dir / "backup_forced_data_dir_only.zip"
    with ZipFile(data_dir_only_backup, "w") as zip_file:
        zip_file.writestr("data/", "")

    fake_db = FakeBackupDB()
    monkeypatch.setattr(backup_module, "get_backup_dir", lambda: backup_dir)
    monkeypatch.setattr(backup_module, "DATA_DIR", data_dir)
    monkeypatch.setattr(backup_module, "get_app_context", lambda: fake_db)

    def forced_ready(_path: Path) -> dict[str, Any]:
        return {
            "valid_zip": True,
            "contains_data_dir": True,
            "entry_count": 1,
            "top_level_entries": ["data"],
            "error": "",
            "ready_to_restore": True,
        }

    monkeypatch.setattr(backup_module, "_inspect_backup_archive", forced_ready)
    restore_backup = _unwrap(backup_module.restore_backup)

    with backup_route_app.test_request_context("/api/backup/restore/backup_forced_data_dir_only.zip", method="POST"):
        response, status = _unwrap_response(asyncio.run(restore_backup("backup_forced_data_dir_only.zip")))
        payload = response.get_json() or {}

    assert status == 400
    assert payload["success"] is False
    assert payload["error"] == "备份文件缺少可恢复的 data/ 文件"
    assert (data_dir / "existing.txt").read_text(encoding="utf-8") == "existing"
    assert list(backup_dir.glob("restore_temp_*")) == []
    assert fake_db.audit_logs[-1]["operation_type"] == "backup_restored"
    assert fake_db.audit_logs[-1]["status"] == "failed"
