from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
PROTECTED_DIRECTORIES = (
    REPO_ROOT / '.git',
    REPO_ROOT / '.tmp' / 'ecc-unpacked',
    REPO_ROOT / 'src' / 'web' / 'node_modules',
    REPO_ROOT / '.cursor',
    REPO_ROOT / 'backup',
    REPO_ROOT / 'output',
    REPO_ROOT / 'logs',
    REPO_ROOT / 'uploads',
    REPO_ROOT / 'data' / 'logs',
    REPO_ROOT / 'data' / 'uploads',
)
DESTRUCTIVE_PROCESS_PATTERNS = (
    re.compile(r'(?i)\btaskkill\s+/f\s+/im\s+python(?:\.exe)?\b'),
    re.compile(r'(?i)\bstop-process\s+-name\s+python(?:\.exe)?\b'),
)
DANGEROUS_GIT_PATTERNS = (
    re.compile(r'(?i)\bgit\s+reset\s+--hard(?:\s|$)'),
    re.compile(r'(?i)\bgit\s+clean\s+-[^\r\n]*\bf\b[^\r\n]*\bd\b'),
    re.compile(r'(?i)\bgit\s+checkout\s+--\s+\.'),
    re.compile(r'(?i)\bgit\s+restore(?:\s+--[\w=-]+)*\s+\.'),
)
RUNTIME_DELETE_PATTERNS = (
    re.compile(r'(?i)\b(?:rm|del|erase|unlink|remove-item)\b[^\r\n]*\bdata[\\/](?:bills\.db(?:-(?:wal|shm))?|config(?:[\\/]|\b))'),
)
COMMAND_TOOL_NAMES = {'bash', 'powershell', 'shell', 'execute', 'run_in_terminal'}
WRITE_TOOL_NAMES = {'edit', 'write', 'create', 'apply_patch', 'create_file'}
DELETE_TOOL_NAMES = {'delete', 'remove', 'delete_file'}
PATH_KEYS = ('file_path', 'path', 'filePath')
PROTECTED_DIRECTORY_LABELS = {
    str((REPO_ROOT / '.git').resolve()): '.git',
    str((REPO_ROOT / '.tmp' / 'ecc-unpacked').resolve()): '.tmp/ecc-unpacked',
    str((REPO_ROOT / 'src' / 'web' / 'node_modules').resolve()): 'src/web/node_modules',
    str((REPO_ROOT / '.cursor').resolve()): '.cursor',
    str((REPO_ROOT / 'backup').resolve()): 'backup/',
    str((REPO_ROOT / 'output').resolve()): 'output/',
    str((REPO_ROOT / 'logs').resolve()): 'logs/',
    str((REPO_ROOT / 'uploads').resolve()): 'uploads/',
    str((REPO_ROOT / 'data' / 'logs').resolve()): 'data/logs/',
    str((REPO_ROOT / 'data' / 'uploads').resolve()): 'data/uploads/',
}

RUNTIME_CRITICAL_PATHS = (
    (REPO_ROOT / 'data' / 'bills.db').resolve(),
    (REPO_ROOT / 'data' / 'bills.db-shm').resolve(),
    (REPO_ROOT / 'data' / 'bills.db-wal').resolve(),
    (REPO_ROOT / 'data' / 'config').resolve(),
)


def _is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def load_payload(stdin_text: str) -> dict[str, Any] | None:
    raw_text = stdin_text.strip()
    if not raw_text:
        return None

    try:
        payload = json.loads(raw_text)
    except json.JSONDecodeError:
        return None

    return payload if isinstance(payload, dict) else None


def get_tool_name(payload: dict[str, Any]) -> str:
    raw_name = payload.get('tool_name') or payload.get('toolName') or ''
    return str(raw_name)


def get_tool_input(payload: dict[str, Any]) -> dict[str, Any]:
    if 'tool_input' in payload and isinstance(payload['tool_input'], dict):
        return payload['tool_input']

    raw_args = payload.get('toolArgs')
    if isinstance(raw_args, dict):
        return raw_args
    if isinstance(raw_args, str):
        try:
            decoded = json.loads(raw_args)
        except json.JSONDecodeError:
            return {}
        return decoded if isinstance(decoded, dict) else {}

    return {}


def extract_command_text(tool_input: dict[str, Any]) -> str:
    command = str(tool_input.get('command') or '').strip()
    args = tool_input.get('args')
    if isinstance(args, list):
        command = ' '.join(part for part in [command, *[str(item) for item in args]] if part)
    return command


def resolve_candidate_path(path_value: Any, cwd_value: Any) -> Path | None:
    raw_path = str(path_value or '').strip()
    if not raw_path:
        return None

    candidate = Path(raw_path)
    if candidate.is_absolute():
        return candidate.resolve()

    raw_cwd = str(cwd_value or '').strip()
    if raw_cwd:
        return (Path(raw_cwd) / candidate).resolve()

    return (REPO_ROOT / candidate).resolve()


def evaluate_pre_tool_use(payload: dict[str, Any]) -> str | None:
    tool_name = get_tool_name(payload).lower()
    tool_input = get_tool_input(payload)

    if tool_name in COMMAND_TOOL_NAMES:
        command_text = extract_command_text(tool_input)
        for pattern in DESTRUCTIVE_PROCESS_PATTERNS:
            if pattern.search(command_text):
                return '仓库规则禁止使用批量杀进程命令（例如 taskkill /f /im python.exe 或 Stop-Process -Name python）。请只终止当前测试或服务进程。'
        for pattern in DANGEROUS_GIT_PATTERNS:
            if pattern.search(command_text):
                return '仓库 guard hook 阻止危险 git 工作区清理命令（例如 git reset --hard、git clean -fd、git checkout -- .、git restore .）。请改为精确处理目标文件。'
        for pattern in RUNTIME_DELETE_PATTERNS:
            if pattern.search(command_text):
                return '仓库 guard hook 阻止删除 runtime 关键数据文件（如 data/bills.db、data/config）。如需重建环境，请使用 tests/.runtime 或专用脚本。'

    if tool_name in WRITE_TOOL_NAMES | DELETE_TOOL_NAMES:
        for path_key in PATH_KEYS:
            candidate_path = resolve_candidate_path(tool_input.get(path_key), payload.get('cwd'))
            if candidate_path is None:
                continue

            for protected_dir in PROTECTED_DIRECTORIES:
                resolved_protected_dir = protected_dir.resolve()
                if _is_relative_to(candidate_path, resolved_protected_dir):
                    label = PROTECTED_DIRECTORY_LABELS.get(str(resolved_protected_dir), str(resolved_protected_dir))
                    return f'仓库 guard hook 阻止直接修改受保护目录：{label}。请改运行时代码、测试夹具或仓库入口文件，不要直接编辑第三方/参考/已移除兼容层内容。'

            if tool_name in DELETE_TOOL_NAMES:
                for protected_path in RUNTIME_CRITICAL_PATHS:
                    if candidate_path == protected_path or _is_relative_to(candidate_path, protected_path):
                        return '仓库 guard hook 阻止删除 runtime 关键文件或配置目录（data/bills.db* / data/config）。如需测试隔离，请改用 tests/.runtime 或专用脚本。'

    return None


def build_hook_output(payload: dict[str, Any], reason: str) -> dict[str, Any]:
    if 'tool_name' in payload:
        return {
            'hookSpecificOutput': {
                'hookEventName': 'PreToolUse',
                'permissionDecision': 'deny',
                'permissionDecisionReason': reason,
            }
        }

    return {
        'permissionDecision': 'deny',
        'permissionDecisionReason': reason,
    }


def main() -> int:
    payload = load_payload(sys.stdin.read())
    if payload is None:
        return 0

    reason = evaluate_pre_tool_use(payload)
    if reason is None:
        return 0

    print(json.dumps(build_hook_output(payload, reason), ensure_ascii=False))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
