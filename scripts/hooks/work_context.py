from __future__ import annotations

# ruff: noqa: I001

from dataclasses import dataclass
from pathlib import Path
import subprocess
from typing import TYPE_CHECKING


if TYPE_CHECKING:
    from collections.abc import Sequence

REPO_ROOT = Path(__file__).resolve().parents[2]
AI_CUSTOMIZATION_PREFIXES = (
    ".github/",
    ".agents/",
    ".claude/",
    ".codex/",
    "scripts/hooks/",
)
BACKEND_PREFIX = "src/bill_analyser/"
FRONTEND_PREFIX = "src/web/"
TEST_PREFIX = "tests/"


@dataclass(frozen=True)
class GitState:
    branch: str
    head: str
    staged_files: tuple[str, ...]
    unstaged_files: tuple[str, ...]
    untracked_files: tuple[str, ...]


def run_git(args: Sequence[str], repo_root: Path) -> subprocess.CompletedProcess[str] | None:
    try:
        return subprocess.run(
            ["git", *args],
            cwd=repo_root,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    except FileNotFoundError:
        return None


def normalize_paths(paths: Sequence[str]) -> tuple[str, ...]:
    normalized = {
        path.replace("\\", "/").strip()
        for path in paths
        if path and path.strip()
    }
    return tuple(sorted(normalized))


def split_stdout_lines(result: subprocess.CompletedProcess[str] | None) -> tuple[str, ...]:
    if result is None or result.returncode != 0:
        return ()

    return normalize_paths(result.stdout.splitlines())


def resolve_git_ai_path(relative_path: Path, repo_root: Path = REPO_ROOT) -> Path | None:
    git_path_result = run_git(["rev-parse", "--git-path", relative_path.as_posix()], repo_root)
    if git_path_result and git_path_result.returncode == 0:
        raw_path = git_path_result.stdout.strip()
        if raw_path:
            candidate = Path(raw_path)
            if candidate.is_absolute():
                return candidate.resolve()
            return (repo_root / candidate).resolve()

    git_dir = repo_root / ".git"
    if git_dir.exists() and git_dir.is_dir():
        return (git_dir / relative_path).resolve()

    return None


def format_display_path(path: Path, repo_root: Path = REPO_ROOT) -> str:
    try:
        return path.relative_to(repo_root).as_posix()
    except ValueError:
        return str(path)


def collect_git_state(repo_root: Path = REPO_ROOT) -> GitState:
    branch_result = run_git(["rev-parse", "--abbrev-ref", "HEAD"], repo_root)
    head_result = run_git(["rev-parse", "--short", "HEAD"], repo_root)
    staged_result = run_git(["diff", "--cached", "--name-only"], repo_root)
    unstaged_result = run_git(["diff", "--name-only"], repo_root)
    untracked_result = run_git(["ls-files", "--others", "--exclude-standard"], repo_root)

    branch = branch_result.stdout.strip() if branch_result and branch_result.returncode == 0 else "<unknown>"
    head = head_result.stdout.strip() if head_result and head_result.returncode == 0 else "<unknown>"

    return GitState(
        branch=branch or "<unknown>",
        head=head or "<unknown>",
        staged_files=split_stdout_lines(staged_result),
        unstaged_files=split_stdout_lines(unstaged_result),
        untracked_files=split_stdout_lines(untracked_result),
    )


def detect_scopes(paths: Sequence[str]) -> tuple[str, ...]:
    normalized_paths = normalize_paths(paths)
    scopes: list[str] = []

    if any(path.startswith(BACKEND_PREFIX) for path in normalized_paths):
        scopes.append("backend-runtime")
    if any(path.startswith(FRONTEND_PREFIX) for path in normalized_paths):
        scopes.append("frontend")
    if any(
        path == "AGENTS.md"
        or path == "CLAUDE.md"
        or path == "opencode.json"
        or path == "scripts/agent_stack_health.py"
        or any(path.startswith(prefix) for prefix in AI_CUSTOMIZATION_PREFIXES)
        for path in normalized_paths
    ):
        scopes.append("ai-customization")
    if any(path.startswith(TEST_PREFIX) for path in normalized_paths):
        scopes.append("tests")
    if any(path.startswith("docs/") or path.endswith(".md") for path in normalized_paths):
        scopes.append("docs")

    return tuple(scopes)


def build_verification_steps(paths: Sequence[str]) -> tuple[str, ...]:
    normalized_paths = normalize_paths(paths)
    steps: list[str] = []

    if any(path.startswith(BACKEND_PREFIX) for path in normalized_paths):
        steps.append("运行受影响的 pytest 与 repository-baseline pylint。")
        steps.append("如果触及业务运行时代码，验收前必须全量运行 `./.venv/Scripts/python.exe -m pytest tests/ -v`。")

    if any(path.startswith(FRONTEND_PREFIX) for path in normalized_paths):
        steps.append("在 `src/web` 下运行 `npm run lint`，必要时补最小构建验证。")

    if any(
        path == "AGENTS.md"
        or path == "CLAUDE.md"
        or path == "opencode.json"
        or path == "scripts/agent_stack_health.py"
        or any(path.startswith(prefix) for prefix in AI_CUSTOMIZATION_PREFIXES)
        for path in normalized_paths
    ):
        steps.append(
            "运行 `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo` "
            "与相关 hook / 健康检查 pytest。"
        )

    if any(
        path.startswith("src/bill_analyser/api/routes/") or path == "src/bill_analyser/api/app.py"
        for path in normalized_paths
    ):
        steps.append("复核 `src/web/src/lib/services.ts` 与相关 store 是否已同步 API 契约。")

    if any(
        "statistics" in path
        or "budget" in path
        or path.startswith("src/bill_analyser/core/bills/")
        or path.startswith("src/bill_analyser/core/investment/")
        or path.endswith("bill_service.py")
        or path.endswith("smart_dedup.py")
        or path.endswith("category_engine.py")
        for path in normalized_paths
    ):
        steps.append("人工复核元/分转换、统计口径与导入链路调用顺序。")

    if not steps:
        steps.append("先查看当前 `git status` / `git diff`，再补最相关的最小验证。")

    return tuple(steps)


def choose_next_step(paths: Sequence[str]) -> str:
    scopes = set(detect_scopes(paths))

    if {"backend-runtime", "frontend"} <= scopes:
        return "先核对前后端 API 契约与 services/store 映射，再分别运行后端与前端验证。"
    if "ai-customization" in scopes:
        return (
            "先运行 agent_stack_health 与相关 hook 测试，"
            "确认 AI 定制层、handoff 工作流与 task-state 持久化链路仍然健康。"
        )
    if "backend-runtime" in scopes:
        return "先补或更新回归测试，再运行受影响 pytest / pylint；验收前再跑全量 pytest。"
    if "frontend" in scopes:
        return "先在 `src/web` 下运行 lint，并检查 services/store 契约是否同步。"

    return "先查看 `git status` / `git diff`，再从最近编辑的文件继续当前任务。"
