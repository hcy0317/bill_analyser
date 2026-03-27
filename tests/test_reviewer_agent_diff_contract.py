from __future__ import annotations

from pathlib import Path
import re

import pytest


REPO_ROOT = Path(__file__).resolve().parents[1]
REVIEWER_SECTION_TITLE = "## Review Scope Contract"
BLOCKED_SENTINEL = "BLOCKED: review scope unavailable"

REVIEWER_FILES = (
    ".github/agents/code-reviewer.agent.md",
    ".github/agents/python-reviewer.agent.md",
    ".github/agents/security-reviewer.agent.md",
    ".cursor/agents/code-reviewer.md",
    ".cursor/agents/python-reviewer.md",
    ".cursor/agents/security-reviewer.md",
)

REVIEWER_CONTRACT_SNIPPETS = (
    "caller-provided review context",
    "base_ref",
    "head_ref",
    "changed_files",
    "diff_text",
)

GUIDE_FILES = {
    ".github/copilot-instructions.md": (
        "## Reviewer Subagent Contract",
        "base_ref",
        "head_ref",
        "changed_files",
        "diff_text",
    ),
    ".github/instructions/ecc/common-agents.instructions.md": (
        "## Reviewer Invocation Contract",
        "base_ref",
        "head_ref",
        "changed_files",
        "diff_text",
    ),
    ".cursor/rules/common-agents.md": (
        "## Reviewer Invocation Contract",
        "base_ref",
        "head_ref",
        "changed_files",
        "diff_text",
    ),
    "AGENTS.md": (
        "## Reviewer Diff Contract",
        "base_ref",
        "head_ref",
        "changed_files",
        "diff_text",
    ),
}

REVIEWER_FILE_PAIRS = (
    (".github/agents/code-reviewer.agent.md", ".cursor/agents/code-reviewer.md"),
    (".github/agents/python-reviewer.agent.md", ".cursor/agents/python-reviewer.md"),
    (".github/agents/security-reviewer.agent.md", ".cursor/agents/security-reviewer.md"),
)

GUIDE_FILE_PAIRS = (
    (
        ".github/instructions/ecc/common-agents.instructions.md",
        ".cursor/rules/common-agents.md",
        "## Reviewer Invocation Contract",
    ),
)


def _read_repo_file(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


def _normalize_whitespace(text: str) -> str:
    return re.sub(r"\s+", " ", text.strip())


def _extract_markdown_section(text: str, section_title: str) -> str:
    lines = text.splitlines()
    collected_lines: list[str] = []
    is_collecting = False

    for line in lines:
        if line.strip() == section_title:
            is_collecting = True
        elif is_collecting and line.startswith("## "):
            break

        if is_collecting:
            collected_lines.append(line)

    assert collected_lines, f"Missing markdown section: {section_title}"
    return "\n".join(collected_lines)


@pytest.mark.parametrize("relative_path", REVIEWER_FILES)
def test_reviewer_agent_files_require_explicit_diff_context(relative_path: str) -> None:
    text = _read_repo_file(relative_path)
    review_scope_section = _extract_markdown_section(text, REVIEWER_SECTION_TITLE)
    normalized_section = _normalize_whitespace(review_scope_section).lower()

    for snippet in REVIEWER_CONTRACT_SNIPPETS:
        assert snippet in normalized_section, (
            f"{relative_path} is missing reviewer diff-contract snippet inside {REVIEWER_SECTION_TITLE}: {snippet}"
        )

    assert BLOCKED_SENTINEL in review_scope_section, (
        f"{relative_path} must keep the exact fail-fast sentinel text: {BLOCKED_SENTINEL}"
    )


@pytest.mark.parametrize(
    ("relative_path", "section_title", "required_snippets"),
    tuple(
        (relative_path, config[0], config[1:])
        for relative_path, config in GUIDE_FILES.items()
    ),
)
def test_workspace_guides_document_reviewer_diff_contract(
    relative_path: str,
    section_title: str,
    required_snippets: tuple[str, ...],
) -> None:
    text = _read_repo_file(relative_path)
    review_scope_section = _extract_markdown_section(text, section_title)
    normalized_section = _normalize_whitespace(review_scope_section).lower()

    for snippet in required_snippets:
        assert snippet.lower() in normalized_section, (
            f"{relative_path} is missing reviewer orchestration snippet inside {section_title}: {snippet}"
        )


@pytest.mark.parametrize(("left_path", "right_path"), REVIEWER_FILE_PAIRS)
def test_reviewer_agent_pairs_stay_in_sync(left_path: str, right_path: str) -> None:
    left_section = _extract_markdown_section(_read_repo_file(left_path), REVIEWER_SECTION_TITLE)
    right_section = _extract_markdown_section(_read_repo_file(right_path), REVIEWER_SECTION_TITLE)

    assert _normalize_whitespace(left_section) == _normalize_whitespace(right_section), (
        f"Reviewer contract drift detected between {left_path} and {right_path}"
    )


@pytest.mark.parametrize(("left_path", "right_path", "section_title"), GUIDE_FILE_PAIRS)
def test_shared_reviewer_guide_pairs_stay_in_sync(
    left_path: str,
    right_path: str,
    section_title: str,
) -> None:
    left_section = _extract_markdown_section(_read_repo_file(left_path), section_title)
    right_section = _extract_markdown_section(_read_repo_file(right_path), section_title)

    assert _normalize_whitespace(left_section) == _normalize_whitespace(right_section), (
        f"Reviewer orchestration drift detected between {left_path} and {right_path}"
    )