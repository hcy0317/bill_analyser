---
name: 'common-development-workflow'
description: 'Development workflow: plan, TDD, review, commit pipeline'
applyTo: '**'
---

# Development Workflow

> This rule extends the git workflow rule with the default implementation loop that should happen before git operations.

The repository-preferred workflow is: restore context, make the smallest useful change, run the right verification for the touched paths, then review and commit.

## Default Implementation Workflow

1. **Restore Context First**
   - Read `AGENTS.md` and `docs/PROJECT_OVERVIEW.md`
   - Inspect current `git status` / `git diff`
   - If the previous session was interrupted, also inspect `.git/ai/last-session.md`

2. **Plan Proportionally**
   - Use the current agent for small, well-scoped changes
   - Use **planner** only for cross-module features, significant refactors, or ambiguous requests

3. **Drive Risky Behavior with Tests**
   - For new behavior or bug fixes, add/update the most relevant regression test before implementation
   - Use **tdd-guide** when the task benefits from an explicit RED → GREEN → REFACTOR loop

4. **Implement the Smallest Correct Change**
   - Prefer focused diffs over broad cleanup
   - Keep API contracts, money-unit conversions, and async bridge behavior explicit

5. **Verify by Path**
   - `src/bill_analyser/**` → affected pytest + pylint; business-code acceptance still requires full `./.venv/Scripts/python.exe -m pytest tests/ -v`
   - `src/web/**` → `npm run lint` and minimal build validation when needed
   - `.github/**` / `.agents/**` / `.claude/**` / `scripts/hooks/**` → `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo` plus relevant hook / health tests

6. **Review with Explicit Scope**
   - Use **code-reviewer** or other specialist reviewers only after there is a meaningful diff
   - Pass a `Review Context` bundle with `base_ref`, `head_ref`, `changed_files`, and `diff_text`

7. **Commit & Push**
   - Follow conventional commits format
   - See the git workflow rule for commit message format and PR process
