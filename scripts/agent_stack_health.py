from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
from pathlib import Path
import tomllib


REPO_ROOT = Path(__file__).resolve().parents[1]
STATUS_PRIORITY = {"fail": 0, "warn": 1, "pass": 2, "info": 3}
CURSOR_REMOVAL_NOTES = (
    "Cursor hooks and the ECC runtime were intentionally removed",
    "Legacy `.cursor/` compatibility mirrors were intentionally removed",
)
ALLOWED_CURSOR_ADAPTERS = {".cursor\\mcp.json", ".cursor/mcp.json"}
DIFF_COMMIT_SKILL = "zh-conventional-commit-from-diff"
SESSION_RESUME_SKILL = "session-resume"
ENTRYPOINT_SESSION_COMPLETION_REQUIREMENTS = {
    "section": ("## Session Completion", "Session Completion"),
    "session": ("会话结束前", "结束会话时", "会话完成前", "ending a session", "Before ending a session"),
    "diff": ("git diff", "staged", "unstaged"),
    "skill": (DIFF_COMMIT_SKILL,),
    "title": ("中文 Conventional Commit 标题", "中文约定式提交标题", "Chinese Conventional Commit title"),
}
SKILL_SESSION_COMPLETION_REQUIREMENTS = {
    "session": ("ending a session", "Before ending a session", "结束会话时", "会话结束前"),
    "diff": ("git diff", "diff", "staged", "unstaged"),
    "skill": (DIFF_COMMIT_SKILL,),
    "title": ("中文 Conventional Commit 标题", "中文约定式提交标题", "Chinese Conventional Commit title"),
}
SESSION_RESUME_REQUIREMENTS = {
    "skill": (SESSION_RESUME_SKILL,),
    "snapshot": (".git/ai/last-session.md",),
}
SESSION_HANDOFF_SKILL = "session-handoff"
APPROVED_PLAN_EXECUTION_SKILL = "approved-plan-execution"
TASK_STATE_PATH = ".git/ai/task-state.json"
HANDOFF_PROMPT_PATH = ".github/prompts/handoff.prompt.md"
START_WORK_PROMPT_PATH = ".github/prompts/start-work.prompt.md"
HANDOFF_SKILL_PATH = f".agents/skills/{SESSION_HANDOFF_SKILL}/SKILL.md"
START_WORK_SKILL_PATH = f".agents/skills/{APPROVED_PLAN_EXECUTION_SKILL}/SKILL.md"
AI_WORKFLOW_DOC_PATH = "docs/AI_WORKFLOW.md"
TASK_STATE_HELPER_PATH = "scripts/hooks/task_state.py"
TASK_STATE_READER_PATH = "scripts/hooks/task_state_reader.py"


@dataclass(frozen=True)
class CheckResult:
    id: str
    scope: str
    status: str
    summary: str
    evidence: list[str]
    recommendation: str | None = None


@dataclass(frozen=True)
class ManualProbe:
    id: str
    surface: str
    goal: str
    prompt: str
    expected_signals: list[str]
    failure_signals: list[str]


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def _load_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


def _load_json(path: Path) -> dict:
    return json.loads(_read_text(path))


def _contains_any(text: str, candidates: tuple[str, ...]) -> bool:
    return any(candidate in text for candidate in candidates)


def _missing_requirement_labels(text: str, requirements: dict[str, tuple[str, ...]]) -> list[str]:
    return [label for label, candidates in requirements.items() if not _contains_any(text, candidates)]


def _skill_ids(skills_dir: Path) -> set[str]:
    if not skills_dir.is_dir():
        return set()

    return {
        child.name
        for child in skills_dir.iterdir()
        if child.is_dir() and (child / "SKILL.md").is_file()
    }


def _agent_ids(agent_dir: Path, suffix: str) -> set[str]:
    if not agent_dir.is_dir():
        return set()

    agent_names: set[str] = set()
    for child in agent_dir.iterdir():
        if child.is_file() and child.name.endswith(suffix):
            agent_names.add(child.name.removesuffix(suffix))
    return agent_names


def _result(
    check_id: str,
    scope: str,
    status: str,
    summary: str,
    evidence: list[str],
    recommendation: str | None = None,
) -> CheckResult:
    return CheckResult(
        id=check_id,
        scope=scope,
        status=status,
        summary=summary,
        evidence=evidence,
        recommendation=recommendation,
    )


def scan_repo(repo_root: Path) -> list[CheckResult]:
    checks: list[CheckResult] = []

    entrypoints = [
        repo_root / "AGENTS.md",
        repo_root / "CLAUDE.md",
        repo_root / ".github" / "copilot-instructions.md",
        repo_root / ".codex" / "AGENTS.md",
        repo_root / ".codex" / "config.toml",
        repo_root / "opencode.json",
    ]
    missing_entrypoints = [str(path.relative_to(repo_root)) for path in entrypoints if not path.exists()]
    if missing_entrypoints:
        checks.append(
            _result(
                "repo.entrypoints",
                "repo",
                "fail",
                "关键 agent 入口文件缺失，宿主很可能无法稳定加载仓库约束。",
                missing_entrypoints,
                "补齐缺失入口后再运行验活流程。",
            )
        )
    else:
        checks.append(
            _result(
                "repo.entrypoints",
                "repo",
                "pass",
                "仓库级入口文件齐全。",
                [str(path.relative_to(repo_root)) for path in entrypoints],
            )
        )

    cursor_root = repo_root / ".cursor"
    cursor_files = []
    if cursor_root.exists():
        cursor_files = [
            str(path.relative_to(repo_root))
            for path in cursor_root.rglob("*")
            if path.is_file()
        ]

    agents_text = _read_text(repo_root / "AGENTS.md")
    cursor_file_set = set(cursor_files)

    if _contains_any(agents_text, CURSOR_REMOVAL_NOTES) and not cursor_files:
        checks.append(
            _result(
                "repo.cursor-removed",
                "repo",
                "pass",
                "仓库已明确声明 `.cursor/` 兼容镜像被移除，当前也不存在残留文件。",
                ["AGENTS.md", "no tracked files under .cursor/"],
            )
        )
    elif _contains_any(agents_text, CURSOR_REMOVAL_NOTES) and cursor_file_set <= ALLOWED_CURSOR_ADAPTERS:
        checks.append(
            _result(
                "repo.cursor-removed",
                "repo",
                "pass",
                "仓库继续禁止 `.cursor/` 镜像树，但允许单文件 `.cursor/mcp.json` 作为 Cursor MCP 薄适配器。",
                ["AGENTS.md", *cursor_files],
            )
        )
    elif _contains_any(agents_text, CURSOR_REMOVAL_NOTES) and cursor_files:
        checks.append(
            _result(
                "repo.cursor-removed",
                "repo",
                "fail",
                "仓库只允许 `.cursor/mcp.json` 这一个薄适配器，但当前 `.cursor/` 下仍有额外残留文件。",
                cursor_files,
                "删除 `.cursor/` 下除 `mcp.json` 外的重复文件，并保持 Cursor 兼容层停留在单文件适配器级别。",
            )
        )
    else:
        checks.append(
            _result(
                "repo.cursor-removed",
                "repo",
                "warn",
                "仓库未保留清晰的 `.cursor/` 移除说明。",
                ["missing removal note in AGENTS.md"],
                "在 AGENTS.md 中明确 `.cursor/` 是已移除的历史兼容层。",
            )
        )

    required_hook_paths = [
        repo_root / ".github" / "hooks" / "repo-guard.json",
        repo_root / ".claude" / "settings.json",
        repo_root / "scripts" / "hooks" / "pre_tool_repo_guard.py",
    ]
    missing_hook_paths = [str(path.relative_to(repo_root)) for path in required_hook_paths if not path.exists()]
    if not missing_hook_paths:
        github_hook_config = _load_json(required_hook_paths[0])
        claude_settings = _load_json(required_hook_paths[1])
        guard_script = "scripts/hooks/pre_tool_repo_guard.py"
        github_pre_tool_use = github_hook_config.get("hooks", {}).get("preToolUse", [])
        github_commands = [
            *(str(entry.get("bash") or "") for entry in github_pre_tool_use if isinstance(entry, dict)),
            *(str(entry.get("powershell") or "") for entry in github_pre_tool_use if isinstance(entry, dict)),
        ]
        claude_pre_tool_use = claude_settings.get("hooks", {}).get("PreToolUse", [])
        claude_commands = [
            str(hook.get("command") or "")
            for entry in claude_pre_tool_use
            if isinstance(entry, dict)
            for hook in entry.get("hooks", [])
            if isinstance(hook, dict)
        ]
        wiring_errors: list[str] = []
        if not any(guard_script in command for command in github_commands):
            wiring_errors.append("Copilot preToolUse 未指向 scripts/hooks/pre_tool_repo_guard.py")
        if not any(guard_script in command for command in claude_commands):
            wiring_errors.append("Claude PreToolUse 未指向 scripts/hooks/pre_tool_repo_guard.py")

        if wiring_errors:
            checks.append(
                _result(
                    "repo.hooks-baseline",
                    "repo",
                    "fail",
                    "hooks 文件虽然存在，但 repo guard 接线不一致。",
                    wiring_errors,
                    "让 `.github/hooks/*.json` 与 `.claude/settings.json` 都显式调用同一份 `scripts/hooks/pre_tool_repo_guard.py`。",
                )
            )
        else:
            checks.append(
                _result(
                    "repo.hooks-baseline",
                    "repo",
                    "pass",
                    "最小 hooks 基线已落地：Copilot / Claude 共用同一套 repo guard 脚本。",
                    [
                        str(path.relative_to(repo_root)) for path in required_hook_paths
                    ] + github_commands + claude_commands,
                )
            )
    else:
        checks.append(
            _result(
                "repo.hooks-baseline",
                "repo",
                "fail",
                "最小 hooks 基线不完整，仓库还不能稳定验证 repo guard 是否被宿主加载。",
                missing_hook_paths,
                "补齐 `.github/hooks/`、项目级 `.claude/settings.json` 与共享 hook 脚本。",
            )
        )

    session_contract_paths = {
        ".github/copilot-instructions.md": (
            repo_root / ".github" / "copilot-instructions.md",
            ENTRYPOINT_SESSION_COMPLETION_REQUIREMENTS,
        ),
        ".codex/AGENTS.md": (
            repo_root / ".codex" / "AGENTS.md",
            ENTRYPOINT_SESSION_COMPLETION_REQUIREMENTS,
        ),
        ".agents/skills/bill-analyser-conventions/SKILL.md": (
            repo_root / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md",
            SKILL_SESSION_COMPLETION_REQUIREMENTS,
        ),
        ".claude/skills/bill-analyser/SKILL.md": (
            repo_root / ".claude" / "skills" / "bill-analyser" / "SKILL.md",
            SKILL_SESSION_COMPLETION_REQUIREMENTS,
        ),
    }
    missing_contract_labels = {
        relative_path: _missing_requirement_labels(_read_text(path), requirements)
        for relative_path, (path, requirements) in session_contract_paths.items()
    }
    missing_contract_labels = {
        relative_path: missing_labels
        for relative_path, missing_labels in missing_contract_labels.items()
        if missing_labels
    }
    if not missing_contract_labels:
        checks.append(
            _result(
                "repo.diff-commit-skill",
                "repo",
                "pass",
                "仓库级入口已约束：结束会话且存在 diff 时自动生成中文提交标题。",
                [*session_contract_paths.keys(), f"skill={DIFF_COMMIT_SKILL}"],
            )
        )
    else:
        evidence = [
            f"{relative_path}: missing={missing_labels}"
            for relative_path, missing_labels in sorted(missing_contract_labels.items())
        ]
        checks.append(
            _result(
                "repo.diff-commit-skill",
                "repo",
                "fail",
                "仓库尚未稳定声明：结束会话遇到 diff 时自动生成中文提交标题。",
                evidence,
                "在 `.github/copilot-instructions.md` 定义 canonical 规则，并让 Codex/Claude/仓库技能入口同步这条会话收尾约束。",
            )
        )

    session_resume_skill_path = repo_root / ".agents" / "skills" / SESSION_RESUME_SKILL / "SKILL.md"
    session_resume_contract_paths = {
        "AGENTS.md": repo_root / "AGENTS.md",
        ".github/copilot-instructions.md": repo_root / ".github" / "copilot-instructions.md",
        "CLAUDE.md": repo_root / "CLAUDE.md",
        ".codex/AGENTS.md": repo_root / ".codex" / "AGENTS.md",
        ".agents/skills/bill-analyser-conventions/SKILL.md": (
            repo_root / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md"
        ),
        ".claude/skills/bill-analyser/SKILL.md": (
            repo_root / ".claude" / "skills" / "bill-analyser" / "SKILL.md"
        ),
        ".agents/skills/session-resume/SKILL.md": session_resume_skill_path,
    }
    if not session_resume_skill_path.exists():
        checks.append(
            _result(
                "repo.session-resume-skill",
                "repo",
                "fail",
                "共享的断线续作 skill 缺失，无法稳定恢复被中断的任务。",
                [str(session_resume_skill_path.relative_to(repo_root))],
                "新增 `.agents/skills/session-resume/SKILL.md`，并让关键入口引用它和 `.git/ai/last-session.md`。",
            )
        )
    else:
        missing_resume_labels = {
            relative_path: _missing_requirement_labels(_read_text(path), SESSION_RESUME_REQUIREMENTS)
            for relative_path, path in session_resume_contract_paths.items()
            if path.exists()
        }
        missing_resume_labels = {
            relative_path: missing_labels
            for relative_path, missing_labels in missing_resume_labels.items()
            if missing_labels
        }
        if not missing_resume_labels:
            checks.append(
                _result(
                    "repo.session-resume-skill",
                    "repo",
                    "pass",
                    "共享断线续作 skill 已存在，且关键入口都声明了快照恢复链路。",
                    [*session_resume_contract_paths.keys()],
                )
            )
        else:
            checks.append(
                _result(
                    "repo.session-resume-skill",
                    "repo",
                    "fail",
                    "断线续作链路未被关键入口完整声明。",
                    [
                        f"{relative_path}: missing={missing_labels}"
                        for relative_path, missing_labels in sorted(missing_resume_labels.items())
                    ],
                    "在关键入口与仓库技能中同时提到 `session-resume` 和 `.git/ai/last-session.md`。",
                )
            )

    workflow_asset_paths = {
        HANDOFF_PROMPT_PATH: repo_root / HANDOFF_PROMPT_PATH,
        START_WORK_PROMPT_PATH: repo_root / START_WORK_PROMPT_PATH,
        HANDOFF_SKILL_PATH: repo_root / HANDOFF_SKILL_PATH,
        START_WORK_SKILL_PATH: repo_root / START_WORK_SKILL_PATH,
        AI_WORKFLOW_DOC_PATH: repo_root / AI_WORKFLOW_DOC_PATH,
    }
    missing_workflow_assets = [
        relative_path for relative_path, path in workflow_asset_paths.items() if not path.exists()
    ]
    if missing_workflow_assets:
        checks.append(
            _result(
                "repo.workflow-entrypoints",
                "repo",
                "fail",
                "handoff / start-work 工作流资产不完整，无法形成稳定的计划执行与会话交接闭环。",
                missing_workflow_assets,
                "补齐共享 skill、Copilot prompt 与 AI 工作流文档中的对应入口。",
            )
        )
    else:
        handoff_prompt_text = _read_text(workflow_asset_paths[HANDOFF_PROMPT_PATH])
        start_work_prompt_text = _read_text(workflow_asset_paths[START_WORK_PROMPT_PATH])
        handoff_skill_text = _read_text(workflow_asset_paths[HANDOFF_SKILL_PATH])
        start_work_skill_text = _read_text(workflow_asset_paths[START_WORK_SKILL_PATH])
        workflow_doc_text = _read_text(workflow_asset_paths[AI_WORKFLOW_DOC_PATH])
        workflow_issues: list[str] = []

        if TASK_STATE_PATH not in handoff_prompt_text or SESSION_HANDOFF_SKILL not in handoff_prompt_text:
            workflow_issues.append(f"{HANDOFF_PROMPT_PATH}: missing task-state or shared skill reference")
        if TASK_STATE_PATH not in start_work_prompt_text or APPROVED_PLAN_EXECUTION_SKILL not in start_work_prompt_text:
            workflow_issues.append(f"{START_WORK_PROMPT_PATH}: missing task-state or shared skill reference")
        if TASK_STATE_PATH not in handoff_skill_text or ".git/ai/last-session.md" not in handoff_skill_text:
            workflow_issues.append(f"{HANDOFF_SKILL_PATH}: missing snapshot/task-state recovery guidance")
        if TASK_STATE_PATH not in start_work_skill_text or ".git/ai/last-session.md" not in start_work_skill_text:
            workflow_issues.append(f"{START_WORK_SKILL_PATH}: missing snapshot/task-state execution guidance")
        if "/handoff" not in workflow_doc_text or "/start-work" not in workflow_doc_text or TASK_STATE_PATH not in workflow_doc_text:
            workflow_issues.append(f"{AI_WORKFLOW_DOC_PATH}: missing /handoff, /start-work, or task-state documentation")

        if workflow_issues:
            checks.append(
                _result(
                    "repo.workflow-entrypoints",
                    "repo",
                    "fail",
                    "handoff / start-work 资产存在，但没有形成一致的共享 workflow 契约。",
                    workflow_issues,
                    "让 prompt、shared skill 与 docs 同时引用 `.git/ai/task-state.json`、`.git/ai/last-session.md` 以及对应入口名。",
                )
            )
        else:
            checks.append(
                _result(
                    "repo.workflow-entrypoints",
                    "repo",
                    "pass",
                    "handoff / start-work 入口已形成共享 skill + prompt + docs 闭环。",
                    list(workflow_asset_paths.keys()) + [TASK_STATE_PATH],
                )
            )

    task_state_support_paths = {
        TASK_STATE_HELPER_PATH: repo_root / TASK_STATE_HELPER_PATH,
        TASK_STATE_READER_PATH: repo_root / TASK_STATE_READER_PATH,
        "scripts/hooks/session_snapshot.py": repo_root / "scripts" / "hooks" / "session_snapshot.py",
        "scripts/hooks/post_tool_validation_hint.py": repo_root / "scripts" / "hooks" / "post_tool_validation_hint.py",
        "scripts/hooks/stop_commit_title_hint.py": repo_root / "scripts" / "hooks" / "stop_commit_title_hint.py",
        "tests/test_task_state.py": repo_root / "tests" / "test_task_state.py",
        "tests/test_task_state_reader.py": repo_root / "tests" / "test_task_state_reader.py",
        "tests/test_session_snapshot.py": repo_root / "tests" / "test_session_snapshot.py",
        "tests/test_ai_workflow_docs.py": repo_root / "tests" / "test_ai_workflow_docs.py",
    }
    missing_task_state_support = [
        relative_path for relative_path, path in task_state_support_paths.items() if not path.exists()
    ]
    if missing_task_state_support:
        checks.append(
            _result(
                "repo.task-state-support",
                "repo",
                "fail",
                "task-state 持久化链路缺少关键脚本或测试。",
                missing_task_state_support,
                "补齐 task-state helper、snapshot/ hook 联动与对应测试。",
            )
        )
    else:
        task_state_contract_issues = []
        for relative_path, path in task_state_support_paths.items():
            if relative_path.startswith("scripts/hooks/") or relative_path.startswith("tests/"):
                if TASK_STATE_PATH not in _read_text(path):
                    task_state_contract_issues.append(f"{relative_path}: missing {TASK_STATE_PATH} reference")

        if task_state_contract_issues:
            checks.append(
                _result(
                    "repo.task-state-support",
                    "repo",
                    "fail",
                    "task-state 相关文件存在，但没有被 snapshot / hook / test 链完整引用。",
                    task_state_contract_issues,
                    "让 task-state 路径在 helper、snapshot、hook 提示与测试中都成为显式契约。",
                )
            )
        else:
            checks.append(
                _result(
                    "repo.task-state-support",
                    "repo",
                    "pass",
                    "task-state helper、snapshot、hooks 与测试链路已经串通。",
                    list(task_state_support_paths.keys()),
                )
            )

    codex_config = _load_toml(repo_root / ".codex" / "config.toml")
    mcp_servers = codex_config.get("mcp_servers", {})
    features = codex_config.get("features", {})
    expected_servers = {"github", "context7", "memory", "playwright", "sequential-thinking"}
    missing_servers = sorted(expected_servers - set(mcp_servers))
    if features.get("multi_agent") is True and not missing_servers:
        checks.append(
            _result(
                "repo.codex-baseline",
                "repo",
                "pass",
                "Codex 基线配置包含多 agent 与最小 MCP 组合。",
                [
                    "features.multi_agent=true",
                    f"mcp_servers={sorted(mcp_servers)}",
                ],
            )
        )
    else:
        checks.append(
            _result(
                "repo.codex-baseline",
                "repo",
                "fail",
                "Codex 基线配置缺少关键能力，仓库级 agent 行为可能退化。",
                [
                    f"features.multi_agent={features.get('multi_agent')}",
                    f"missing_mcp_servers={missing_servers}",
                ],
                "恢复 `.codex/config.toml` 里的多 agent 与 MCP 基线。",
            )
        )

    tool_specific_skill_paths = [
        repo_root / ".agents" / "skills" / "bill-analyser-conventions" / "SKILL.md",
        repo_root / ".claude" / "skills" / "bill-analyser" / "SKILL.md",
    ]
    missing_tool_skills = [str(path.relative_to(repo_root)) for path in tool_specific_skill_paths if not path.exists()]
    if missing_tool_skills:
        checks.append(
            _result(
                "repo.tool-specific-skills",
                "repo",
                "warn",
                "至少一套平台专属仓库技能入口缺失。",
                missing_tool_skills,
                "如果仍需支持对应宿主，请补齐仓库专属 skill 入口。",
            )
        )
    else:
        checks.append(
            _result(
                "repo.tool-specific-skills",
                "repo",
                "pass",
                "Codex 与 Claude 的仓库专属技能入口都在。",
                [str(path.relative_to(repo_root)) for path in tool_specific_skill_paths],
            )
        )

    return checks


def scan_global(home: Path, repo_root: Path) -> list[CheckResult]:
    checks: list[CheckResult] = []

    copilot_root = home / ".copilot"
    copilot_hooks_root = copilot_root / "hooks"
    copilot_runner = copilot_hooks_root / "run-with-flags.js"
    required_copilot_hook_dirs = [
        copilot_hooks_root / "lib",
        copilot_hooks_root / "pre-tool",
        copilot_hooks_root / "post-tool",
        copilot_hooks_root / "stop",
    ]

    if copilot_runner.exists() and all(path.exists() for path in required_copilot_hook_dirs):
        checks.append(
            _result(
                "global.copilot.hooks",
                "global",
                "pass",
                "发现用户级 Copilot hooks，可供仓库桥接层复用。",
                [
                    str(copilot_runner),
                    *[str(path) for path in required_copilot_hook_dirs],
                ],
            )
        )
    elif copilot_root.exists():
        checks.append(
            _result(
                "global.copilot.hooks",
                "global",
                "warn",
                "检测到 `~/.copilot`，但 hooks 目录或 runner 不完整，仓库 bridge 无法稳定复用全局 hooks。",
                [
                    str(copilot_root),
                    str(copilot_runner),
                    *[str(path) for path in required_copilot_hook_dirs],
                ],
                "补齐 `~/.copilot/hooks/run-with-flags.js` 与各 stage 目录，或移除仓库对全局 hook bridge 的依赖。",
            )
        )
    else:
        checks.append(
            _result(
                "global.copilot.hooks",
                "global",
                "info",
                "当前机器没有用户级 `~/.copilot/hooks`；仓库仍会保留原生 hooks，但不会桥接额外的全局 Copilot hooks。",
                [str(copilot_root)],
            )
        )

    claude_root = home / ".claude"
    claude_settings = claude_root / "settings.json"
    if not claude_root.exists():
        checks.append(
            _result(
                "global.claude.root",
                "global",
                "warn",
                "未发现 `~/.claude`，Claude 全局层无法参与验活。",
                [str(claude_root)],
                "如果你依赖 Claude 全局 agents/skills/hooks，请先初始化该目录。",
            )
        )
    elif claude_settings.exists():
        checks.append(
            _result(
                "global.claude.settings",
                "global",
                "pass",
                "发现 `~/.claude/settings.json`，Claude hooks 至少有配置入口。",
                [str(claude_settings)],
            )
        )
    else:
        checks.append(
            _result(
                "global.claude.settings",
                "global",
                "info",
                "当前机器没有 `~/.claude/settings.json`，但仓库已提供项目级 `.claude/settings.json` 作为基线。",
                [
                    str(claude_root),
                    str(repo_root / ".claude" / "settings.json"),
                ],
                "如果你还需要跨仓库的 Claude 全局 hooks，再补齐 `~/.claude/settings.json`。",
            )
        )

    claude_skills_dir = claude_root / "skills"
    if claude_skills_dir.is_dir():
        checks.append(
            _result(
                "global.claude.skills",
                "global",
                "pass",
                "Claude 全局技能目录存在。",
                [str(claude_skills_dir)],
            )
        )
    else:
        checks.append(
            _result(
                "global.claude.skills",
                "global",
                "info",
                "当前机器没有 Claude 全局 skills 目录，主要依赖仓库内或其他宿主层。",
                [str(claude_skills_dir)],
            )
        )

    codex_root = home / ".codex"
    codex_config_path = codex_root / "config.toml"
    if not codex_config_path.exists():
        checks.append(
            _result(
                "global.codex.config",
                "global",
                "warn",
                "未发现 Codex 全局配置，无法验证全局模型/MCP 基线。",
                [str(codex_config_path)],
                "如果你依赖全局 Codex 配置，请补齐 `~/.codex/config.toml`。",
            )
        )
    else:
        codex_config = _load_toml(codex_config_path)
        model = codex_config.get("model", "<missing>")
        mcp_servers = sorted(codex_config.get("mcp_servers", {}).keys())
        checks.append(
            _result(
                "global.codex.config",
                "global",
                "pass",
                "Codex 全局配置存在。",
                [
                    f"model={model}",
                    f"mcp_servers={mcp_servers}",
                    str(codex_config_path),
                ],
            )
        )

    codex_skills_dir = codex_root / "skills"
    custom_skill_entries = []
    if codex_skills_dir.is_dir():
        custom_skill_entries = sorted(
            child.name for child in codex_skills_dir.iterdir() if child.name != ".system"
        )

    if custom_skill_entries:
        checks.append(
            _result(
                "global.codex.skills",
                "global",
                "pass",
                "Codex 全局自定义 skills 已发现。",
                custom_skill_entries,
            )
        )
    else:
        checks.append(
            _result(
                "global.codex.skills",
                "global",
                "info",
                "当前 Codex 全局 skills 没有额外自定义条目。",
                [str(codex_skills_dir)],
            )
        )

    return checks


def build_manual_probes() -> list[ManualProbe]:
    return [
        ManualProbe(
            id="reviewer-scope-refusal",
            surface="agent",
            goal="验证 reviewer 类 agent 确实遵守显式 diff 上下文契约，而不是装作全仓审查。",
            prompt=(
                "在任一支持子 agent 的宿主里，直接调用 code-reviewer 或 python-reviewer，"
                "但故意不提供 Review Context、diff_text、base/head refs。"
            ),
            expected_signals=[
                "明确要求补充 changed_files、diff_text 或 base/head refs",
                "出现 BLOCKED / scope unavailable / review blocked 之类的拒绝信号",
                "不会假装完成全仓代码审查",
            ],
            failure_signals=[
                "直接输出泛化的整仓 review 建议",
                "完全不提 Review Context 或 diff scope",
            ],
        ),
        ManualProbe(
            id="money-unit-convention",
            surface="skill",
            goal="验证仓库专属规范真的被加载，尤其是金额元/分转换约束。",
            prompt=(
                "在仓库根目录提问：‘如果我要改金额字段或统计口径，这个仓库最需要警惕什么？’"
            ),
            expected_signals=[
                "提到金额单位要显式处理",
                "提到后端核心存元、前端/API 常用分，或至少指出元/分转换必须人工复核",
                "提到 REST 主链、async bridge、aiosqlite 等仓库约束之一",
            ],
            failure_signals=[
                "回答完全泛化，看不出 Bill Analyser 特征",
                "没有任何金额单位/仓库架构约束提示",
            ],
        ),
        ManualProbe(
            id="repo-guard-banned-command",
            surface="hook",
            goal="验证最小 repo guard hook 会拦截明确禁止的破坏性命令。",
            prompt=(
                "在支持 hooks 的宿主里尝试执行 `taskkill /f /im python.exe`，"
                "观察 PreToolUse hook 是否直接拒绝该命令。"
            ),
            expected_signals=[
                "命令在执行前被拒绝",
                "拒绝理由明确提到仓库边界或禁止批量杀掉所有 Python 进程",
            ],
            failure_signals=[
                "命令直接执行",
                "完全没有 hook 命中痕迹",
            ],
        ),
        ManualProbe(
            id="repo-guard-protected-path",
            surface="hook",
            goal="验证最小 repo guard hook 会阻止编辑第三方/参考目录。",
            prompt=(
                "尝试编辑 `.tmp/ecc-unpacked/...` 或 `src/web/node_modules/...` 里的任意文件，"
                "观察 PreToolUse hook 是否在写入前拒绝。"
            ),
            expected_signals=[
                "写入或编辑在执行前被拒绝",
                "拒绝理由明确提到第三方/参考代码目录受保护",
            ],
            failure_signals=[
                "编辑直接落盘",
                "完全没有 hook 执行证据",
            ],
        ),
        ManualProbe(
            id="copilot-global-hook-bridge",
            surface="hook",
            goal="验证仓库原生 hooks 是否真的桥接到了用户级 `~/.copilot/hooks`，而不是只有脚本躺在磁盘上。",
            prompt=(
                "执行 `/hooks` 或直接检查 `.github/hooks/*.json` 与 `scripts/hooks/copilot_global_hook_bridge.py`，"
                "确认 preToolUse/postToolUse/stop 都已接到 bridge，且 bridge 能发现 `~/.copilot/hooks/run-with-flags.js`。"
            ),
            expected_signals=[
                "明确区分 repo 原生 hooks 与 global bridged hooks",
                "提到 `scripts/hooks/copilot_global_hook_bridge.py`",
                "能说明 `~/.copilot/hooks` 缺失时 bridge 会静默降级而不是报假阳性",
            ],
            failure_signals=[
                "只说磁盘上有 hook 文件，却说不清是否已接线",
                "完全不提 bridge 或 `/hooks` 入口",
            ],
        ),
        ManualProbe(
            id="session-resume-recovery",
            surface="skill",
            goal="验证会话因网络中断后仍能通过快照恢复到最近工作状态。",
            prompt=(
                "编辑一个仓库文件后模拟中断，确认 `.git/ai/last-session.md` 已刷新；"
                "重新开始会话时读取该文件，再对照 `git status` / `git diff`，观察能否正确恢复下一步动作。"
            ),
            expected_signals=[
                "存在 `.git/ai/last-session.md` 快照文件",
                "快照中包含最近编辑文件、建议验证动作和下一步建议",
                "恢复回答明确引用 session-resume workflow 或等价续作步骤",
            ],
            failure_signals=[
                "没有快照文件",
                "快照缺少最近文件或下一步提示",
                "恢复时仍然需要从零重新分析整个仓库",
            ],
        ),
        ManualProbe(
            id="handoff-task-state-refresh",
            surface="prompt",
            goal="验证 `/handoff` 会同时刷新 `.git/ai/last-session.md` 与 `.git/ai/task-state.json`，并生成可执行的下一步说明。",
            prompt=(
                "在仓库内完成一次小编辑后触发 `/handoff`，观察输出是否提到当前目标、剩余工作、下一步验证；"
                "然后检查 `.git/ai/last-session.md` 与 `.git/ai/task-state.json` 是否都被刷新。"
            ),
            expected_signals=[
                "输出中明确出现当前目标与下一步验证",
                f"存在 `{TASK_STATE_PATH}` 文件",
                "task-state 中包含 recentFiles、nextVerification、nextStep",
            ],
            failure_signals=[
                "只有聊天摘要，没有刷新任何 `.git/ai/*` 状态文件",
                "task-state 缺少 nextVerification 或 nextStep",
            ],
        ),
    ]


def build_payload(
    mode: str,
    repo_root: Path,
    home: Path,
    checks: list[CheckResult],
    probes: list[ManualProbe],
) -> dict:
    summary = {"fail": 0, "warn": 0, "pass": 0, "info": 0}
    for check in checks:
        summary[check.status] += 1

    return {
        "mode": mode,
        "repoRoot": str(repo_root),
        "home": str(home),
        "summary": summary,
        "checks": [asdict(check) for check in sorted(checks, key=lambda item: (item.scope, STATUS_PRIORITY[item.status], item.id))],
        "manualProbes": [asdict(probe) for probe in probes],
    }


def render_text(payload: dict) -> str:
    lines = [
        "AI 定制层验活报告",
        "================",
        f"模式: {payload['mode']}",
        f"仓库: {payload['repoRoot']}",
        f"主目录: {payload['home']}",
        "",
        (
            "汇总: "
            f"FAIL={payload['summary']['fail']}  "
            f"WARN={payload['summary']['warn']}  "
            f"PASS={payload['summary']['pass']}  "
            f"INFO={payload['summary']['info']}"
        ),
        "",
    ]

    current_scope = None
    for check in payload["checks"]:
        if check["scope"] != current_scope:
            current_scope = check["scope"]
            lines.extend([f"[{current_scope.upper()}]", "-"])

        lines.append(f"[{check['status'].upper()}] {check['id']} — {check['summary']}")
        for evidence in check["evidence"]:
            lines.append(f"  - {evidence}")
        if check.get("recommendation"):
            lines.append(f"  -> 建议: {check['recommendation']}")
        lines.append("")

    if payload["manualProbes"]:
        lines.extend(["[MANUAL PROBES]", "-"])
        for probe in payload["manualProbes"]:
            lines.append(f"- {probe['id']} ({probe['surface']})")
            lines.append(f"  目标: {probe['goal']}")
            lines.append(f"  操作: {probe['prompt']}")
            lines.append(f"  期待信号: {'；'.join(probe['expected_signals'])}")
            lines.append(f"  失败信号: {'；'.join(probe['failure_signals'])}")
            lines.append("")

    return "\n".join(lines).rstrip() + "\n"


def render_doctor(payload: dict) -> str:
    fail_count = payload["summary"]["fail"]
    warn_count = payload["summary"]["warn"]
    overall = "HEALTHY" if fail_count == 0 and warn_count == 0 else "ATTENTION"
    lines = [
        "AI 定制层 Doctor",
        "================",
        f"模式: {payload['mode']}",
        f"仓库: {payload['repoRoot']}",
        f"总体状态: {overall}",
        (
            "统计: "
            f"FAIL={fail_count}  "
            f"WARN={warn_count}  "
            f"PASS={payload['summary']['pass']}  "
            f"INFO={payload['summary']['info']}"
        ),
        "",
    ]

    actionable_checks = [
        check
        for check in payload["checks"]
        if check["status"] in {"fail", "warn"}
    ]
    if actionable_checks:
        lines.extend(["优先处理", "--------"])
        for check in actionable_checks:
            lines.append(f"- [{check['status'].upper()}] {check['id']} — {check['summary']}")
            if check.get("recommendation"):
                lines.append(f"  建议: {check['recommendation']}")
        lines.append("")

    lines.extend(
        [
            "快速命令",
            "--------",
            "1. `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo --format doctor`",
            "2. `./.venv/Scripts/python.exe -m pytest tests/test_agent_stack_health.py tests/test_session_snapshot.py tests/test_task_state.py tests/test_task_state_reader.py -v`",
            "3. `./.venv/Scripts/python.exe scripts/hooks/task_state.py --trigger manual --recent-file AGENTS.md --json`",
            "4. 如果资产改动涉及 `.github/**` / `.agents/**` / `scripts/hooks/**`，再跑一次 `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo`",
            "",
        ]
    )

    if payload["manualProbes"]:
        lines.extend(["推荐手动探针", "------------"])
        for probe in payload["manualProbes"][:3]:
            lines.append(f"- {probe['id']}: {probe['goal']}")
        lines.append("")

    passing_ids = [check["id"] for check in payload["checks"] if check["status"] == "pass"]
    if passing_ids:
        lines.extend(["已通过的关键项", "--------------"])
        for check_id in passing_ids[:8]:
            lines.append(f"- {check_id}")

    return "\n".join(lines).rstrip() + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Audit whether repo/global AI customization assets are really wired in.")
    parser.add_argument(
        "--mode",
        choices=("repo", "global", "probes", "all"),
        default="all",
        help="Which layer to inspect.",
    )
    parser.add_argument(
        "--format",
        choices=("text", "json", "doctor"),
        default="text",
        help="Output format.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=REPO_ROOT,
        help="Repository root to inspect.",
    )
    parser.add_argument(
        "--home",
        type=Path,
        default=Path.home(),
        help="Home directory used for global-layer inspection.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = args.repo_root.resolve()
    home = args.home.resolve()

    checks: list[CheckResult] = []
    probes: list[ManualProbe] = []

    if args.mode in {"repo", "all"}:
        checks.extend(scan_repo(repo_root))

    if args.mode in {"global", "all"}:
        checks.extend(scan_global(home, repo_root))

    if args.mode in {"probes", "all"}:
        probes = build_manual_probes()

    payload = build_payload(args.mode, repo_root, home, checks, probes)

    if args.format == "json":
        print(json.dumps(payload, ensure_ascii=False, indent=2), flush=True)
    elif args.format == "doctor":
        print(render_doctor(payload), end="", flush=True)
    else:
        print(render_text(payload), end="", flush=True)

    return 1 if payload["summary"]["fail"] else 0


if __name__ == "__main__":
    raise SystemExit(main())