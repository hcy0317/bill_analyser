"""Regression tests for the repository pre-tool guard hook."""

from __future__ import annotations

# pylint: disable=missing-function-docstring,too-many-lines

import importlib
import json
import subprocess
from pathlib import Path
from unittest.mock import patch

import pytest


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


@pytest.mark.parametrize(
    'command',
    (
        'git reset --hard HEAD',
        'git -C . reset --hard HEAD',
        'git clean -fd',
        'git clean -df',
        'git -C . clean -fd',
        'git checkout .',
        'git checkout -- ./',
        'git checkout HEAD .',
        'git checkout -- .',
        'git restore -s HEAD .',
        'git restore ./',
        'git restore -- .',
        'git restore .',
    ),
)
def test_dangerous_git_command_is_denied(command: str) -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': command,
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'git' in reason.lower()


def test_git_clean_dry_run_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'git clean -n fixtures/',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


@pytest.mark.parametrize(
    'command',
    (
        'rm data/bills.db',
        'del data/config',
        'rm data/config other_path',
    ),
)
def test_runtime_delete_command_is_denied(command: str) -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': command,
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'runtime' in reason.lower() or 'data/config' in reason


def test_runtime_delete_parent_directory_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'rm -rf data/',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert (
        'runtime' in reason.lower()
        or 'data/config' in reason
        or '受保护目录' in reason
    )


def test_windows_style_command_path_resolves_under_repo_cross_platform() -> None:
    resolved = guard.resolve_candidate_path(
        'output\\guard-test.txt',
        str(guard.REPO_ROOT),
    )

    assert resolved == (guard.REPO_ROOT / 'output' / 'guard-test.txt').resolve()
    assert guard.format_display_path(
        guard.resolve_candidate_path('bills\\fixture.txt', str(guard.REPO_ROOT))
    ) == 'bills/fixture.txt'


def test_windows_absolute_command_path_is_outside_repo_cross_platform() -> None:
    resolved = guard.resolve_candidate_path('C:\\temp\\input.txt', str(guard.REPO_ROOT))

    assert resolved is not None
    assert guard.outside_repo_denial_reason(resolved) is not None


def test_unc_like_command_path_is_outside_repo_cross_platform() -> None:
    resolved = guard.resolve_candidate_path(
        '\\\\server\\share\\input.txt',
        str(guard.REPO_ROOT),
    )

    assert resolved is not None
    assert guard.outside_repo_denial_reason(resolved) is not None


def test_path_token_normalization_preserves_posix_absolute_paths_and_urls() -> None:
    assert guard.normalize_path_token_text('/tmp/name\\with-backslash.txt') == (
        '/tmp/name\\with-backslash.txt'
    )
    assert guard.normalize_path_token_text('https://example.test/a\\b') == (
        'https://example.test/a\\b'
    )


def test_command_tool_write_to_protected_directory_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Set-Content output\\guard-test.txt blocked',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'output/' in reason or '受保护目录' in reason


def test_command_tool_write_outside_repo_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Out-File -FilePath ..\\outside.txt -InputObject blocked',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '仓库外路径' in reason


def test_command_tool_move_to_protected_directory_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Move-Item src\\safe.txt .git\\config',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_command_tool_call_operator_bypass_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': '& Set-Content output\\blocked.txt x',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'output/' in reason or '受保护目录' in reason


def test_command_tool_value_flag_outside_repo_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Set-Content -Value blocked ..\\outside.txt',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '仓库外路径' in reason


def test_command_tool_inputobject_protected_path_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Out-File -InputObject blocked .git\\config',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_command_tool_delete_gitignored_path_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Remove-Item bills\\fixture.txt',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills/fixture.txt' in reason


def test_command_tool_numbered_redirection_to_protected_directory_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'echo blocked 1> output\\blocked.txt',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'output/' in reason or '受保护目录' in reason


def test_command_tool_write_target_and_redirection_both_checked() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Set-Content .git\\config blocked > nul',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_nested_powershell_wrapper_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -Command "Set-Content output\\bypass.txt blocked"',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '嵌套 shell/interpreter 包装器' in reason


def test_encoded_powershell_wrapper_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -EncodedCommand ZQBjAGgAbwAgAHgA',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '嵌套 shell/interpreter 包装器' in reason


def test_abbreviated_encoded_powershell_wrapper_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -EncodedCom ZQBjAGgAbwAgAHgA',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '嵌套 shell/interpreter 包装器' in reason


def test_invoke_expression_wrapper_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'IEX "Set-Content .git\\config blocked"',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '嵌套 shell/interpreter 包装器' in reason


def test_inline_python_interpreter_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': (
                'python -c "from pathlib import Path; '
                'Path(\'.git/guard-bypass.txt\').write_text(\'x\')"'
            ),
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '内联解释器' in reason


def test_inline_python_module_execution_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'python -m zipfile -e payload.zip output\\',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '内联解释器' in reason


def test_safe_python_module_execution_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'python -m pytest tests\\test_pre_tool_repo_guard.py',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_safe_repo_script_execution_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'python scripts\\agent_stack_health.py --mode repo',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_documented_repo_start_script_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': '.\\start_backend.ps1',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_command_tool_copy_from_outside_repo_to_repo_target_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Copy-Item C:\\temp\\input.txt src\\bill_analyser\\parsers\\factory.py',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '仓库外路径' in reason


def test_command_tool_path_first_value_form_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Set-Content output\\guard-test.txt -Value blocked',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'output/' in reason or '受保护目录' in reason


def test_command_tool_dev_null_redirection_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'echo blocked > /dev/null',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_command_tool_path_first_inputobject_form_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Out-File .git\\config -InputObject blocked',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_command_tool_compound_prefix_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Get-Date; Set-Content output\\blocked.txt x',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '链式/管道分隔符' in reason


def test_wrapped_dangerous_git_clean_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -Command "git clean -fd"',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '危险 git 工作区清理命令' in reason


def test_wrapped_git_path_mutation_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -Command "git add -f bills\\fixture.txt"',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '嵌套 shell/interpreter 包装器' in reason


def test_command_tool_directory_creation_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'New-Item -ItemType Directory .git\\test-dir',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_command_tool_alias_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'sc .git\\config x',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_external_script_execution_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -File scripts\\danger.ps1 output\\blocked.txt',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '外部脚本文件' in reason


def test_direct_script_execution_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': '& .\\scripts\\danger.ps1 .git\\config',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '外部脚本文件' in reason


def test_benign_wrapped_command_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'powershell -Command "Get-ChildItem"',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_benign_inline_module_command_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'python -m pip --version',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_benign_direct_script_help_is_allowed() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': '& .\\scripts\\danger.ps1 --help',
        },
    }

    assert guard.evaluate_pre_tool_use(payload) is None


def test_variable_path_write_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': 'Set-Content $PWD\\.git\\config blocked',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'shell 变量' in reason


def test_expression_path_write_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': "Set-Content ([IO.Path]::Combine('.git','guard-bypass.txt')) x",
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '表达式生成的路径参数' in reason


def test_powershell_expression_mutation_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'powershell',
        'tool_input': {
            'command': "[System.IO.File]::WriteAllText('.git/guard-bypass.txt','x')",
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'PowerShell/.NET 表达式' in reason


def test_git_move_into_protected_directory_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=1,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'git mv AGENTS.md output\\AGENTS.md',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'output/' in reason or '受保护目录' in reason


def test_git_add_force_ignored_target_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'git add -f bills\\fixture.txt',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills/fixture.txt' in reason


def test_git_apply_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'bash',
        'tool_input': {
            'command': 'git apply --unsafe-paths ..\\outside.patch',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'git apply' in reason


def test_safe_runtime_edit_is_allowed() -> None:
    fake_git_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'Write',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'src' / 'bill_analyser' / 'parsers' / 'factory.py'),
            'content': 'safe change',
        },
    }

    with patch.object(guard, 'run_git', return_value=fake_git_result):
        assert guard.evaluate_pre_tool_use(payload) is None


def test_gitignored_path_write_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'Write',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'bills' / 'fixture.txt'),
            'content': 'ignored change',
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'ignore' in reason
    assert '.gitignore' in reason
    assert 'git check-ignore -v -- bills/fixture.txt' in reason
    assert 'git ls-files -- bills/fixture.txt' in reason


def test_write_outside_repo_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'Write',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT.parent / 'outside.txt'),
            'content': 'outside change',
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '仓库外路径' in reason


def test_delete_tool_denies_runtime_critical_db() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'delete_file',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'data' / 'bills.db'),
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills.db' in reason


def test_delete_tool_denies_runtime_parent_directory() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'delete_file',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'data'),
        },
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert (
        'data/config' in reason
        or 'runtime' in reason.lower()
        or '受保护目录' in reason
    )


def test_delete_tool_denies_gitignored_path() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'delete_file',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'bills' / 'fixture.txt'),
        },
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills/fixture.txt' in reason


def test_all_candidate_paths_are_checked() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'Write',
        'tool_input': {
            'file_path': str(guard.REPO_ROOT / 'src' / 'bill_analyser' / 'parsers' / 'factory.py'),
            'path': str(guard.REPO_ROOT / 'bills' / 'fixture.txt'),
            'content': 'ignored change',
        },
    }
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=0,
        stdout='',
        stderr='',
    )
    untracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )

    with patch.object(
        guard,
        'run_git',
        side_effect=[tracked_result, untracked_result, ignored_result],
    ):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills/fixture.txt' in reason


def test_apply_patch_gitignored_target_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'apply_patch',
        'toolArgs': (
            '*** Begin Patch\n'
            '*** Add File: bills/fixture.txt\n'
            '+ignored change\n'
            '*** End Patch\n'
        ),
    }

    with patch.object(guard, 'run_git', side_effect=[tracked_result, ignored_result]):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills/fixture.txt' in reason


def test_apply_patch_move_to_gitignored_target_is_denied() -> None:
    tracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=0,
        stdout='',
        stderr='',
    )
    untracked_result = subprocess.CompletedProcess(
        args=['git', 'ls-files'],
        returncode=1,
        stdout='',
        stderr='',
    )
    ignored_result = subprocess.CompletedProcess(
        args=['git', 'check-ignore'],
        returncode=0,
        stdout='',
        stderr='',
    )
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'apply_patch',
        'toolArgs': (
            '*** Begin Patch\n'
            '*** Update File: src/bill_analyser/parsers/factory.py\n'
            '*** Move to: bills/fixture.txt\n'
            '@@\n'
            '-old\n'
            '+new\n'
            '*** End Patch\n'
        ),
    }

    with patch.object(
        guard,
        'run_git',
        side_effect=[tracked_result, untracked_result, ignored_result],
    ):
        reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert 'bills/fixture.txt' in reason


def test_apply_patch_move_to_protected_target_is_denied() -> None:
    payload = {
        'cwd': str(guard.REPO_ROOT),
        'tool_name': 'apply_patch',
        'toolArgs': (
            '*** Begin Patch\n'
            '*** Update File: src/bill_analyser/parsers/factory.py\n'
            '*** Move to: .git/config\n'
            '@@\n'
            '-old\n'
            '+new\n'
            '*** End Patch\n'
        ),
    }

    reason = guard.evaluate_pre_tool_use(payload)

    assert reason is not None
    assert '.git' in reason


def test_hook_configs_share_the_same_repo_guard_script() -> None:
    repo_root = guard.REPO_ROOT
    github_hooks = json.loads(
        (repo_root / '.github' / 'hooks' / 'repo-guard.json').read_text(
            encoding='utf-8'
        )
    )
    claude_settings = json.loads(
        (repo_root / '.claude' / 'settings.json').read_text(encoding='utf-8')
    )

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

    assert any(
        'scripts/hooks/pre_tool_repo_guard.py' in str(command)
        for command in github_commands
    )
    assert any(
        'scripts/hooks/pre_tool_repo_guard.py' in str(command)
        for command in claude_commands
    )
    assert any(
        'python -X utf8 scripts/hooks/pre_tool_repo_guard.py' in str(command)
        for command in claude_commands
    )


def test_hook_configs_also_wire_global_bridge_for_pre_tool() -> None:
    repo_root = guard.REPO_ROOT
    github_hooks = json.loads(
        (repo_root / '.github' / 'hooks' / 'repo-guard.json').read_text(
            encoding='utf-8'
        )
    )
    claude_settings = json.loads(
        (repo_root / '.claude' / 'settings.json').read_text(encoding='utf-8')
    )

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

    assert any(
        'scripts/hooks/copilot_global_hook_bridge.py pre-tool' in str(command)
        for command in github_commands
    )
    assert any(
        'scripts/hooks/copilot_global_hook_bridge.py pre-tool' in str(command)
        for command in claude_commands
    )


def test_claude_pre_tool_matcher_covers_create() -> None:
    repo_root = guard.REPO_ROOT
    claude_settings = json.loads(
        (repo_root / '.claude' / 'settings.json').read_text(encoding='utf-8')
    )

    matchers = [
        entry.get('matcher')
        for entry in claude_settings['hooks']['PreToolUse']
        if isinstance(entry, dict)
    ]

    assert any('Create' in str(matcher) for matcher in matchers)


def test_claude_pre_tool_matcher_covers_powershell_and_delete_tools() -> None:
    repo_root = guard.REPO_ROOT
    claude_settings = json.loads(
        (repo_root / '.claude' / 'settings.json').read_text(encoding='utf-8')
    )

    matchers = [
        str(entry.get('matcher'))
        for entry in claude_settings['hooks']['PreToolUse']
        if isinstance(entry, dict)
    ]

    assert any('Bash' in matcher for matcher in matchers)
    assert any('PowerShell' in matcher for matcher in matchers)
    assert any('Shell' in matcher for matcher in matchers)
    assert any('Execute' in matcher for matcher in matchers)
    assert any('run_in_terminal' in matcher for matcher in matchers)
    assert any('Delete' in matcher for matcher in matchers)
    assert any('delete_file' in matcher for matcher in matchers)
    assert any('apply_patch' in matcher for matcher in matchers)
    assert any('create_file' in matcher for matcher in matchers)
