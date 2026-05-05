from __future__ import annotations

import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
DB_CRATE = ROOT / "crates" / "bill-analyser-db"


def _load_toml(path: str) -> dict:
    with (ROOT / path).open("rb") as file:
        return tomllib.load(file)


def test_workspace_declares_internal_sqlite_db_crate() -> None:
    workspace = _load_toml("Cargo.toml")["workspace"]

    assert "crates/bill-analyser-core" in workspace["members"]
    assert "crates/bill-analyser-db" in workspace["members"]

    manifest = _load_toml("crates/bill-analyser-db/Cargo.toml")
    assert manifest["package"]["name"] == "bill-analyser-db"
    assert manifest["package"]["publish"] is False
    assert manifest["lib"]["name"] == "bill_analyser_db"
    assert "pyo3" not in manifest.get("dependencies", {})
    assert "sqlx" not in manifest.get("dependencies", {})
    assert "rusqlite" in manifest.get("dependencies", {})


def test_sqlite_runtime_sources_keep_real_db_path_guard_and_no_business_primary_claim() -> None:
    lib_rs = (DB_CRATE / "src" / "lib.rs").read_text(encoding="utf-8")
    path_rs = (DB_CRATE / "src" / "path.rs").read_text(encoding="utf-8")
    schema_rs = (DB_CRATE / "src" / "schema.rs").read_text(encoding="utf-8")

    assert "no business database write path is Rust-primary" in lib_rs
    assert "data/bills.db" in path_rs
    assert "deny_real_data_path" in path_rs
    assert "src/bill_analyser/core/database/runtime.py" in schema_rs
    assert "src/bill_analyser/core/database/schema/__init__.py" in schema_rs


def test_migration_docs_record_s3_foundation_without_python_runtime_cleanup() -> None:
    plan_text = (ROOT / "docs" / "rust-backend-migration-plan.md").read_text(encoding="utf-8")
    database_overview = (ROOT / "docs" / "overview-database.md").read_text(encoding="utf-8")

    assert "## S3 SQLite Schema Runtime Foundation" in plan_text
    assert "crates/bill-analyser-db" in plan_text
    assert "Rust-primary" in plan_text
    assert "Rust DB runtime foundation" in database_overview
    assert "Flask/Python Database façade" in database_overview
