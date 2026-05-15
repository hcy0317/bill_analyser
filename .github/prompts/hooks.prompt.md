---
description: Inspect active repository hooks, optional global Copilot hook bridge, and why a hook did or did not fire. Use for hook visibility and diagnostics.
---

# Hooks Command

Use `/hooks` to inspect what hook surfaces are actually active for this repository.

## What This Command Checks

1. Repository hook config adapters under `.codex/hooks.json`, `.claude/settings.json`, and `.github/hooks/*.json`
2. Active command entrypoints referenced by those config files
3. Whether each referenced entrypoint exists in the current checkout
4. Optional user-level global hooks under `~/.codex/hooks.json` or `~/.copilot/hooks/`
5. Relevant static checks and validation commands

## Instructions

When invoked, do the following in order:

1. Read `.codex/hooks.json`, `.claude/settings.json`, and `.github/hooks/*.json` and summarize active stages:
   - `preToolUse`
   - `postToolUse`
   - `stop`
2. Confirm which repo scripts are wired directly.
3. If a config references `scripts/hooks/**` or another missing file, report that entry as **broken**, not active.
4. If user-level hooks are relevant, list them separately from repo-local hooks.
5. Explain the difference between:
   - automatic repo hooks
   - optional user-level hooks
   - manual diagnostics
6. If the user asks to validate behavior, run:
   - JSON parsing for the hook adapter files
   - a missing-entrypoint scan over `.codex`, `.claude`, and `.github/hooks`

## Output Format

Produce a concise report:

```text
HOOKS: [ACTIVE/PARTIAL/MISSING]

Repo PreToolUse:  [...]
Repo PostToolUse: [...]
Repo Stop:        [...]
User Hooks:       [ON/OFF]
Global Hooks:     [...]

Usable Entry Points:
- automatic native hooks
- /hooks diagnostic command

Next Step:
- [one concrete action]
```

## Notes

- `/hooks` is the discovery and diagnostics entrypoint; it does not replace automatic hooks.
- If hooks exist on disk but are not wired into `.github/hooks/*.json`, report them as **not active**.
