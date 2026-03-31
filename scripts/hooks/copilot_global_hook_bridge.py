from __future__ import annotations

import json
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
if str(REPO_ROOT) not in sys.path:
    sys.path.insert(0, str(REPO_ROOT))

from scripts.hooks.pre_tool_repo_guard import build_hook_output, load_payload


@dataclass(frozen=True)
class HookSpec:
    hook_id: str
    script_relative_path: str
    profiles: str


@dataclass(frozen=True)
class HookInvocation:
    exit_code: int
    stdout: str
    stderr: str


@dataclass(frozen=True)
class StageResult:
    exit_code: int
    stdout: str = ""
    stderr: str = ""


HOOKS_BY_STAGE: dict[str, tuple[HookSpec, ...]] = {
    "pre-tool": (
        HookSpec("pre:config-protection", "pre-tool/config-protection.js", "standard,strict"),
        HookSpec("pre:secret-scan", "pre-tool/secret-scan.js", "standard,strict"),
        HookSpec("pre:danger-guard", "pre-tool/danger-guard.js", "minimal,standard,strict"),
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


def normalize_stage(raw_stage: str) -> str | None:
    key = str(raw_stage or "").strip().lower().replace("_", "-")
    return STAGE_ALIASES.get(key)


def locate_runner(home: Path | None = None) -> Path:
    base_home = home or Path.home()
    return base_home / ".copilot" / "hooks" / "run-with-flags.js"


def locate_node() -> str | None:
    return shutil.which("node") or shutil.which("node.exe")


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
    result = subprocess.run(
        [node_binary, str(runner_path), spec.hook_id, spec.script_relative_path, spec.profiles],
        cwd=REPO_ROOT,
        input=payload_raw,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
    )

    return HookInvocation(
        exit_code=result.returncode,
        stdout=_strip_raw_passthrough(result.stdout, payload_raw),
        stderr=(result.stderr or "").strip(),
    )


def _merge_messages(parts: list[str]) -> str:
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

        if normalized_stage == "pre-tool" and invocation.exit_code == 2:
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

    return StageResult(
        exit_code=0,
        stdout=_merge_messages(stdout_parts + stderr_parts),
    )


def main(argv: list[str] | None = None) -> int:
    args = argv if argv is not None else sys.argv[1:]
    stage = args[0] if args else ""
    payload_raw = sys.stdin.read()
    result = execute_stage(stage, payload_raw)

    if result.stdout:
        sys.stdout.write(result.stdout)
        if not result.stdout.endswith("\n"):
            sys.stdout.write("\n")
    if result.stderr:
        sys.stderr.write(result.stderr)
        if not result.stderr.endswith("\n"):
            sys.stderr.write("\n")
    return result.exit_code


if __name__ == "__main__":
    raise SystemExit(main())