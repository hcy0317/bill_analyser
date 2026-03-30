---
name: 'common-agents'
description: 'Agent orchestration: available agents, parallel execution, multi-perspective analysis'
applyTo: '**'
---

# Agent Orchestration

## Available Agents

Located in `~/.claude/agents/`:

| Agent | Purpose | When to Use |
|-------|---------|-------------|
| planner | Implementation planning | Complex features, refactoring |
| architect | System design | Architectural decisions |
| tdd-guide | Test-driven development | New features, bug fixes |
| code-reviewer | Code review | After writing code |
| security-reviewer | Security analysis | Before commits |
| build-error-resolver | Fix build errors | When build fails |
| e2e-runner | E2E testing | Critical user flows |
| refactor-cleaner | Dead code cleanup | Code maintenance |
| doc-updater | Documentation | Updating docs |

## Default Agent Usage

Start with the current agent for ordinary tasks and only escalate when a specialist provides clear value:
1. Cross-module feature work, major refactors, unclear requirements - Use **planner** agent
2. Non-trivial code changes that are ready for review - Use **code-reviewer** agent with explicit Review Context
3. Bug fixes or new behavior that should be driven by tests first - Use **tdd-guide** agent
4. Architectural decision - Use **architect** agent
5. Build/lint/type failures - Use **build-error-resolver** agent

## Reviewer Invocation Contract

When invoking `code-reviewer`, `python-reviewer`, or `security-reviewer`, do not assume the subagent can recover the correct patch on its own.

Before launching a reviewer subagent, assemble and pass a compact `Review Context` block that includes:

- `base_ref`
- `head_ref`
- `changed_files`
- `diff_text` or representative patch hunks
- relevant test, lint, or scanner output when available

If the diff is too large, pass the changed file list, the most relevant hunks, and the refs needed to recover the rest. If you cannot supply either diff text or usable refs, do not launch the reviewer blindly; report that review is blocked by missing scope.

## Parallel Task Execution

Use parallel Task execution for independent read-only or clearly separable operations:

```markdown
# GOOD: Parallel execution
Launch 3 agents in parallel:
1. Agent 1: Security analysis of auth module
2. Agent 2: Performance review of cache system
3. Agent 3: Type checking of utilities

# BAD: Sequential when unnecessary
First agent 1, then agent 2, then agent 3
```

Keep stateful edits, terminal runs, and verification steps sequential when they depend on each other.

## Multi-Perspective Analysis

For complex problems, use split role sub-agents:
- Factual reviewer
- Senior engineer
- Security expert
- Consistency reviewer
- Redundancy checker
