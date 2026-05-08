"""Repository layout regression tests."""

from __future__ import annotations

import re
from pathlib import Path


WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = WORKSPACE_ROOT / "src"
TESTS_ROOT = WORKSPACE_ROOT / "tests"
FRONTEND_SRC_ROOT = WORKSPACE_ROOT / "src" / "web" / "src"
FRONTEND_TEST_ROOT = TESTS_ROOT / "web"
SRC_LOGS_ROOT = SRC_ROOT / "logs"
SHADOW_SRC_DIRS = tuple(
    SRC_ROOT / name
    for name in ("api", "config", "core", "data", "parsers", "uploads", "utils")
)
PYTHON_TEST_FILE_PATTERN = re.compile(
    r"^(test_.*|.*_test|check_.*|debug_.*|quick_.*|manual_test_.*|e2e_.*)\.py$"
)
ALLOWED_NON_TEST_CHECK_SCRIPTS = {
    "scripts/check_rust_workspace_dependencies.py",
}
FRONTEND_TEST_SUFFIXES = (
    ".test.ts",
    ".spec.ts",
    ".test.tsx",
    ".spec.tsx",
    ".test.js",
    ".spec.js",
    ".test.jsx",
    ".spec.jsx",
)


def _is_relative_to(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _iter_shadow_noncache_files():
    for directory in SHADOW_SRC_DIRS:
        if not directory.exists():
            continue
        for path in directory.rglob("*"):
            if path.is_dir():
                continue
            if "__pycache__" in path.parts or path.suffix == ".pyc":
                continue
            yield path.relative_to(WORKSPACE_ROOT).as_posix()


def _iter_repository_python_files_outside_tests():
    yield from WORKSPACE_ROOT.glob("*.py")

    scripts_root = WORKSPACE_ROOT / "scripts"
    if scripts_root.exists():
        yield from scripts_root.rglob("*.py")

    backend_src_root = WORKSPACE_ROOT / "src"
    if backend_src_root.exists():
        yield from backend_src_root.rglob("*.py")


def test_shadow_src_directories_do_not_hold_runtime_files():
    unexpected_files = sorted(_iter_shadow_noncache_files())
    assert not unexpected_files, (
        "发现不应再存在的 src 顶层阴影目录文件，请统一到 src/bill_analyser 或根目录规范位置: "
        + "; ".join(unexpected_files)
    )


def test_python_test_like_files_live_under_tests_directory():
    unexpected_files: list[str] = []

    for path in _iter_repository_python_files_outside_tests():
        if _is_relative_to(path, TESTS_ROOT):
            continue
        if PYTHON_TEST_FILE_PATTERN.match(path.name):
            relative_path = path.relative_to(WORKSPACE_ROOT).as_posix()
            if relative_path not in ALLOWED_NON_TEST_CHECK_SCRIPTS:
                unexpected_files.append(relative_path)

    assert not unexpected_files, (
        "发现 tests/ 之外的 Python 测试/调试/检查脚本，请统一迁移到 tests/: "
        + "; ".join(sorted(unexpected_files))
    )


def test_frontend_tests_are_centralized_under_tests_web():
    unexpected_entries: list[str] = []

    if FRONTEND_SRC_ROOT.exists():
        for path in FRONTEND_SRC_ROOT.rglob("*"):
            if path.is_dir() and path.name == "__tests__":
                unexpected_entries.append(path.relative_to(WORKSPACE_ROOT).as_posix())
                continue
            if not path.is_file():
                continue
            if path.name.endswith(FRONTEND_TEST_SUFFIXES):
                unexpected_entries.append(path.relative_to(WORKSPACE_ROOT).as_posix())

    assert not unexpected_entries, (
        "发现 src/web/src 下仍残留前端测试文件，请统一迁移到 tests/web/: "
        + "; ".join(sorted(unexpected_entries))
    )


def test_repo_level_frontend_test_directory_exists():
    assert FRONTEND_TEST_ROOT.exists(), "前端测试应统一存放在 tests/web/ 目录"


def test_src_logs_does_not_contain_runtime_log_artifacts():
    if not SRC_LOGS_ROOT.exists():
        return

    allowed_files = {".gitkeep", "README.md", "README.txt"}
    unexpected_entries = [
        path.relative_to(WORKSPACE_ROOT).as_posix()
        for path in SRC_LOGS_ROOT.rglob("*")
        if path.is_file() and path.name not in allowed_files
    ]

    assert not unexpected_entries, (
        "发现 src/logs 下仍存在运行日志产物，请统一迁移到根目录 logs/: "
        + "; ".join(sorted(unexpected_entries))
    )
