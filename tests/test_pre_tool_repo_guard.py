from __future__ import annotations

import importlib
import json
from pathlib import Path


guard = importlib.import_module('scripts.hooks.pre_tool_repo_guard')


def test_copilot_payload_denies_banned_python_kill_command() -> None:
    payload = {
        'cwd': str(Path.cwd()),
        'toolName': 'powershell',
        'toolArgs': '{"command":"taskkill /f /im python.exe","description":"Kill python"}',
    }

    reason = guard.evaluate_pre_tool_use(payload)
    decision = guard.build_hook_output(payload, reason)

    assert reason is not None
    assert 'taskkill /f /im python.exe' in reason
    assert decision['permissionDecision'] == 'deny'


def test_claude_payload_denies_editing_protected_reference_tree() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'Edit',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / '.tmp' / 'ecc-unpacked' / 'fixture.txt'),
            'old_string': 'before',
            'new_string': 'after',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)
    decision = guard.build_hook_output(payload, reason)

    assert reason is not None
    assert '.tmp/ecc-unpacked' in reason
    assert decision['hookSpecificOutput']['permissionDecision'] == 'deny'
    assert decision['hookSpecificOutput']['hookEventName'] == 'PreToolUse'


def test_safe_runtime_edit_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'Write',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'src' / 'bill_analyser' / 'parsers' / 'factory.py'),
            'content': 'safe change',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_hook_configs_share_the_same_repo_guard_script() -> None:
    repo_root = guard.REPO_ROOT
    github_hooks = json.loads((repo_root / '.github' / 'hooks' / 'repo-guard.json').read_text(encoding='utf-8'))
    claude_settings = json.loads((repo_root / '.claude' / 'settings.json').read_text(encoding='utf-8'))

    github_commands = [
        entry.get('bash')
        for entry in github_hooks['hooks']['preToolUse']
        if isinstance(entry, dict)
    ] + [
        entry.get('powershell')
        for entry in github_hooks['hooks']['preToolUse']
        if isinstance(entry, dict)
    ]
    claude_commands = [
        hook.get('command')
        for entry in claude_settings['hooks']['PreToolUse']
        if isinstance(entry, dict)
        for hook in entry.get('hooks', [])
        if isinstance(hook, dict)
    ]

    assert any('scripts/hooks/pre_tool_repo_guard.py' in str(command) for command in github_commands)
    assert any('scripts/hooks/pre_tool_repo_guard.py' in str(command) for command in claude_commands)


def test_hook_configs_also_wire_global_bridge_for_pre_tool() -> None:
    repo_root = guard.REPO_ROOT
    github_hooks = json.loads((repo_root / '.github' / 'hooks' / 'repo-guard.json').read_text(encoding='utf-8'))
    claude_settings = json.loads((repo_root / '.claude' / 'settings.json').read_text(encoding='utf-8'))

    github_commands = [
        entry.get('bash')
        for entry in github_hooks['hooks']['preToolUse']
        if isinstance(entry, dict)
    ] + [
        entry.get('powershell')
        for entry in github_hooks['hooks']['preToolUse']
        if isinstance(entry, dict)
    ]
    claude_commands = [
        hook.get('command')
        for entry in claude_settings['hooks']['PreToolUse']
        if isinstance(entry, dict)
        for hook in entry.get('hooks', [])
        if isinstance(hook, dict)
    ]

    assert any('scripts/hooks/copilot_global_hook_bridge.py pre-tool' in str(command) for command in github_commands)
    assert any('scripts/hooks/copilot_global_hook_bridge.py pre-tool' in str(command) for command in claude_commands)
