from __future__ import annotations

from pathlib import Path

import pytest

from scripts import rust_migration_inventory


@pytest.fixture(scope="module")
def repo_root() -> Path:
    return Path(__file__).resolve().parents[4]


def test_inventory_covers_all_backend_python_files_and_current_rust_count(repo_root: Path) -> None:
    inventory = rust_migration_inventory.build_inventory(repo_root)
    expected_python_paths = tuple(
        sorted(path.relative_to(repo_root).as_posix() for path in (repo_root / "src/bill_analyser").rglob("*.py"))
    )

    assert len(expected_python_paths) == 314
    assert inventory.summary["python_backend_files"] == 314
    assert tuple(record.path for record in inventory.python_files) == expected_python_paths
    assert inventory.summary["rust_backend_files"] == 32
    assert "crates/bill-analyser-core/src/lib.rs" in inventory.rust_files
    assert "crates/bill-analyser-db/src/lib.rs" in inventory.rust_files


def test_every_python_file_has_migration_domain_status_and_no_verified_dead(repo_root: Path) -> None:
    inventory = rust_migration_inventory.build_inventory(repo_root)
    records = {record.path: record for record in inventory.python_files}

    assert records
    assert inventory.summary["verified_dead_files"] == 0
    assert {record.initial_status for record in inventory.python_files} <= {"port", "facade", "deferred"}
    assert all(record.domain for record in inventory.python_files)
    assert all(record.initial_status != "verified_dead" for record in inventory.python_files)

    assert records["src/bill_analyser/api/app.py"].domain == "api-runtime-shell"
    assert records["src/bill_analyser/api/app.py"].initial_status == "facade"
    assert records["src/bill_analyser/core/db.py"].domain == "database-facade"
    assert records["src/bill_analyser/core/db.py"].initial_status == "facade"
    assert records["src/bill_analyser/core/bills/service_parts/import_v2_pipeline.py"].domain == "bills-import"
    assert records["src/bill_analyser/core/database/bills/__init__.py"].domain == "bills-import"
    assert records["src/bill_analyser/core/database/bills/__init__.py"].initial_status == "port"
    assert records["src/bill_analyser/core/database/bills/__init__.py"].role == "package-implementation"
    assert records["src/bill_analyser/core/category_engine/engine.py"].domain == "classification-rules"
    assert records["src/bill_analyser/core/auth_rust_bridge.py"].domain == "auth-security"
    assert records["src/bill_analyser/parsers/wechat.py"].domain == "import-parsers"


def test_inventory_markdown_is_deterministic_and_contains_auditable_counts(repo_root: Path) -> None:
    inventory = rust_migration_inventory.build_inventory(repo_root)

    first_render = rust_migration_inventory.render_inventory_markdown(inventory)
    second_render = rust_migration_inventory.render_inventory_markdown(inventory)

    assert first_render == second_render
    assert "# Rust Backend Migration Inventory" in first_render
    assert "- Python backend files: 314" in first_render
    assert "- Rust backend files: 32" in first_render
    assert "- Verified dead files: 0" in first_render
    assert "| src/bill_analyser/api/app.py | api-runtime-shell | api-shell | facade |" in first_render
    assert "- crates/bill-analyser-core/src/lib.rs" in first_render
    assert "- crates/bill-analyser-db/src/lib.rs" in first_render


def test_migration_plan_markdown_is_deterministic_and_keeps_s0_non_runtime(repo_root: Path) -> None:
    inventory = rust_migration_inventory.build_inventory(repo_root)

    first_render = rust_migration_inventory.render_plan_markdown(inventory)
    second_render = rust_migration_inventory.render_plan_markdown(inventory)

    assert first_render == second_render
    assert "# Rust Backend Migration Plan Baseline" in first_render
    assert "S0 does not add a Rust runtime, does not change Flask route behavior" in first_render
    assert "Every Python backend file remains preserved unless a later slice proves verified-dead" in first_render
