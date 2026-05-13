from __future__ import annotations

from scripts import pr_body_template


def test_standard_pr_body_template_uses_current_sections() -> None:
    template = pr_body_template.render_template()

    for heading in pr_body_template.STANDARD_SECTIONS:
        assert heading in template

    for legacy_heading in pr_body_template.LEGACY_HEADINGS:
        assert legacy_heading not in template


def test_validate_pr_body_rejects_legacy_headings() -> None:
    errors = pr_body_template.validate_pr_body(
        "\n".join(
            (
                "Summary",
                "Remove a route.",
                "Test plan",
                "`pytest`",
                "Open gates",
                "- [ ] CI",
            )
        )
    )

    assert any("legacy PR heading" in error for error in errors)


def test_validate_pr_body_accepts_standard_contract() -> None:
    body = "\n\n".join(
        (
            "### 目标\n- 完成一个切片。",
            "### 变更范围\n- 更新代码和测试。",
            "### 验证证据\n- [x] `pytest tests/test_pr_body_template.py -v`",
            "### 风险与开放门禁\n- [ ] CI 通过后合并。",
        )
    )

    assert pr_body_template.validate_pr_body(body) == []
