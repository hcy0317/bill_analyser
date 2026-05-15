---
name: verification-loop
description: "A comprehensive verification system for Claude Code sessions."
---

# Verification Loop Skill

A comprehensive verification system for Claude Code sessions.

## Entry Point Positioning

- Prefer `/verify` as the default, repository-facing verification entrypoint.
- Use `/quality-gate` only for quick path-scoped lint/format/type checks.
- Reach for this `verification-loop` skill directly only when you are refining verification behavior or working on the underlying workflow assets themselves.

## When to Use

Invoke this skill:
- After completing a feature or significant code change
- Before creating a PR
- When you want to ensure quality gates pass
- After refactoring

## Verification Phases

### Phase 0: Scope Detection

Inspect `git status`, staged diff, and unstaged diff first. Verification should follow the touched paths instead of running a generic one-size-fits-all pipeline.

### Phase 1: Rust Runtime Verification

Run these when `src/backend/**`, `tests/backend/**`, or other Rust runtime files changed:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

If the diff touches business runtime code under `src/backend/**`, audit acceptance still requires:

```powershell
cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90
```

### Phase 2: Frontend Verification

Run this when `src/web/**` changed:

```powershell
Set-Location src\web
npm run lint
```

If the change is UI-heavy or contract-sensitive, add the smallest useful build or test validation on top.

### Phase 3: AI Customization / Hook Verification

Run this when `.github/**`, `.agents/**`, `.claude/**`, `scripts/hooks/**`, or `scripts/agent_stack_health.py` changed:

```powershell
./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo
./.venv/Scripts/python.exe -m pytest tests/test_agent_stack_health.py tests/test_pre_tool_repo_guard.py -v
```

Add the most relevant hook-specific tests if post-tool or stop-hook behavior changed.

### Phase 4: Contract and Money Review

- If API routes or adapters changed, confirm `src/web/src/lib/services.ts` and related stores still match the backend contract.
- If amount fields, budget math, import normalization, or statistics changed, manually review yuan/cents conversion once.

### Phase 5: Diff Review

Review each changed file for:
- unintended edits
- missing error handling
- verification gaps
- hidden API or money-unit drift

## Output Format

After running all phases, produce a verification report:

```
VERIFICATION REPORT
==================

Scope:     [backend/frontend/ai-customization/mixed]
Rust:      [PASS/FAIL/N-A]
Frontend:  [PASS/FAIL/N-A]
Coverage:  [PASS/FAIL/N-A]
Hooks:     [PASS/FAIL/N-A]
Contracts: [PASS/FAIL/N-A]
Diff:      [X files changed]

Overall:   [READY/NOT READY] for PR

Issues to Fix:
1. ...
2. ...
```

## Continuous Mode

For long sessions, run verification every 15 minutes or after major changes:

```markdown
Set a mental checkpoint:
- After completing each function
- After finishing a component
- Before moving to next task

Run: /verify
```

## Integration with Hooks

This skill complements PostToolUse hooks but provides deeper verification.
Hooks catch issues immediately; this skill provides comprehensive review.
