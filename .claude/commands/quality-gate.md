---
description: Quick path-scoped quality check for one file or folder. Prefer /verify for comprehensive repository-aware verification.
---

# Quality Gate Command

Run a lightweight, path-scoped quality check on demand.

## Usage

`/quality-gate [path|.] [--fix] [--strict]`

- default target: current directory (`.`)
- `--fix`: allow auto-format/fix where configured
- `--strict`: fail on warnings where supported

## Positioning

- Use `/quality-gate` for **quick local feedback** on one file or folder.
- Use `/verify` for **repository-default comprehensive verification** before handoff, review, or PR.
- `verification-loop` is the underlying deep workflow; most day-to-day usage should prefer `/verify` rather than invoking overlapping verification entrypoints mentally.

## Pipeline

1. Detect language/tooling for target.
2. Run formatter checks.
3. Run lint/type checks when available.
4. Produce a concise remediation list.

## Notes

This command mirrors hook behavior but stays intentionally narrower than `/verify`:

- it does **not** replace full pytest audit acceptance
- it does **not** replace API contract or yuan/cents manual review
- it does **not** replace AI customization health checks for `.github/**`, `.agents/**`, `.claude/**`, or `scripts/hooks/**`

## Arguments

$ARGUMENTS:
- `[path|.]` optional target path
- `--fix` optional
- `--strict` optional
