"""Regression checks for the Rust refactor readiness audit artifact contract."""

from __future__ import annotations

from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
AUDIT_ROOT = REPO_ROOT / ".omx" / "audits" / "rust-refactor-readiness"

if not AUDIT_ROOT.exists():
    pytest.skip(
        ".omx readiness audit artifacts are local ignored outputs; enforce the contract only when present",
        allow_module_level=True,
    )


REQUIRED_ARTIFACTS = [
    "00-intake.md",
    "01-go-no-go.md",
    "02-ownership-matrix.md",
    "03-runtime-graph.md",
    "04-bridge-db-contract-gaps.md",
    "05-packaging-deployment.md",
    "06-rust-owned-candidates.md",
    "07-deletion-candidates.md",
    "08-next-slices.md",
]

REQUIRED_EVIDENCE = [
    "migration-inventory.json",
    "route-map.txt",
    "bridge-scan.txt",
    "db-owner-scan.txt",
    "packaging-scan.txt",
    "command-log.md",
]

REQUIRED_LANES = [
    "REST runtime owner",
    "critical domain owner",
    "DB writer/migration owner",
    "Python↔Rust bridge owner",
    "packaging/deployment owner",
    "CI/release/start scripts owner",
]


def _read(relative_path: str) -> str:
    return (AUDIT_ROOT / relative_path).read_text(encoding="utf-8")


def test_audit_package_contains_required_artifacts_and_evidence() -> None:
    missing_artifacts = [name for name in REQUIRED_ARTIFACTS if not (AUDIT_ROOT / name).is_file()]
    missing_evidence = [name for name in REQUIRED_EVIDENCE if not (AUDIT_ROOT / "evidence" / name).is_file()]

    assert missing_artifacts == []
    assert missing_evidence == []


def test_go_no_go_uses_conservative_no_go_gate_with_minimum_lanes() -> None:
    content = _read("01-go-no-go.md")

    assert "Decision: NO-GO" in content
    assert "ownership" in content.lower()
    for lane in REQUIRED_LANES:
        assert lane in content


def test_audit_artifacts_do_not_claim_verified_dead_python_or_go_status() -> None:
    combined = "\n".join(_read(name) for name in REQUIRED_ARTIFACTS)

    assert "Decision: GO" not in combined
    assert "cargo test" in combined
    assert "verified-dead" in combined
    assert "verified_dead_files=0" in combined
    assert "TODO" not in combined
    assert "TBD" not in combined
