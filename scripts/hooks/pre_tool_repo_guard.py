"""Thin pre-tool guard for extreme destructive operations only."""

from __future__ import annotations

import base64
import binascii
import json
import re
import shlex
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]

COMMAND_TOOL_NAMES = {"bash", "powershell", "shell", "execute", "run_in_terminal"}
DELETE_TOOL_NAMES = {"delete", "remove", "delete_file"}
PATH_KEYS = ("file_path", "path", "filePath")

DELETE_VERBS = {"rm", "del", "erase", "rd", "rmdir", "ri", "remove-item", "unlink"}
DELETE_PATH_FLAGS = {"-path", "-literalpath"}
POWERSHELL_INLINE_WRAPPERS = {"powershell", "powershell.exe", "pwsh", "pwsh.exe"}
POWERSHELL_INLINE_COMMAND_SHORT_PARAMETERS = {"c"}
POWERSHELL_INLINE_COMMAND_FULL_FLAG = "-command"
POWERSHELL_ENCODED_COMMAND_SHORT_PARAMETERS = {"e", "ec"}
POWERSHELL_ENCODED_COMMAND_FULL_FLAG = "-encodedcommand"
POSIX_INLINE_WRAPPERS = {"bash", "bash.exe", "sh", "sh.exe"}
POSIX_INLINE_COMMAND_FLAGS = {"-c", "-lc"}
CMD_INLINE_WRAPPERS = {"cmd", "cmd.exe"}
CMD_INLINE_COMMAND_FLAGS = {"/c", "/k"}
OPAQUE_INLINE_COMMAND = "__OPAQUE_INLINE_COMMAND__"
FLAGS_WITH_VALUES = {
    "-exclude",
    "-filter",
    "-include",
    "-stream",
    "-whatif",
}
GIT_GLOBAL_FLAGS_WITH_VALUES = {
    "-c",
    "-C",
    "--config-env",
    "--exec-path",
    "--git-dir",
    "--namespace",
    "--super-prefix",
    "--work-tree",
}

DANGEROUS_GIT_PATTERNS = (
    re.compile(r"(?i)\bgit\s+(?:-[^\s]+\s+)*reset\b[^\r\n]*\s--hard(?:\s|$)"),
    re.compile(
        r"(?i)\bgit\s+(?:checkout|restore)\b[^\r\n]*(?:\s--\s*)?\.(?:\s|$)"
    ),
)
GIT_CLEAN_PATTERN = re.compile(
    r"(?i)\bgit\s+(?:-[^\s]+\s+)*clean\b(?P<args>[^\r\n]*)"
)
GIT_CLEAN_FORCE_FLAG_PATTERN = re.compile(
    r"(?i)(?:^|\s)(?:-[a-z]*f[a-z]*|--force\b)"
)
GIT_CLEAN_DRY_RUN_FLAG_PATTERN = re.compile(
    r"(?i)(?:^|\s)(?:-[a-z]*n[a-z]*|--dry-run\b)"
)
SQL_WIPE_PATTERN = re.compile(
    r"(?is)\b(?:drop\s+(?:database|schema|table)|truncate\s+table|delete\s+from)\b"
)
DB_CONTEXT_PATTERN = re.compile(
    r"(?i)\b(?:sqlite3|psql|mysql|bills\.db|data[\\/]+bills\.db)\b"
)


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
    return str(payload.get("tool_name") or payload.get("toolName") or "")


def get_tool_input(payload: dict[str, Any]) -> dict[str, Any]:
    """Extract tool arguments from Claude/Copilot payload variants."""
    if isinstance(payload.get("tool_input"), dict):
        return payload["tool_input"]

    raw_args = payload.get("toolArgs")
    if isinstance(raw_args, dict):
        return raw_args
    if isinstance(raw_args, str):
        try:
            decoded = json.loads(raw_args)
        except json.JSONDecodeError:
            if raw_args.lstrip().startswith("*** Begin Patch"):
                return {"patch": raw_args}
            return {}
        return decoded if isinstance(decoded, dict) else {}

    return {}


def extract_command_text(tool_input: dict[str, Any]) -> str:
    """Flatten command plus args into one string for regex checks."""
    command = str(tool_input.get("command") or "").strip()
    args = tool_input.get("args")
    if isinstance(args, list):
        command = " ".join(
            part for part in [command, *[str(item) for item in args]] if part
        )
    return command


def strip_wrapping_quotes(value: str) -> str:
    """Remove one layer of matching single or double quotes."""
    text = str(value).strip()
    if len(text) >= 2 and text[0] == text[-1] and text[0] in {"'", '"'}:
        return text[1:-1]
    return text


def tokenize_command_text(command_text: str) -> tuple[str, ...]:
    """Tokenize enough of a shell command to find obvious delete targets."""
    try:
        return tuple(shlex.split(command_text, posix=False))
    except ValueError:
        return tuple(command_text.split())


def normalize_command_verb(token: str) -> str:
    """Normalize command aliases used by PowerShell and cmd."""
    verb = Path(strip_wrapping_quotes(token)).name.lower()
    return {"rd": "rmdir", "ri": "remove-item"}.get(verb, verb)


def is_option_token(token: str) -> bool:
    """Return whether a token is an option rather than a filesystem target."""
    text = strip_wrapping_quotes(token)
    return (
        len(text) > 1
        and (text.startswith("-") or text.startswith("/"))
        and text not in {"/", "/*"}
    )


def split_option_value(token: str) -> tuple[str, str | None]:
    """Split PowerShell-style `-Name:value` or `--name=value` option tokens."""
    text = strip_wrapping_quotes(token).strip()
    if not text.startswith(("-", "/")):
        return text.lower(), None
    for separator in (":", "="):
        if separator in text:
            name, value = text.split(separator, 1)
            if name:
                return name.lower(), strip_wrapping_quotes(value)
    return text.lower(), None


def decode_powershell_encoded_command(value: str) -> str | None:
    """Decode a PowerShell EncodedCommand token when it is inspectable."""
    try:
        raw_bytes = base64.b64decode(strip_wrapping_quotes(value), validate=True)
    except (binascii.Error, ValueError):
        return None
    for encoding in ("utf-16le", "utf-8"):
        try:
            decoded = raw_bytes.decode(encoding).strip()
        except UnicodeDecodeError:
            continue
        if decoded:
            return decoded
    return None


def append_inline_command_argument(
    commands: list[str],
    tokens: tuple[str, ...],
    index: int,
    flag_value: str | None,
) -> None:
    """Append inline command text from either `-Flag:value` or following token."""
    if flag_value is not None:
        commands.append(flag_value)
    elif index + 1 < len(tokens):
        commands.append(strip_wrapping_quotes(tokens[index + 1]))


def is_powershell_encoded_command_flag(flag_name: str) -> bool:
    """Return whether a token is a PowerShell EncodedCommand flag or prefix."""
    normalized = flag_name.lower()
    if normalized.startswith("/"):
        normalized = f"-{normalized[1:]}"
    if not normalized.startswith("-"):
        return False
    parameter = normalized[1:]
    return parameter in POWERSHELL_ENCODED_COMMAND_SHORT_PARAMETERS or (
        bool(parameter) and POWERSHELL_ENCODED_COMMAND_FULL_FLAG[1:].startswith(parameter)
    )


def is_powershell_command_flag(flag_name: str) -> bool:
    """Return whether a token is a PowerShell Command flag or prefix."""
    normalized = flag_name.lower()
    if normalized.startswith("/"):
        normalized = f"-{normalized[1:]}"
    if not normalized.startswith("-"):
        return False
    parameter = normalized[1:]
    return parameter in POWERSHELL_INLINE_COMMAND_SHORT_PARAMETERS or (
        bool(parameter) and POWERSHELL_INLINE_COMMAND_FULL_FLAG[1:].startswith(parameter)
    )


def extract_inline_command_texts(command_text: str) -> tuple[str, ...]:
    """Extract nested shell command strings that can contain delete verbs."""
    tokens = tokenize_command_text(command_text)
    commands: list[str] = []
    index = 0
    while index < len(tokens):
        wrapper = normalize_command_verb(tokens[index])
        if wrapper not in (
            POWERSHELL_INLINE_WRAPPERS | POSIX_INLINE_WRAPPERS | CMD_INLINE_WRAPPERS
        ):
            index += 1
            continue

        next_index = index + 1
        while next_index < len(tokens):
            flag_name, flag_value = split_option_value(tokens[next_index])
            if wrapper in POWERSHELL_INLINE_WRAPPERS and is_powershell_command_flag(flag_name):
                append_inline_command_argument(commands, tokens, next_index, flag_value)
                break
            if wrapper in POWERSHELL_INLINE_WRAPPERS and is_powershell_encoded_command_flag(flag_name):
                encoded_value = flag_value
                if encoded_value is None and next_index + 1 < len(tokens):
                    encoded_value = tokens[next_index + 1]
                decoded = (
                    decode_powershell_encoded_command(encoded_value)
                    if encoded_value is not None
                    else None
                )
                commands.append(decoded or OPAQUE_INLINE_COMMAND)
                break
            if wrapper in POSIX_INLINE_WRAPPERS and flag_name in POSIX_INLINE_COMMAND_FLAGS:
                append_inline_command_argument(commands, tokens, next_index, flag_value)
                break
            if wrapper in CMD_INLINE_WRAPPERS and flag_name in CMD_INLINE_COMMAND_FLAGS:
                append_inline_command_argument(commands, tokens, next_index, flag_value)
                break
            next_index += 1
        index = next_index + 1
    return tuple(commands)


def extract_delete_targets(command_text: str) -> tuple[str, ...]:
    """Extract explicit target tokens following common delete verbs."""
    tokens = tokenize_command_text(command_text)
    targets: list[str] = []
    for inline_command in extract_inline_command_texts(command_text):
        targets.extend(extract_delete_targets(inline_command))

    index = 0
    while index < len(tokens):
        verb = normalize_command_verb(tokens[index])
        if verb not in DELETE_VERBS:
            index += 1
            continue

        index += 1
        while index < len(tokens):
            token = tokens[index]
            lower_token = strip_wrapping_quotes(token).lower()
            option_name, option_value = split_option_value(token)
            if option_name in DELETE_PATH_FLAGS and option_value is not None:
                targets.append(option_value)
                index += 1
                continue
            if option_name in FLAGS_WITH_VALUES and option_value is not None:
                index += 1
                continue
            if lower_token in DELETE_PATH_FLAGS and index + 1 < len(tokens):
                targets.append(tokens[index + 1])
                index += 2
                continue
            if lower_token in FLAGS_WITH_VALUES and index + 1 < len(tokens):
                index += 2
                continue
            if is_option_token(token):
                index += 1
                continue
            targets.append(token)
            index += 1
    return tuple(targets)


def is_windows_drive_root_token(value: str) -> bool:
    """Return whether a token targets an entire Windows drive root."""
    text = strip_wrapping_quotes(value).replace("/", "\\").strip()
    return bool(re.fullmatch(r"[A-Za-z]:(?:\\)?(?:\*)?", text))


def is_system_drive_root_token(value: str) -> bool:
    """Return whether a token targets the Windows system drive root."""
    text = strip_wrapping_quotes(value).replace("/", "\\").strip().lower()
    return text in {
        "$env:systemdrive\\",
        "$env:systemdrive\\*",
        "%systemdrive%\\",
        "%systemdrive%\\*",
    }


def is_posix_root_token(value: str) -> bool:
    """Return whether a token targets the POSIX filesystem root."""
    text = strip_wrapping_quotes(value).replace("\\", "/").strip()
    return text in {"/", "/*"}


def normalize_path_token(value: str) -> str:
    """Normalize simple path tokens before resolving them."""
    text = strip_wrapping_quotes(value).strip()
    if text.endswith("\\*") or text.endswith("/*"):
        text = text[:-2]
    if not re.match(r"^[A-Za-z]:[\\/]", text) and "\\" in text:
        text = text.replace("\\", "/")
    return text


def resolve_candidate_path(path_value: Any, cwd_value: Any = None) -> Path | None:
    """Resolve a candidate path against cwd or the repository root."""
    raw_path = normalize_path_token(str(path_value or ""))
    if (
        not raw_path
        or is_windows_drive_root_token(raw_path)
        or is_system_drive_root_token(raw_path)
    ):
        return None

    candidate = Path(raw_path)
    base = Path(str(cwd_value or REPO_ROOT))
    try:
        return (candidate if candidate.is_absolute() else base / candidate).resolve()
    except (OSError, RuntimeError, ValueError):
        return None


def _is_relative_to(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def path_touches_guarded_target(candidate_path: Path, target_path: Path) -> bool:
    """Return whether candidate equals, contains, or sits inside the target."""
    candidate = candidate_path.resolve()
    target = target_path.resolve()
    return (
        candidate == target
        or _is_relative_to(candidate, target)
        or _is_relative_to(target, candidate)
    )


def path_equals_or_contains_target(candidate_path: Path, target_path: Path) -> bool:
    """Return whether deleting candidate would delete target too."""
    candidate = candidate_path.resolve()
    target = target_path.resolve()
    return candidate == target or _is_relative_to(target, candidate)


def is_repo_wipe_target(path: Path | None) -> bool:
    """Return whether a target would wipe the repository itself."""
    if path is None:
        return False
    return path_equals_or_contains_target(
        path,
        REPO_ROOT,
    ) or path_touches_guarded_target(
        path,
        REPO_ROOT / ".git",
    )


def path_is_runtime_database_family(path: Path, data_root: Path) -> bool:
    """Return whether a target names `data/bills.db*` directly."""
    try:
        relative = path.resolve().relative_to(data_root.resolve())
    except ValueError:
        return False
    return len(relative.parts) == 1 and relative.name.startswith("bills.db")


def is_runtime_wipe_target(path: Path | None) -> bool:
    """Return whether a target would wipe the live Bill Analyser runtime database state."""
    if path is None:
        return False
    data_root = REPO_ROOT / "data"
    guarded_files = (
        data_root / "bills.db",
        REPO_ROOT / "data" / "bills.db-shm",
        REPO_ROOT / "data" / "bills.db-wal",
        REPO_ROOT / "data" / "config",
    )
    return (
        path_equals_or_contains_target(path, data_root)
        or path_is_runtime_database_family(path, data_root)
    ) or any(
        path_touches_guarded_target(path, target)
        for target in guarded_files
    )


def is_dangerous_git_clean(command_text: str) -> bool:
    """Return whether git clean is destructive rather than a dry-run preview."""
    for match in GIT_CLEAN_PATTERN.finditer(command_text):
        args = match.group("args")
        has_force = GIT_CLEAN_FORCE_FLAG_PATTERN.search(args) is not None
        has_dry_run = GIT_CLEAN_DRY_RUN_FLAG_PATTERN.search(args) is not None
        if has_force and not has_dry_run:
            return True
    return False


def iter_git_commands(command_text: str) -> tuple[tuple[str, tuple[str, ...]], ...]:
    """Extract git subcommands while skipping global options and their values."""
    tokens = tokenize_command_text(command_text)
    commands: list[tuple[str, tuple[str, ...]]] = []
    index = 0
    separators = {"&&", "||", ";", "|"}
    while index < len(tokens):
        if normalize_command_verb(tokens[index]) not in {"git", "git.exe"}:
            index += 1
            continue

        index += 1
        while index < len(tokens):
            token = tokens[index]
            if token in separators:
                break
            option_name, option_value = split_option_value(token)
            if option_name in GIT_GLOBAL_FLAGS_WITH_VALUES:
                index += 1 if option_value is not None else 2
                continue
            if is_option_token(token):
                index += 1
                continue
            subcommand = strip_wrapping_quotes(token).lower()
            arg_start = index + 1
            arg_end = arg_start
            while arg_end < len(tokens) and tokens[arg_end] not in separators:
                arg_end += 1
            commands.append((subcommand, tokens[arg_start:arg_end]))
            index = arg_end
            break
    return tuple(commands)


def git_flag_letters(token: str) -> str:
    """Return combined short-option letters for a git flag token."""
    text = strip_wrapping_quotes(token)
    if text.startswith("--") or not text.startswith("-"):
        return ""
    return text.lstrip("-").lower()


def is_git_worktree_root_pathspec(token: str) -> bool:
    """Return whether a git pathspec targets the whole worktree root."""
    text = strip_wrapping_quotes(token).replace("\\", "/").strip().lower()
    return text in {".", "./", ":/"} or text.rstrip("/") == "."


def is_dangerous_git_command(command_text: str) -> bool:
    """Return whether tokenized git usage would wipe repo state."""
    for subcommand, args in iter_git_commands(command_text):
        lower_args = tuple(strip_wrapping_quotes(arg).lower() for arg in args)
        if subcommand == "reset" and "--hard" in lower_args:
            return True
        if subcommand in {"checkout", "restore"} and any(
            is_git_worktree_root_pathspec(arg) for arg in lower_args
        ):
            return True
        if subcommand == "clean":
            has_force = any(
                arg == "--force" or "f" in git_flag_letters(arg)
                for arg in lower_args
            )
            has_dry_run = any(
                arg == "--dry-run" or "n" in git_flag_letters(arg)
                for arg in lower_args
            )
            if has_force and not has_dry_run:
                return True
    return False


def command_denial_reason(command_text: str, cwd_value: Any = None) -> str | None:
    """Return a denial reason for extreme destructive shell commands."""
    for inline_command in extract_inline_command_texts(command_text):
        if inline_command == OPAQUE_INLINE_COMMAND:
            return "repo guard 阻止不透明内联 PowerShell 命令；请改用可审计的明文命令。"
        nested_reason = command_denial_reason(inline_command, cwd_value)
        if nested_reason is not None:
            return nested_reason

    if is_dangerous_git_command(command_text):
        return "repo guard 阻止危险 git 清库命令；请改用精确文件级操作。"

    for pattern in DANGEROUS_GIT_PATTERNS:
        if pattern.search(command_text):
            return "repo guard 阻止危险 git 清库命令；请改用精确文件级操作。"

    if is_dangerous_git_clean(command_text):
        return "repo guard 阻止危险 git 清库命令；请改用精确文件级操作。"

    if SQL_WIPE_PATTERN.search(command_text) and DB_CONTEXT_PATTERN.search(
        command_text
    ):
        return "repo guard 阻止显式数据库清库 SQL；请改用测试库或专用迁移脚本。"

    for target in extract_delete_targets(command_text):
        if (
            is_windows_drive_root_token(target)
            or is_system_drive_root_token(target)
            or is_posix_root_token(target)
        ):
            return "repo guard 阻止删除整盘或文件系统根目录。"

        resolved_target = resolve_candidate_path(target, cwd_value)
        if is_repo_wipe_target(resolved_target):
            return "repo guard 阻止清空当前仓库或删除 .git。"
        if is_runtime_wipe_target(resolved_target):
            return "repo guard 阻止清空 Bill Analyser runtime 数据库状态。"

    return None


def collect_delete_tool_paths(
    tool_input: dict[str, Any],
    cwd_value: Any = None,
) -> tuple[Path | None, ...]:
    """Collect delete-tool paths without policing ordinary write/edit operations."""
    paths: list[Path | None] = []
    for path_key in PATH_KEYS:
        if path_key in tool_input:
            paths.append(resolve_candidate_path(tool_input.get(path_key), cwd_value))
    return tuple(paths)


def evaluate_pre_tool_use(payload: dict[str, Any]) -> str | None:
    """Return a denial reason when the requested tool action is wildly destructive."""
    tool_name = get_tool_name(payload).lower()
    tool_input = get_tool_input(payload)

    if tool_name in COMMAND_TOOL_NAMES:
        return command_denial_reason(
            extract_command_text(tool_input),
            payload.get("cwd"),
        )

    if tool_name in DELETE_TOOL_NAMES:
        for candidate_path in collect_delete_tool_paths(tool_input, payload.get("cwd")):
            if is_repo_wipe_target(candidate_path):
                return "repo guard 阻止删除当前仓库或 .git。"
            if is_runtime_wipe_target(candidate_path):
                return "repo guard 阻止删除 Bill Analyser runtime 数据库状态。"

    return None


def build_hook_output(payload: dict[str, Any], reason: str) -> dict[str, Any]:
    """Build the host-specific deny payload."""
    if "tool_name" in payload:
        return {
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": reason,
            }
        }

    return {
        "permissionDecision": "deny",
        "permissionDecisionReason": reason,
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


if __name__ == "__main__":
    raise SystemExit(main())
