---
name: bill-analyser
description: Claude Code bridge for the canonical Bill Analyser repository workflow skill.
---

# Bill Analyser Skill (Claude bridge)

## When to Use

- Bill import workflow changes
- Budget or statistics logic changes
- Category, account, tag, or API contract updates
- Full-stack fixes spanning Flask and Vue/TypeScript

## Canonical source

The canonical cross-tool Bill Analyser workflow skill lives in
[`../../../.agents/skills/bill-analyser-conventions/SKILL.md`](../../../.agents/skills/bill-analyser-conventions/SKILL.md).
This `.claude/skills` entry exists so Claude Code can discover the repository workflow guidance natively without requiring another tool-specific copy.

## How to Use This Bridge

- Read `AGENTS.md` first for shared repository rules.
- Then load the canonical shared skill in `.agents/skills/bill-analyser-conventions/`.
- Use `.claude/rules/` only for concise Claude-specific reminders or path-scoped overlays.
- Before ending a session that leaves any git diff behind, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title.
- Follow `AGENTS.md` for commit/PR closeout details: dash-separated subtopics when useful, multiple commits per reviewable slice, and source branch deletion after merge.
- If a session is interrupted, inspect `.git/ai/last-session.md` and resume with the shared `.agents/skills/session-resume/SKILL.md` workflow.

## Examples

- Fixing bill import preview field mapping
- Updating a REST route and the matching frontend store call
- Correcting yuan/cents conversion in budget or statistics code
