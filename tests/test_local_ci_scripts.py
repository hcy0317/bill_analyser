from __future__ import annotations

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def test_pyproject_local_test_task_matches_ci_parallel_pytest() -> None:
    text = (REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8")

    assert "--cov-report=json:coverage.json" in text
    assert "tests/ -v -n auto --dist loadfile" in text


def test_frontend_test_scripts_use_parallel_workers() -> None:
    text = (REPO_ROOT / "src/web/package.json").read_text(encoding="utf-8")

    assert "--runInBand" not in text
    assert "--maxWorkers=50%" in text


def test_local_ci_and_cache_trim_scripts_mirror_ci_contract() -> None:
    local_ci = (REPO_ROOT / "scripts/run_ci_local.ps1").read_text(encoding="utf-8")
    cache_trim = (REPO_ROOT / "scripts/trim_ci_caches.ps1").read_text(encoding="utf-8")

    assert "cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90" in local_ci
    assert "--cov-report=json:coverage.json --cov-fail-under=90 tests/ -v -n auto --dist loadfile" in local_ci
    assert "npm run test:coverage" in local_ci
    assert "trim_ci_caches.ps1" in local_ci

    for expected in ("target", "workspace.lcov", "coverage.json", ".coverage", "src\\web\\coverage", "src\\web\\dist"):
        assert expected in cache_trim
    assert "CargoCacheLimitMb = 450" in cache_trim
    assert "PipCacheLimitMb = 300" in cache_trim
    assert "NpmCacheLimitMb = 300" in cache_trim
    assert "AllowedRoot" in cache_trim
    assert "Refusing to remove path outside allowed root" in cache_trim
