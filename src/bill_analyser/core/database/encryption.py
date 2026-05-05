"""Database encryption manager using SQLCipher.

Provides opt-in database encryption via sqlcipher3-binary.
Encryption is controlled by environment variables:
  - BILL_DB_ENCRYPT: set to "1", "true", or "yes" to enable
  - BILL_DB_KEY: the encryption passphrase (required when enabled)
"""

from __future__ import annotations

import importlib
import os
from dataclasses import dataclass

from bill_analyser.utils.logger import get_logger

logger = get_logger("DBEncryption")

_patched = False


@dataclass(frozen=True)
class EncryptionConfig:
    """Immutable encryption configuration."""

    enabled: bool = False
    key: str = ""
    kdf_iter: int = 256000
    cipher_page_size: int = 4096


def is_sqlcipher_available() -> bool:
    """Check if sqlcipher3 module is importable."""
    try:
        importlib.import_module("sqlcipher3")
        return True
    except ImportError:
        return False


def get_encryption_config() -> EncryptionConfig:
    """Load encryption config from environment variables."""
    enabled = os.environ.get("BILL_DB_ENCRYPT", "").lower() in ("1", "true", "yes")
    key = os.environ.get("BILL_DB_KEY", "")

    if enabled and not key:
        logger.warning("Encryption enabled but no key provided (BILL_DB_KEY). Disabling.")
        enabled = False

    if enabled and not is_sqlcipher_available():
        logger.warning(
            "Encryption enabled but sqlcipher3 not installed. "
            "Run: pip install sqlcipher3-binary"
        )
        enabled = False

    return EncryptionConfig(enabled=enabled, key=key)


def patch_aiosqlite_for_sqlcipher() -> bool:
    """Replace aiosqlite's sqlite3 module with sqlcipher3.

    Call this BEFORE any aiosqlite.connect() if encryption is desired.
    This is idempotent — safe to call multiple times.
    """
    global _patched  # noqa: PLW0603
    if _patched:
        return True

    try:
        import sqlcipher3  # noqa: F401
        import aiosqlite.core  # noqa: F401

        aiosqlite.core.sqlite3 = sqlcipher3.dbapi2  # type: ignore[attr-defined]
        _patched = True
        logger.info("aiosqlite patched to use sqlcipher3")
        return True
    except ImportError:
        logger.error("sqlcipher3 not installed. Run: pip install sqlcipher3-binary")
        return False


async def apply_encryption_pragmas(conn, config: EncryptionConfig) -> None:
    """Apply SQLCipher PRAGMA statements to an open connection.

    Must be called IMMEDIATELY after connection, before any other operations.
    """
    if not config.enabled or not config.key:
        return

    await conn.execute(f"PRAGMA key = '{config.key}'")
    await conn.execute(f"PRAGMA cipher_page_size = {config.cipher_page_size}")
    await conn.execute(f"PRAGMA kdf_iter = {config.kdf_iter}")
    await conn.execute("PRAGMA cipher_compatibility = 4")
    logger.info("SQLCipher encryption pragmas applied")


async def verify_encryption(conn) -> bool:
    """Verify that the database is accessible (correct key or unencrypted)."""
    try:
        await conn.execute("SELECT count(*) FROM sqlite_master")
        return True
    except Exception as e:
        logger.error("Database verification failed (wrong key?): %s", e)
        return False
