"""Pytest shared teardown hooks."""

import asyncio
import inspect
import pkgutil
import sys
from importlib import import_module
from pathlib import Path
from types import ModuleType
from typing import Any, cast

from tests.runtime_paths import (
    cleanup_test_runtime_databases,
    configure_test_runtime_environment,
    remove_test_database_family,
)

REPO_ROOT = Path(__file__).resolve().parents[1]
SRC_ROOT = REPO_ROOT / "src"

configure_test_runtime_environment()
cleanup_test_runtime_databases()


def _ensure_src_layout_on_path() -> None:
    """Make the repository's src layout importable in clean CI environments."""
    src_root = str(SRC_ROOT)
    if SRC_ROOT.is_dir() and src_root not in sys.path:
        sys.path.insert(0, src_root)


def _import_bill_analyser_package() -> ModuleType:
    """Import the root package, bootstrapping the src path when needed."""
    try:
        return import_module("bill_analyser")
    except ModuleNotFoundError as exc:
        if exc.name != "bill_analyser":
            raise

        _ensure_src_layout_on_path()
        return import_module("bill_analyser")


def _is_missing_target_module(exc: ImportError, target_name: str) -> bool:
    """Return True only when the requested module itself is missing."""
    missing_name = getattr(exc, "name", None)
    return missing_name == target_name


def _import_optional_module(target_name: str) -> ModuleType | None:
    """Import an optional module without hiding nested import regressions."""
    try:
        return import_module(target_name)
    except ImportError as exc:
        if _is_missing_target_module(exc, target_name):
            return None
        raise


def _install_src_compat_aliases() -> None:
    """Alias legacy src.* imports to the current bill_analyser.* package tree."""
    root_package = _import_bill_analyser_package()
    sys.modules.setdefault("src", root_package)

    alias_roots = ("api", "core", "parsers", "utils")
    for root in alias_roots:
        target_name = f"bill_analyser.{root}"
        target_module = _import_optional_module(target_name)
        if target_module is None:
            continue
        sys.modules.setdefault(f"src.{root}", target_module)

        if not hasattr(target_module, "__path__"):
            continue

        for module_info in pkgutil.walk_packages(target_module.__path__, prefix=f"{target_name}."):
            try:
                imported_module = import_module(module_info.name)
            except ImportError:
                continue
            alias_name = f"src{module_info.name[len('bill_analyser'):]}"
            sys.modules.setdefault(alias_name, imported_module)

    for module_name in ("constants",):
        imported_module = _import_optional_module(f"bill_analyser.{module_name}")
        if imported_module is None:
            continue
        sys.modules.setdefault(f"src.{module_name}", imported_module)


_install_src_compat_aliases()


def _close_db_instance(db_instance) -> None:
    """Close async database instance safely in sync pytest hook."""
    if not db_instance or not hasattr(db_instance, "close"):
        return

    close_method = db_instance.close

    if inspect.iscoroutinefunction(close_method):
        original_policy = None
        policy_changed = False
        windows_selector_policy = cast("Any", getattr(asyncio, "WindowsSelectorEventLoopPolicy", None))

        if sys.platform == "win32" and windows_selector_policy is not None:
            original_policy = asyncio.get_event_loop_policy()
            if not isinstance(original_policy, windows_selector_policy):
                asyncio.set_event_loop_policy(windows_selector_policy())
                policy_changed = True

        loop = asyncio.new_event_loop()
        try:
            asyncio.set_event_loop(loop)
            loop.run_until_complete(close_method())
            loop.run_until_complete(loop.shutdown_asyncgens())
        finally:
            loop.close()
            if policy_changed and original_policy is not None:
                asyncio.set_event_loop_policy(original_policy)
        return

    close_method()


def pytest_sessionfinish(session, exitstatus):  # pylint: disable=unused-argument
    """Ensure global DB connections are closed so worker threads can exit."""
    try:
        from src.api import app as api_app  # pylint: disable=import-outside-toplevel

        db_from_config = api_app.app.config.get("DB_INSTANCE") if hasattr(api_app, "app") else None
        db_global = getattr(api_app, "db", None)
        db_path = getattr(db_from_config, "db_path", None) or getattr(db_global, "db_path", None)

        # 同时尝试两个引用；在 Database 实现中，close 是幂等的。
        _close_db_instance(db_from_config)
        if db_global is not db_from_config:
            _close_db_instance(db_global)

        if db_path:
            remove_test_database_family(Path(db_path))
    except Exception:
        # 在清理收尾阶段，绝不能让测试流程因为异常而失败。
        pass

    cleanup_test_runtime_databases()
