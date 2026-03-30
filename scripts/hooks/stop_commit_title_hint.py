from __future__ import annotations

import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Sequence

REPO_ROOT = Path(__file__).resolve().parents[2]
DOC_FILES = {"README.md", "AGENTS.md", "CLAUDE.md"}
HOOK_SCOPE_PATHS = (
    ".github/hooks/",
    "scripts/hooks/",
)
AGENT_STACK_PATHS = (
    ".github/instructions/",
    ".github/skills/",
    ".agents/skills/",
    ".claude/skills/",
    ".github/copilot-instructions.md",
    "AGENTS.md",
    "CLAUDE.md",
    ".claude/settings.json",
)
RUNTIME_PREFIXES = (
    "src/bill_analyser/",
    "src/web/src/",
)
TEST_PREFIXES = (
    "tests/",
    "src/web/tests/",
)


def _starts_with_any(path_text: str, prefixes: Sequence[str]) -> bool:
    return any(path_text.startswith(prefix) for prefix in prefixes)


def _run_git(args: Sequence[str]) -> subprocess.CompletedProcess[str] | None:
    try:
        return subprocess.run(
            ["git", *args],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
        )
    except FileNotFoundError:
        return None


def _split_lines(text: str) -> list[str]:
    return [line.strip() for line in text.splitlines() if line.strip()]


def _normalize_path(path_text: str) -> str:
    return path_text.replace("\\", "/").strip()


def _get_diff_snapshot() -> tuple[str | None, list[str], str]:
    staged_names = _run_git(["diff", "--cached", "--name-only"])
    if staged_names and staged_names.returncode == 0:
        staged_files = [_normalize_path(item) for item in _split_lines(staged_names.stdout)]
        if staged_files:
            staged_diff = _run_git(["diff", "--cached", "--"])
            return "staged", staged_files, staged_diff.stdout if staged_diff else ""

    unstaged_names = _run_git(["diff", "--name-only"])
    if unstaged_names and unstaged_names.returncode == 0:
        unstaged_files = [_normalize_path(item) for item in _split_lines(unstaged_names.stdout)]
        if unstaged_files:
            unstaged_diff = _run_git(["diff", "--"])
            return "unstaged", unstaged_files, unstaged_diff.stdout if unstaged_diff else ""

    return None, [], ""


def _is_docs_path(path_text: str) -> bool:
    return path_text.startswith("docs/") or path_text.endswith(".md") or path_text in DOC_FILES


def _is_test_path(path_text: str) -> bool:
    return _starts_with_any(path_text, TEST_PREFIXES) or path_text.endswith(("_test.py", ".spec.ts", ".spec.js"))


def _classify_scope(changed_files: list[str]) -> str | None:
    if not changed_files:
        return None

    scores: Counter[str] = Counter()
    for path_text in changed_files:
        if any(path_text.startswith(prefix) for prefix in HOOK_SCOPE_PATHS):
            scores["hooks"] += 3
        if any(path_text == item or path_text.startswith(item) for item in AGENT_STACK_PATHS):
            scores["agent-stack"] += 2
        if path_text.startswith("src/bill_analyser/parsers/") or "fixtures/import_samples" in path_text:
            scores["parser"] += 3
        elif path_text.startswith("src/bill_analyser/api/"):
            scores["api"] += 3
        elif path_text.startswith("src/bill_analyser/core/"):
            scores["core"] += 2
        elif path_text.startswith("src/web/"):
            scores["web"] += 2
        elif _is_test_path(path_text):
            scores["tests"] += 1
        elif _is_docs_path(path_text):
            scores["docs"] += 1

    return scores.most_common(1)[0][0] if scores else None


def _classify_type(changed_files: list[str], diff_text: str) -> str:
    if not changed_files:
        return "chore"

    diff_lower = diff_text.lower()
    docs_only = all(_is_docs_path(path_text) for path_text in changed_files)
    tests_only = all(_is_test_path(path_text) for path_text in changed_files)
    hook_only = all(
        path_text.startswith(HOOK_SCOPE_PATHS)
        or path_text == ".claude/settings.json"
        or path_text.startswith(".github/instructions/")
        or path_text.startswith(".github/skills/")
        or path_text.startswith(".agents/skills/")
        or path_text.startswith(".claude/skills/")
        or path_text in DOC_FILES
        for path_text in changed_files
    )

    if docs_only:
        return "docs"
    if tests_only:
        return "test"
    if hook_only:
        return "chore"

    if any(_starts_with_any(path_text, RUNTIME_PREFIXES) for path_text in changed_files):
        if any(token in diff_lower for token in ("fix", "fallback", "guard", "abort(", "404", "decode", "skip", "warning", "error")):
            return "fix"
        if any(token in diff_lower for token in ("add ", "create ", "新增", "support", "introduce")):
            return "feat"
        return "fix"

    return "chore"


def _choose_description(commit_type: str, scope: str | None, changed_files: list[str], diff_text: str) -> str:
    diff_lower = diff_text.lower()

    if scope == "hooks":
        if any("stop" in path_text.lower() for path_text in changed_files):
            return "补充会话结束提交标题提示"
        return "完善仓库 hook 自动化规则"

    if scope == "agent-stack":
        return "完善 AI 工作流约束与提示"

    if commit_type == "docs":
        return "更新仓库说明与工作流文档"

    if commit_type == "test":
        return "补充回归测试覆盖"

    if scope == "parser":
        if any(token in diff_lower for token in ("utf-8", "utf8", "gbk", "gb18030", "decode", "encoding")):
            return "修复账单解析编码兼容与回退"
        return "修复账单解析兼容问题"

    if scope == "api":
        if any(token in diff_lower for token in ("404", "abort(", "invalid request", "validate", "boundary")):
            return "修复接口路由与边界处理"
        return "修复接口行为问题"

    if scope == "core":
        return "修复核心服务边界处理"

    if scope == "web":
        return "完善前端交互与状态处理"

    return "整理仓库配置与辅助脚本"


def _build_commit_title(changed_files: list[str], diff_text: str) -> str:
    commit_type = _classify_type(changed_files, diff_text)
    scope = _classify_scope(changed_files)
    description = _choose_description(commit_type, scope, changed_files, diff_text)
    scope_segment = f"({scope})" if scope and scope != "docs" else ""
    return f"{commit_type}{scope_segment}: {description}"


def _print_hint(source: str, changed_files: list[str], title: str) -> None:
    print("[hook] 检测到 git diff，已生成中文 Conventional Commit 标题建议：")
    print(f"[hook] 来源：{source}")
    print(f"[hook] 建议：{title}")
    if len(changed_files) <= 8:
        print("[hook] 影响文件：")
        for path_text in changed_files:
            print(f"  - {path_text}")
    else:
        print(f"[hook] 影响文件数量：{len(changed_files)}")
    if len({_classify_scope(changed_files), "tests" if any(_is_test_path(path_text) for path_text in changed_files) else None} - {None}) > 1:
        print("[hook] 提示：本次改动跨多个区域，若提交意图不止一个，建议拆分提交。")


def main() -> int:
    if not sys.stdin.isatty():
        _ = sys.stdin.read()
    source, changed_files, diff_text = _get_diff_snapshot()
    if not source or not changed_files:
        return 0

    title = _build_commit_title(changed_files, diff_text)
    _print_hint(source, changed_files, title)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
