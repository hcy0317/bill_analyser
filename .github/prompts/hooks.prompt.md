---
description: Inspect active repository hooks, optional global Copilot hook bridge, and why a hook did or did not fire. Use for hook visibility and diagnostics.
---

# Hooks Command

Use `/hooks` to inspect what hook surfaces are actually active for this repository.

## What This Command Checks

1. Repository-native hooks under `.github/hooks/*.json`
2. Repo hook implementations under `scripts/hooks/*.py`
3. Optional global Copilot hook bridge via `scripts/hooks/copilot_global_hook_bridge.py`
4. Optional user-level global hooks under `~/.copilot/hooks/`
5. Relevant health checks and validation commands

## Instructions

When invoked, do the following in order:

1. Read `.github/hooks/*.json` and summarize active stages:
   - `preToolUse`
   - `postToolUse`
   - `stop`
2. Confirm which repo scripts are wired directly.
3. Confirm whether `scripts/hooks/copilot_global_hook_bridge.py` is wired and whether `~/.copilot/hooks/` exists.
4. If global bridge is available, list bridged hooks by stage.
5. Explain the difference between:
   - automatic repo hooks
   - optional bridged global hooks
   - manual diagnostics such as `agent_stack_health`
6. If the user asks to validate behavior, run:
   - `./.venv/Scripts/python.exe scripts/agent_stack_health.py --mode repo`
   - relevant hook pytest files

## Output Format

Produce a concise report:

```text
HOOKS: [ACTIVE/PARTIAL/MISSING]

Repo PreToolUse:  [...]
Repo PostToolUse: [...]
Repo Stop:        [...]
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
- If hooks exist on disk but are not wired into `.github/hooks/*.json`, report them as **not active**.