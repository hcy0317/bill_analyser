# Bill Analyser Agent Guide

This file is the cross-tool entry map for agent assets. The canonical workspace-level project rules live in [.github/copilot-instructions.md](.github/copilot-instructions.md).

This repository uses multiple layers of agent customization:

1. Project-specific rules in [.github/copilot-instructions.md](.github/copilot-instructions.md)
2. Imported ECC catalog for reusable agents, prompts, skills, rules, hooks, and MCP configs
3. Tool-specific compatibility entry points for Cursor, Codex, and Claude Code

The ECC-derived directories listed below describe the repository's retained asset inventory. They do not imply that every upstream layer remains active in every tool. In particular, Cursor is intentionally limited to a trimmed compatibility layer.

## Precedence

- Repository-specific instructions for Bill Analyser take priority over generic ECC guidance when they conflict.
- Apply [.github/copilot-instructions.md](.github/copilot-instructions.md) first.
- Use this file as the asset map and tool-entry index, then load tool-specific files only when they add something the workspace instructions do not already define.

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
- After applying [.github/copilot-instructions.md](.github/copilot-instructions.md), Cursor users should rely on the trimmed `.cursor` layer only for tool-specific compatibility behavior.
- Cursor hooks and the ECC runtime were intentionally removed to avoid restoring the full upstream package.
- Codex users should apply [.github/copilot-instructions.md](.github/copilot-instructions.md) first, then [.codex/AGENTS.md](.codex/AGENTS.md).
- Claude Code users should apply [.github/copilot-instructions.md](.github/copilot-instructions.md) first, then [CLAUDE.md](CLAUDE.md) for Claude-specific additions.

## Reviewer Diff Contract

Across Copilot, Cursor, and Claude-compatible reviewer assets, treat review scope as an explicit input instead of an implicit side effect.

- Pass a compact `Review Context` bundle with `base_ref`, `head_ref`, `changed_files`, and `diff_text` (or representative hunks) whenever invoking `code-reviewer`, `python-reviewer`, or `security-reviewer`.
- If the diff is too large, pass the critical hunks plus refs that let the reviewer recover the rest.
- If neither diff text nor usable refs are available, the reviewer should stop and report a blocked scope instead of pretending to review the whole repository.

## Practical Rule

- Put project-wide coding behavior in [.github/copilot-instructions.md](.github/copilot-instructions.md).
- Use this file to explain where agent assets live and which tool-specific entrypoints matter.
- Prefer linking to existing docs instead of copying project rules into multiple agent files.