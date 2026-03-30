from __future__ import annotations

import shutil
import sys
import types
import zipfile
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core import sync as sync_module


class LoggerRecorder:
    """Collect SyncManager log calls without touching the real logger stack."""

    def __init__(self) -> None:
        self.debug_messages: list[str] = []
        self.info_messages: list[str] = []
        self.warning_messages: list[str] = []
        self.error_messages: list[str] = []

    def debug(self, message: str, *args: object, **_kwargs: object) -> None:
        self.debug_messages.append(message % args if args else message)

    def info(self, message: str, *args: object, **_kwargs: object) -> None:
        self.info_messages.append(message % args if args else message)

    def warning(self, message: str, *args: object, **_kwargs: object) -> None:
        self.warning_messages.append(message % args if args else message)

    def error(self, message: str, *args: object, **_kwargs: object) -> None:
        self.error_messages.append(message % args if args else message)


@pytest.fixture(name="sync_manager")
def sync_manager_fixture(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> tuple[sync_module.SyncManager, LoggerRecorder]:
    """Provide a SyncManager isolated to temp data/backup directories."""
    data_dir = tmp_path / "data"
    backup_dir = tmp_path / "backup"
    data_dir.mkdir()
    backup_dir.mkdir()

    logger = LoggerRecorder()
    monkeypatch.setattr(sync_module, "DATA_DIR", data_dir)
    monkeypatch.setattr(sync_module, "BACKUP_DIR", backup_dir)
    monkeypatch.setattr(sync_module, "get_logger", lambda _name: logger)

    manager = sync_module.SyncManager()
    return manager, logger


def _write_data_file(base_dir: Path, relative_path: str, content: str) -> Path:
    file_path = base_dir / relative_path
    file_path.parent.mkdir(parents=True, exist_ok=True)
    file_path.write_text(content, encoding="utf-8")
    return file_path


def _install_fake_oss(
    monkeypatch: pytest.MonkeyPatch,
    calls: list[tuple[Any, ...]],
    should_fail: bool = False,
) -> None:
    module = types.ModuleType("oss2")

    class FakeAuth:
        def __init__(self, access_key: str, secret_key: str) -> None:
            calls.append(("auth", access_key, secret_key))

    class FakeBucket:
        def __init__(self, auth: object, endpoint: str, bucket: str) -> None:
            calls.append(("bucket", endpoint, bucket, isinstance(auth, FakeAuth)))

        def put_object_from_file(self, object_key: str, file_path: str) -> None:
            calls.append(("put", object_key, file_path))
            if should_fail:
                raise RuntimeError("oss upload failed")

    setattr(module, "Auth", FakeAuth)
    setattr(module, "Bucket", FakeBucket)
    monkeypatch.setitem(sys.modules, "oss2", module)


def _install_fake_boto3(
    monkeypatch: pytest.MonkeyPatch,
    calls: list[tuple[Any, ...]],
    should_fail: bool = False,
) -> None:
    module = types.ModuleType("boto3")

    class FakeS3Client:
        def upload_file(self, local_path: str, bucket: str, object_key: str) -> None:
            calls.append(("upload", local_path, bucket, object_key))
            if should_fail:
                raise RuntimeError("s3 upload failed")

    def client(service_name: str, **kwargs: Any) -> FakeS3Client:
        calls.append(("client", service_name, kwargs))
        return FakeS3Client()

    setattr(module, "client", client)
    monkeypatch.setitem(sys.modules, "boto3", module)


def _install_fake_qcloud_cos(
    monkeypatch: pytest.MonkeyPatch,
    calls: list[tuple[Any, ...]],
    should_fail: bool = False,
) -> None:
    module = types.ModuleType("qcloud_cos")

    class CosConfig:
        def __init__(self, **kwargs: Any) -> None:
            calls.append(("config", kwargs))

    class CosS3Client:
        def __init__(self, _config: CosConfig) -> None:
            calls.append(("client", "cos"))

        def put_object(self, **kwargs: Any) -> None:
            calls.append(("put", kwargs["Bucket"], kwargs["Key"]))
            if should_fail:
                raise RuntimeError("cos upload failed")

    setattr(module, "CosConfig", CosConfig)
    setattr(module, "CosS3Client", CosS3Client)
    monkeypatch.setitem(sys.modules, "qcloud_cos", module)


def _install_fake_azure_blob(
    monkeypatch: pytest.MonkeyPatch,
    calls: list[tuple[Any, ...]],
    should_fail: bool = False,
) -> None:
    azure_module = types.ModuleType("azure")
    storage_module = types.ModuleType("azure.storage")
    blob_module = types.ModuleType("azure.storage.blob")

    class FakeBlobClient:
        def upload_blob(self, _data: object, overwrite: bool = False) -> None:
            calls.append(("upload", overwrite))
            if should_fail:
                raise RuntimeError("azure upload failed")

    class FakeContainerClient:
        def __init__(self, bucket: str) -> None:
            self.bucket = bucket

        def get_blob_client(self, blob_name: str) -> FakeBlobClient:
            calls.append(("blob", self.bucket, blob_name))
            return FakeBlobClient()

    class BlobServiceClient:
        @classmethod
        def from_connection_string(cls, connection_string: str) -> "BlobServiceClient":
            calls.append(("connection", connection_string))
            return cls()

        def get_container_client(self, bucket: str) -> FakeContainerClient:
            calls.append(("container", bucket))
            return FakeContainerClient(bucket)

    setattr(blob_module, "BlobServiceClient", BlobServiceClient)
    monkeypatch.setitem(sys.modules, "azure", azure_module)
    monkeypatch.setitem(sys.modules, "azure.storage", storage_module)
    monkeypatch.setitem(sys.modules, "azure.storage.blob", blob_module)


def _install_fake_webdav(
    monkeypatch: pytest.MonkeyPatch,
    calls: list[tuple[Any, ...]],
    should_fail: bool = False,
) -> None:
    webdav_module = types.ModuleType("webdav3")
    client_module = types.ModuleType("webdav3.client")

    class Client:
        def __init__(self, options: dict[str, Any]) -> None:
            calls.append(("client", options))

        def mkdir(self, prefix: str) -> None:
            calls.append(("mkdir", prefix))
            if should_fail:
                raise RuntimeError("mkdir failed")

        def upload_sync(self, remote_path: str, local_path: str) -> None:
            calls.append(("upload", remote_path, local_path))
            if should_fail:
                raise RuntimeError("webdav upload failed")

    setattr(client_module, "Client", Client)
    monkeypatch.setitem(sys.modules, "webdav3", webdav_module)
    monkeypatch.setitem(sys.modules, "webdav3.client", client_module)


@pytest.mark.asyncio
async def test_backup_local_creates_zip_and_skips_when_hash_is_unchanged(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
) -> None:
    manager, logger = sync_manager
    _write_data_file(manager.data_dir, "nested/records.json", '{"ok": true}')

    first_backup = await manager.backup_local()
    second_backup = await manager.backup_local()

    assert first_backup is not None
    assert second_backup is None
    with zipfile.ZipFile(first_backup, "r") as backup_zip:
        assert backup_zip.namelist() == ["data/nested/records.json"]
    assert any("跳过备份" in message for message in logger.info_messages)


@pytest.mark.asyncio
async def test_backup_local_returns_none_when_data_directory_is_missing(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
) -> None:
    manager, logger = sync_manager
    shutil.rmtree(manager.data_dir)

    assert await manager.backup_local() is None
    assert any("数据目录不存在" in message for message in logger.warning_messages)


@pytest.mark.asyncio
async def test_sync_to_cloud_handles_missing_provider_shortcuts_and_exceptions(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manager, logger = sync_manager
    backup_file = manager.backup_dir / "manual.zip"
    backup_file.write_bytes(b"zip")

    async def no_changes() -> None:
        return None

    async def existing_backup() -> str:
        return str(backup_file)

    async def exploding_backup() -> str:
        raise RuntimeError("boom")

    assert await manager.sync_to_cloud({}) is False

    monkeypatch.setattr(manager, "backup_local", no_changes)
    assert await manager.sync_to_cloud({"provider": "oss"}) is True

    monkeypatch.setattr(manager, "backup_local", existing_backup)
    assert await manager.sync_to_cloud({"provider": "unknown"}) is False

    monkeypatch.setattr(manager, "backup_local", exploding_backup)
    assert await manager.sync_to_cloud({"provider": "s3"}) is False
    assert logger.error_messages


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("provider", "method_name"),
    [
        ("oss", "_upload_to_aliyun_oss"),
        ("s3", "_upload_to_aws_s3"),
        ("cos", "_upload_to_tencent_cos"),
        ("azure", "_upload_to_azure_blob"),
        ("webdav", "_upload_to_webdav"),
    ],
)
async def test_sync_to_cloud_dispatches_to_expected_provider(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
    provider: str,
    method_name: str,
) -> None:
    manager, _logger = sync_manager
    backup_file = manager.backup_dir / f"{provider}.zip"
    backup_file.write_bytes(provider.encode("utf-8"))

    async def existing_backup() -> str:
        return str(backup_file)

    async def fake_upload(file_path: Path, config: dict[str, Any]) -> bool:
        assert file_path == backup_file
        assert config["provider"] == provider
        return True

    monkeypatch.setattr(manager, "backup_local", existing_backup)
    monkeypatch.setattr(manager, method_name, fake_upload)

    assert await manager.sync_to_cloud({"provider": provider}) is True


@pytest.mark.asyncio
async def test_upload_to_aliyun_oss_success_and_failure(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manager, _logger = sync_manager
    backup_file = manager.backup_dir / "oss.zip"
    backup_file.write_bytes(b"oss")
    config = {"endpoint": "https://oss.example.com", "bucket": "bucket", "access_key": "ak", "secret_key": "sk", "prefix": "prefix/"}
    upload_oss = getattr(manager, "_upload_to_aliyun_oss")

    success_calls: list[tuple[Any, ...]] = []
    _install_fake_oss(monkeypatch, success_calls)
    assert await upload_oss(backup_file, config) is True
    assert ("put", "prefix/oss.zip", str(backup_file)) in success_calls

    failure_calls: list[tuple[Any, ...]] = []
    _install_fake_oss(monkeypatch, failure_calls, should_fail=True)
    assert await upload_oss(backup_file, config) is False


@pytest.mark.asyncio
async def test_upload_to_aws_s3_success_and_failure(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manager, _logger = sync_manager
    backup_file = manager.backup_dir / "s3.zip"
    backup_file.write_bytes(b"s3")
    config = {"endpoint": "https://s3.example.com", "bucket": "bucket", "access_key": "ak", "secret_key": "sk", "prefix": "prefix/"}
    upload_s3 = getattr(manager, "_upload_to_aws_s3")

    success_calls: list[tuple[Any, ...]] = []
    _install_fake_boto3(monkeypatch, success_calls)
    assert await upload_s3(backup_file, config) is True
    assert any(call[0] == "upload" for call in success_calls)

    failure_calls: list[tuple[Any, ...]] = []
    _install_fake_boto3(monkeypatch, failure_calls, should_fail=True)
    assert await upload_s3(backup_file, config) is False


@pytest.mark.asyncio
async def test_upload_to_tencent_cos_success_and_failure(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manager, _logger = sync_manager
    backup_file = manager.backup_dir / "cos.zip"
    backup_file.write_bytes(b"cos")
    config = {
        "endpoint": "https://cos.ap-shanghai.myqcloud.com",
        "bucket": "bucket",
        "access_key": "ak",
        "secret_key": "sk",
        "prefix": "prefix/",
    }
    upload_cos = getattr(manager, "_upload_to_tencent_cos")

    success_calls: list[tuple[Any, ...]] = []
    _install_fake_qcloud_cos(monkeypatch, success_calls)
    assert await upload_cos(backup_file, config) is True
    assert ("put", "bucket", "prefix/cos.zip") in success_calls

    failure_calls: list[tuple[Any, ...]] = []
    _install_fake_qcloud_cos(monkeypatch, failure_calls, should_fail=True)
    assert await upload_cos(backup_file, config) is False


@pytest.mark.asyncio
async def test_upload_to_azure_blob_success_and_failure(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manager, _logger = sync_manager
    backup_file = manager.backup_dir / "azure.zip"
    backup_file.write_bytes(b"azure")
    config = {"bucket": "container", "access_key": "account", "secret_key": "secret", "prefix": "prefix/"}
    upload_azure = getattr(manager, "_upload_to_azure_blob")

    success_calls: list[tuple[Any, ...]] = []
    _install_fake_azure_blob(monkeypatch, success_calls)
    assert await upload_azure(backup_file, config) is True
    assert ("blob", "container", "prefix/azure.zip") in success_calls

    failure_calls: list[tuple[Any, ...]] = []
    _install_fake_azure_blob(monkeypatch, failure_calls, should_fail=True)
    assert await upload_azure(backup_file, config) is False


@pytest.mark.asyncio
async def test_upload_to_webdav_success_and_failure(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    manager, _logger = sync_manager
    backup_file = manager.backup_dir / "webdav.zip"
    backup_file.write_bytes(b"webdav")
    config = {
        "endpoint": "https://dav.example.com/remote.php/webdav/",
        "access_key": "user",
        "secret_key": "password",
        "prefix": "prefix/",
    }
    upload_webdav = getattr(manager, "_upload_to_webdav")

    success_calls: list[tuple[Any, ...]] = []
    _install_fake_webdav(monkeypatch, success_calls)
    assert await upload_webdav(backup_file, config) is True
    assert ("mkdir", "prefix/") in success_calls
    assert ("upload", "prefix/webdav.zip", str(backup_file)) in success_calls

    failure_calls: list[tuple[Any, ...]] = []
    _install_fake_webdav(monkeypatch, failure_calls, should_fail=True)
    assert await upload_webdav(backup_file, config) is False


@pytest.mark.asyncio
async def test_restore_from_backup_handles_missing_and_successful_restore(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
) -> None:
    manager, _logger = sync_manager
    missing_backup = manager.backup_dir / "missing.zip"

    original_file = _write_data_file(manager.data_dir, "old.txt", "old")
    assert original_file.exists()

    backup_path = manager.backup_dir / "restore.zip"
    with zipfile.ZipFile(backup_path, "w", zipfile.ZIP_DEFLATED) as backup_zip:
        backup_zip.writestr("data/restored.txt", "restored")

    assert await manager.restore_from_backup(str(missing_backup)) is False
    assert await manager.restore_from_backup(str(backup_path)) is True
    assert not original_file.exists()
    assert (manager.data_dir / "restored.txt").read_text(encoding="utf-8") == "restored"
    assert not (manager.data_dir.parent / "temp_restore").exists()


def test_cleanup_old_backups_keeps_latest_files_only(
    sync_manager: tuple[sync_module.SyncManager, LoggerRecorder],
) -> None:
    manager, _logger = sync_manager
    for name in ["backup_20260101_010101.zip", "backup_20260102_010101.zip", "backup_20260103_010101.zip"]:
        (manager.backup_dir / name).write_bytes(name.encode("utf-8"))

    manager.cleanup_old_backups(keep_count=2)

    remaining = sorted(path.name for path in manager.backup_dir.glob("backup_*.zip"))
    assert remaining == ["backup_20260102_010101.zip", "backup_20260103_010101.zip"]

    manager.cleanup_old_backups(keep_count=5)
    assert sorted(path.name for path in manager.backup_dir.glob("backup_*.zip")) == remaining