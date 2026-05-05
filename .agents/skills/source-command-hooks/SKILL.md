---
name: "source-command-hooks"
description: "Inspect active repository hooks, optional global Copilot hook bridge, and why a hook did or did not fire. Use for hook visibility and diagnostics."
---

# source-command-hooks

Use this skill when the user asks to run the migrated source command `hooks`.

## Command Template

# Hooks Command

Use `/hooks` to inspect what hook surfaces are actually active for this repository.

## What This Command Checks

1. Repository-native hooks under `.Codex/settings.json` (PreToolUse, PostToolUse, Stop)
2. Repository-native hooks under `.github/hooks/*.json` (Copilot)
3. Repo hook implementations under `scripts/hooks/*.py`
4. Optional global Copilot hook bridge via `scripts/hooks/copilot_global_hook_bridge.py`
5. Optional user-level global hooks under `~/.copilot/hooks/`
6. Relevant health checks and validation commands

## Instructions

When invoked, do the following in order:

1. Read `.Codex/settings.json` and summarize active Codex hook stages:
   - `PreToolUse`
   - `PostToolUse`
   - `Stop`
2. Read `.github/hooks/*.json` and summarize active Copilot hook stages.
3. Confirm which repo scripts are wired directly.
4. Confirm whether `scripts/hooks/copilot_global_hook_bridge.py` is wired and whether `~/.copilot/hooks/` exists.
5. If global bridge is available, list bridged hooks by stage.
6. Explain the difference between:
   - automatic repo hooks
   - optional bridged global hooks
   - manual diagnostics such as `agent_stack_health`
7. If the user asks to validate behavior, run:
   - `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo`
   - relevant hook pytest files

## Output Format

Produce a concise report:

```text
HOOKS: [ACTIVE/PARTIAL/MISSING]

Codex PreToolUse:  [...]
Codex PostToolUse: [...]
Codex Stop:        [...]
Copilot PreToolUse: [...]
Copilot PostToolUse:[...]
Copilot Stop:       [...]
Global Bridge:    [ON/OFF]
Global Hooks:     [...]

Usable Entry Points:
- automatic native hooks
- /hooks diagnostic command

Next Step:
- [one concrete action]
```

## Notes

- `/hooks` is the discovery and diagnostics entrypoint; it does not replace automatic hooks.
- If hooks exist on disk but are not wired into `.Codex/settings.json` or `.github/hooks/*.json`, report them as **not active**.
