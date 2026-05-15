from __future__ import annotations

import re
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_DIR = REPO_ROOT / ".gitea" / "workflows"
EXPECTED_WORKFLOW_NAMES = {
    "ci.yml",
}
EXPECTED_JOB_IDS = {
    "backend-ci",
    "frontend-ci",
    "agent-stack-health",
}


def _workflow_files() -> dict[str, Path]:
    workflow_paths = sorted(WORKFLOW_DIR.glob("*.yml")) + sorted(WORKFLOW_DIR.glob("*.yaml"))
    return {path.name: path for path in workflow_paths}


def _read(name: str) -> str:
    return _workflow_files()[name].read_text(encoding="utf-8")


def test_all_expected_gitea_workflows_exist() -> None:
    workflow_files = _workflow_files()
    assert EXPECTED_WORKFLOW_NAMES.issubset(workflow_files), "missing expected Gitea workflows"
    for path in workflow_files.values():
        assert path.exists(), f"missing workflow: {path}"


def test_gitea_ci_workflow_uses_parallel_jobs_instead_of_top_level_concurrency() -> None:
    text = _read("ci.yml")

    assert "concurrency:" not in text, "ci.yml should avoid undocumented top-level concurrency on Gitea"
    assert re.search(r"(?m)^\s*needs:\s*", text) is None, "ci.yml jobs should stay independent for parallel execution"
    for job_id in EXPECTED_JOB_IDS:
        assert re.search(rf"(?m)^  {re.escape(job_id)}:\s*$", text), f"ci.yml should declare job {job_id}"

    assert "TZ: Asia/Shanghai" in text, "ci.yml should pin the test timezone for deterministic date assertions"
    assert "JWT_SECRET_KEY: ci-test-jwt-secret" in text, "ci.yml should inject a deterministic JWT secret for tests"
    assert (
        "BILL_ANALYSER_OPERATION_PASSWORD: ci-test-operation-password" in text
    ), "ci.yml should inject a deterministic operation password for auth-related tests"

    assert "timeout-minutes:" not in text, "ci.yml must avoid unsupported timeout-minutes"
    assert "continue-on-error:" not in text, "ci.yml must avoid unsupported continue-on-error"
    assert re.search(r"(?m)^\s*environment:\s*", text) is None, "ci.yml must avoid unsupported job environment"


def test_ci_workflow_avoids_duplicate_feature_branch_push_runs() -> None:
    text = _read("ci.yml")

    assert re.search(
        r"(?ms)^  push:\n    branches:\n      - main\n    paths:",
        text,
    ), "feature branches should rely on pull_request CI instead of duplicate push CI"
    assert "refs/heads/codex/" not in text


def test_gitea_workflows_pin_read_only_contents_permissions() -> None:
    for name in _workflow_files():
        text = _read(name)
        assert "permissions:" in text, f"{name} must declare least-privilege permissions"
        assert re.search(r"(?m)^permissions:\n  contents: read$", text), f"{name} must pin contents: read"


def test_gitea_workflows_use_absolute_action_urls() -> None:
    for name in _workflow_files():
        text = _read(name)
        assert "uses: actions/" not in text, f"{name} should not rely on instance-default action source resolution"
        uses_lines = re.findall(r"(?m)^\s*uses:\s+(.+)$", text)
        assert uses_lines, f"{name} should declare explicit action sources"
        for uses in uses_lines:
            assert uses.startswith("https://github.com/"), f"{name} must use absolute GitHub action URLs on Gitea"


def test_ci_workflow_covers_gitea_contract_regression() -> None:
    text = _read("ci.yml")
    assert ".gitea/**" in text, "ci workflow should trigger on Gitea workflow changes"
    assert ".omx/plans/**" in text, "ci workflow should trigger on committed OMX plan writebacks"
    assert "tests/test_gitea_workflows.py" in text, "agent-stack job should cover the Gitea workflow contract test"
    assert (
        "python -m pytest tests/test_reviewer_agent_diff_contract.py tests/test_agent_stack_health.py tests/test_ai_workflow_docs.py tests/test_task_state.py tests/test_task_state_reader.py tests/test_gitea_workflows.py -v"
        in text
    )


def test_ci_workflow_enforces_backend_rust_and_frontend_coverage_gates() -> None:
    text = _read("ci.yml")

    assert "python scripts/check_rust_workspace_dependencies.py --json" in text
    assert "uses: https://github.com/actions/cache@v4" in text
    assert "Restore Rust toolchain cache" in text
    assert text.index("Restore Rust toolchain cache") < text.index("Setup Rust stable")
    assert "uses: https://github.com/actions/cache/restore@v4" in text
    assert "id: rust-toolchain-cache" in text
    assert "id: rust-toolchain" in text
    assert "~/.rustup/toolchains" in text
    assert "~/.rustup/update-hashes" in text
    assert "~/.rustup/settings.toml" in text
    assert "~/.cargo/bin/rustup" in text
    assert "~/.cargo/bin/cargo" in text
    assert "~/.cargo/bin/rustc" in text
    assert "key: ${{ runner.os }}-rust-toolchain-slim-v2-bootstrap" in text
    assert "${{ runner.os }}-rust-toolchain-slim-v2-" in text
    assert "~/.cargo/registry/index" in text
    assert "~/.cargo/registry/cache" in text
    assert re.search(r"(?m)^            ~/.cargo/registry/src$", text) is None
    assert "~/.cargo/git/db" in text
    assert "~/.cargo/bin/cargo-llvm-cov" in text
    assert re.search(r"(?m)^            target$", text) is None
    assert "key: ${{ runner.os }}-cargo-slim-v1-stable-${{ hashFiles('Cargo.lock') }}" in text
    assert "${{ runner.os }}-cargo-slim-v1-stable-" in text
    assert "Trim backend caches before cache save" in text
    assert text.index("Run Rust coverage") < text.index("Run pytest coverage")
    assert text.index("Run pytest coverage") < text.index("Trim backend caches before cache save")
    assert "cache_size_mb()" in text
    assert "rm -rf target" in text
    assert "rm -f workspace.lcov coverage.json .coverage .coverage.*" in text
    assert "cargo_cache_mb=" in text
    assert "cache_size_mb ~/.cargo/registry ~/.cargo/git ~/.cargo/bin/cargo-llvm-cov" in text
    assert "rm -rf ~/.cargo/registry/src ~/.cargo/git/checkouts" in text
    assert "rm -rf ~/.cargo/registry/cache ~/.cargo/git/db" in text
    assert "~/.cache/pip" in text
    assert "key: ${{ runner.os }}-pip-slim-v1-${{ hashFiles('pyproject.toml', 'uv.lock', '.python-version') }}" in text
    assert "${{ runner.os }}-pip-slim-v1-" in text
    assert "pip_cache_mb=" in text
    assert "python -m pip cache purge || rm -rf ~/.cache/pip" in text
    assert "rustup_cache_mb=" in text
    assert (
        "cache_size_mb ~/.rustup/toolchains ~/.rustup/update-hashes "
        "~/.rustup/settings.toml ~/.cargo/bin/rustup ~/.cargo/bin/cargo"
    ) in text
    assert 'if [ "$rustup_cache_mb" -gt 1450 ]; then' in text
    assert "rust_toolchain_cache_save=false" in text
    assert "rust_toolchain_cache_save=true" in text
    assert "rm -rf ~/.rustup/toolchains ~/.rustup/update-hashes ~/.rustup/settings.toml" in text
    assert "rm -f ~/.cargo/bin/rustup ~/.cargo/bin/cargo" in text
    assert "Resolve Rust toolchain cache key" in text
    assert "steps.rust-toolchain.outputs.cachekey" in text
    assert "Save Rust toolchain cache" in text
    assert "uses: https://github.com/actions/cache/save@v4" in text
    assert "steps.backend-cache-trim.outputs.rust_toolchain_cache_save == 'true'" in text
    assert (
        "steps.rust-toolchain-cache.outputs.cache-matched-key "
        "!= steps.rust-toolchain-cache-key.outputs.key"
    ) in text
    assert "cargo install cargo-llvm-cov --locked" in text
    assert "if ! command -v cargo-llvm-cov >/dev/null 2>&1; then" in text
    assert "cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90" in text
    assert (
        "python -m pytest --cov=src/bill_analyser --cov-report=term-missing --cov-report=json:coverage.json --cov-fail-under=90 tests/ -v -n auto --dist loadfile"
        in text
    )
    assert "pytest-xdist" in text
    assert "Restore npm cache" in text
    assert "path: ~/.npm" in text
    assert "key: ${{ runner.os }}-npm-slim-v1-${{ hashFiles('src/web/package-lock.json') }}" in text
    assert "${{ runner.os }}-npm-slim-v1-" in text
    assert "cache: npm" not in text
    assert "cache-dependency-path: src/web/package-lock.json" not in text
    assert "npm run test:coverage" in text
    assert "Trim frontend caches before cache save" in text
    assert text.index("npm run test:coverage") < text.index("npm run build")
    assert text.index("npm run build") < text.index("Trim frontend caches before cache save")
    assert "rm -rf node_modules coverage dist" in text
    assert "npm cache verify || true" in text
    assert "npm_cache_mb=" in text
    assert 'if [ -e "$npm_cache_dir" ]; then' in text
    assert "npm cache clean --force" in text
