# Bill Analyser Agent Guide

This repository uses multiple layers of agent customization:

1. Project-specific rules in [.github/copilot-instructions.md](.github/copilot-instructions.md)
2. Imported ECC catalog for reusable agents, prompts, skills, rules, hooks, and MCP configs
3. Tool-specific compatibility entry points for Cursor, Codex, and Claude Code

The ECC-derived directories listed below describe the repository's retained asset inventory. They do not imply that every upstream layer remains active in every tool. In particular, Cursor is intentionally limited to a trimmed compatibility layer.

## Precedence

- Repository-specific instructions for Bill Analyser take priority over generic ECC guidance when they conflict.
- Keep existing architecture constraints intact:
  - Flask routes bridge async services with a dedicated event loop per request.
  - Database operations stay async with aiosqlite.
  - REST is the primary runtime API chain.
  - Money units must be handled explicitly: backend stores yuan, many frontend APIs use cents.

## Imported ECC Assets

- Copilot agents: [.github/agents](.github/agents)
- Copilot prompts: [.github/prompts](.github/prompts)
- Copilot skills: [.github/skills](.github/skills)
- Copilot instructions: [.github/instructions/ecc](.github/instructions/ecc)
- Cursor agents: [.cursor/agents](.cursor/agents)
- Cursor commands: [.cursor/commands](.cursor/commands)
- Cursor skills: [.cursor/skills](.cursor/skills)
- Cursor rules: [.cursor/rules](.cursor/rules)
- Cursor MCP config: [.cursor/mcp.json](.cursor/mcp.json)
- VS Code MCP config: [.vscode/mcp.json](.vscode/mcp.json)
- Shared MCP catalog: [mcp-configs/ecc-mcp-servers.json](mcp-configs/ecc-mcp-servers.json)
- Codex config: [.codex/config.toml](.codex/config.toml), [.codex/AGENTS.md](.codex/AGENTS.md)
- Codex skills: [.agents/skills](.agents/skills)
- Claude Code entry point: [CLAUDE.md](CLAUDE.md)
- Claude Code commands, rules, skills: [.claude](.claude)

## Usage Expectations

- Use planning and TDD-oriented agents/prompts for multi-file work.
- Use security and review agents before risky refactors or merges.
- Do not enable every MCP server at once. Prefer a small set relevant to the current task.
- Keep the retained ECC layer focused on Python, TypeScript, testing, API work, and project-specific maintenance.
- Cursor users should rely on the trimmed `.cursor` layer only: agents, commands, skills, rules, and a minimal MCP config.
- Cursor hooks and the ECC runtime were intentionally removed to avoid restoring the full upstream package.
- Codex users should start from [AGENTS.md](AGENTS.md), then apply [.codex/AGENTS.md](.codex/AGENTS.md).
- Claude Code users should start from [CLAUDE.md](CLAUDE.md), then follow [AGENTS.md](AGENTS.md) and [.github/copilot-instructions.md](.github/copilot-instructions.md).

## Project-Specific Reminder

When editing this repository, always read and follow [.github/copilot-instructions.md](.github/copilot-instructions.md).