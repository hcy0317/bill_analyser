from __future__ import annotations

from scripts.hooks import work_context


def test_detect_scopes_marks_crates_as_rust_runtime() -> None:
    scopes = work_context.detect_scopes(("crates/bill-analyser-http/src/router.rs",))

    assert "rust-runtime" in scopes
    assert "backend-runtime" not in scopes


def test_build_verification_steps_uses_cargo_for_rust_runtime() -> None:
    steps = work_context.build_verification_steps(("crates/bill-analyser-http/src/router.rs",))

    assert any("cargo test" in step for step in steps)
    assert any("cargo llvm-cov" in step for step in steps)
    assert not any("pytest" in step for step in steps)


def test_choose_next_step_prefers_rust_runtime_over_python_backend() -> None:
    next_step = work_context.choose_next_step(("crates/bill-analyser-http/src/router.rs",))

    assert "Rust" in next_step
    assert "cargo" in next_step


def test_cargo_manifest_is_treated_as_rust_runtime() -> None:
    scopes = work_context.detect_scopes(("Cargo.toml",))

    assert "rust-runtime" in scopes
