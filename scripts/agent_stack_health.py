from __future__ import annotations

import argparse
from dataclasses import asdict, dataclass
import json
from pathlib import Path
import tomllib


REPO_ROOT = Path(__file__).resolve().parents[1]
STATUS_PRIORITY = {"fail": 0, "warn": 1, "pass": 2, "info": 3}
CURSOR_HOOKS_NOTE = "Cursor hooks and the ECC runtime were intentionally removed"
DIFF_COMMIT_SKILL = "zh-conventional-commit-from-diff"
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

    github_agents = _agent_ids(repo_root / ".github" / "agents", ".agent.md")
    cursor_agents = _agent_ids(repo_root / ".cursor" / "agents", ".md")
    if github_agents and github_agents == cursor_agents:
        checks.append(
            _result(
                "repo.agent-parity",
                "repo",
                "pass",
                "`.github/agents` 与 `.cursor/agents` 的 agent 名单保持同步。",
                [f"{len(github_agents)} mirrored agents"],
            )
        )
    else:
        only_github = sorted(github_agents - cursor_agents)
        only_cursor = sorted(cursor_agents - github_agents)
        checks.append(
            _result(
                "repo.agent-parity",
                "repo",
                "fail",
                "Copilot 与 Cursor 的 agent 清单发生漂移。",
                [
                    f"only_in_.github={only_github}",
                    f"only_in_.cursor={only_cursor}",
                ],
                "同步两个目录中的 agent 文件名与职责。",
            )
        )

    github_skills = _skill_ids(repo_root / ".github" / "skills")
    cursor_skills = _skill_ids(repo_root / ".cursor" / "skills")
    if github_skills and github_skills == cursor_skills:
        checks.append(
            _result(
                "repo.skill-parity",
                "repo",
                "pass",
                "`.github/skills` 与 `.cursor/skills` 的共享技能目录保持同步。",
                [f"{len(github_skills)} mirrored skills"],
            )
        )
    else:
        only_github = sorted(github_skills - cursor_skills)
        only_cursor = sorted(cursor_skills - github_skills)
        checks.append(
            _result(
                "repo.skill-parity",
                "repo",
                "fail",
                "Copilot 与 Cursor 的技能目录发生漂移。",
                [
                    f"only_in_.github={only_github}",
                    f"only_in_.cursor={only_cursor}",
                ],
                "优先同步共享技能，再决定是否保留平台专属差异。",
            )
        )

    agents_text = _read_text(repo_root / "AGENTS.md")
    cursor_hooks_runtime_paths = [
        repo_root / ".cursor" / "hooks.json",
        repo_root / ".cursor" / "hooks",
    ]
    runtime_present = [str(path.relative_to(repo_root)) for path in cursor_hooks_runtime_paths if path.exists()]
    if CURSOR_HOOKS_NOTE in agents_text and not runtime_present:
        checks.append(
            _result(
                "repo.cursor-hooks-trimmed",
                "repo",
                "pass",
                "仓库已明确声明 Cursor hooks 被裁剪，并且运行态文件确实不存在。",
                ["AGENTS.md", "no .cursor/hooks.json", "no .cursor/hooks/"],
            )
        )
    elif CURSOR_HOOKS_NOTE in agents_text and runtime_present:
        checks.append(
            _result(
                "repo.cursor-hooks-trimmed",
                "repo",
                "fail",
                "文档说 Cursor hooks 已移除，但仓库里又出现了运行态 hook 文件。",
                runtime_present,
                "要么恢复并记录完整 hooks 方案，要么移除残留文件，避免误导。",
            )
        )
    else:
        checks.append(
            _result(
                "repo.cursor-hooks-trimmed",
                "repo",
                "warn",
                "仓库未保留清晰的 Cursor hook 运行态说明。",
                ["missing trim note in AGENTS.md"],
                "在 AGENTS.md 中写明 hooks 是可运行、已移除，还是仅作文档提示。",
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
                "warn",
                "仓库 hook 规则指向 `~/.claude/settings.json`，但当前机器没有这个文件。",
                [
                    str(claude_root),
                    str(repo_root / ".cursor" / "rules" / "python-hooks.md"),
                    str(repo_root / ".cursor" / "rules" / "typescript-hooks.md"),
                ],
                "若想验证 Claude hook 真在工作，请补齐 `~/.claude/settings.json` 中的 hook 配置。",
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

    cursor_root = home / ".cursor"
    cursor_manifest = cursor_root / "skills-cursor" / ".cursor-managed-skills-manifest.json"
    if cursor_manifest.exists():
        checks.append(
            _result(
                "global.cursor.skills-manifest",
                "global",
                "pass",
                "Cursor 全局技能托管清单存在。",
                [str(cursor_manifest)],
            )
        )
    else:
        checks.append(
            _result(
                "global.cursor.skills-manifest",
                "global",
                "warn",
                "未发现 Cursor 全局技能托管清单。",
                [str(cursor_manifest)],
                "如果你依赖 Cursor 全局技能，请确认 `skills-cursor` 是否安装完成。",
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
            id="python-hook-warning",
            surface="hook",
            goal="验证 Python hook 是否真的会对编辑行为产生反馈。",
            prompt=(
                "新建一个临时 `.py` 文件，加入 `print(\"hook probe\")` 或故意的格式问题后保存，"
                "观察宿主是否自动格式化、类型检查或弹出 warning。"
            ),
            expected_signals=[
                "保存后触发格式化或类型检查",
                "对 print() 给出 warning，或输出与 Python hooks 对应的提示",
            ],
            failure_signals=[
                "保存后完全无反馈，且宿主日志/界面也没有 hook 触发痕迹",
            ],
        ),
        ManualProbe(
            id="typescript-hook-warning",
            surface="hook",
            goal="验证 JS/TS hook 是否真的会对 `console.log` 或格式问题做出响应。",
            prompt=(
                "新建一个临时 `.ts`/`.tsx` 文件，加入 `console.log('probe')` 后保存，"
                "观察宿主是否自动格式化、跑类型检查或发出 console.log warning。"
            ),
            expected_signals=[
                "保存后触发格式化或 TypeScript 检查",
                "对 console.log 给出 warning，或在会话结束前审计提示中出现它",
            ],
            failure_signals=[
                "保存后完全无反馈，且没有任何 hook 执行证据",
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
        choices=("text", "json"),
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
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(render_text(payload), end="")

    return 1 if payload["summary"]["fail"] else 0


if __name__ == "__main__":
    raise SystemExit(main())