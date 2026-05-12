"""Regression tests for repository/data path detection."""

from bill_analyser.constants import CONFIG_DIR, DATA_DIR, LOG_DIR, PROJECT_ROOT, TEST_DB_DIR_ENV
from bill_analyser.core.db import Database
from tests import runtime_paths


def test_project_root_points_to_repository_root():
    """PROJECT_ROOT should point at the repository root directory."""
    assert (PROJECT_ROOT / "pyproject.toml").exists()
    assert (PROJECT_ROOT / "src" / "bill_analyser").exists()


def test_runtime_dirs_resolve_under_data_directory():
    """Runtime config/log/db directories should be colocated under data/."""
    assert DATA_DIR.exists()
    assert CONFIG_DIR == DATA_DIR / "config"
    assert LOG_DIR == DATA_DIR / "logs"


def test_relative_test_database_path_redirects_to_tests_runtime(monkeypatch, tmp_path):
    """Relative ./data/test_*.db paths should also be redirected into tests runtime storage."""
    runtime_db_dir = tmp_path / "tests-runtime-db"
    monkeypatch.setenv(TEST_DB_DIR_ENV, str(runtime_db_dir))

    database = Database("./data/test_relative_redirect.db")

    assert database.db_path == runtime_db_dir / "test_relative_redirect.db"


def test_xdist_test_database_paths_use_worker_subdirectories(monkeypatch, tmp_path):
    """Parallel pytest workers should not share SQLite runtime files."""
    runtime_db_dir = tmp_path / "tests-runtime-db"
    monkeypatch.setenv(runtime_paths.TEST_DB_DIR_ENV, str(runtime_db_dir))
    monkeypatch.setenv(runtime_paths.PYTEST_XDIST_WORKER_ENV, "gw3")

    first_path = runtime_paths.get_test_db_path("shared_name.db")
    second_path = runtime_paths.get_test_db_path("another.db")

    assert first_path == runtime_db_dir / "gw3" / "shared_name.db"
    assert second_path == runtime_db_dir / "gw3" / "another.db"
    assert runtime_paths.configure_test_runtime_environment() == runtime_db_dir / "gw3"
