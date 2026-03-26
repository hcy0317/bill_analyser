"""Regression tests for repository root path detection."""

from bill_analyser.constants import PROJECT_ROOT


def test_project_root_points_to_repository_root():
    """PROJECT_ROOT should point at the repository root directory."""
    assert (PROJECT_ROOT / "pyproject.toml").exists()
    assert (PROJECT_ROOT / "src" / "bill_analyser").exists()


def test_project_root_uses_top_level_config_and_data_dirs():
    """PROJECT_ROOT should resolve top-level config and data directories."""
    assert (PROJECT_ROOT / "config").exists()
    assert (PROJECT_ROOT / "data").exists()
