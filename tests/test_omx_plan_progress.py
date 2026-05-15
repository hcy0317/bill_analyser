from __future__ import annotations

import json
from pathlib import Path

from scripts import omx_plan_progress


def test_phase_order_gate_blocks_jump_until_prior_phases_are_completed() -> None:
    state: dict = {}
    progress = omx_plan_progress.ensure_progress(state)

    assert omx_plan_progress.blocked_prior_phases(progress, "P10")[:5] == ["P0", "P1", "P2", "P6", "P3"]

    for phase in ("P0", "P1", "P2", "P6", "P3", "P4", "P5", "P9"):
        omx_plan_progress.mark_phase(state, phase=phase, status="completed", evidence=[f"{phase} evidence"])

    assert omx_plan_progress.blocked_prior_phases(progress, "P10") == []


def test_plan_ledger_backfills_stale_state_until_next_allowed_phase(tmp_path: Path) -> None:
    plan_path = tmp_path / "plan.md"
    plan_path.write_text(
        "\n".join(
            [
                "## 13. Execution Progress Ledger",
                "",
                "| Phase | Status | Evidence | Gate result |",
                "| --- | --- | --- | --- |",
                "| P0 | completed | governance | ok |",
                "| P1 | completed | http | ok |",
                "| P2 | completed | db | ok |",
                "| P6 | completed | parser | ok |",
                "| P3 | completed | auth | ok |",
                "| P4 | completed | taxonomy | ok |",
                "| P5 | completed | bills | ok |",
                "| P9 | completed | budgets | ok |",
                "| P10 | completed | statistics | ok |",
                "| P7 | completed | import | ok |",
                "| P11a | completed | llm config | ok |",
                "| P11d | completed | llm providers | ok |",
                "| P8a | completed | recurring | ok |",
                "| P8b | completed | matching | ok |",
                "| P12c | completed | backup sync | ok |",
                "",
                "Next allowed phase by gate: P13 residual tooling/hooks/CI/global cleanup only.",
            ]
        ),
        encoding="utf-8",
    )
    state: dict = {}
    progress = omx_plan_progress.ensure_progress(state)

    omx_plan_progress.hydrate_progress_from_plan(progress, plan_path)

    assert omx_plan_progress.blocked_prior_phases(progress, "P13") == []
    assert omx_plan_progress.blocked_prior_phases(progress, "P14") == ["P13"]
    assert omx_plan_progress.phase_status(progress, "P12") == "completed"
    assert "next allowed phase: P13" in omx_plan_progress.render_status(state, plan_path=plan_path)


def test_plan_ledger_explicit_blocker_wins_over_next_allowed_summary(tmp_path: Path) -> None:
    plan_path = tmp_path / "plan.md"
    plan_path.write_text(
        "\n".join(
            [
                "## 13. Execution Progress Ledger",
                "",
                "| Phase | Status | Evidence | Gate result |",
                "| --- | --- | --- | --- |",
                "| P0 | completed | governance | ok |",
                "| P1 | blocked | http | stale blocker |",
                "",
                "Next allowed phase by gate: P2 stale summary.",
            ]
        ),
        encoding="utf-8",
    )
    state: dict = {}
    progress = omx_plan_progress.ensure_progress(state)

    omx_plan_progress.hydrate_progress_from_plan(progress, plan_path)

    assert omx_plan_progress.phase_status(progress, "P1") == "blocked"
    assert omx_plan_progress.blocked_prior_phases(progress, "P2") == ["P1"]


def test_plan_ledger_overrides_stale_completed_state(tmp_path: Path) -> None:
    plan_path = tmp_path / "plan.md"
    plan_path.write_text(
        "\n".join(
            [
                "## 13. Execution Progress Ledger",
                "",
                "| Phase | Status | Evidence | Gate result |",
                "| --- | --- | --- | --- |",
                "| P0 | completed | governance | ok |",
                "| P1 | blocked | http | regression |",
                "",
                "Next allowed phase by gate: P2 stale summary.",
            ]
        ),
        encoding="utf-8",
    )
    state: dict = {"plan_progress": {"phases": {"P1": {"status": "completed"}}}}
    progress = omx_plan_progress.ensure_progress(state)

    omx_plan_progress.hydrate_progress_from_plan(progress, plan_path)

    assert omx_plan_progress.phase_status(progress, "P1") == "blocked"
    assert omx_plan_progress.blocked_prior_phases(progress, "P2") == ["P1"]


def test_plan_ledger_parent_stays_pending_when_any_subphase_is_pending(tmp_path: Path) -> None:
    plan_path = tmp_path / "plan.md"
    plan_path.write_text(
        "\n".join(
            [
                "## 13. Execution Progress Ledger",
                "",
                "| Phase | Status | Evidence | Gate result |",
                "| --- | --- | --- | --- |",
                "| P0 | completed | governance | ok |",
                "| P11a | pending | llm config | not ready |",
                "| P11b | completed | llm candidates | ok |",
            ]
        ),
        encoding="utf-8",
    )
    state: dict = {}
    progress = omx_plan_progress.ensure_progress(state)

    omx_plan_progress.hydrate_progress_from_plan(progress, plan_path)

    assert omx_plan_progress.phase_status(progress, "P11") == "pending"


def test_mark_slice_persists_pr_and_merge_commit(tmp_path: Path) -> None:
    state_path = tmp_path / "autopilot-state.json"
    state_path.write_text("{}", encoding="utf-8")
    state = omx_plan_progress.load_state(state_path)

    omx_plan_progress.mark_slice(
        state,
        phase="P4",
        title="delete Flask categories route shell",
        pr="68",
        merge_commit="039518433",
        next_slice="tags.py",
    )
    omx_plan_progress.write_state(state_path, state)

    persisted = json.loads(state_path.read_text(encoding="utf-8"))
    assert persisted["completed_prs"] == ["68"]
    assert persisted["last_merged_commit"] == "039518433"
    assert persisted["next_slice"] == "tags.py"
    assert persisted["plan_progress"]["phases"]["P4"]["slices"][0]["pr"] == "68"


def test_early_phase_audit_reports_completed_parser_and_auth_gates() -> None:
    audits = {audit.phase: audit for audit in omx_plan_progress.audit_early_phases(Path.cwd())}

    assert audits["P0"].status == "completed"
    assert audits["P1"].status == "completed"
    assert audits["P2"].status == "completed"
    assert audits["P6"].status == "completed"
    assert audits["P6"].gaps == ()
    assert audits["P3"].status == "completed"
    assert audits["P3"].gaps == ()
