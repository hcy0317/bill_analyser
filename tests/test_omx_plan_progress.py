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
        next_slice="category_rules.py",
    )
    omx_plan_progress.write_state(state_path, state)

    persisted = json.loads(state_path.read_text(encoding="utf-8"))
    assert persisted["completed_prs"] == ["68"]
    assert persisted["last_merged_commit"] == "039518433"
    assert persisted["next_slice"] == "category_rules.py"
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
