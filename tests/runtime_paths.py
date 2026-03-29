"""Shared helpers for pytest runtime artifacts."""

import os
from pathlib import Path


TEST_DB_DIR_ENV = "BILL_ANALYSER_TEST_DB_DIR"
REPO_ROOT = Path(__file__).resolve().parents[1]
TESTS_RUNTIME_DIR = REPO_ROOT / "tests" / ".runtime"
TESTS_DB_DIR = TESTS_RUNTIME_DIR / "db"
TEST_DB_SIDE_CAR_SUFFIXES = ("", "-wal", "-shm", "-journal")


def configure_test_runtime_environment() -> Path:
    """Ensure runtime directories exist and expose the DB root via environment."""
    TESTS_DB_DIR.mkdir(parents=True, exist_ok=True)
    os.environ.setdefault(TEST_DB_DIR_ENV, str(TESTS_DB_DIR))
    return TESTS_DB_DIR


def cleanup_test_runtime_databases() -> None:
    """Best-effort cleanup of leftover test databases from previous runs."""
    db_dir = configure_test_runtime_environment()
    for db_file in db_dir.glob("*.db"):
        remove_test_database_family(db_file)


def remove_test_database_family(db_path: Path) -> None:
    """Best-effort cleanup of a sqlite database file and its sidecar files."""
    for suffix in TEST_DB_SIDE_CAR_SUFFIXES:
        candidate = Path(f"{db_path}{suffix}") if suffix else db_path
        try:
            if candidate.exists():
                candidate.unlink()
        except OSError:
            continue


def get_test_db_path(filename: str) -> Path:
    """Return a concrete test DB path inside tests runtime storage."""
    db_dir = configure_test_runtime_environment()
    return db_dir / filename
