---
name: bill-analyser-conventions
description: Repository-specific conventions for Bill Analyser. Use this for import, budgeting, statistics, Rust/Python migration, and full-stack changes.
---

# Bill Analyser Conventions

This is the canonical shared Bill Analyser workflow skill for cross-tool reuse.
Tool-specific entries should stay thin and point back here.

## When to Use

Use this skill when you are:

- modifying Rust HTTP, Rust DB primitives, Flask sidecar routes, services, or SQLite access
- changing Vue/TypeScript screens or stores
- working on bill import, categories, budgets, statistics, accounts, tags, matching, learning, OCR, or LLM behavior
- validating API contracts, money-unit conversions, Rust route ownership, DB write semantics, or frontend import flow

## Core Rules

- Follow `AGENTS.md` first, then use tool-specific adapter files only for platform-native deltas.
- Read `docs/PROJECT_OVERVIEW.md` before changing import, budgeting, statistics, accounts, categories, tags, or other cross-module flows.
- Rust is the primary HTTP service. Unmigrated domains are reverse-proxied to the Python/Flask sidecar.
- Preserve the Flask-to-async bridge pattern for Python sidecar routes.
- Keep Python database code async with aiosqlite.
- Treat REST `/api/...` as the runtime API chain; do not revive `/api/v1/*`.
- Keep yuan and cents conversions explicit.
- Do not delete Python import paths until Rust route runtime, DB write semantics, frontend import flow, full coverage, and no residual references all pass together.
- After stable business behavior, API contracts, or module relationships change, update the relevant section of `docs/PROJECT_OVERVIEW.md`.
- Write `docs/PROJECT_OVERVIEW.md` as current-state business/architecture documentation, not as an update log.

## Default Execution Loop

1. Restore context from `AGENTS.md`, `docs/PROJECT_OVERVIEW.md`, and the current `git diff`.
2. Prefer focused edits unless the task clearly needs planner/reviewer/security/build-fix escalation.
3. Add or update the most relevant regression test before risky behavior changes.
4. Implement the smallest change that satisfies the verified scenario.
5. Run path-sensitive verification before moving on.

## Verification Baseline

- `src/bill_analyser/**`: first run affected pytest and pylint; before delivery run `./.venv/Scripts/python.exe -m pytest --cov=src/bill_analyser --cov-report=term-missing --cov-report=json:coverage.json --cov-fail-under=90 tests/ -v`, and require total coverage above 90% plus changed business code coverage above 90%.
- `crates/**`: run focused `cargo test`, then `cargo clippy --workspace --all-targets -- -D warnings`; business Rust changes require `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`.
- `src/web/**`: at least run `npm run lint` in `src/web`; frontend delivery also requires `npm run test:coverage` and coverage above 90%.
- `.gitea/**`: load `.agents/skills/gitea-ci-cache-discipline/SKILL.md`, then run `./.venv/Scripts/python.exe -m pytest tests/test_gitea_workflows.py -v`, YAML parsing for `.gitea/workflows/ci.yml`, and `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo`.
- `.github/**`, `.agents/**`, `.claude/**`, `.codex/**`, and `scripts/hooks/**`: run `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo` plus relevant hook or health pytest.
- API route or contract changes require checking `src/web/src/lib/services.ts` and related stores.
- Amount, statistics, and import-chain changes require manual yuan/cents and call-order review.

## Session Completion

- Before ending a session with any staged or unstaged `git diff`, invoke `zh-conventional-commit-from-diff`.
- Produce a Chinese Conventional Commit title for the current change set.
- If staged diff exists, base the title on staged diff first; otherwise use the full working diff.
- For one coherent commit that still has multiple notable subchanges, keep the subject to one line like `type(scope): main title`; put reviewable subtopics in newline body bullets instead of appending them to the subject.
- For plan-driven or OMX slices, prefer multiple small commits in one PR when rules, implementation, tests, and docs are independently reviewable.
- PR titles must use a Conventional Commit heading format like `type(scope): main title` while describing the functional-domain outcome; do not blindly copy the first commit title. PR bodies should include Summary, Test plan, and Open gates when any gate remains blocked.
- Prefer squash merge when merging a PR. After a PR is merged, delete the source feature branch through the forge auto-delete option when available, otherwise delete the remote branch after merge is confirmed.
- Do not require or automatically append `Co-authored-by: OmX <omx@oh-my-codex.dev>`; add that trailer only when the user explicitly asks for it.

## Interrupted-session Recovery

- If the session is interrupted, first inspect `.git/ai/last-session.md`.
- Compare the snapshot with current `git status` / `git diff`.
- Continue with `.agents/skills/session-resume/SKILL.md` and the `session-resume` workflow.
- Treat the snapshot as metadata only; never store secrets, environment variables, or full patch contents inside it.

## Common Commands

- `./.venv/Scripts/python.exe -m pytest tests/ -v`
- `./.venv/Scripts/python.exe -m pytest --cov=src/bill_analyser --cov-report=term-missing --cov-report=json:coverage.json --cov-fail-under=90 tests/ -v`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo llvm-cov --workspace --lcov --output-path workspace.lcov --fail-under-lines 90`
- `./start_backend.ps1`
- `./start_frontend.ps1`
