from __future__ import annotations

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def test_ai_workflow_doc_exists_and_explains_default_entrypoints() -> None:
    doc_path = REPO_ROOT / "docs" / "AI_WORKFLOW.md"
    text = doc_path.read_text(encoding="utf-8")

    assert doc_path.exists()
    assert "/verify" in text
    assert "/quality-gate" in text
    assert "/handoff" in text
    assert "/start-work" in text
    assert "verification-loop" in text
    assert ".git/ai/last-session.md" in text
    assert ".git/ai/task-state.json" in text
    assert "session-resume" in text
    assert "/hooks" in text
    assert "gitea-ci-cache-discipline" in text
    assert "Rust 后端改动" in text
    assert "cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90" in text
    assert "残留 Python sidecar" in text


def test_verification_assets_define_non_overlapping_roles() -> None:
    verify_prompt = (REPO_ROOT / ".github" / "prompts" / "verify.prompt.md").read_text(encoding="utf-8")
    quality_gate_prompt = (REPO_ROOT / ".github" / "prompts" / "quality-gate.prompt.md").read_text(encoding="utf-8")
    verification_skill = (
        REPO_ROOT / ".github" / "skills" / "verification-loop" / "SKILL.md"
    ).read_text(encoding="utf-8")

    assert "default verification entrypoint" in verify_prompt.lower()
    assert "prefer `/verify`" in quality_gate_prompt
    assert "Prefer `/verify` as the default" in verification_skill


def test_handoff_and_start_work_prompts_reference_shared_workflows() -> None:
    handoff_prompt = (REPO_ROOT / ".github" / "prompts" / "handoff.prompt.md").read_text(encoding="utf-8")
    start_work_prompt = (REPO_ROOT / ".github" / "prompts" / "start-work.prompt.md").read_text(encoding="utf-8")
    verify_prompt = (REPO_ROOT / ".github" / "prompts" / "verify.prompt.md").read_text(encoding="utf-8")

    assert "session-handoff" in handoff_prompt
    assert ".git/ai/task-state.json" in handoff_prompt
    assert "approved-plan-execution" in start_work_prompt
    assert ".git/ai/task-state.json" in start_work_prompt
    assert "nextVerification" in verify_prompt


def test_ai_workflow_doc_lists_parser_standard_flow_in_entry_table() -> None:
    doc_text = (REPO_ROOT / "docs" / "AI_WORKFLOW.md").read_text(encoding="utf-8")
    matching_rows = [
        line.strip()
        for line in doc_text.splitlines()
        if line.strip().startswith("|") and "`add-parser-standard-flow` skill" in line
    ]

    assert matching_rows, "AI_WORKFLOW must list add-parser-standard-flow in the entry table"
    row = matching_rows[0]
    assert "新增 Rust parser workflow" in row
    assert "Rust parser-first" in row
    assert "ParserFactory" in row
    assert "新增解析器" in row or "新增 parser" in row
    assert "不要拿它代替" in row
    assert "API/DB" in row or "通用导入调试" in row


def test_ai_workflow_doc_lists_ui_style_reference_skill_in_entry_table() -> None:
    doc_text = (REPO_ROOT / "docs" / "AI_WORKFLOW.md").read_text(encoding="utf-8")
    matching_rows = [
        line.strip()
        for line in doc_text.splitlines()
        if line.strip().startswith("|") and "`bill-analyser-ui-style-reference` skill" in line
    ]

    assert matching_rows, "AI_WORKFLOW must list bill-analyser-ui-style-reference in the entry table"
    row = matching_rows[0]
    assert "页面布局" in row
    assert "按钮" in row
    assert "颜色" in row
    assert "Vuetify" in row
    assert "Framework7" in row


def test_ai_workflow_doc_lists_gitea_ci_cache_discipline_skill_in_entry_table() -> None:
    doc_text = (REPO_ROOT / "docs" / "AI_WORKFLOW.md").read_text(encoding="utf-8")
    matching_rows = [
        line.strip()
        for line in doc_text.splitlines()
        if line.strip().startswith("|") and "`gitea-ci-cache-discipline` skill" in line
    ]

    assert matching_rows, "AI_WORKFLOW must list gitea-ci-cache-discipline in the entry table"
    row = matching_rows[0]
    assert "Gitea CI 缓存治理" in row
    assert ".gitea/workflows/ci.yml" in row
    assert "actions/cache" in row
    assert "act_runner" in row
    assert "远端 runner 存储清理" in row


def test_gitea_ci_cache_discipline_skill_locks_cache_contract() -> None:
    skill_path = REPO_ROOT / ".agents" / "skills" / "gitea-ci-cache-discipline" / "SKILL.md"
    text = skill_path.read_text(encoding="utf-8")

    assert skill_path.exists()
    assert "target" in text
    assert "node_modules" in text
    assert "~/.cargo/registry/src" in text
    assert "~/.cargo/git/checkouts" in text
    assert "Trim backend caches before cache save" in text
    assert "rust_toolchain_cache_save=true" in text
    assert "cargo-llvm-cov" in text
    assert "tests/test_gitea_workflows.py" in text
    assert "agent_stack_health.py --mode repo" in text
    assert "actcache" in text
    assert "runner/cache storage cleanup" in text
