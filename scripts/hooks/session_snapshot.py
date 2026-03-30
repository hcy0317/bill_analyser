from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
import subprocess
from typing import Sequence


REPO_ROOT = Path(__file__).resolve().parents[2]
SNAPSHOT_RELATIVE_PATH = Path(".git") / "ai" / "last-session.md"
GIT_INTERNAL_SNAPSHOT_PATH = Path("ai") / "last-session.md"

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


@dataclass(frozen=True)
class SnapshotWriteResult:
    snapshot_path: Path
    snapshot_display_path: str
    scopes: tuple[str, ...]
    next_step: str
    verification_steps: tuple[str, ...]


def _run_git(args: Sequence[str], repo_root: Path) -> subprocess.CompletedProcess[str] | None:
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


def _normalize_paths(paths: Sequence[str]) -> tuple[str, ...]:
    normalized = {
        path.replace("\\", "/").strip()
        for path in paths
        if path and path.strip()
    }
    return tuple(sorted(normalized))


def _split_stdout_lines(result: subprocess.CompletedProcess[str] | None) -> tuple[str, ...]:
    if result is None or result.returncode != 0:
        return ()

    return _normalize_paths(result.stdout.splitlines())


def resolve_snapshot_path(repo_root: Path = REPO_ROOT) -> Path | None:
    git_path_result = _run_git(["rev-parse", "--git-path", GIT_INTERNAL_SNAPSHOT_PATH.as_posix()], repo_root)
    if git_path_result and git_path_result.returncode == 0:
        raw_path = git_path_result.stdout.strip()
        if raw_path:
            candidate = Path(raw_path)
            if candidate.is_absolute():
                return candidate.resolve()
            return (repo_root / candidate).resolve()

    git_dir = repo_root / ".git"
    if git_dir.exists() and git_dir.is_dir():
        return (git_dir / "ai" / "last-session.md").resolve()

    return None


def format_snapshot_display_path(snapshot_path: Path, repo_root: Path = REPO_ROOT) -> str:
    try:
        return snapshot_path.relative_to(repo_root).as_posix()
    except ValueError:
        return str(snapshot_path)


def collect_git_state(repo_root: Path = REPO_ROOT) -> GitState:
    branch_result = _run_git(["rev-parse", "--abbrev-ref", "HEAD"], repo_root)
    head_result = _run_git(["rev-parse", "--short", "HEAD"], repo_root)
    staged_result = _run_git(["diff", "--cached", "--name-only"], repo_root)
    unstaged_result = _run_git(["diff", "--name-only"], repo_root)
    untracked_result = _run_git(["ls-files", "--others", "--exclude-standard"], repo_root)

    branch = branch_result.stdout.strip() if branch_result and branch_result.returncode == 0 else "<unknown>"
    head = head_result.stdout.strip() if head_result and head_result.returncode == 0 else "<unknown>"

    return GitState(
        branch=branch or "<unknown>",
        head=head or "<unknown>",
        staged_files=_split_stdout_lines(staged_result),
        unstaged_files=_split_stdout_lines(unstaged_result),
        untracked_files=_split_stdout_lines(untracked_result),
    )


def detect_scopes(paths: Sequence[str]) -> tuple[str, ...]:
    normalized_paths = _normalize_paths(paths)
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
    normalized_paths = _normalize_paths(paths)
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
        steps.append("运行 `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo` 与相关 hook / 健康检查 pytest。")

    if any(path.startswith("src/bill_analyser/api/routes/") or path == "src/bill_analyser/api/app.py" for path in normalized_paths):
        steps.append("复核 `src/web/src/lib/services.ts` 与相关 store 是否已同步 API 契约。")

    if any(
        "statistics" in path
        or "budget" in path
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
        return "先运行 agent_stack_health 与相关 hook 测试，确认 AI 定制层和断线续作链路仍然健康。"
    if "backend-runtime" in scopes:
        return "先补或更新回归测试，再运行受影响 pytest / pylint；验收前再跑全量 pytest。"
    if "frontend" in scopes:
        return "先在 `src/web` 下运行 lint，并检查 services/store 契约是否同步。"

    return "先查看 `git status` / `git diff`，再从最近编辑的文件继续当前任务。"


def build_snapshot_markdown(
    *,
    trigger: str,
    git_state: GitState,
    recent_files: Sequence[str],
    snapshot_display_path: str,
) -> str:
    normalized_recent_files = _normalize_paths(recent_files)
    all_paths = _normalize_paths(
        (*git_state.staged_files, *git_state.unstaged_files, *git_state.untracked_files, *normalized_recent_files)
    )
    scopes = detect_scopes(all_paths)
    verification_steps = build_verification_steps(all_paths)
    next_step = choose_next_step(all_paths)
    timestamp = datetime.now(tz=timezone.utc).astimezone().isoformat(timespec="seconds")

    def _render_list(items: Sequence[str], empty_text: str) -> str:
        if not items:
            return f"- {empty_text}"
        return "\n".join(f"- `{item}`" for item in items)

    scope_block = _render_list(scopes, "未识别到明确焦点域")
    recent_block = _render_list(normalized_recent_files, "本次 hook 未提供新的编辑路径")
    staged_block = _render_list(git_state.staged_files, "暂无 staged diff")
    unstaged_block = _render_list(git_state.unstaged_files, "暂无 unstaged diff")
    untracked_block = _render_list(git_state.untracked_files, "暂无 untracked 文件")
    verification_block = "\n".join(f"{index}. {step}" for index, step in enumerate(verification_steps, start=1))

    return "\n".join(
        (
            "# Last Session Snapshot",
            "",
            f"- Updated: {timestamp}",
            f"- Trigger: `{trigger}`",
            f"- Branch: `{git_state.branch}`",
            f"- HEAD: `{git_state.head}`",
            f"- Logical Snapshot Path: `{SNAPSHOT_RELATIVE_PATH.as_posix()}`",
            f"- Resolved Snapshot Path: `{snapshot_display_path}`",
            "",
            "## Recent edited files",
            recent_block,
            "",
            "## Current diff state",
            "### Staged",
            staged_block,
            "",
            "### Unstaged",
            unstaged_block,
            "",
            "### Untracked",
            untracked_block,
            "",
            "## Detected focus areas",
            scope_block,
            "",
            "## Suggested next verification",
            verification_block,
            "",
            "## Suggested next step",
            f"- {next_step}",
            "",
            "## Resume checklist",
            "1. Read `AGENTS.md`.",
            "2. Read `docs/PROJECT_OVERVIEW.md`.",
            (
                "3. Read `.git/ai/last-session.md` if it exists; under worktree or special git layouts, "
                "resolve the real path with `git rev-parse --git-path ai/last-session.md`."
            ),
            "4. Inspect `git status` and `git diff`.",
            "5. Continue the pending task from the files and verification hints above.",
            "",
            "## Safety",
            "- This snapshot stores metadata only; never put secrets, environment variables, or full patch contents here.",
            "",
        )
    )


def write_session_snapshot(
    *,
    trigger: str,
    recent_files: Sequence[str],
    repo_root: Path = REPO_ROOT,
) -> SnapshotWriteResult | None:
    snapshot_path = resolve_snapshot_path(repo_root)
    if snapshot_path is None:
        return None

    git_state = collect_git_state(repo_root)
    all_paths = _normalize_paths((*git_state.staged_files, *git_state.unstaged_files, *git_state.untracked_files, *recent_files))
    snapshot_display_path = format_snapshot_display_path(snapshot_path, repo_root)
    snapshot_text = build_snapshot_markdown(
        trigger=trigger,
        git_state=git_state,
        recent_files=recent_files,
        snapshot_display_path=snapshot_display_path,
    )
    try:
        snapshot_path.parent.mkdir(parents=True, exist_ok=True)
        snapshot_path.write_text(snapshot_text, encoding="utf-8")
    except OSError:
        return None

    return SnapshotWriteResult(
        snapshot_path=snapshot_path,
        snapshot_display_path=snapshot_display_path,
        scopes=detect_scopes(all_paths),
        next_step=choose_next_step(all_paths),
        verification_steps=build_verification_steps(all_paths),
    )