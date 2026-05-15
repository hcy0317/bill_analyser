---
description: Default repository-aware verification entrypoint. Use for final scoped verification before review, handoff, or PR.
---

# Verification Command

Run comprehensive verification on current codebase state.

`/verify` is the **default verification entrypoint** for this repository.

- Prefer `/verify` for comprehensive, path-aware validation.
- Use `/quality-gate` only for quick file/folder checks.
- Treat `verification-loop` as the underlying deep workflow that powers this verification style.

## Instructions

Execute verification in this exact order:

1. **Detect Scope First**
   - Inspect `git status`, staged diff, and unstaged diff
   - Group changed files into `src/backend/**`, `tests/backend/**`, `src/web/**`, and AI customization / hook paths

2. **Rust Runtime Checks**
   - For `src/backend/**` or `tests/backend/**`, run affected `cargo test` first when useful
   - If business runtime code changed, require `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90` before reporting PASS

3. **Frontend Checks**
   - For `src/web/**`, run `npm run lint` inside `src/web`
   - If the UI or contract change is broad, add the smallest useful build/test verification

4. **AI Customization / Hook Checks**
   - For `.github/**`, `.agents/**`, `.claude/**`, `scripts/hooks/**`, or `scripts/agent_stack_health.py`, run `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo`
   - Run relevant hook / health pytest files for the changed scripts

5. **Contract & Money Review**
   - If API routes changed, confirm `src/web/src/lib/services.ts` and related stores are still aligned
   - If money, budget, import, or statistics logic changed, explicitly review yuan/cents conversion once

6. **Git Status Review**
   - Show uncommitted changes and summarize any remaining verification gaps
   - If `.git/ai/task-state.json` exists, mention whether its `nextVerification` still matches the current diff scope

## Output

Produce a concise verification report:

```
VERIFICATION: [PASS/FAIL]

Scope:    [backend/frontend/ai-customization/mixed]
Rust:     [OK/FAIL/N-A]
Frontend: [OK/FAIL/N-A]
Coverage: [OK/FAIL/N-A]
Hooks:    [OK/FAIL/N-A]
Contract: [OK/FAIL/N-A]

Ready for PR: [YES/NO]
```

If any critical issues, list them with fix suggestions.

## Arguments

$ARGUMENTS can be:
- `quick` - Only build + types
- `full` - All checks (default)
- `pre-commit` - Checks relevant for commits
- `pre-pr` - Full checks plus security scan
