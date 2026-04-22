"""Regression tests for repo-local reviewer diff-contract guidance."""

from __future__ import annotations

# pylint: disable=missing-function-docstring

from pathlib import Path
import re

import pytest


REPO_ROOT = Path(__file__).resolve().parents[1]
REVIEWER_SECTION_TITLE = "## Review Scope Contract"
BLOCKED_SENTINEL = "BLOCKED: review scope unavailable"

REVIEWER_FILES = tuple(
    relative_path
    for relative_path in (
        ".github/agents/code-reviewer.agent.md",
        ".github/agents/python-reviewer.agent.md",
        ".github/agents/security-reviewer.agent.md",
    )
    if (REPO_ROOT / relative_path).exists()
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
    "AGENTS.md": (
        "## Reviewer Diff Contract",
        "base_ref",
        "head_ref",
        "changed_files",
        "diff_text",
    ),
}


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


if REVIEWER_FILES:

    @pytest.mark.parametrize("relative_path", REVIEWER_FILES)
    def test_reviewer_agent_files_require_explicit_diff_context(relative_path: str) -> None:
        text = _read_repo_file(relative_path)
        review_scope_section = _extract_markdown_section(text, REVIEWER_SECTION_TITLE)
        normalized_section = _normalize_whitespace(review_scope_section).lower()

        for snippet in REVIEWER_CONTRACT_SNIPPETS:
            assert snippet in normalized_section, (
                f"{relative_path} is missing reviewer diff-contract snippet "
                f"inside {REVIEWER_SECTION_TITLE}: {snippet}"
            )

        assert BLOCKED_SENTINEL in review_scope_section, (
            f"{relative_path} must keep the exact fail-fast sentinel text: {BLOCKED_SENTINEL}"
        )

else:

    def test_reviewer_agent_files_require_explicit_diff_context() -> None:
        pytest.skip(
            "repo-local reviewer agent manifests were intentionally removed; "
            "contract lives in workspace guides"
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
            f"{relative_path} is missing reviewer orchestration snippet "
            f"inside {section_title}: {snippet}"
        )
