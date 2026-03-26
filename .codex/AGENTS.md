# Bill Analyser Codex Guide

This file supplements [AGENTS.md](AGENTS.md) with Codex-specific guidance.

## Start Order

1. Read [AGENTS.md](AGENTS.md).
2. Read [.github/copilot-instructions.md](.github/copilot-instructions.md).
3. Use this file only for Codex-specific runtime details.

## Recommended Focus

- Prefer Python, TypeScript, testing, API review, and documentation tasks.
- Use the repository skill in [.agents/skills/bill-analyser-conventions](.agents/skills/bill-analyser-conventions).
- Keep Codex work grounded in the current repository architecture rather than generic ECC defaults.

## MCP Baseline

The project-local Codex baseline is defined in [.codex/config.toml](.codex/config.toml).
It keeps a small MCP set aligned with the repository's VS Code setup.

## Multi-Agent Roles

- `.codex/agents/explorer.toml` for read-only evidence gathering
- `.codex/agents/reviewer.toml` for correctness and regression review
- `.codex/agents/docs-researcher.toml` for API and docs verification