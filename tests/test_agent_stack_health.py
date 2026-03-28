from __future__ import annotations

import importlib
import json
from pathlib import Path
import re
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "agent_stack_health.py"


def _assert_session_completion_contract(text: str, requirement_groups: tuple[tuple[str, ...], ...]) -> None:

    for candidates in requirement_groups:
        assert any(candidate in text for candidate in candidates)


def _run_agent_stack_health(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(SCRIPT_PATH), *args],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _checks_by_id(payload: dict) -> dict[str, dict]:
    return {check["id"]: check for check in payload["checks"]}


def test_repo_scan_reports_expected_contracts() -> None:
    result = _run_agent_stack_health("--mode", "repo", "--format", "json")

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    checks = _checks_by_id(payload)

    assert checks["repo.entrypoints"]["status"] == "pass"
    assert checks["repo.agent-parity"]["status"] == "pass"
    assert checks["repo.skill-parity"]["status"] == "pass"
    assert checks["repo.cursor-hooks-trimmed"]["status"] == "pass"
    assert checks["repo.diff-commit-skill"]["status"] == "pass"
    assert checks["repo.codex-baseline"]["status"] == "pass"


def test_global_scan_warns_when_claude_hook_settings_are_missing(tmp_path: Path) -> None:
    fake_home = tmp_path / "home"
    (fake_home / ".claude").mkdir(parents=True)
    (fake_home / ".cursor" / "skills-cursor").mkdir(parents=True)
    (fake_home / ".cursor" / "skills-cursor" / ".cursor-managed-skills-manifest.json").write_text(
        "{}",
        encoding="utf-8",
    )
    (fake_home / ".codex").mkdir(parents=True)
    (fake_home / ".codex" / "config.toml").write_text(
        'model = "gpt-5.4"\n\n[mcp_servers.playwright]\ncommand = "npx"\n',
        encoding="utf-8",
    )

    result = _run_agent_stack_health("--mode", "global", "--format", "json", "--home", str(fake_home))

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    checks = _checks_by_id(payload)

    assert checks["global.claude.settings"]["status"] == "warn"
    assert checks["global.cursor.skills-manifest"]["status"] == "pass"
    assert checks["global.codex.config"]["status"] == "pass"


def test_probe_catalog_exposes_manual_behavior_checks() -> None:
    result = _run_agent_stack_health("--mode", "probes", "--format", "json")

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    probe_ids = {probe["id"] for probe in payload["manualProbes"]}

    assert {
        "reviewer-scope-refusal",
        "money-unit-convention",
        "python-hook-warning",
        "typescript-hook-warning",
    } <= probe_ids


def test_pytest_conftest_bootstraps_src_layout_in_clean_python() -> None:
    conftest_path = REPO_ROOT / "tests" / "conftest.py"
    src_path = REPO_ROOT / "src"
    bootstrap_probe = "\n".join(
        (
            "from pathlib import Path",
            "import runpy",
            "import sys",
            f"repo_root = Path(r'{REPO_ROOT}')",
            f"src_path = Path(r'{src_path}')",
            "sys.path[:] = [entry for entry in sys.path if entry and Path(entry).resolve() != src_path]",
            "sys.path.insert(0, str(repo_root))",
            f"runpy.run_path(r'{conftest_path}')",
            "import bill_analyser",
            "print(bill_analyser.__file__)",
        )
    )

    result = subprocess.run(
        [sys.executable, "-S", "-c", bootstrap_probe],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0, result.stderr or result.stdout
    assert "src\\bill_analyser\\__init__.py" in result.stdout.replace("/", "\\")


def test_account_adapter_imports_under_python3() -> None:
    adapter_path = REPO_ROOT / "src" / "bill_analyser" / "api" / "adapters" / "account_adapter.py"
    source = adapter_path.read_text(encoding="utf-8")

    compile(source, str(adapter_path), "exec")
    module = importlib.import_module("bill_analyser.api.adapters.account_adapter")

    assert hasattr(module, "AccountAdapter")


def test_workspace_instructions_require_diff_commit_skill() -> None:
    instructions_text = (REPO_ROOT / ".github" / "copilot-instructions.md").read_text(encoding="utf-8")
    codex_text = (REPO_ROOT / ".codex" / "AGENTS.md").read_text(encoding="utf-8")
    repo_skill_text = (
        REPO_ROOT / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md"
    ).read_text(encoding="utf-8")
    claude_skill_text = (
        REPO_ROOT / ".claude" / "skills" / "bill-analyser" / "SKILL.md"
    ).read_text(encoding="utf-8")

    entrypoint_requirements = (
        ("## Session Completion", "Session Completion"),
        ("会话结束前", "结束会话时", "会话完成前", "ending a session", "Before ending a session"),
        ("git diff", "staged", "unstaged"),
        ("zh-conventional-commit-from-diff",),
        ("中文 Conventional Commit 标题", "中文约定式提交标题", "Chinese Conventional Commit title"),
    )
    skill_requirements = (
        ("会话结束前", "结束会话时", "ending a session", "Before ending a session"),
        ("git diff", "diff", "staged", "unstaged"),
        ("zh-conventional-commit-from-diff",),
        ("中文 Conventional Commit 标题", "中文约定式提交标题", "Chinese Conventional Commit title"),
    )

    for contract_text in (instructions_text, codex_text):
        _assert_session_completion_contract(contract_text, entrypoint_requirements)

    for contract_text in (repo_skill_text, claude_skill_text):
        _assert_session_completion_contract(contract_text, skill_requirements)


def test_agent_stack_workflow_uses_runner_compatible_python() -> None:
    workflow_text = (REPO_ROOT / ".github" / "workflows" / "agent-stack-health.yml").read_text(encoding="utf-8")
    match = re.search(r'python-version:\s*["\']?(?P<version>[^"\'\s]+)["\']?', workflow_text)

    assert match is not None, "agent-stack-health workflow must declare setup-python version"
    assert match.group("version") == "3.13", (
        "agent-stack-health workflow should stay on Python 3.13 until the runner reliably supports 3.14 setup"
    )