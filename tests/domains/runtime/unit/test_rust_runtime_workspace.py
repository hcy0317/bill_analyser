from __future__ import annotations

import tomllib
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[4]


def _load_toml(path: str) -> dict:
    with (REPO_ROOT / path).open("rb") as file:
        return tomllib.load(file)


def test_rust_workspace_declares_internal_runtime_crates() -> None:
    workspace = _load_toml("Cargo.toml")["workspace"]

    assert workspace["resolver"] == "2"
    assert workspace["members"] == [
        "crates/bill-analyser-core",
        "crates/bill-analyser-db",
        "crates/bill-analyser-http",
    ]


def test_core_crate_keeps_rust_as_internal_library_boundary() -> None:
    manifest = _load_toml("crates/bill-analyser-core/Cargo.toml")

    assert manifest["package"]["name"] == "bill-analyser-core"
    assert manifest["lib"]["name"] == "bill_analyser_core"
    assert manifest["package"]["publish"] is False
    assert "pyo3" not in manifest.get("dependencies", {})


def test_http_crate_is_opt_in_proxy_shell_not_business_owner() -> None:
    manifest = _load_toml("crates/bill-analyser-http/Cargo.toml")

    assert manifest["package"]["name"] == "bill-analyser-http"
    assert manifest["lib"]["name"] == "bill_analyser_http"
    assert manifest["package"]["publish"] is False
    assert "axum" in manifest["dependencies"]
    assert "reqwest" in manifest["dependencies"]
    assert "pyo3" not in manifest.get("dependencies", {})


def test_architecture_docs_keep_flask_default_and_http_shell_opt_in() -> None:
    architecture = (REPO_ROOT / "docs/overview-architecture.md").read_text(encoding="utf-8")
    migration_plan = (REPO_ROOT / "docs/rust-backend-migration-plan.md").read_text(encoding="utf-8")

    assert "Flask REST 外壳" in architecture
    assert "opt-in `rust-http-shell:proxy-only`" in architecture
    assert "`api_takeover=false`、`business_migration=none`" in architecture
    assert "S1 Rust Runtime Shell" in migration_plan
    assert "S1b Opt-in Rust HTTP Ingress" in migration_plan
    assert "不接管任何业务 API" in migration_plan
    assert "proxied Python routes are not Rust business-owned" in migration_plan


def test_auth_bridge_runtime_is_built_before_backend_pytest_and_startup() -> None:
    start_backend = (REPO_ROOT / "start_backend.ps1").read_text(encoding="utf-8")
    gitea_ci = (REPO_ROOT / ".gitea/workflows/ci.yml").read_text(encoding="utf-8")

    assert 'Package = "bill-analyser-core"' in start_backend
    assert 'Bin = "bill_auth_bridge"' in start_backend
    assert "$env:BILL_ANALYSER_RUST_AUTH_BRIDGE" in start_backend
    assert 'Package = "bill-analyser-db"' in start_backend
    assert 'Bin = "bill_taxonomy_bridge"' in start_backend
    assert "$env:BILL_ANALYSER_RUST_TAXONOMY_BRIDGE" in start_backend
    assert "Error: Rust toolchain not found" in start_backend
    assert "Configured Rust $($BridgeSpec.DisplayName) bridge not found" in start_backend
    assert "https://github.com/dtolnay/rust-toolchain@stable" in gitea_ci
    assert "cargo build -p bill-analyser-core --bin bill_auth_bridge" in gitea_ci
    assert "cargo build -p bill-analyser-db --bin bill_taxonomy_bridge" in gitea_ci
