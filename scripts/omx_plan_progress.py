from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

DEFAULT_PLAN_PATH = Path(".omx/plans/rust-full-rewrite-total-plan.md")
DEFAULT_STATE_PATH = Path(".omx/state/autopilot-state.json")
PHASE_ORDER = ("P0", "P1", "P2", "P6", "P3", "P4", "P5", "P9", "P10", "P7", "P11", "P8", "P12", "P13", "P14", "P15")
TERMINAL_STATUSES = {"completed", "skipped"}


@dataclass(frozen=True)
class PhaseAudit:
    phase: str
    status: str
    evidence: tuple[str, ...]
    gaps: tuple[str, ...]


def _now_iso() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def load_state(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def write_state(path: Path, state: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def ensure_progress(state: dict[str, Any]) -> dict[str, Any]:
    progress = state.setdefault("plan_progress", {})
    progress.setdefault("schema_version", 1)
    progress.setdefault("phase_order", list(PHASE_ORDER))
    progress.setdefault("phases", {})
    progress.setdefault("completed_slices", [])
    progress.setdefault("updated_at", _now_iso())
    return progress


def phase_status(progress: dict[str, Any], phase: str) -> str:
    return str(progress.get("phases", {}).get(phase, {}).get("status", "pending"))


def blocked_prior_phases(progress: dict[str, Any], phase: str) -> list[str]:
    phase_order = tuple(progress.get("phase_order") or PHASE_ORDER)
    if phase not in phase_order:
        raise ValueError(f"unknown phase: {phase}")
    index = phase_order.index(phase)
    return [candidate for candidate in phase_order[:index] if phase_status(progress, candidate) not in TERMINAL_STATUSES]


def mark_slice(
    state: dict[str, Any],
    *,
    phase: str,
    title: str,
    pr: str | None = None,
    merge_commit: str | None = None,
    next_slice: str | None = None,
) -> dict[str, Any]:
    progress = ensure_progress(state)
    phases = progress.setdefault("phases", {})
    phase_entry = phases.setdefault(phase, {"status": "in_progress", "slices": []})
    phase_entry["status"] = "in_progress"
    slices = phase_entry.setdefault("slices", [])
    slice_entry = {
        "title": title,
        "status": "completed",
        "completed_at": _now_iso(),
    }
    if pr:
        slice_entry["pr"] = str(pr)
    if merge_commit:
        slice_entry["merge_commit"] = merge_commit
    slices.append(slice_entry)

    progress.setdefault("completed_slices", []).append({"phase": phase, **slice_entry})
    progress["updated_at"] = _now_iso()
    state["current_slice"] = phase
    if next_slice:
        state["next_slice"] = next_slice
    if pr:
        completed_prs = state.setdefault("completed_prs", [])
        if str(pr) not in [str(item) for item in completed_prs]:
            completed_prs.append(str(pr))
    if merge_commit:
        state["last_merged_commit"] = merge_commit
    return state


def mark_phase(state: dict[str, Any], *, phase: str, status: str, evidence: list[str] | None = None) -> dict[str, Any]:
    progress = ensure_progress(state)
    phase_entry = progress.setdefault("phases", {}).setdefault(phase, {})
    phase_entry["status"] = status
    phase_entry["updated_at"] = _now_iso()
    if evidence:
        phase_entry["evidence"] = evidence
    progress["updated_at"] = _now_iso()
    return state


def render_status(state: dict[str, Any]) -> str:
    progress = ensure_progress(state)
    lines = ["phase status:"]
    for phase in progress.get("phase_order", PHASE_ORDER):
        lines.append(f"- {phase}: {phase_status(progress, phase)}")
    next_allowed = next(
        (phase for phase in progress.get("phase_order", PHASE_ORDER) if phase_status(progress, phase) not in TERMINAL_STATUSES),
        None,
    )
    if next_allowed:
        lines.append(f"next allowed phase: {next_allowed}")
    else:
        lines.append("next allowed phase: complete")
    return "\n".join(lines)


def audit_early_phases(repo_root: Path) -> list[PhaseAudit]:
    def exists(relative: str) -> bool:
        return (repo_root / relative).exists()

    audits: list[PhaseAudit] = []

    p0_evidence = [
        "crates/bill-analyser-core/src/migration_governance.rs",
        "crates/bill-analyser-core/tests/migration_governance_contracts.rs",
        "scripts/check_rust_workspace_dependencies.py",
    ]
    audits.append(PhaseAudit("P0", "completed" if all(exists(path) for path in p0_evidence) else "blocked", tuple(p0_evidence), ()))

    p1_evidence = [
        "crates/bill-analyser-http/src/router.rs",
        "crates/bill-analyser-http/src/proxy.rs",
        "crates/bill-analyser-http/tests/proxy_contract.rs",
        "crates/bill-analyser-http/tests/import_skeleton_contract.rs",
    ]
    audits.append(PhaseAudit("P1", "completed" if all(exists(path) for path in p1_evidence) else "blocked", tuple(p1_evidence), ()))

    p2_evidence = [
        "crates/bill-analyser-db/src/schema.rs",
        "crates/bill-analyser-db/src/connection.rs",
        "crates/bill-analyser-db/src/user_scope.rs",
        "crates/bill-analyser-db/tests/sqlite_runtime.rs",
    ]
    audits.append(PhaseAudit("P2", "completed" if all(exists(path) for path in p2_evidence) else "blocked", tuple(p2_evidence), ()))

    p6_gaps: list[str] = []
    if not exists("crates/bill-analyser-parsers"):
        p6_gaps.append("missing pure parser crate `crates/bill-analyser-parsers` required by P6")
    if not exists("crates/bill-analyser-core/tests/fixtures/parser_golden_contracts.json"):
        p6_gaps.append("missing parser golden fixture ledger")
    audits.append(
        PhaseAudit(
            "P6",
            "completed" if not p6_gaps else "blocked",
            ("crates/bill-analyser-core/src/parsers.rs", "crates/bill-analyser-core/tests/parser_contracts.rs"),
            tuple(p6_gaps),
        )
    )

    governance = (repo_root / "crates/bill-analyser-core/src/migration_governance.rs").read_text(encoding="utf-8")
    p3_gaps: list[str] = []
    if "real OAuth provider exchange remains Python-proxied" in governance:
        p3_gaps.append("P3 still records real OAuth provider exchange as Python-proxied")
    if "auth-security-user-data" not in governance:
        p3_gaps.append("missing auth-security-user-data governance domain")
    audits.append(
        PhaseAudit(
            "P3",
            "completed" if not p3_gaps else "blocked",
            ("crates/bill-analyser-http/tests/auth_runtime_contract.rs", "crates/bill-analyser-core/tests/auth_security_contracts.rs"),
            tuple(p3_gaps),
        )
    )

    return audits


def render_audit(audits: list[PhaseAudit]) -> str:
    lines: list[str] = []
    for audit in audits:
        lines.append(f"{audit.phase}: {audit.status}")
        for item in audit.evidence:
            lines.append(f"  evidence: {item}")
        for gap in audit.gaps:
            lines.append(f"  gap: {gap}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Track OMX P0-P15 plan progress and enforce phase order.")
    parser.add_argument("--state", type=Path, default=DEFAULT_STATE_PATH)
    parser.add_argument("--repo-root", type=Path, default=Path("."))
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("status")

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("--phase", required=True)

    mark_slice_parser = subparsers.add_parser("mark-slice")
    mark_slice_parser.add_argument("--phase", required=True)
    mark_slice_parser.add_argument("--slice", required=True)
    mark_slice_parser.add_argument("--pr")
    mark_slice_parser.add_argument("--merge-commit")
    mark_slice_parser.add_argument("--next-slice")

    mark_phase_parser = subparsers.add_parser("mark-phase")
    mark_phase_parser.add_argument("--phase", required=True)
    mark_phase_parser.add_argument("--status", required=True, choices=("pending", "in_progress", "blocked", "completed", "skipped"))
    mark_phase_parser.add_argument("--evidence", action="append")

    subparsers.add_parser("audit-early")

    args = parser.parse_args(argv)

    if args.command == "audit-early":
        audits = audit_early_phases(args.repo_root.resolve())
        print(render_audit(audits))
        return 1 if any(audit.status != "completed" for audit in audits) else 0

    state = load_state(args.state)
    ensure_progress(state)

    if args.command == "status":
        print(render_status(state))
        return 0

    if args.command == "check":
        blockers = blocked_prior_phases(ensure_progress(state), args.phase)
        if blockers:
            print(f"blocked: {args.phase} cannot run before {', '.join(blockers)}", file=sys.stderr)
            return 1
        print(f"phase order gate: {args.phase} allowed")
        return 0

    if args.command == "mark-slice":
        mark_slice(
            state,
            phase=args.phase,
            title=args.slice,
            pr=args.pr,
            merge_commit=args.merge_commit,
            next_slice=args.next_slice,
        )
        write_state(args.state, state)
        return 0

    if args.command == "mark-phase":
        mark_phase(state, phase=args.phase, status=args.status, evidence=args.evidence)
        write_state(args.state, state)
        return 0

    return 2


if __name__ == "__main__":
    raise SystemExit(main())
