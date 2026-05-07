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

    assert len(expected_python_paths) == 321
    assert inventory.summary["python_backend_files"] == 321
    assert tuple(record.path for record in inventory.python_files) == expected_python_paths
    assert inventory.summary["rust_backend_files"] == 66
    assert "crates/bill-analyser-core/src/ai_ocr_llm.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/lib.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/parsers.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/smart_dedup.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/import_learning.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/import_pipeline.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/matching.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/budgets.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/statistics.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/ops.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/ai_ocr_llm_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/budget_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/import_learning_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/import_pipeline_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/matching_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/bin/bill_category_rule_bridge.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/src/category_rules/mod.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/parser_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/smart_dedup_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/statistics_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/ops_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-core/tests/transaction_adapter_contracts.rs" in inventory.rust_files
    assert "crates/bill-analyser-db/src/lib.rs" in inventory.rust_files
    assert "crates/bill-analyser-db/src/bin/bill_taxonomy_bridge.rs" in inventory.rust_files
    assert "crates/bill-analyser-http/src/lib.rs" in inventory.rust_files
    assert "crates/bill-analyser-http/src/proxy.rs" in inventory.rust_files
    assert "crates/bill-analyser-http/tests/proxy_contract.rs" in inventory.rust_files


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
    assert records["src/bill_analyser/import_contracts/preview_selection.py"].domain == "import-contracts"
    assert records["src/bill_analyser/import_contracts/preview_selection.py"].initial_status == "facade"
    assert records["src/bill_analyser/core/category_engine/engine.py"].domain == "classification-rules"
    assert records["src/bill_analyser/core/account_rust_bridge.py"].domain == "accounts"
    assert records["src/bill_analyser/core/auth_rust_bridge.py"].domain == "auth-security"
    assert records["src/bill_analyser/core/category_rule_rust_bridge.py"].domain == "classification-rules"
    assert records["src/bill_analyser/core/category_rust_bridge.py"].domain == "classification-rules"
    assert records["src/bill_analyser/core/settings_bundle_rust_bridge.py"].domain == "settings-bundle"
    assert records["src/bill_analyser/core/tag_rust_bridge.py"].domain == "tags-templates"
    assert records["src/bill_analyser/core/template_rust_bridge.py"].domain == "tags-templates"
    assert records["src/bill_analyser/api/routes/templates.py"].initial_status == "facade"
    assert records["src/bill_analyser/core/database/templates/recurring.py"].initial_status == "facade"
    assert records["src/bill_analyser/core/database/templates/schedule.py"].initial_status == "facade"
    assert records["src/bill_analyser/core/database/templates/serialization.py"].initial_status == "facade"
    assert records["src/bill_analyser/parsers/wechat.py"].domain == "import-parsers"


def test_inventory_markdown_is_deterministic_and_contains_auditable_counts(repo_root: Path) -> None:
    inventory = rust_migration_inventory.build_inventory(repo_root)

    first_render = rust_migration_inventory.render_inventory_markdown(inventory)
    second_render = rust_migration_inventory.render_inventory_markdown(inventory)

    assert first_render == second_render
    assert "# Rust Backend Migration Inventory" in first_render
    assert "- Python backend files: 321" in first_render
    assert "- Rust backend files: 66" in first_render
    assert "- Verified dead files: 0" in first_render
    assert "| src/bill_analyser/api/app.py | api-runtime-shell | api-shell | facade |" in first_render
    assert "| src/bill_analyser/core/account_rust_bridge.py | accounts | core-service | port |" in first_render
    assert (
        "| src/bill_analyser/core/category_rule_rust_bridge.py | classification-rules | core-service | port |"
        in first_render
    )
    assert "| src/bill_analyser/core/category_rust_bridge.py | classification-rules | core-service | port |" in first_render
    assert "| src/bill_analyser/core/settings_bundle_rust_bridge.py | settings-bundle | core-service | port |" in first_render
    assert "| src/bill_analyser/core/tag_rust_bridge.py | tags-templates | core-service | port |" in first_render
    assert "| src/bill_analyser/core/template_rust_bridge.py | tags-templates | core-service | port |" in first_render
    assert "| src/bill_analyser/api/routes/templates.py | tags-templates | api-route | facade |" in first_render
    assert (
        "| src/bill_analyser/core/database/templates/schedule.py | tags-templates | database-access | facade |"
        in first_render
    )
    assert "- crates/bill-analyser-core/src/lib.rs" in first_render
    assert "- crates/bill-analyser-core/src/parsers.rs" in first_render
    assert "- crates/bill-analyser-core/src/smart_dedup.rs" in first_render
    assert "- crates/bill-analyser-core/src/import_learning.rs" in first_render
    assert "- crates/bill-analyser-core/src/import_pipeline.rs" in first_render
    assert "- crates/bill-analyser-core/src/matching.rs" in first_render
    assert "- crates/bill-analyser-core/src/budgets.rs" in first_render
    assert "- crates/bill-analyser-core/src/statistics.rs" in first_render
    assert "- crates/bill-analyser-core/src/ops.rs" in first_render
    assert "- crates/bill-analyser-core/src/ai_ocr_llm.rs" in first_render
    assert "- crates/bill-analyser-core/src/bin/bill_category_rule_bridge.rs" in first_render
    assert "- crates/bill-analyser-core/src/category_rules/mod.rs" in first_render
    assert "- crates/bill-analyser-core/tests/parser_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/smart_dedup_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/import_learning_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/import_pipeline_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/matching_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/budget_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/statistics_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/ops_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/ai_ocr_llm_contracts.rs" in first_render
    assert "- crates/bill-analyser-core/tests/transaction_adapter_contracts.rs" in first_render
    assert "- crates/bill-analyser-db/src/lib.rs" in first_render
    assert "- crates/bill-analyser-db/src/bin/bill_taxonomy_bridge.rs" in first_render
    assert "- crates/bill-analyser-http/src/lib.rs" in first_render
    assert "- crates/bill-analyser-http/src/proxy.rs" in first_render
    assert "- crates/bill-analyser-http/tests/proxy_contract.rs" in first_render


def test_migration_plan_markdown_is_deterministic_and_keeps_s0_non_runtime(repo_root: Path) -> None:
    inventory = rust_migration_inventory.build_inventory(repo_root)

    first_render = rust_migration_inventory.render_plan_markdown(inventory)
    second_render = rust_migration_inventory.render_plan_markdown(inventory)

    assert first_render == second_render
    assert "# Rust Backend Migration Plan Baseline" in first_render
    assert "S0 does not add a Rust runtime, does not change Flask route behavior" in first_render
    assert "Every Python backend file remains preserved unless a later slice proves verified-dead" in first_render
