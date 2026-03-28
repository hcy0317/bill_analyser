---
name: bill-analyser
description: Repository-specific guidance for Bill Analyser workflows in Claude Code.
---

# Bill Analyser Skill

## When to Use

- Bill import workflow changes
- Budget or statistics logic changes
- Category, account, tag, or API contract updates
- Full-stack fixes spanning Flask and Vue/TypeScript

## How It Works

- Start from the repository entry files: CLAUDE.md, AGENTS.md, and .github/copilot-instructions.md.
- Preserve repository-specific architecture rather than generic ECC defaults.
- Validate changes with focused tests and keep edits narrow.
- Before ending a session that leaves any git diff behind, invoke `zh-conventional-commit-from-diff` and produce a Chinese Conventional Commit title.

## Examples

- Fixing bill import preview field mapping
- Updating a REST route and the matching frontend store call
- Correcting yuan/cents conversion in budget or statistics code