from __future__ import annotations

from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def test_ai_workflow_doc_exists_and_explains_default_entrypoints() -> None:
    doc_path = REPO_ROOT / "docs" / "AI_WORKFLOW.md"
    text = doc_path.read_text(encoding="utf-8")

    assert doc_path.exists()
    assert "/verify" in text
    assert "/quality-gate" in text
    assert "verification-loop" in text
    assert ".git/ai/last-session.md" in text
    assert "session-resume" in text


def test_verification_assets_define_non_overlapping_roles() -> None:
    verify_prompt = (REPO_ROOT / ".github" / "prompts" / "verify.prompt.md").read_text(encoding="utf-8")
    quality_gate_prompt = (REPO_ROOT / ".github" / "prompts" / "quality-gate.prompt.md").read_text(encoding="utf-8")
    verification_skill = (
        REPO_ROOT / ".github" / "skills" / "verification-loop" / "SKILL.md"
    ).read_text(encoding="utf-8")

    assert "default verification entrypoint" in verify_prompt.lower()
    assert "prefer `/verify`" in quality_gate_prompt
    assert "Prefer `/verify` as the default" in verification_skill