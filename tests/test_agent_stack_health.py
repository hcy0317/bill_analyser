from __future__ import annotations

import importlib
import json
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "agent_stack_health.py"


def _assert_session_completion_contract(text: str, requirement_groups: tuple[tuple[str, ...], ...]) -> None:

    for candidates in requirement_groups:
        assert any(candidate in text for candidate in candidates)


def _run_agent_stack_health(*args: str) -> subprocess.CompletedProcess[str]:
    agent_stack_health = importlib.import_module("scripts.agent_stack_health")

    argv = list(args)

    def _get_arg(name: str, default: str) -> str:
        if name not in argv:
            return default

        index = argv.index(name)
        if index + 1 >= len(argv):
            return default

        return argv[index + 1]

    mode = _get_arg("--mode", "all")
    output_format = _get_arg("--format", "text")
    repo_root = Path(_get_arg("--repo-root", str(REPO_ROOT))).resolve()
    home = Path(_get_arg("--home", str(Path.home()))).resolve()

    checks = []
    probes = []

    if mode in {"repo", "all"}:
        checks.extend(agent_stack_health.scan_repo(repo_root))

    if mode in {"global", "all"}:
        checks.extend(agent_stack_health.scan_global(home, repo_root))

    if mode in {"probes", "all"}:
        probes = agent_stack_health.build_manual_probes()

    payload = agent_stack_health.build_payload(mode, repo_root, home, checks, probes)
    if output_format == "json":
        stdout = json.dumps(payload, ensure_ascii=False, indent=2)
    elif output_format == "doctor":
        stdout = agent_stack_health.render_doctor(payload)
    else:
        stdout = agent_stack_health.render_text(payload)

    return subprocess.CompletedProcess(
        args=[sys.executable, str(SCRIPT_PATH), *args],
        returncode=1 if payload["summary"]["fail"] else 0,
        stdout=stdout,
        stderr="",
    )


def _checks_by_id(payload: dict) -> dict[str, dict]:
    return {check["id"]: check for check in payload["checks"]}


def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _build_minimal_parser_standard_flow_fake_repo(
    repo_root: Path,
    *,
    ai_workflow_text: str,
    include_parser_doc: bool = True,
) -> None:
    _write_text(repo_root / "AGENTS.md", "Legacy `.cursor/` compatibility mirrors were intentionally removed\n")
    _write_text(repo_root / ".github" / "copilot-instructions.md", "")
    _write_text(repo_root / ".codex" / "AGENTS.md", "")
    _write_text(repo_root / ".codex" / "config.toml", 'model = "gpt-5.4"\n')
    _write_text(repo_root / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md", "")
    _write_text(repo_root / ".claude" / "skills" / "bill-analyser" / "SKILL.md", "")
    _write_text(
        repo_root / ".agents" / "skills" / "add-parser-standard-flow" / "SKILL.md",
        "\n".join(
            (
                "crates/bill-analyser-parsers/src/lib.rs",
                "crates/bill-analyser-parsers/tests/parser_contracts.rs",
                "crates/bill-analyser-http/tests/import_runtime_contract.rs",
                "parse_dedicated_import_bytes",
                "src/bill_analyser/parsers/factory.py",
                "src/bill_analyser/parsers/base.py",
                "tests/test_parser_base_factory.py",
                "tests/new_ui/test_import_parser_alignment.py",
                "ParserFactory",
                "StandardBill",
            )
        ),
    )
    if include_parser_doc:
        _write_text(
            repo_root / "docs" / "parsers" / "add-parser-standard-flow.md",
            "\n".join(
                (
                    ".agents/skills/add-parser-standard-flow/SKILL.md",
                    "crates/bill-analyser-parsers/src/lib.rs",
                    "crates/bill-analyser-parsers/tests/parser_contracts.rs",
                    "crates/bill-analyser-http/tests/import_runtime_contract.rs",
                    "src/bill_analyser/parsers/factory.py",
                    "src/bill_analyser/parsers/base.py",
                    "tests/test_parser_base_factory.py",
                    "tests/new_ui/test_import_parser_alignment.py",
                )
            ),
        )
    _write_text(repo_root / "docs" / "AI_WORKFLOW.md", ai_workflow_text)


def _build_minimal_ui_style_reference_fake_repo(
    repo_root: Path,
    *,
    ai_workflow_text: str,
    include_agents_reference: bool = True,
    missing_source_paths: tuple[str, ...] = (),
) -> None:
    _write_text(repo_root / "AGENTS.md", (
        "## Default AI workflow\n"
        "- 如果任务主要在做页面布局、按钮样式、颜色、弹窗、表格或整体视觉一致性，再补读 "
        "`.agents/skills/bill-analyser-ui-style-reference/SKILL.md`。\n"
        if include_agents_reference
        else "Legacy `.cursor/` compatibility mirrors were intentionally removed\n"
    ))
    _write_text(repo_root / ".github" / "copilot-instructions.md", "")
    _write_text(repo_root / ".codex" / "AGENTS.md", "")
    _write_text(repo_root / ".codex" / "config.toml", 'model = "gpt-5.4"\n')
    _write_text(repo_root / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md", "")
    _write_text(repo_root / ".claude" / "skills" / "bill-analyser" / "SKILL.md", "")
    source_paths = (
        "src/web/src/desktop-main.ts",
        "src/web/src/MobileApp.vue",
        "src/web/src/styles/desktop/global.scss",
        "src/web/src/styles/mobile/global.scss",
        "src/web/src/styles/desktop/amount-color.scss",
        "src/web/src/styles/mobile/amount-color.scss",
        "src/web/src/core/color.ts",
        "src/web/src/index.html",
        "src/web/src/components/common/PinCodeInput.vue",
        "src/web/src/styles/desktop/template/vuetify/components/_button.scss",
        "src/web/src/styles/desktop/template/vuetify/components/_field.scss",
        "src/web/src/styles/desktop/template/vuetify/components/_table.scss",
        "src/web/src/styles/desktop/template/vuetify/components/_dialog.scss",
        "src/web/src/views/desktop/MainLayout.vue",
        "src/web/src/components/desktop/ConfirmDialog.vue",
    )
    _write_text(
        repo_root / ".agents" / "skills" / "bill-analyser-ui-style-reference" / "SKILL.md",
        "## Source-of-Truth Map\n\n"
        + "\n".join(f"- `{path}`" for path in source_paths),
    )
    for relative_path in source_paths:
        if relative_path in missing_source_paths:
            continue
        _write_text(repo_root / Path(relative_path), "")
    _write_text(repo_root / "docs" / "AI_WORKFLOW.md", ai_workflow_text)


def _scan_repo_check(repo_root: Path, check_id: str):
    agent_stack_health = importlib.import_module("scripts.agent_stack_health")
    checks = {check.id: check for check in agent_stack_health.scan_repo(repo_root)}
    return checks[check_id]


def test_repo_scan_reports_expected_contracts() -> None:
    result = _run_agent_stack_health("--mode", "repo", "--format", "json")

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    checks = _checks_by_id(payload)

    assert checks["repo.entrypoints"]["status"] == "pass"
    assert checks["repo.cursor-removed"]["status"] == "pass"
    assert checks["repo.hooks-baseline"]["status"] == "pass"
    assert checks["repo.diff-commit-skill"]["status"] == "pass"
    assert checks["repo.pr-title-format-gate"]["status"] == "pass"
    assert checks["repo.pr-body-template-gate"]["status"] == "pass"
    assert checks["repo.session-resume-skill"]["status"] == "pass"
    assert checks["repo.workflow-entrypoints"]["status"] == "pass"
    assert checks["repo.ui-style-skill"]["status"] == "pass"
    assert checks["repo.task-state-support"]["status"] == "pass"
    assert checks["repo.codex-baseline"]["status"] == "pass"


def test_global_scan_warns_when_claude_hook_settings_are_missing(tmp_path: Path) -> None:
    fake_home = tmp_path / "home"
    (fake_home / ".claude").mkdir(parents=True)
    (fake_home / ".codex").mkdir(parents=True)
    (fake_home / ".codex" / "config.toml").write_text(
        'model = "gpt-5.4"\n\n[mcp_servers.playwright]\ncommand = "npx"\n',
        encoding="utf-8",
    )

    result = _run_agent_stack_health("--mode", "global", "--format", "json", "--home", str(fake_home))

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    checks = _checks_by_id(payload)

    assert checks["global.claude.settings"]["status"] == "info"
    assert checks["global.codex.config"]["status"] == "pass"
    assert checks["global.copilot.hooks"]["status"] == "info"


def test_global_scan_detects_complete_copilot_hooks(tmp_path: Path) -> None:
    fake_home = tmp_path / "home"
    copilot_hooks = fake_home / ".copilot" / "hooks"
    for relative in ("lib", "pre-tool", "post-tool", "stop"):
        (copilot_hooks / relative).mkdir(parents=True, exist_ok=True)
    (copilot_hooks / "run-with-flags.js").write_text("#!/usr/bin/env node\n", encoding="utf-8")
    (fake_home / ".codex").mkdir(parents=True)
    (fake_home / ".codex" / "config.toml").write_text(
        'model = "gpt-5.4"\n\n[mcp_servers.playwright]\ncommand = "npx"\n',
        encoding="utf-8",
    )

    result = _run_agent_stack_health("--mode", "global", "--format", "json", "--home", str(fake_home))

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    checks = _checks_by_id(payload)

    assert checks["global.copilot.hooks"]["status"] == "pass"


def test_probe_catalog_exposes_manual_behavior_checks() -> None:
    result = _run_agent_stack_health("--mode", "probes", "--format", "json")

    assert result.returncode == 0, result.stderr or result.stdout
    payload = json.loads(result.stdout)
    probe_ids = {probe["id"] for probe in payload["manualProbes"]}

    assert {
        "reviewer-scope-refusal",
        "money-unit-convention",
        "repo-guard-banned-command",
        "repo-guard-runtime-wipe",
        "repo-guard-outside-path-allowed",
        "copilot-global-hook-bridge",
        "session-resume-recovery",
        "handoff-task-state-refresh",
    } <= probe_ids


def test_doctor_format_highlights_quick_actions() -> None:
    result = _run_agent_stack_health("--mode", "repo", "--format", "doctor")

    assert result.returncode == 0, result.stderr or result.stdout
    assert "AI 定制层 Doctor" in result.stdout
    assert "快速命令" in result.stdout
    assert (
        "repo.pr-title-format-gate" in result.stdout
        or "repo.workflow-entrypoints" in result.stdout
        or "repo.task-state-support" in result.stdout
    )


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
    agents_text = (REPO_ROOT / "AGENTS.md").read_text(encoding="utf-8")
    instructions_text = (REPO_ROOT / ".github" / "copilot-instructions.md").read_text(encoding="utf-8")
    codex_text = (REPO_ROOT / ".codex" / "AGENTS.md").read_text(encoding="utf-8")
    repo_skill_text = (
        REPO_ROOT / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md"
    ).read_text(encoding="utf-8")
    claude_skill_text = (
        REPO_ROOT / ".claude" / "skills" / "bill-analyser" / "SKILL.md"
    ).read_text(encoding="utf-8")

    entrypoint_requirements = (
        ("## Session Completion", "Session Completion", "## Session completion", "Session completion"),
        ("会话结束前", "结束会话时", "会话完成前", "ending a session", "Before ending a session"),
        ("git diff", "staged", "unstaged"),
        ("zh-conventional-commit-from-diff",),
        ("中文 Conventional Commit 标题", "中文约定式提交标题", "Chinese Conventional Commit title"),
        ("PR 标题", "PR titles", "PR title"),
        ("type(scope):", "type(scope): 主标题", "type(scope): title"),
    )
    skill_requirements = (
        ("会话结束前", "结束会话时", "ending a session", "Before ending a session"),
        ("git diff", "diff", "staged", "unstaged"),
        ("zh-conventional-commit-from-diff",),
        ("中文 Conventional Commit 标题", "中文约定式提交标题", "Chinese Conventional Commit title"),
        ("PR 标题", "PR titles", "PR title"),
        ("type(scope):", "type(scope): 主标题", "type(scope): title"),
    )

    for contract_text in (agents_text, instructions_text, codex_text):
        _assert_session_completion_contract(contract_text, entrypoint_requirements)

    for contract_text in (repo_skill_text, claude_skill_text):
        _assert_session_completion_contract(contract_text, skill_requirements)


def test_pr_title_format_gate_requires_conventional_commit_heading() -> None:
    agent_stack_health = importlib.import_module("scripts.agent_stack_health")

    for title in (
        "feat(rust): 接管账号恢复路由",
        "fix(auth): 修复邮箱验证令牌绑定",
        "chore(agent-stack): 约束 PR 标题格式",
    ):
        assert agent_stack_health.is_conventional_pr_title(title)

    for title in (
        "接管账号恢复路由",
        "feat: 接管账号恢复路由",
        "Feat(rust): 接管账号恢复路由",
        "feat(rust): ",
    ):
        assert not agent_stack_health.is_conventional_pr_title(title)


def test_workspace_guides_require_session_resume_recovery_path() -> None:
    tracked_texts = {
        "AGENTS.md": (REPO_ROOT / "AGENTS.md").read_text(encoding="utf-8"),
        ".github/copilot-instructions.md": (REPO_ROOT / ".github" / "copilot-instructions.md").read_text(encoding="utf-8"),
        "CLAUDE.md": (REPO_ROOT / "CLAUDE.md").read_text(encoding="utf-8"),
        ".codex/AGENTS.md": (REPO_ROOT / ".codex" / "AGENTS.md").read_text(encoding="utf-8"),
        ".agents/skills/bill-analyser-conventions/SKILL.md": (
            REPO_ROOT / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md"
        ).read_text(encoding="utf-8"),
        ".claude/skills/bill-analyser/SKILL.md": (
            REPO_ROOT / ".claude" / "skills" / "bill-analyser" / "SKILL.md"
        ).read_text(encoding="utf-8"),
        ".agents/skills/session-resume/SKILL.md": (
            REPO_ROOT / ".agents" / "skills" / "session-resume" / "SKILL.md"
        ).read_text(encoding="utf-8"),
    }

    for relative_path, text in tracked_texts.items():
        assert "session-resume" in text, f"{relative_path} must reference session-resume"
        assert ".git/ai/last-session.md" in text, f"{relative_path} must reference .git/ai/last-session.md"


def test_agent_stack_workflow_uses_runner_compatible_python() -> None:
    workflow_text = (REPO_ROOT / ".github" / "workflows" / "agent-stack-health.yml").read_text(encoding="utf-8")
    match = re.search(r'python-version:\s*["\']?(?P<version>[^"\'\s]+)["\']?', workflow_text)

    assert match is not None, "agent-stack-health workflow must declare setup-python version"
    assert match.group("version") == "3.14", (
        "agent-stack-health workflow should stay on Python 3.14 so repo-level AI customization checks match the current runtime baseline"
    )
    assert "allow-prereleases: true" in workflow_text, (
        "setup-python step must set allow-prereleases: true so 3.14 installs on runners that still list it as pre-release"
    )


def test_parser_standard_flow_skill_and_doc_define_repo_specific_parser_contract() -> None:
    skill_path = REPO_ROOT / ".agents" / "skills" / "add-parser-standard-flow" / "SKILL.md"
    doc_path = REPO_ROOT / "docs" / "parsers" / "add-parser-standard-flow.md"

    assert skill_path.exists(), "missing shared parser standard-flow skill"
    assert doc_path.exists(), "missing parser standard-flow documentation"

    skill_text = skill_path.read_text(encoding="utf-8")
    doc_text = doc_path.read_text(encoding="utf-8")

    for expected in (
        "crates/bill-analyser-parsers/src/lib.rs",
        "crates/bill-analyser-parsers/tests/parser_contracts.rs",
        "crates/bill-analyser-http/tests/import_runtime_contract.rs",
        "parse_dedicated_import_bytes",
        "src/bill_analyser/parsers/factory.py",
        "src/bill_analyser/parsers/base.py",
        "tests/test_parser_base_factory.py",
        "tests/new_ui/test_import_parser_alignment.py",
        "ParserFactory",
        "StandardBill",
    ):
        assert expected in skill_text, f"skill must reference {expected}"

    for expected in (
        ".agents/skills/add-parser-standard-flow/SKILL.md",
        "crates/bill-analyser-parsers/src/lib.rs",
        "crates/bill-analyser-parsers/tests/parser_contracts.rs",
        "crates/bill-analyser-http/tests/import_runtime_contract.rs",
        "src/bill_analyser/parsers/factory.py",
        "src/bill_analyser/parsers/base.py",
        "tests/test_parser_base_factory.py",
        "tests/new_ui/test_import_parser_alignment.py",
    ):
        assert expected in doc_text, f"doc must reference {expected}"


def test_repo_parser_standard_flow_check_is_exposed_in_payload_text_and_doctor() -> None:
    json_result = _run_agent_stack_health("--mode", "repo", "--format", "json")
    text_result = _run_agent_stack_health("--mode", "repo", "--format", "text")
    doctor_result = _run_agent_stack_health("--mode", "repo", "--format", "doctor")

    assert json_result.returncode == 0, json_result.stderr or json_result.stdout
    assert text_result.returncode == 0, text_result.stderr or text_result.stdout
    assert doctor_result.returncode == 0, doctor_result.stderr or doctor_result.stdout

    payload = json.loads(json_result.stdout)
    checks = _checks_by_id(payload)
    parser_check = checks["repo.parser-standard-flow"]

    assert parser_check["status"] == "pass"
    assert "parser 标准流程" in parser_check["summary"]
    assert ".agents/skills/add-parser-standard-flow/SKILL.md" in parser_check["evidence"]
    assert "docs/parsers/add-parser-standard-flow.md" in parser_check["evidence"]
    assert "docs/AI_WORKFLOW.md" in parser_check["evidence"]
    assert any(
        "Rust parser-first" in item and "ParserFactory" in item and "StandardBill" in item
        for item in parser_check["evidence"]
    )

    assert "repo.parser-standard-flow" in text_result.stdout
    assert "parser 标准流程" in text_result.stdout
    assert "repo.parser-standard-flow" in doctor_result.stdout
    assert "parser 标准流程" in doctor_result.stdout


def test_repo_parser_standard_flow_check_fails_when_assets_are_missing(tmp_path: Path) -> None:
    fake_repo = tmp_path / "repo"
    _build_minimal_parser_standard_flow_fake_repo(
        fake_repo,
        ai_workflow_text=(
            "| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |\n"
            "|---|---|---|---|\n"
            "| `add-parser-standard-flow` skill | 新增 Rust parser workflow | 新增解析器、收紧 Rust parser-first / `ParserFactory` parity 检测、补 parser 对齐回归时 | 不要拿它代替通用导入调试或 API/DB 变更流程 |\n"
        ),
        include_parser_doc=False,
    )

    parser_check = _scan_repo_check(fake_repo, "repo.parser-standard-flow")

    assert parser_check.status == "fail"
    assert "资产不完整" in parser_check.summary
    assert "docs/parsers/add-parser-standard-flow.md" in parser_check.evidence


def test_repo_parser_standard_flow_check_fails_when_ai_workflow_entry_row_is_malformed(tmp_path: Path) -> None:
    fake_repo = tmp_path / "repo"
    _build_minimal_parser_standard_flow_fake_repo(
        fake_repo,
        ai_workflow_text=(
            "| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |\n"
            "|---|---|---|---|\n"
            "| `add-parser-standard-flow` skill | parser workflow | 文档里随便提一嘴 skill 名称 | 这里只是普通说明 |\n"
        ),
    )

    parser_check = _scan_repo_check(fake_repo, "repo.parser-standard-flow")

    assert parser_check.status == "fail"
    assert "契约闭环" in parser_check.summary
    assert any(item.startswith("docs/AI_WORKFLOW.md:") for item in parser_check.evidence)


def test_repo_ui_style_skill_check_is_exposed_in_payload_text_and_doctor() -> None:
    json_result = _run_agent_stack_health("--mode", "repo", "--format", "json")
    text_result = _run_agent_stack_health("--mode", "repo", "--format", "text")
    doctor_result = _run_agent_stack_health("--mode", "repo", "--format", "doctor")

    assert json_result.returncode == 0, json_result.stderr or json_result.stdout
    assert text_result.returncode == 0, text_result.stderr or text_result.stdout
    assert doctor_result.returncode == 0, doctor_result.stderr or doctor_result.stdout

    payload = json.loads(json_result.stdout)
    checks = _checks_by_id(payload)
    ui_style_check = checks["repo.ui-style-skill"]

    assert ui_style_check["status"] == "pass"
    assert "UI 风格参考" in ui_style_check["summary"]
    assert ".agents/skills/bill-analyser-ui-style-reference/SKILL.md" in ui_style_check["evidence"]
    assert "AGENTS.md" in ui_style_check["evidence"]
    assert "docs/AI_WORKFLOW.md" in ui_style_check["evidence"]
    assert any("desktop-main.ts" in item and "MobileApp.vue" in item for item in ui_style_check["evidence"])

    assert "repo.ui-style-skill" in text_result.stdout
    assert "UI 风格参考" in text_result.stdout
    assert "UI style workflow 基线" in doctor_result.stdout
    assert "repo.ui-style-skill" in doctor_result.stdout
    assert "UI 风格参考" in doctor_result.stdout


def test_repo_ui_style_skill_check_fails_when_ai_workflow_entry_row_is_malformed(tmp_path: Path) -> None:
    fake_repo = tmp_path / "repo"
    _build_minimal_ui_style_reference_fake_repo(
        fake_repo,
        ai_workflow_text=(
            "| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |\n"
            "|---|---|---|---|\n"
            "| `bill-analyser-ui-style-reference` skill | UI skill | 提一嘴样式 | 普通说明 |\n"
        ),
    )

    ui_style_check = _scan_repo_check(fake_repo, "repo.ui-style-skill")

    assert ui_style_check.status == "fail"
    assert "入口闭环" in ui_style_check.summary
    assert any(item.startswith("docs/AI_WORKFLOW.md:") for item in ui_style_check.evidence)


def test_repo_ui_style_skill_check_fails_when_agents_default_workflow_entry_is_missing(tmp_path: Path) -> None:
    fake_repo = tmp_path / "repo"
    _build_minimal_ui_style_reference_fake_repo(
        fake_repo,
        ai_workflow_text=(
            "| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |\n"
            "|---|---|---|---|\n"
            "| `bill-analyser-ui-style-reference` skill | 仓库专属 UI 风格参考 | 改页面布局、按钮、颜色、表格、弹窗、响应式一致性时 | 不替代 Vuetify / Framework7 官方文档 |\n"
        ),
        include_agents_reference=False,
    )

    ui_style_check = _scan_repo_check(fake_repo, "repo.ui-style-skill")

    assert ui_style_check.status == "fail"
    assert "入口闭环" in ui_style_check.summary
    assert any(item.startswith("AGENTS.md:") for item in ui_style_check.evidence)


def test_repo_ui_style_skill_check_fails_when_agents_workflow_entry_loses_skill_path(tmp_path: Path) -> None:
    fake_repo = tmp_path / "repo"
    _build_minimal_ui_style_reference_fake_repo(
        fake_repo,
        ai_workflow_text=(
            "| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |\n"
            "|---|---|---|---|\n"
            "| `bill-analyser-ui-style-reference` skill | 仓库专属 UI 风格参考 | 改页面布局、按钮、颜色、表格、弹窗、响应式一致性时 | 不替代 Vuetify / Framework7 官方文档 |\n"
        ),
    )
    _write_text(
        fake_repo / "AGENTS.md",
        "\n".join(
            (
                "## Default AI workflow",
                "- 如果任务主要在做页面布局、按钮样式、颜色、弹窗、表格或整体视觉一致性，再补读。",
                "",
                "## Reference docs",
                "- `.agents/skills/bill-analyser-ui-style-reference/SKILL.md`",
            )
        ),
    )

    ui_style_check = _scan_repo_check(fake_repo, "repo.ui-style-skill")

    assert ui_style_check.status == "fail"
    assert "入口闭环" in ui_style_check.summary
    assert any("Default AI workflow UI style entry missing" in item for item in ui_style_check.evidence)


def test_repo_ui_style_skill_check_fails_when_source_of_truth_path_is_missing(tmp_path: Path) -> None:
    fake_repo = tmp_path / "repo"
    _build_minimal_ui_style_reference_fake_repo(
        fake_repo,
        ai_workflow_text=(
            "| 入口 | 默认用途 | 什么时候用 | 什么时候别用 |\n"
            "|---|---|---|---|\n"
            "| `bill-analyser-ui-style-reference` skill | 仓库专属 UI 风格参考 | 改页面布局、按钮、颜色、表格、弹窗、响应式一致性时 | 不替代 Vuetify / Framework7 官方文档 |\n"
        ),
        missing_source_paths=("src/web/src/MobileApp.vue",),
    )

    ui_style_check = _scan_repo_check(fake_repo, "repo.ui-style-skill")

    assert ui_style_check.status == "fail"
    assert "入口闭环" in ui_style_check.summary
    assert any("missing_source_paths" in item for item in ui_style_check.evidence)
