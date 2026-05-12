"""Shared helpers for pytest runtime artifacts."""

import os
from pathlib import Path

TEST_DB_DIR_ENV = "BILL_ANALYSER_TEST_DB_DIR"
REPO_ROOT = Path(__file__).resolve().parents[1]
RUNTIME_DB_PATH = REPO_ROOT / "data" / "bills.db"
TESTS_RUNTIME_DIR = REPO_ROOT / "tests" / ".runtime"
TESTS_DB_DIR = TESTS_RUNTIME_DIR / "db"
TEST_DB_SIDE_CAR_SUFFIXES = ("", "-wal", "-shm", "-journal")
PYTEST_XDIST_WORKER_ENV = "PYTEST_XDIST_WORKER"


def _test_db_base_dir() -> Path:
    configured = os.environ.get(TEST_DB_DIR_ENV)
    if not configured:
        return TESTS_DB_DIR

    path = Path(configured)
    worker_id = os.environ.get(PYTEST_XDIST_WORKER_ENV)
    if worker_id and path.name == worker_id:
        return path.parent
    return path


def _test_db_dir() -> Path:
    db_dir = _test_db_base_dir()
    worker_id = os.environ.get(PYTEST_XDIST_WORKER_ENV)
    if worker_id:
        return db_dir / worker_id
    return db_dir


def configure_test_runtime_environment() -> Path:
    """Ensure runtime directories exist and expose the DB root via environment."""
    db_dir = _test_db_dir()
    db_dir.mkdir(parents=True, exist_ok=True)
    os.environ[TEST_DB_DIR_ENV] = str(db_dir)
    return db_dir


def cleanup_test_runtime_databases() -> None:
    """Best-effort cleanup of leftover test databases from previous runs."""
    db_dir = configure_test_runtime_environment()
    worker_id = os.environ.get(PYTEST_XDIST_WORKER_ENV)
    db_files = db_dir.glob("*.db") if worker_id else db_dir.rglob("*.db")
    for db_file in db_files:
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


def build_sqlite_uri(db_path: Path, readonly: bool = True) -> str:
    """Build a sqlite file URI for the provided database path."""
    resolved_path = db_path.resolve()
    mode = "ro" if readonly else "rw"
    return f"file:{resolved_path.as_posix()}?mode={mode}"


def get_runtime_db_path() -> Path:
    """Return the repository runtime bills.db path."""
    return RUNTIME_DB_PATH


def get_runtime_db_uri(readonly: bool = True) -> str:
    """Return a sqlite file URI for the repository runtime bills.db path."""
    return build_sqlite_uri(RUNTIME_DB_PATH, readonly=readonly)
