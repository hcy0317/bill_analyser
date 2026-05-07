"""Bridge repo-local hook entrypoints to optional user-level Copilot hooks."""

from __future__ import annotations

# pylint: disable=wrong-import-position

import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import TextIO

REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.hooks.pre_tool_repo_guard import build_hook_output, load_payload


@dataclass(frozen=True)
class HookSpec:
    """Metadata needed to invoke one global hook."""

    hook_id: str
    script_relative_path: str
    profiles: str


@dataclass(frozen=True)
class HookInvocation:
    """Normalized subprocess result from one hook invocation."""

    exit_code: int
    stdout: str
    stderr: str


@dataclass(frozen=True)
class StageResult:
    """Merged output for one hook stage."""

    exit_code: int
    stdout: str = ""
    stderr: str = ""


HOOKS_BY_STAGE: dict[str, tuple[HookSpec, ...]] = {
    "pre-tool": (
        HookSpec(
            "pre:config-protection",
            "pre-tool/config-protection.js",
            "standard,strict",
        ),
        HookSpec("pre:secret-scan", "pre-tool/secret-scan.js", "standard,strict"),
        HookSpec(
            "pre:danger-guard",
            "pre-tool/danger-guard.js",
            "minimal,standard,strict",
        ),
    ),
    "post-tool": (
        HookSpec("post:quality-reminder", "post-tool/quality-reminder.js", "standard,strict"),
    ),
    "stop": (
        HookSpec("stop:session-persist", "stop/session-persist.js", "standard,strict"),
    ),
}

STAGE_ALIASES = {
    "pre": "pre-tool",
    "pre-tool": "pre-tool",
    "pretool": "pre-tool",
    "pretooluse": "pre-tool",
    "pretoolusehook": "pre-tool",
    "post": "post-tool",
    "post-tool": "post-tool",
    "posttool": "post-tool",
    "posttooluse": "post-tool",
    "posttoolusehook": "post-tool",
    "stop": "stop",
    "stophook": "stop",
}

UTF8 = "utf-8"
GLOBAL_HOOK_TIMEOUT_S = 5
GLOBAL_HOOK_TIMEOUT_EXIT_CODE = 124
ENCODING_TRIGGER = "hook-bridge:windows-stdio-encoding"
ENCODING_ACTION = "force-utf8-stdio-or-buffer-fallback"
OBSERVATION_KEYS_EMITTED: set[str] = set()


def normalize_stage(raw_stage: str) -> str | None:
    """Map host-specific stage aliases to the canonical hook stage names."""
    key = str(raw_stage or "").strip().lower().replace("_", "-")
    return STAGE_ALIASES.get(key)


def locate_runner(home: Path | None = None) -> Path:
    """Locate the user-level Copilot hook runner."""
    base_home = home or Path.home()
    return base_home / ".copilot" / "hooks" / "run-with-flags.js"


def locate_learning_engine(home: Path | None = None) -> Path:
    """Locate the optional learning engine used for environment observations."""
    base_home = home or Path.home()
    return base_home / ".copilot" / "scripts" / "learning-engine.js"


def locate_node() -> str | None:
    """Locate a usable Node.js binary."""
    return shutil.which("node") or shutil.which("node.exe")


def record_learning_observation(
    description: str,
    *,
    context: dict[str, object] | None = None,
    once_key: str | None = None,
) -> None:
    """Best-effort emission of one environment observation to the user-level engine."""
    if once_key and once_key in OBSERVATION_KEYS_EMITTED:
        return

    node_binary = locate_node()
    learning_engine = locate_learning_engine()
    if not node_binary or not learning_engine.exists():
        return

    if once_key:
        OBSERVATION_KEYS_EMITTED.add(once_key)

    payload = {
        "trigger": ENCODING_TRIGGER,
        "action": ENCODING_ACTION,
        "domain": "environment-issue",
        "description": description,
        "context": context or {},
    }

    try:
        subprocess.Popen(  # pylint: disable=consider-using-with
            [
                node_binary,
                str(learning_engine),
                "observe",
                "bill-analyser",
                "pattern-detected",
                json.dumps(payload, ensure_ascii=False),
            ],
            cwd=REPO_ROOT,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
            creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0),
        )
    except (OSError, ValueError):
        if once_key:
            OBSERVATION_KEYS_EMITTED.discard(once_key)
        return


def _is_utf8_encoding(value: str | None) -> bool:
    return str(value or "").strip().lower().replace("_", "-") == UTF8


def ensure_utf8_text_stream(stream: TextIO) -> bool:
    """Try to reconfigure a text stream to UTF-8 in place."""
    encoding = getattr(stream, "encoding", None)
    if _is_utf8_encoding(encoding):
        return False

    reconfigure = getattr(stream, "reconfigure", None)
    if callable(reconfigure):
        try:
            reconfigure(encoding=UTF8, errors="replace")
            return True
        except (AttributeError, LookupError, ValueError):
            return False

    return False


def prepare_standard_streams_for_unicode(
    *,
    stdout: TextIO | None = None,
    stderr: TextIO | None = None,
) -> tuple[str, ...]:
    """Reconfigure stdout/stderr to UTF-8 when the host starts in a legacy code page."""
    changed: list[str] = []
    stdout_stream = stdout or sys.stdout
    stderr_stream = stderr or sys.stderr
    if ensure_utf8_text_stream(stdout_stream):
        changed.append("stdout")
    if ensure_utf8_text_stream(stderr_stream):
        changed.append("stderr")
    if changed:
        record_learning_observation(
            "Hook bridge auto-reconfigured "
            f"{', '.join(changed)} to utf-8 for Unicode-safe hook output.",
            context={"streams": changed, "strategy": "reconfigure"},
            once_key="stdio-reconfigure",
        )
    return tuple(changed)


def write_text(stream: TextIO, text: str, stream_name: str) -> None:
    """Write text with UTF-8 fallbacks when the current stream encoding is unsafe."""
    if not text:
        return

    try:
        stream.write(text)
        return
    except UnicodeEncodeError:
        pass

    buffer = getattr(stream, "buffer", None)
    if buffer is not None:
        buffer.write(text.encode(UTF8, errors="replace"))
        record_learning_observation(
            f"Hook bridge used UTF-8 buffer fallback for {stream_name} "
            "after UnicodeEncodeError.",
            context={"stream": stream_name, "strategy": "buffer-fallback"},
            once_key=f"{stream_name}-buffer-fallback",
        )
        return

    encoding = getattr(stream, "encoding", None) or UTF8
    safe_text = text.encode(encoding, "backslashreplace").decode(encoding, "strict")
    stream.write(safe_text)
    record_learning_observation(
        f"Hook bridge used backslashreplace fallback for {stream_name} "
        "because no binary buffer was available.",
        context={"stream": stream_name, "strategy": "backslashreplace", "encoding": encoding},
        once_key=f"{stream_name}-backslashreplace",
    )


def _strip_raw_passthrough(stdout_text: str, payload_raw: str) -> str:
    if not stdout_text:
        return ""

    stripped_stdout = stdout_text.strip()
    stripped_payload = payload_raw.strip()
    if stripped_stdout and stripped_stdout == stripped_payload:
        return ""

    return stdout_text.strip()


def invoke_global_hook(
    spec: HookSpec,
    payload_raw: str,
    *,
    runner_path: Path,
    node_binary: str,
) -> HookInvocation:
    """Invoke one optional user-level hook and normalize timeout handling."""
    try:
        result = subprocess.run(
            [
                node_binary,
                str(runner_path),
                spec.hook_id,
                spec.script_relative_path,
                spec.profiles,
            ],
            cwd=REPO_ROOT,
            input=payload_raw,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
            timeout=GLOBAL_HOOK_TIMEOUT_S,
        )
    except subprocess.TimeoutExpired:
        return HookInvocation(
            exit_code=GLOBAL_HOOK_TIMEOUT_EXIT_CODE,
            stdout="",
            stderr=f"Global hook `{spec.hook_id}` timed out after {GLOBAL_HOOK_TIMEOUT_S}s.",
        )

    return HookInvocation(
        exit_code=result.returncode,
        stdout=_strip_raw_passthrough(result.stdout, payload_raw),
        stderr=(result.stderr or "").strip(),
    )


def _merge_messages(parts: list[str]) -> str:
    """Join unique non-empty messages in arrival order."""
    merged: list[str] = []
    seen: set[str] = set()
    for part in parts:
        text = part.strip()
        if not text or text in seen:
            continue
        seen.add(text)
        merged.append(text)
    return "\n".join(merged)


def execute_stage(stage: str, payload_raw: str) -> StageResult:
    """Run all configured hooks for one stage and merge the resulting output."""
    normalized_stage = normalize_stage(stage)
    if not normalized_stage:
        return StageResult(exit_code=1, stderr=f"Unknown hook stage: {stage}")

    runner_path = locate_runner()
    node_binary = locate_node()
    if not runner_path.exists() or not node_binary:
        return StageResult(exit_code=0)

    hook_specs = HOOKS_BY_STAGE[normalized_stage]
    payload = load_payload(payload_raw) or {}
    stdout_parts: list[str] = []
    stderr_parts: list[str] = []

    for spec in hook_specs:
        invocation = invoke_global_hook(
            spec,
            payload_raw,
            runner_path=runner_path,
            node_binary=node_binary,
        )

        if invocation.stdout:
            stdout_parts.append(invocation.stdout)
        if invocation.stderr:
            stderr_parts.append(invocation.stderr)

        if normalized_stage == "pre-tool" and invocation.exit_code in {
            2,
            GLOBAL_HOOK_TIMEOUT_EXIT_CODE,
        }:
            reason = invocation.stderr or f"全局 Copilot hook `{spec.hook_id}` 阻止了当前操作。"
            decision = build_hook_output(payload, reason)
            return StageResult(
                exit_code=0,
                stdout=json.dumps(decision, ensure_ascii=False),
            )

    if normalized_stage == "pre-tool":
        return StageResult(
            exit_code=0,
            stderr=_merge_messages(stderr_parts + stdout_parts),
        )

    if normalized_stage == "stop":
        return StageResult(
            exit_code=0,
            stdout="{}",
            stderr=_merge_messages(stdout_parts + stderr_parts),
        )

    return StageResult(
        exit_code=0,
        stdout=_merge_messages(stdout_parts + stderr_parts),
    )


def main(argv: list[str] | None = None) -> int:
    """CLI entrypoint for the Copilot global hook bridge."""
    args = argv if argv is not None else sys.argv[1:]
    stage = args[0] if args else ""
    prepare_standard_streams_for_unicode()
    payload_raw = sys.stdin.read()
    result = execute_stage(stage, payload_raw)

    if result.stdout:
        write_text(sys.stdout, result.stdout, "stdout")
        if not result.stdout.endswith("\n"):
            write_text(sys.stdout, "\n", "stdout")
    if result.stderr:
        write_text(sys.stderr, result.stderr, "stderr")
        if not result.stderr.endswith("\n"):
            write_text(sys.stderr, "\n", "stderr")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
