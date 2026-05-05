from __future__ import annotations

import tomllib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[4]


def _load_toml(path: str) -> dict:
    with (REPO_ROOT / path).open("rb") as file:
        return tomllib.load(file)


def test_rust_workspace_declares_single_internal_core_crate() -> None:
    workspace = _load_toml("Cargo.toml")["workspace"]

    assert workspace["resolver"] == "2"
    assert workspace["members"] == ["crates/bill-analyser-core"]


def test_core_crate_keeps_rust_as_internal_library_boundary() -> None:
    manifest = _load_toml("crates/bill-analyser-core/Cargo.toml")

    assert manifest["package"]["name"] == "bill-analyser-core"
    assert manifest["lib"]["name"] == "bill_analyser_core"
    assert manifest["package"]["publish"] is False
    assert "pyo3" not in manifest.get("dependencies", {})


def test_architecture_docs_keep_flask_rest_as_runtime_shell() -> None:
    architecture = (REPO_ROOT / "docs/overview-architecture.md").read_text(encoding="utf-8")
    migration_plan = (REPO_ROOT / "docs/rust-backend-migration-plan.md").read_text(encoding="utf-8")

    assert "Flask REST 外壳" in architecture
    assert "Rust 内部库边界" in architecture
    assert "S1 Rust Runtime Shell" in migration_plan
    assert "不接管任何业务 API" in migration_plan
