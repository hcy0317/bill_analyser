from __future__ import annotations

import argparse
import sys
from pathlib import Path

STANDARD_SECTIONS = (
    "### 目标",
    "### 变更范围",
    "### 验证证据",
    "### 风险与开放门禁",
)
LEGACY_HEADINGS = ("Summary", "Test plan", "Open gates")


def render_template() -> str:
    return "\n\n".join(
        (
            "### 目标\n- <本 PR 要完成的功能域结果，不写过程流水账。>",
            "### 变更范围\n- <主要代码/合同/测试/文档变更。>",
            "### 验证证据\n- [ ] `<真实执行过的命令或 CI job>`",
            "### 风险与开放门禁\n- [ ] <仍未关闭的门禁；如果没有，写“无”。>",
        )
    )


def validate_pr_body(text: str) -> list[str]:
    errors: list[str] = []
    stripped_lines = {line.strip() for line in text.splitlines()}

    for heading in STANDARD_SECTIONS:
        if heading not in stripped_lines:
            errors.append(f"missing standard section: {heading}")

    for legacy_heading in LEGACY_HEADINGS:
        if legacy_heading in stripped_lines:
            errors.append(f"legacy PR heading is not allowed: {legacy_heading}")

    if "`" not in text:
        errors.append("verification evidence must quote commands or CI job names with backticks")

    if "- [ ]" not in text and "- [x]" not in text:
        errors.append("PR body must use checklist bullets for verification and gates")

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Render or validate the Bill Analyser PR body contract.")
    parser.add_argument("--template", action="store_true", help="Print the standard PR body template.")
    parser.add_argument("--validate", type=Path, help="Validate a markdown PR body file.")
    args = parser.parse_args(argv)

    if args.template:
        print(render_template())
        return 0

    if args.validate:
        errors = validate_pr_body(args.validate.read_text(encoding="utf-8"))
        if errors:
            for error in errors:
                print(f"ERROR: {error}", file=sys.stderr)
            return 1
        print("PR body contract: pass")
        return 0

    parser.error("use --template or --validate")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
