---
name: approved-plan-execution
description: Execute from an approved Bill Analyser plan, preserve task state, and keep verification and handoff evidence aligned.
---

# Approved Plan Execution

Use this skill when continuing work from an approved `.tmp/plan.md`, OMX plan, or explicit user-approved checklist.

## Source Of Truth

- Treat the approved plan as the execution contract; do not silently replace it with a new plan.
- Before resuming interrupted work, read `.git/ai/last-session.md` when present.
- Read and update `.git/ai/task-state.json` when the current task uses persistent progress state.
- Check `git status` and current diff before editing so user changes are preserved.

## Execution Loop

1. Pick the smallest unfinished slice from the approved plan.
2. Implement only that slice's owned files and contracts.
3. Run the targeted tests for the changed surface.
4. Run the required coverage or health gates before marking a business-code slice done.
5. Write back progress to the plan or task state when the repository workflow requires it.

## Gates

- Business runtime changes require the current Rust coverage gate and focused regression tests.
- Rust runtime changes require the relevant `cargo test`, `cargo clippy`, and coverage gate.
- Frontend changes require lint and coverage for the touched surface.
- Do not mark historical compatibility-path deletion complete until Rust route runtime, DB write semantics, frontend flow, full coverage, and residual-reference gates all pass together.

## `/start-work` Behavior

`/start-work` resumes from the approved plan, `.git/ai/task-state.json`, and `.git/ai/last-session.md`; it should move directly into the next verifiable slice instead of broad rediscovery.
