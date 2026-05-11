---
name: bill-analyser
description: Codex bridge for the canonical Bill Analyser repository workflow skill.
---

# Bill Analyser Skill (Codex bridge)

## When to Use

- Bill import workflow changes
- Budget or statistics logic changes
- Category, account, tag, or API contract updates
- Full-stack fixes spanning Flask and Vue/TypeScript

## Canonical source

The canonical cross-tool Bill Analyser workflow skill lives in
[`../../../.agents/skills/bill-analyser-conventions/SKILL.md`](../../../.agents/skills/bill-analyser-conventions/SKILL.md).
This `.agents/skills` entry exists so Codex can discover the repository workflow guidance natively without requiring another tool-specific copy.

## How to Use This Bridge

- Read `AGENTS.md` first for shared repository rules.
- Then load the canonical shared skill in `.agents/skills/bill-analyser-conventions/`.
- Use `.codex/` only for concise Codex-specific runtime adapters or path-scoped overlays.
- Before ending a session that leaves any git diff behind, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title.
- Follow `AGENTS.md` for commit/PR closeout details: keep commit subjects one line, put subtopics in newline body bullets, prefer multiple commits per reviewable slice, require PR titles to use a `type(scope): title` heading while describing the domain outcome, prefer squash merge, delete the source branch after merge, and never force an OmX co-author trailer.
- If a session is interrupted, inspect `.git/ai/last-session.md` and resume with the shared `.agents/skills/session-resume/SKILL.md` workflow.

## Examples

- Fixing bill import preview field mapping
- Updating a REST route and the matching frontend store call
- Correcting yuan/cents conversion in budget or statistics code
