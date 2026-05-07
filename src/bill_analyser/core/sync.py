"""
同步模块

提供本地备份与云同步功能。
"""

# pylint: disable=import-outside-toplevel,duplicate-code

import asyncio
import hashlib
import shutil
import zipfile
from datetime import datetime
from pathlib import Path, PurePosixPath, PureWindowsPath

from bill_analyser.constants import BACKUP_DIR, DATA_DIR

from ..utils.logger import get_logger, log_method, log_step

DEFAULT_BACKUP_SYNC_PREFIX = "bill_analyser_backups/"


class BackupArchiveValidationError(ValueError):
    """Raised when a backup archive is structurally unsafe or not restorable."""


def normalize_backup_sync_prefix(prefix: object | None) -> str | None:
    """Normalize a cloud backup object prefix, returning None for unsafe values."""
    raw_prefix = str(prefix or "").strip()
    if not raw_prefix:
        return DEFAULT_BACKUP_SYNC_PREFIX

    if "\\" in raw_prefix or any(ord(char) < 32 or ord(char) == 127 for char in raw_prefix):
        return None

    posix_path = PurePosixPath(raw_prefix)
    windows_path = PureWindowsPath(raw_prefix)
    if posix_path.is_absolute() or windows_path.is_absolute() or windows_path.drive:
        return None

    normalized = raw_prefix.strip("/")
    parts = normalized.split("/")
    if any(part in ("", ".", "..") for part in parts):
        return None

    return f"{normalized}/"


class SyncManager:
    """同步与备份管理器"""

    def __init__(self):
        """初始化"""
        self.logger = get_logger("SyncManager")
        self.data_dir = DATA_DIR
        self.backup_dir = BACKUP_DIR
        self.backup_dir.mkdir(parents=True, exist_ok=True)
        self._last_backup_hash: str | None = None

    def _calculate_directory_hash(self, directory: Path) -> str:
        """
        计算目录哈希值

        参数：
            directory: 目录路径

        返回：
            str: 哈希值
        """
        hasher = hashlib.md5()

        for file_path in sorted(directory.rglob("*")):
            if file_path.is_file():
                hasher.update(file_path.name.encode())
                hasher.update(str(file_path.stat().st_size).encode())
                hasher.update(str(file_path.stat().st_mtime).encode())

        return hasher.hexdigest()

    @log_method
    @log_step("本地备份")
    async def backup_local(self) -> str | None:
        """
        备份数据到本地

        返回：
            Optional[str]: 备份文件路径，如果没有变化则返回 None
        """
        if not self.data_dir.exists():
            self.logger.warning("数据目录不存在: %s", self.data_dir)
            return None

        # 计算当前哈希
        current_hash = self._calculate_directory_hash(self.data_dir)

        # 检查是否需要备份
        if current_hash == self._last_backup_hash:
            self.logger.info("数据未变化，跳过备份")
            return None

        # 创建备份文件名
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_filename = f"backup_{timestamp}.zip"
        backup_path = self.backup_dir / backup_filename

        # 执行备份
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(None, self._create_backup_zip, backup_path)

        # 更新哈希
        self._last_backup_hash = current_hash

        self.logger.info("备份完成: %s", backup_path)
        return str(backup_path)

    def _create_backup_zip(self, backup_path: Path):
        """创建备份 ZIP 文件。"""
        with zipfile.ZipFile(backup_path, "w", zipfile.ZIP_DEFLATED) as zipf:
            for file_path in self.data_dir.rglob("*"):
                if file_path.is_file():
                    arcname = file_path.relative_to(self.data_dir.parent)
                    zipf.write(file_path, arcname)

    @staticmethod
    def _is_safe_backup_archive_member(member_name: str) -> bool:
        """Return whether a zip member can be extracted under the restore root."""
        normalized = str(member_name or "").replace("\\", "/").strip()
        if not normalized:
            return False

        posix_path = PurePosixPath(normalized)
        windows_path = PureWindowsPath(member_name)
        if posix_path.is_absolute() or windows_path.is_absolute() or windows_path.drive:
            return False

        return ".." not in posix_path.parts

    @classmethod
    def _validate_backup_archive_members(cls, zip_file: zipfile.ZipFile) -> None:
        """Reject backup archives that would extract outside the restore root."""
        for member_name in zip_file.namelist():
            if not cls._is_safe_backup_archive_member(member_name):
                raise BackupArchiveValidationError(f"备份文件包含不安全路径: {member_name}")

    @classmethod
    def _validate_backup_archive_ready(cls, zip_file: zipfile.ZipFile) -> None:
        """Validate that a safe backup archive contains a restorable data directory."""
        cls._validate_backup_archive_members(zip_file)
        infos = zip_file.infolist()
        if not any(info.filename.startswith("data/") for info in infos):
            raise BackupArchiveValidationError("备份文件缺少 data/ 目录")
        if not any(info.filename.startswith("data/") and not info.is_dir() for info in infos):
            raise BackupArchiveValidationError("备份文件缺少可恢复的 data/ 文件")

    @staticmethod
    def _directory_contains_file(directory: Path) -> bool:
        """Return whether the extracted data directory contains restorable files."""
        return directory.exists() and any(path.is_file() for path in directory.rglob("*"))

    def _build_cloud_object_key(self, config: dict, filename: str) -> str | None:
        """Build a cloud object key from a validated sync prefix."""
        prefix = normalize_backup_sync_prefix(config.get("prefix"))
        if prefix is None:
            self.logger.error("云同步前缀无效: %s", config.get("prefix"))
            return None
        return f"{prefix}{filename}"

    @log_method
    @log_step("云同步")
    async def sync_to_cloud(self, cloud_config: dict) -> bool:
        """
        同步到云端

        Args:
            cloud_config: 云服务配置
                - provider: 云服务商 ('oss', 's3', 'cos', 'azure', 'webdav')
                - endpoint: 端点URL
                - access_key: 访问密钥
                - secret_key: 密钥
                - bucket: 存储桶名称
                - prefix: 对象前缀（可选）

        Returns:
            bool: 是否成功
        """
        provider = cloud_config.get("provider", "").lower()

        if not provider:
            self.logger.error("未指定云服务商")
            return False

        normalized_prefix = normalize_backup_sync_prefix(cloud_config.get("prefix"))
        if normalized_prefix is None:
            self.logger.error("云同步前缀无效: %s", cloud_config.get("prefix"))
            return False
        cloud_config = {**cloud_config, "prefix": normalized_prefix}

        self.logger.info("开始同步到云端: %s", provider)

        try:
            # 首先创建本地备份
            backup_path = await self.backup_local()
            if not backup_path:
                self.logger.warning("无本地变化，跳过云同步")
                return True

            backup_file = Path(backup_path)

            # 根据不同云服务商上传
            if provider == "oss":
                success = await self._upload_to_aliyun_oss(backup_file, cloud_config)
            elif provider == "s3":
                success = await self._upload_to_aws_s3(backup_file, cloud_config)
            elif provider == "cos":
                success = await self._upload_to_tencent_cos(backup_file, cloud_config)
            elif provider == "azure":
                success = await self._upload_to_azure_blob(backup_file, cloud_config)
            elif provider == "webdav":
                success = await self._upload_to_webdav(backup_file, cloud_config)
            else:
                self.logger.error("不支持的云服务商: %s", provider)
                return False

            if success:
                self.logger.info("云同步成功: %s", backup_file.name)
            else:
                self.logger.error("云同步失败")

            return success

        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("云同步异常: %s", exc, exc_info=True)
            return False

    async def _upload_to_aliyun_oss(self, file_path: Path, config: dict) -> bool:
        """上传到阿里云OSS"""
        try:
            import oss2

            self.logger.info("使用阿里云OSS上传")

            # 创建认证对象
            auth = oss2.Auth(config.get("access_key"), config.get("secret_key"))

            # 创建 Bucket 对象
            bucket = oss2.Bucket(auth, config.get("endpoint"), config.get("bucket"))

            # 生成对象 key
            object_key = self._build_cloud_object_key(config, file_path.name)
            if object_key is None:
                return False

            # 上传文件
            self.logger.debug("上传文件: %s", object_key)
            bucket.put_object_from_file(object_key, str(file_path))

            self.logger.info("阿里云OSS上传成功: %s", object_key)
            return True

        except ImportError:
            self.logger.error("阿里云OSS SDK未安装: pip install oss2")
            return False
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("阿里云OSS上传失败: %s", exc, exc_info=True)
            return False

    async def _upload_to_aws_s3(self, file_path: Path, config: dict) -> bool:
        """上传到AWS S3"""
        try:
            import boto3

            self.logger.info("使用AWS S3上传")

            # 创建 S3 客户端
            s3_client = boto3.client(
                "s3",
                endpoint_url=config.get("endpoint"),
                aws_access_key_id=config.get("access_key"),
                aws_secret_access_key=config.get("secret_key"),
            )

            # 生成对象 key
            object_key = self._build_cloud_object_key(config, file_path.name)
            if object_key is None:
                return False

            # 上传文件
            self.logger.debug("上传文件: %s", object_key)
            s3_client.upload_file(str(file_path), config.get("bucket"), object_key)

            self.logger.info("AWS S3上传成功: %s", object_key)
            return True

        except ImportError:
            self.logger.error("AWS SDK未安装: pip install boto3")
            return False
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("AWS S3上传失败: %s", exc, exc_info=True)
            return False

    async def _upload_to_tencent_cos(self, file_path: Path, config: dict) -> bool:
        """上传到腾讯云COS"""
        try:
            from qcloud_cos import CosConfig, CosS3Client

            self.logger.info("使用腾讯云COS上传")

            # 解析 region
            import re

            region_match = re.search(r"cos\.([^.]+)\.myqcloud\.com", config.get("endpoint", ""))
            region = region_match.group(1) if region_match else "ap-guangzhou"

            # 创建配置对象
            cos_config = CosConfig(
                Region=region,
                SecretId=config.get("access_key"),
                SecretKey=config.get("secret_key"),
            )

            # 创建客户端
            client = CosS3Client(cos_config)

            # 生成对象 key
            object_key = self._build_cloud_object_key(config, file_path.name)
            if object_key is None:
                return False

            # 上传文件
            self.logger.debug("上传文件: %s", object_key)
            with open(file_path, "rb") as fp:
                client.put_object(Bucket=config.get("bucket"), Body=fp, Key=object_key)

            self.logger.info("腾讯云COS上传成功: %s", object_key)
            return True

        except ImportError:
            self.logger.error("腾讯云COS SDK未安装: pip install cos-python-sdk-v5")
            return False
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("腾讯云COS上传失败: %s", exc, exc_info=True)
            return False

    async def _upload_to_azure_blob(self, file_path: Path, config: dict) -> bool:
        """上传到 Azure Blob 存储。"""
        try:
            from azure.storage.blob import BlobServiceClient

            self.logger.info("使用Azure Blob Storage上传")

            # 创建连接字符串
            connection_string = (
                f"DefaultEndpointsProtocol=https;"
                f"AccountName={config.get('access_key')};"
                f"AccountKey={config.get('secret_key')};"
                f"EndpointSuffix=core.windows.net"
            )

            # 创建客户端
            blob_service_client = BlobServiceClient.from_connection_string(connection_string)
            container_client = blob_service_client.get_container_client(config.get("bucket"))

            # 生成 Blob 名称
            blob_name = self._build_cloud_object_key(config, file_path.name)
            if blob_name is None:
                return False

            # 上传文件
            self.logger.debug("上传文件: %s", blob_name)
            blob_client = container_client.get_blob_client(blob_name)
            with open(file_path, "rb") as data:
                blob_client.upload_blob(data, overwrite=True)

            self.logger.info("Azure Blob上传成功: %s", blob_name)
            return True

        except ImportError:
            self.logger.error("Azure SDK未安装: pip install azure-storage-blob")
            return False
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("Azure Blob上传失败: %s", exc, exc_info=True)
            return False

    async def _upload_to_webdav(self, file_path: Path, config: dict) -> bool:
        """上传到WebDAV服务器"""
        try:
            from webdav3.client import Client

            self.logger.info("使用WebDAV上传")

            # 创建 WebDAV 客户端
            options = {
                "webdav_hostname": config.get("endpoint"),
                "webdav_login": config.get("access_key"),
                "webdav_password": config.get("secret_key"),
            }
            client = Client(options)

            # 生成远程路径
            prefix = normalize_backup_sync_prefix(config.get("prefix"))
            if prefix is None:
                self.logger.error("云同步前缀无效: %s", config.get("prefix"))
                return False
            remote_path = f"{prefix}{file_path.name}"

            # 确保目录存在
            client.mkdir(prefix)

            # 上传文件
            self.logger.debug("上传文件: %s", remote_path)
            client.upload_sync(remote_path=remote_path, local_path=str(file_path))

            self.logger.info("WebDAV上传成功: %s", remote_path)
            return True

        except ImportError:
            self.logger.error("WebDAV客户端未安装: pip install webdavclient3")
            return False
        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("WebDAV上传失败: %s", exc, exc_info=True)
            return False

    @log_method
    async def restore_from_backup(self, backup_path: str) -> bool:
        """
        从备份恢复

        参数：
            backup_path: 备份文件路径

        返回：
            bool: 是否成功
        """
        backup_file = Path(backup_path)

        if not backup_file.exists():
            self.logger.error("备份文件不存在: %s", backup_path)
            return False

        temp_dir: Path | None = None
        try:
            # 创建临时目录
            temp_dir = self.data_dir.parent / "temp_restore"
            if temp_dir.exists():
                shutil.rmtree(temp_dir)
            temp_dir.mkdir(exist_ok=True)

            # 解压备份
            with zipfile.ZipFile(backup_file, "r") as zipf:
                self._validate_backup_archive_ready(zipf)
                zipf.extractall(temp_dir)

            restored_data_dir = temp_dir / "data"
            if not restored_data_dir.exists():
                raise BackupArchiveValidationError("备份文件缺少 data/ 目录")
            if not self._directory_contains_file(restored_data_dir):
                raise BackupArchiveValidationError("备份文件缺少可恢复的 data/ 文件")

            # 替换数据目录
            if self.data_dir.exists():
                shutil.rmtree(self.data_dir)

            shutil.move(str(restored_data_dir), str(self.data_dir))

            # 清理临时目录
            shutil.rmtree(temp_dir)

            self.logger.info("从备份恢复成功: %s", backup_path)
            return True

        except Exception as e:  # pylint: disable=broad-except
            if temp_dir and temp_dir.exists():
                shutil.rmtree(temp_dir, ignore_errors=True)
            self.logger.error("恢复备份失败: %s", e)
            return False

    @log_method
    def cleanup_old_backups(self, keep_count: int = 10):
        """
        清理旧备份，只保留最新的N个

        参数：
            keep_count: 保留的备份数量
        """
        if keep_count < 0:
            self.logger.error("keep_count 必须大于等于 0: %d", keep_count)
            raise ValueError("keep_count must be greater than or equal to 0")

        backups = list(self.backup_dir.glob("backup_*.zip"))
        backups.extend(self.backup_dir.glob("backup_*.zip.enc"))
        backups = sorted(backups, reverse=True)

        if len(backups) <= keep_count:
            self.logger.info("备份数量 %d，无需清理", len(backups))
            return

        # 删除旧备份
        for backup in backups[keep_count:]:
            try:
                backup.unlink()
                self.logger.info("已删除旧备份: %s", backup.name)
            except Exception as e:  # pylint: disable=broad-except
                self.logger.error("删除备份失败 %s: %s", backup.name, e)
