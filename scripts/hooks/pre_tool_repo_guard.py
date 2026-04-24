"""Repository pre-tool guard for destructive or out-of-policy actions."""

# pylint: disable=too-many-lines

from __future__ import annotations

from collections.abc import Callable
import json
import re
import shlex
import subprocess
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
GIT_SUBPROCESS_TIMEOUT_S = 5
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
    re.compile(
        r'(?i)\bgit\s+clean\b(?=[^\r\n]*(?:-[a-zA-Z]*f[a-zA-Z]*|--force\b))'
    ),
    re.compile(r'(?i)\bgit\s+checkout\b(?:\s+(?!\.(?:\s|$))\S+)*\s+\.(?:\s|$)'),
    re.compile(r'(?i)\bgit\s+restore\b(?:\s+(?!\.(?:\s|$))\S+)*\s+\.(?:\s|$)'),
)
GIT_MUTATION_SIGNAL_PATTERN = re.compile(r'(?i)\bgit\s+(?:add|apply|mv|rm)\b')
RUNTIME_DELETE_PATTERNS = (
    re.compile(
        r'(?i)\b(?:rm|del|erase|unlink|remove-item)\b[^\r\n]*\bdata'
        r'(?:[\\/](?:bills\.db(?:-(?:wal|shm))?|config(?:[\\/]|(?=\s|$)))|[\\/]?(?=\s|$))'
    ),
)
URL_SCHEME_PATTERN = re.compile(r'^[A-Za-z][A-Za-z0-9+.-]*://')
WINDOWS_ABSOLUTE_PATH_PATTERN = re.compile(r'^[A-Za-z]:[\\/]')
UNC_PATH_PATTERN = re.compile(r'^(?:\\\\|//)[^\\/]+[\\/][^\\/]+')
COMMAND_TOOL_NAMES = {'bash', 'powershell', 'shell', 'execute', 'run_in_terminal'}
WRITE_TOOL_NAMES = {'edit', 'write', 'create', 'apply_patch', 'create_file'}
DELETE_TOOL_NAMES = {'delete', 'remove', 'delete_file'}
PATH_KEYS = ('file_path', 'path', 'filePath')
DELETE_COMMAND_VERBS = {'rm', 'del', 'erase', 'unlink', 'remove-item'}
MOVE_COMMAND_VERBS = {'mv', 'move', 'move-item', 'ren', 'rename', 'rename-item'}
COPY_COMMAND_VERBS = {'cp', 'copy', 'copy-item'}
WRITE_COMMAND_VERBS = {
    'add-content',
    'mkdir',
    'new-item',
    'ni',
    'out-file',
    'set-content',
    'touch',
}
COMMAND_PATH_MUTATION_VERBS = (
    DELETE_COMMAND_VERBS | MOVE_COMMAND_VERBS | COPY_COMMAND_VERBS | WRITE_COMMAND_VERBS
)
COMMAND_PATH_FLAGS = {'-path', '-literalpath'}
COMMAND_FILE_PATH_FLAGS = {'-filepath'}
COMMAND_DESTINATION_FLAGS = {'-destination', '-outfile', '-newname'}
COMMAND_VALUE_FLAGS = {'-value', '-inputobject'}
COMMAND_FLAGS_WITH_VALUES = (
    COMMAND_PATH_FLAGS
    | COMMAND_FILE_PATH_FLAGS
    | COMMAND_DESTINATION_FLAGS
    | COMMAND_VALUE_FLAGS
    | {'-file', '-itemtype'}
)
REDIRECTION_TARGET_PATTERN = re.compile(
    r'(?<!\w)(?:\d+)?>>?\s*(?P<path>"[^"]+"|\'[^\']+\'|\S+)'
)
COMPLEX_COMMAND_SEPARATOR_PATTERN = re.compile(r'&&|\|\||[;\n]|(?<!\|)\|(?!\|)')
COMMAND_MUTATION_MARKER_PATTERN = re.compile(
    r'(?i)\b(?:'
    r'add-content|copy|copy-item|cp|del|erase|md|mkdir|move|move-item|mv|'
    r'new-item|ni|out-file|rd|remove-item|ren|rename|rename-item|ri|rm|'
    r'rmdir|sc|set-content|touch|unlink'
    r')\b'
)
GIT_GLOBAL_FLAGS_WITH_VALUE = {
    '-c',
    '-C',
    '--config-env',
    '--exec-path',
    '--git-dir',
    '--namespace',
    '--super-prefix',
    '--work-tree',
}
GIT_MUTATING_SUBCOMMANDS = {'add', 'apply', 'mv', 'rm'}
INLINE_WRAPPER_VERBS = {
    'bash',
    'cmd',
    'cmd.exe',
    'iex',
    'invoke-expression',
    'powershell',
    'powershell.exe',
    'pwsh',
    'pwsh.exe',
    'sh',
}
INLINE_INTERPRETER_VERBS = {
    'node',
    'node.exe',
    'py',
    'python',
    'python.exe',
    'python3',
    'python3.exe',
}
SCRIPT_FILE_SUFFIXES = {
    '.bat',
    '.cmd',
    '.js',
    '.mjs',
    '.cjs',
    '.ps1',
    '.py',
    '.sh',
}
COMMAND_VERB_ALIASES = {
    'md': 'mkdir',
    'rd': 'remove-item',
    'ri': 'remove-item',
    'rmdir': 'remove-item',
    'sc': 'set-content',
}
SAFE_PYTHON_MODULES = {'pylint', 'pytest'}
SAFE_SCRIPT_EXECUTION_PATHS = {
    (REPO_ROOT / 'start_backend.ps1').resolve(),
    (REPO_ROOT / 'start_frontend.ps1').resolve(),
    (REPO_ROOT / '一键启动.ps1').resolve(),
    (REPO_ROOT / '停止服务器.ps1').resolve(),
    (REPO_ROOT / 'scripts' / 'agent_stack_health.py').resolve(),
}
POWERSHELL_EXPRESSION_MUTATION_PATTERN = re.compile(
    r'(?i)\[(?:system\.)?io\.(?:directory|file)\]::'
    r'(?:appendalltext|copy|create|delete|move|writeall(?:bytes|lines|text)|writelines)'
)
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
    """Parse the hook payload from stdin when the host provides JSON."""
    raw_text = stdin_text.strip()
    if not raw_text:
        return None

    try:
        payload = json.loads(raw_text)
    except json.JSONDecodeError:
        return None

    return payload if isinstance(payload, dict) else None


def get_tool_name(payload: dict[str, Any]) -> str:
    """Extract the normalized tool name field used by supported hosts."""
    raw_name = payload.get('tool_name') or payload.get('toolName') or ''
    return str(raw_name)


def get_tool_input(payload: dict[str, Any]) -> dict[str, Any]:
    """Extract tool arguments from Claude/Copilot payload variants."""
    if 'tool_input' in payload and isinstance(payload['tool_input'], dict):
        return payload['tool_input']

    raw_args = payload.get('toolArgs')
    if isinstance(raw_args, dict):
        return raw_args
    if isinstance(raw_args, str):
        try:
            decoded = json.loads(raw_args)
        except json.JSONDecodeError:
            is_apply_patch = get_tool_name(payload).lower() == 'apply_patch'
            if is_apply_patch and raw_args.lstrip().startswith('*** Begin Patch'):
                return {'patch': raw_args}
            return {}
        return decoded if isinstance(decoded, dict) else {}

    return {}


def extract_command_text(tool_input: dict[str, Any]) -> str:
    """Flatten command plus args into one string for regex checks."""
    command = str(tool_input.get('command') or '').strip()
    args = tool_input.get('args')
    if isinstance(args, list):
        command = ' '.join(part for part in [command, *[str(item) for item in args]] if part)
    return command


def strip_wrapping_quotes(value: str) -> str:
    """Remove one layer of matching single or double quotes."""
    text = str(value).strip()
    if len(text) >= 2 and text[0] == text[-1] and text[0] in {'"', "'"}:
        return text[1:-1]
    return text


def tokenize_command_text(command_text: str) -> tuple[str, ...]:
    """Tokenize a command string conservatively for path inspection."""
    try:
        return tuple(shlex.split(command_text, posix=False))
    except ValueError:
        return ()


def unwrap_command_tokens(tokens: tuple[str, ...]) -> tuple[str, ...]:
    """Drop leading wrapper tokens such as the PowerShell call operator."""
    index = 0
    while index < len(tokens) and tokens[index] == '&':
        index += 1
    return tokens[index:]


def normalize_command_verb(token: str) -> str:
    """Normalize a command token to its lowercase executable/cmdlet name."""
    normalized = Path(strip_wrapping_quotes(token)).name.lower()
    return COMMAND_VERB_ALIASES.get(normalized, normalized)


def is_option_token(token: str) -> bool:
    """Return whether a token looks like an option rather than a path."""
    if token.startswith('--'):
        return True
    if token.startswith('-') and len(token) > 1:
        return True
    if token.startswith('/') and len(token) > 1:
        suffix = token[1:]
        if '/' in suffix or '\\' in suffix or ':' in suffix or suffix.startswith('.'):
            return False
        return True
    return False


def command_has_any_flag(tokens: tuple[str, ...], flags: set[str]) -> bool:
    """Return whether any of the provided flags is present in the token stream."""
    for token in tokens:
        lower_token = token.lower()
        if lower_token in flags:
            return True
        if any(lower_token.startswith(f'{flag}=') for flag in flags):
            return True
    return False


def is_powershell_inline_flag(token: str) -> bool:
    """Return whether a PowerShell flag token invokes inline command execution."""
    lower_token = token.lower()
    return (
        lower_token in {'-c', '-enc'}
        or lower_token.startswith('-com')
        or lower_token.startswith('-encodedcom')
    )


def has_inline_wrapper_command(tokens: tuple[str, ...]) -> bool:
    """Return whether tokens launch another shell/interpreter with inline code."""
    if not tokens:
        return False
    verb = normalize_command_verb(tokens[0])
    if verb not in INLINE_WRAPPER_VERBS:
        return False

    if verb in {'iex', 'invoke-expression'}:
        return True
    lower_tokens = tuple(token.lower() for token in tokens[1:])
    if verb in {'powershell', 'powershell.exe', 'pwsh', 'pwsh.exe'}:
        return any(is_powershell_inline_flag(token) for token in lower_tokens)
    if verb in {'cmd', 'cmd.exe'}:
        return '/c' in lower_tokens
    return '-c' in lower_tokens or '-lc' in lower_tokens


def has_inline_interpreter_command(tokens: tuple[str, ...]) -> bool:
    """Return whether tokens launch inline code through a common interpreter."""
    if not tokens:
        return False
    verb = normalize_command_verb(tokens[0])
    if verb in {'python', 'python.exe', 'python3', 'python3.exe', 'py'}:
        lower_tokens = tuple(token.lower() for token in tokens[1:])
        if '-c' in lower_tokens:
            return True
        if '-m' in lower_tokens:
            module_name = extract_python_module_name(tokens)
            return module_name not in SAFE_PYTHON_MODULES
        return False
    if verb in {'node', 'node.exe'}:
        return '-e' in tuple(token.lower() for token in tokens[1:])
    return False


def has_opaque_inline_execution(tokens: tuple[str, ...]) -> bool:
    """Return whether the command carries opaque inline code we cannot audit safely."""
    if not tokens:
        return False

    verb = normalize_command_verb(tokens[0])
    lower_tokens = tuple(token.lower() for token in tokens[1:])
    if verb in {'iex', 'invoke-expression'}:
        return True
    if verb in {'powershell', 'powershell.exe', 'pwsh', 'pwsh.exe'}:
        return any(
            token in {'-enc'} or token.startswith('-encodedcom')
            for token in lower_tokens
        )
    if verb in {'python', 'python.exe', 'python3', 'python3.exe', 'py'}:
        return '-c' in lower_tokens
    if verb in {'node', 'node.exe'}:
        return '-e' in lower_tokens
    return False


def extract_python_module_name(tokens: tuple[str, ...]) -> str | None:
    """Extract the Python module name passed to `-m`, if present."""
    for index, token in enumerate(tokens[1:], start=1):
        if token.lower() == '-m' and index + 1 < len(tokens):
            return strip_wrapping_quotes(tokens[index + 1]).strip().lower() or None
    return None


def extract_script_execution_path(tokens: tuple[str, ...], cwd_value: Any) -> Path | None:
    """Extract the script path from a shell/interpreter invocation when possible."""
    if not tokens:
        return None
    verb = normalize_command_verb(tokens[0])
    for index, token in enumerate(tokens[1:], start=1):
        lower_token = token.lower()
        if lower_token == '-m':
            return None
        if lower_token == '-file' and index + 1 < len(tokens):
            return resolve_candidate_path(tokens[index + 1], cwd_value)
        if is_option_token(token):
            continue
        stripped_token = strip_wrapping_quotes(token)
        if Path(stripped_token).suffix.lower() in SCRIPT_FILE_SUFFIXES:
            return resolve_candidate_path(stripped_token, cwd_value)
        if verb not in INLINE_WRAPPER_VERBS | INLINE_INTERPRETER_VERBS:
            break
    first_token = strip_wrapping_quotes(tokens[0])
    if Path(first_token).suffix.lower() in SCRIPT_FILE_SUFFIXES:
        return resolve_candidate_path(first_token, cwd_value)
    return None


def has_script_file_execution(tokens: tuple[str, ...], cwd_value: Any) -> bool:
    """Return whether tokens invoke an external script file via a shell/interpreter."""
    script_path = extract_script_execution_path(tokens, cwd_value)
    return script_path is not None and script_path not in SAFE_SCRIPT_EXECUTION_PATHS


def contains_unexpanded_path_variable(path_tokens: tuple[str, ...]) -> bool:
    """Return whether a path token still depends on shell variable expansion."""
    return any(
        ('$' in strip_wrapping_quotes(token)) or ('%' in strip_wrapping_quotes(token))
        for token in path_tokens
    )


def contains_unresolved_path_expression(path_tokens: tuple[str, ...]) -> bool:
    """Return whether a path token still contains shell/expression syntax."""
    return any(
        marker in strip_wrapping_quotes(token)
        for token in path_tokens
        for marker in ('::', '$(', '[', ']')
    )


def extract_command_path_token_groups(command_text: str) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Extract raw source and target path tokens from a mutating command string."""
    tokens = unwrap_command_tokens(tokenize_command_text(command_text))
    target_tokens = list(extract_redirection_target_tokens(command_text))
    source_tokens: list[str] = []
    if not tokens:
        return (), tuple(target_tokens)

    verb = normalize_command_verb(tokens[0])
    positional_tokens = list(extract_positional_path_tokens(tokens))
    source_tokens.extend(extract_flagged_path_tokens(tokens, COMMAND_PATH_FLAGS))
    target_tokens.extend(extract_flagged_path_tokens(tokens, COMMAND_FILE_PATH_FLAGS))
    target_tokens.extend(extract_flagged_path_tokens(tokens, COMMAND_DESTINATION_FLAGS))
    has_value_flags = command_has_any_flag(tokens, COMMAND_VALUE_FLAGS)

    if verb in DELETE_COMMAND_VERBS:
        target_tokens.extend(positional_tokens)
    elif verb in MOVE_COMMAND_VERBS | COPY_COMMAND_VERBS:
        if positional_tokens:
            source_tokens.extend(positional_tokens[:-1])
            target_tokens.append(positional_tokens[-1])
    elif verb in WRITE_COMMAND_VERBS and positional_tokens:
        if verb == 'out-file' or has_value_flags:
            target_tokens.append(positional_tokens[-1])
        else:
            target_tokens.append(positional_tokens[0])

    return tuple(source_tokens), tuple(target_tokens)


def looks_like_path_token(token: str) -> bool:
    """Return whether a token looks like a file-system path argument."""
    value = strip_wrapping_quotes(token)
    return bool(value) and (
        ('\\' in value)
        or ('/' in value)
        or value.startswith('.')
        or bool(Path(value).suffix)
    )


# pylint: disable=too-many-return-statements,too-many-branches
def extract_delegated_path_argument_tokens(tokens: tuple[str, ...]) -> tuple[str, ...]:
    """Extract non-option path-like args passed through script/interpreter delegation."""
    if not tokens:
        return ()

    verb = normalize_command_verb(tokens[0])
    index = 1
    if verb in {'python', 'python.exe', 'python3', 'python3.exe', 'py'}:
        if index >= len(tokens):
            return ()
        lower_token = tokens[index].lower()
        if lower_token in {'-c', '-m'}:
            index += 2
        else:
            while index < len(tokens) and is_option_token(tokens[index]):
                index += 1
            if index >= len(tokens):
                return ()
            suffix = Path(strip_wrapping_quotes(tokens[index])).suffix.lower()
            if suffix not in SCRIPT_FILE_SUFFIXES:
                return ()
            index += 1
    else:
        script_path = extract_script_execution_path(tokens, None)
        if script_path is None:
            return ()
        if normalize_command_verb(tokens[0]) in INLINE_WRAPPER_VERBS | INLINE_INTERPRETER_VERBS:
            for index, token in enumerate(tokens[1:], start=1):
                lower_token = token.lower()
                if lower_token == '-file' and index + 1 < len(tokens):
                    index += 2
                    break
                if is_option_token(token):
                    continue
                if Path(strip_wrapping_quotes(token)).suffix.lower() in SCRIPT_FILE_SUFFIXES:
                    index += 1
                    break
            else:
                return ()

    return tuple(
        strip_wrapping_quotes(token)
        for token in tokens[index:]
        if not is_option_token(token) and looks_like_path_token(token)
    )


def extract_git_pathspec_tokens(tokens: tuple[str, ...]) -> tuple[str, ...]:
    """Extract positional git pathspec tokens from a subcommand token list."""
    values: list[str] = []
    index = 1
    while index < len(tokens):
        token = tokens[index]
        if token == '--':
            index += 1
            break
        if token.startswith('-'):
            index += 1
            continue
        values.append(strip_wrapping_quotes(token))
        index += 1
    for token in tokens[index:]:
        values.append(strip_wrapping_quotes(token))
    return tuple(value for value in values if value)


def extract_git_mutation_path_token_groups(
    command_text: str,
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Extract source and target path tokens from mutating git subcommands."""
    tokens = unwrap_command_tokens(tokenize_command_text(command_text))
    if not tokens or normalize_command_verb(tokens[0]) != 'git':
        return (), ()

    subcommand_tokens = split_git_subcommand_tokens(tokens)
    if not subcommand_tokens:
        return (), ()

    subcommand = normalize_command_verb(subcommand_tokens[0])
    pathspec_tokens = extract_git_pathspec_tokens(subcommand_tokens)
    if subcommand == 'mv' and len(pathspec_tokens) >= 2:
        return pathspec_tokens[:-1], (pathspec_tokens[-1],)
    if subcommand in {'add', 'rm'}:
        return (), pathspec_tokens
    return (), ()


def extract_flagged_path_tokens(
    tokens: tuple[str, ...],
    flags: set[str],
) -> tuple[str, ...]:
    """Extract path-like values passed through named command flags."""
    values: list[str] = []
    for index, token in enumerate(tokens):
        lower_token = token.lower()
        if lower_token in flags and index + 1 < len(tokens):
            value = strip_wrapping_quotes(tokens[index + 1])
            if value:
                values.append(value)
            continue
        for flag in flags:
            prefix = f'{flag}='
            if lower_token.startswith(prefix):
                value = strip_wrapping_quotes(token[len(prefix):])
                if value:
                    values.append(value)
                break
    return tuple(values)


def extract_positional_path_tokens(tokens: tuple[str, ...]) -> tuple[str, ...]:
    """Extract non-option positional tokens after the command verb."""
    values: list[str] = []
    index = 1
    while index < len(tokens):
        token = tokens[index]
        if is_option_token(token):
            lower_token = token.lower()
            if (
                lower_token in COMMAND_FLAGS_WITH_VALUES
                or any(lower_token.startswith(f'{flag}=') for flag in COMMAND_FLAGS_WITH_VALUES)
            ):
                index += 2 if '=' not in token else 1
                continue
            index += 1
            continue
        value = strip_wrapping_quotes(token)
        if value:
            values.append(value)
        index += 1
    return tuple(values)


def extract_redirection_target_tokens(command_text: str) -> tuple[str, ...]:
    """Extract shell redirection targets from a command string."""
    values: list[str] = []
    for match in REDIRECTION_TARGET_PATTERN.finditer(command_text):
        value = strip_wrapping_quotes(match.group('path'))
        if value and not is_null_device_path(value):
            values.append(value)
    return tuple(values)


def is_null_device_path(value: str) -> bool:
    """Return whether a path token points to a device/null sink."""
    normalized = strip_wrapping_quotes(value).strip().lower().replace('\\', '/')
    return normalized in {'/dev/null', 'nul'}


def resolve_path_tokens(path_tokens: tuple[str, ...], cwd_value: Any) -> tuple[Path, ...]:
    """Resolve unique path tokens against cwd or repo root."""
    paths: list[Path] = []
    for token in path_tokens:
        resolved_path = resolve_candidate_path(token, cwd_value)
        if resolved_path is None or resolved_path in paths:
            continue
        paths.append(resolved_path)
    return tuple(paths)


def extract_command_path_groups(
    command_text: str,
    cwd_value: Any,
) -> tuple[tuple[Path, ...], tuple[Path, ...]]:
    """Extract command-tool source and target paths for common mutating verbs."""
    source_tokens, target_tokens = extract_command_path_token_groups(command_text)
    return (
        resolve_path_tokens(source_tokens, cwd_value),
        resolve_path_tokens(target_tokens, cwd_value),
    )


def command_has_path_mutation(command_text: str) -> bool:
    """Return whether a command text appears to mutate file-system paths."""
    tokens = unwrap_command_tokens(tokenize_command_text(command_text))
    if tokens:
        verb = normalize_command_verb(tokens[0])
        if verb in COMMAND_PATH_MUTATION_VERBS:
            return True
        if verb == 'git':
            subcommand_tokens = split_git_subcommand_tokens(tokens)
            subcommand = (
                normalize_command_verb(subcommand_tokens[0]) if subcommand_tokens else ''
            )
            if subcommand in GIT_MUTATING_SUBCOMMANDS:
                return True
        if extract_delegated_path_argument_tokens(tokens):
            return True
    return bool(
        extract_redirection_target_tokens(command_text)
        or GIT_MUTATION_SIGNAL_PATTERN.search(command_text)
    )


def split_git_subcommand_tokens(tokens: tuple[str, ...]) -> tuple[str, ...]:
    """Strip leading git global flags and return the remaining subcommand tokens."""
    index = 1
    while index < len(tokens):
        token = tokens[index]
        if token == '--':
            index += 1
            break
        if not token.startswith('-'):
            break
        lower_token = token.lower()
        if (
            lower_token in GIT_GLOBAL_FLAGS_WITH_VALUE
            and '=' not in token
            and index + 1 < len(tokens)
        ):
            index += 2
            continue
        index += 1
    return tokens[index:]


def is_dangerous_git_command(command_text: str) -> bool:
    """Return whether a git command is one of the blocked destructive forms."""
    if any(pattern.search(command_text) for pattern in DANGEROUS_GIT_PATTERNS):
        return True

    tokens = unwrap_command_tokens(tokenize_command_text(command_text))
    if not tokens or normalize_command_verb(tokens[0]) != 'git':
        return False

    subcommand_tokens = split_git_subcommand_tokens(tokens)
    if not subcommand_tokens:
        return False

    subcommand = normalize_command_verb(subcommand_tokens[0])
    arguments = tuple(strip_wrapping_quotes(token) for token in subcommand_tokens[1:])
    if subcommand == 'reset':
        return '--hard' in arguments
    if subcommand == 'clean':
        return any(
            token == '--force' or (token.startswith('-') and 'f' in token[1:])
            for token in arguments
        )
    if subcommand in {'checkout', 'restore'}:
        return any(is_current_directory_pathspec(token) for token in arguments)
    return False


def is_current_directory_pathspec(token: str) -> bool:
    """Return whether a git pathspec token refers to the current directory."""
    normalized = strip_wrapping_quotes(token).replace('\\', '/')
    normalized = normalized.rstrip('/')
    return normalized == '.'


def normalize_path_token_text(value: str) -> str:
    """Normalize Windows-style command path tokens before pathlib resolution."""
    text = strip_wrapping_quotes(value).strip()
    if not text or URL_SCHEME_PATTERN.match(text):
        return text
    if text.startswith('/') and not text.startswith('//'):
        return text
    if '\\' in text:
        return text.replace('\\', '/')
    return text


def is_windows_absolute_path_text(value: str) -> bool:
    """Return whether text looks like a Windows absolute or UNC path."""
    return bool(
        WINDOWS_ABSOLUTE_PATH_PATTERN.match(value) or UNC_PATH_PATTERN.match(value)
    )


def resolve_candidate_path(path_value: Any, cwd_value: Any) -> Path | None:
    """Resolve an edited path against cwd or repo root."""
    raw_path = normalize_path_token_text(str(path_value or ''))
    if not raw_path:
        return None

    candidate = Path(raw_path)
    if candidate.is_absolute():
        return candidate.resolve()
    if is_windows_absolute_path_text(raw_path):
        return (Path('/') / raw_path).resolve()

    raw_cwd = str(cwd_value or '').strip()
    if raw_cwd:
        return (Path(raw_cwd) / candidate).resolve()

    return (REPO_ROOT / candidate).resolve()


def run_git(
    args: list[str],
    repo_root: Path = REPO_ROOT,
) -> subprocess.CompletedProcess[str] | None:
    """Run a git command for guard-side checks without raising."""
    try:
        return subprocess.run(
            ['git', *args],
            cwd=repo_root,
            capture_output=True,
            text=True,
            encoding='utf-8',
            errors='replace',
            check=False,
            timeout=GIT_SUBPROCESS_TIMEOUT_S,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None


def format_display_path(path: Path, repo_root: Path = REPO_ROOT) -> str:
    """Render repo-relative paths when possible."""
    resolved_path = path.resolve()
    resolved_root = repo_root.resolve()
    try:
        return resolved_path.relative_to(resolved_root).as_posix()
    except ValueError:
        return str(resolved_path)


def build_git_check_failure_reason(
    result: subprocess.CompletedProcess[str] | None,
    unavailable_message: str,
    unexpected_message: str,
) -> str | None:
    """Map git command failure states to a deny reason."""
    if result is None:
        return unavailable_message
    if result.returncode not in {0, 1}:
        return unexpected_message
    return None


def path_touches_guarded_target(candidate_path: Path, target_path: Path) -> bool:
    """Return whether the candidate equals, is inside, or contains the target."""
    return (
        candidate_path == target_path
        or _is_relative_to(candidate_path, target_path)
        or _is_relative_to(target_path, candidate_path)
    )


def extract_apply_patch_paths(patch_text: str) -> tuple[str, ...]:
    """Extract repo-relative target paths from apply_patch payloads."""
    paths: list[str] = []
    for raw_line in patch_text.splitlines():
        for prefix in (
            '*** Add File: ',
            '*** Update File: ',
            '*** Delete File: ',
            '*** Move to: ',
        ):
            if raw_line.startswith(prefix):
                path_text = raw_line.removeprefix(prefix).strip()
                if path_text and path_text not in paths:
                    paths.append(path_text)
    return tuple(paths)


def collect_candidate_paths(tool_input: dict[str, Any], cwd_value: Any) -> tuple[Path, ...]:
    """Collect unique candidate paths from the known path keys in stable order."""
    candidates: list[Path] = []
    for path_key in PATH_KEYS:
        candidate_path = resolve_candidate_path(tool_input.get(path_key), cwd_value)
        if candidate_path is None or candidate_path in candidates:
            continue
        candidates.append(candidate_path)
    patch_text = tool_input.get('patch')
    if isinstance(patch_text, str):
        for patch_path in extract_apply_patch_paths(patch_text):
            candidate_path = resolve_candidate_path(patch_path, str(REPO_ROOT))
            if candidate_path is None or candidate_path in candidates:
                continue
            candidates.append(candidate_path)
    return tuple(candidates)


def ignored_path_denial_reason(path: Path, repo_root: Path = REPO_ROOT) -> str | None:
    """Return a deny reason for ignored paths or unavailable git state."""
    denial_reason: str | None = None
    try:
        relative_path = path.resolve().relative_to(repo_root).as_posix()
    except ValueError:
        label = format_display_path(path, repo_root)
        denial_reason = (
            f'仓库 guard hook 阻止直接修改仓库外路径：{label}。'
            '请把目标文件定位到仓库内 source-of-truth 路径后再继续。'
        )
    else:
        tracked_result = run_git(
            ['ls-files', '--error-unmatch', '--', relative_path],
            repo_root,
        )
        denial_reason = build_git_check_failure_reason(
            tracked_result,
            '仓库 guard hook 无法验证 git 跟踪状态（git 不可用或超时）。'
            '为避免无视 ignore 配置，本次写入已被阻止；'
            '请先确认 git 环境正常，再重试。',
            '仓库 guard hook 无法验证 git 跟踪状态（`git ls-files` 返回异常）。'
            '为避免无视 ignore 配置，本次写入已被阻止；'
            '请先修复 git 环境，再重试。',
        )
        if denial_reason is None and tracked_result is not None and tracked_result.returncode != 0:
            ignore_result = run_git(
                ['check-ignore', '-q', '--', relative_path],
                repo_root,
            )
            denial_reason = build_git_check_failure_reason(
                ignore_result,
                '仓库 guard hook 无法验证 git ignore 规则（git 不可用或超时）。'
                '为避免无视 ignore 配置，本次写入已被阻止；'
                '请先确认 git 环境正常，再重试。',
                '仓库 guard hook 无法验证 git ignore 规则（`git check-ignore` 返回异常）。'
                '为避免无视 ignore 配置，本次写入已被阻止；'
                '请先修复 git 环境，再重试。',
            )
            if (
                denial_reason is None
                and ignore_result is not None
                and ignore_result.returncode == 0
            ):
                label = format_display_path(path, repo_root)
                denial_reason = (
                    f"仓库 guard hook 阻止直接写入匹配 ignore 规则的路径：{label}。"
                    "CLI 提交 / 上传必须尊重 `.gitignore`、`.git/info/exclude` 与 "
                    "`core.excludesFile`。"
                    f"如需确认规则来源，请先运行 `git check-ignore -v -- {label}`；"
                    f"如该路径已被 git 追踪，请先运行 `git ls-files -- {label}` "
                    "确认是否为已跟踪文件，再决定是调整 ignore 规则还是继续修改；"
                    "如需纳入版本控制，请先调整 ignore 规则。"
                )

    return denial_reason


def outside_repo_denial_reason(path: Path, repo_root: Path = REPO_ROOT) -> str | None:
    """Return a deny reason when a delete target resolves outside the repo."""
    resolved_path = path.resolve()
    if _is_relative_to(resolved_path, repo_root):
        return None
    label = format_display_path(resolved_path, repo_root)
    return (
        f'仓库 guard hook 阻止直接修改仓库外路径：{label}。'
        '请把目标文件定位到仓库内 source-of-truth 路径后再继续。'
    )


def protected_path_denial_reason(path: Path) -> str | None:
    """Return a deny reason when a path touches a protected directory."""
    for protected_dir in PROTECTED_DIRECTORIES:
        resolved_protected_dir = protected_dir.resolve()
        if not path_touches_guarded_target(path, resolved_protected_dir):
            continue
        label = PROTECTED_DIRECTORY_LABELS.get(
            str(resolved_protected_dir),
            str(resolved_protected_dir),
        )
        return (
            f'仓库 guard hook 阻止直接修改受保护目录：{label}。'
            '请改运行时代码、测试夹具或仓库入口文件，'
            '不要直接编辑第三方/参考/已移除兼容层内容。'
        )
    return None


def runtime_delete_denial_reason(path: Path) -> str | None:
    """Return a deny reason when a delete target touches runtime-critical state."""
    for protected_path in RUNTIME_CRITICAL_PATHS:
        if not path_touches_guarded_target(path, protected_path):
            continue
        return (
            '仓库 guard hook 阻止删除 runtime 关键文件或配置目录'
            '（data/bills.db* / data/config）。'
            '如需测试隔离，请改用 tests/.runtime 或专用脚本。'
        )
    return None


def first_path_denial_reason(
    candidate_paths: tuple[Path, ...],
    checkers: tuple[Callable[[Path], str | None], ...],
) -> str | None:
    """Return the first deny reason produced by the provided path checkers."""
    for candidate_path in candidate_paths:
        for checker in checkers:
            denial_reason = checker(candidate_path)
            if denial_reason is not None:
                return denial_reason
    return None


def collect_command_path_tokens(
    command_text: str,
) -> tuple[tuple[str, ...], tuple[str, ...]]:
    """Collect generic and git-specific source/target path tokens."""
    source_path_tokens, target_path_tokens = extract_command_path_token_groups(command_text)
    target_path_tokens = (
        *target_path_tokens,
        *extract_delegated_path_argument_tokens(
            unwrap_command_tokens(tokenize_command_text(command_text))
        ),
    )
    git_source_tokens, git_target_tokens = extract_git_mutation_path_token_groups(command_text)
    if git_source_tokens or git_target_tokens:
        source_path_tokens = (*source_path_tokens, *git_source_tokens)
        target_path_tokens = (*target_path_tokens, *git_target_tokens)
    return source_path_tokens, target_path_tokens


# pylint: disable=too-many-branches
def command_path_early_denial_reason(
    command_text: str,
    cwd_value: Any,
    tokens: tuple[str, ...],
    source_path_tokens: tuple[str, ...],
    target_path_tokens: tuple[str, ...],
) -> tuple[str | None, bool]:
    """Return an early denial reason, or whether path mutation checks should continue."""
    has_mutation_signal = (
        has_opaque_inline_execution(tokens)
        or command_has_path_mutation(command_text)
        or bool(COMMAND_MUTATION_MARKER_PATTERN.search(command_text))
        or bool(POWERSHELL_EXPRESSION_MUTATION_PATTERN.search(command_text))
    )
    if not has_mutation_signal:
        return None, False

    early_denial_reason: str | None = None
    if has_inline_wrapper_command(tokens):
        early_denial_reason = (
            '仓库 guard hook 不允许通过嵌套 shell/interpreter 包装器执行内联命令。'
            '请直接使用结构化文件工具，或改成未包装的单层命令后再继续。'
        )
    elif has_inline_interpreter_command(tokens):
        early_denial_reason = (
            '仓库 guard hook 不允许通过内联解释器执行文件系统变更代码。'
            '请改用结构化文件工具，或改成可明确校验路径的命令后再继续。'
        )
    elif has_script_file_execution(tokens, cwd_value):
        early_denial_reason = (
            '仓库 guard hook 不允许通过外部脚本文件把文件系统变更委托给解释器或 shell。'
            '请直接使用结构化文件工具，或改成可明确校验路径的命令后再继续。'
        )
    elif POWERSHELL_EXPRESSION_MUTATION_PATTERN.search(command_text):
        early_denial_reason = (
            '仓库 guard hook 不允许通过 PowerShell/.NET 表达式直接执行文件系统变更。'
            '请改用结构化文件工具，或改成可明确校验路径的命令后再继续。'
        )
    elif COMPLEX_COMMAND_SEPARATOR_PATTERN.search(command_text) and (
        COMMAND_MUTATION_MARKER_PATTERN.search(command_text)
        or REDIRECTION_TARGET_PATTERN.search(command_text)
    ):
        early_denial_reason = (
            '仓库 guard hook 无法可靠解析包含链式/管道分隔符的路径变更命令。'
            '请改用结构化文件工具，或把命令拆成可明确校验路径的单步操作后再继续。'
        )
    elif contains_unexpanded_path_variable((*source_path_tokens, *target_path_tokens)):
        early_denial_reason = (
            '仓库 guard hook 无法可靠解析包含 shell 变量的路径参数。'
            '请改用结构化文件工具，或先展开为明确路径后再继续。'
        )
    elif contains_unresolved_path_expression((*source_path_tokens, *target_path_tokens)):
        early_denial_reason = (
            '仓库 guard hook 无法可靠解析表达式生成的路径参数。'
            '请改用结构化文件工具，或先展开为明确路径后再继续。'
        )
    elif tokens and normalize_command_verb(tokens[0]) == 'git':
        subcommand_tokens = split_git_subcommand_tokens(tokens)
        if subcommand_tokens and normalize_command_verb(subcommand_tokens[0]) == 'apply':
            early_denial_reason = (
                '仓库 guard hook 不允许通过 `git apply` 直接修改路径。'
                '请改用 apply_patch 或结构化文件工具，并让目标路径经过 guard 校验。'
            )
    return early_denial_reason, True


def command_path_denial_reason(command_text: str, cwd_value: Any) -> str | None:
    """Apply path-based guard checks to common mutating shell/PowerShell commands."""
    tokens = unwrap_command_tokens(tokenize_command_text(command_text))
    source_path_tokens, target_path_tokens = collect_command_path_tokens(command_text)
    early_denial_reason, should_continue = command_path_early_denial_reason(
        command_text,
        cwd_value,
        tokens,
        source_path_tokens,
        target_path_tokens,
    )
    if early_denial_reason is not None:
        return early_denial_reason
    if not should_continue:
        return None

    source_paths, target_paths = (
        resolve_path_tokens(source_path_tokens, cwd_value),
        resolve_path_tokens(target_path_tokens, cwd_value),
    )
    if not source_paths and not target_paths:
        return (
            '仓库 guard hook 无法可靠解析当前命令工具中的路径变更目标。'
            '请改用 Edit/Write/Create/apply_patch/Delete 等结构化工具，'
            '或把命令拆成可明确校验路径的形式后再继续。'
        )

    source_denial_reason = first_path_denial_reason(
        source_paths,
        (
            protected_path_denial_reason,
            ignored_path_denial_reason,
            outside_repo_denial_reason,
            runtime_delete_denial_reason,
        ),
    )
    if source_denial_reason is not None:
        return source_denial_reason

    target_denial_reason = first_path_denial_reason(
        target_paths,
        (protected_path_denial_reason, ignored_path_denial_reason),
    )
    if target_denial_reason is not None:
        return target_denial_reason

    return first_path_denial_reason(
        target_paths,
        (outside_repo_denial_reason, runtime_delete_denial_reason),
    )


# pylint: disable=too-many-return-statements,too-many-branches
def evaluate_pre_tool_use(payload: dict[str, Any]) -> str | None:
    """Return a denial reason when the requested tool action is unsafe."""
    tool_name = get_tool_name(payload).lower()
    tool_input = get_tool_input(payload)

    if tool_name in COMMAND_TOOL_NAMES:
        command_text = extract_command_text(tool_input)
        for pattern in DESTRUCTIVE_PROCESS_PATTERNS:
            if pattern.search(command_text):
                return (
                    '仓库规则禁止使用批量杀进程命令（例如 taskkill /f /im '
                    'python.exe 或 Stop-Process -Name python）。'
                    '请只终止当前测试或服务进程。'
                )
        if is_dangerous_git_command(command_text):
            return (
                '仓库 guard hook 阻止危险 git 工作区清理命令（例如 '
                'git reset --hard、git clean -fd、git checkout -- .、'
                'git restore .）。请改为精确处理目标文件。'
            )
        for pattern in RUNTIME_DELETE_PATTERNS:
            if pattern.search(command_text):
                return (
                    '仓库 guard hook 阻止删除 runtime 关键数据文件（如 '
                    'data/bills.db、data/config）。如需重建环境，请使用 '
                    'tests/.runtime 或专用脚本。'
                )
        denial_reason = command_path_denial_reason(command_text, payload.get('cwd'))
        if denial_reason is not None:
            return denial_reason

    if tool_name in WRITE_TOOL_NAMES | DELETE_TOOL_NAMES:
        candidate_paths = collect_candidate_paths(tool_input, payload.get('cwd'))
        for candidate_path in candidate_paths:
            denial_reason = protected_path_denial_reason(candidate_path)
            if denial_reason is not None:
                return denial_reason
            if tool_name in DELETE_TOOL_NAMES:
                denial_reason = outside_repo_denial_reason(candidate_path)
                if denial_reason is not None:
                    return denial_reason
                denial_reason = runtime_delete_denial_reason(candidate_path)
                if denial_reason is not None:
                    return denial_reason
            denial_reason = ignored_path_denial_reason(candidate_path)
            if denial_reason is not None:
                return denial_reason

    return None


def build_hook_output(payload: dict[str, Any], reason: str) -> dict[str, Any]:
    """Build the host-specific deny payload."""
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
    """Entry point for repo guard hook execution."""
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
