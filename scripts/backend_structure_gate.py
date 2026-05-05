"""Backend structure size gate for Python modules.

The gate is intentionally baseline-aware: legacy hotspots that already exceed
the hard limits are recorded in ``backend_structure_baseline.json`` and must not
grow. New hard-limit violations fail immediately.
"""

from __future__ import annotations

import argparse
import ast
import json
import re
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


SOURCE_ROOT = Path("src/bill_analyser")
DEFAULT_BASELINE = Path(__file__).with_name("backend_structure_baseline.json")

LEGACY_DB_IMPORT_PATTERN = re.compile(
    r"^\s*from\s+(?:"
    r"bill_analyser\.core\.db_[\w.]*"
    r"|bill_analyser\.core\s+import\s+db_\w+"
    r"|\.+db_[\w.]*"
    r"|\.+\s+import\s+db_\w+"
    r")",
    re.MULTILINE,
)

AI_ROOT_ARTIFACTS = {
    "llm_provider.py",
    "llm_prompts.py",
    "llm_learning_service",
    "ocr_provider.py",
    "ocr_service.py",
    "payment_screenshot_parser.py",
}

LEGACY_AI_MODULES = frozenset(
    {
        "bill_analyser.core.llm_provider",
        "bill_analyser.core.llm_prompts",
        "bill_analyser.core.llm_learning_service",
        "bill_analyser.core.ocr_provider",
        "bill_analyser.core.ocr_service",
        "bill_analyser.core.payment_screenshot_parser",
    }
)

DOMAIN_ROOT_ARTIFACTS = {
    "bill_service.py",
    "bill_service_parts",
    "budget.py",
    "budget_execution_summary.py",
    "investment_matching.py",
    "investment_settings.py",
}

LEGACY_DOMAIN_MODULES = frozenset(
    {
        "bill_analyser.core.bill_service",
        "bill_analyser.core.bill_service_parts",
        "bill_analyser.core.budget",
        "bill_analyser.core.budget_execution_summary",
        "bill_analyser.core.investment_matching",
        "bill_analyser.core.investment_settings",
    }
)

LEGACY_DOMAIN_MODULE_PREFIXES = ("bill_analyser.core.bill_service_parts.",)

FUNCTION_TARGET_LINES = 60
FUNCTION_HARD_LINES = 120

CATEGORY_LIMITS = {
    "route": {"target": 300, "hard": 500},
    "api": {"target": 400, "hard": 800},
    "core_service": {"target": 500, "hard": 800},
    "database": {"target": 500, "hard": 800},
    "model_schema": {"target": 400, "hard": 800},
    "parser": {"target": 400, "hard": 800},
    "utility": {"target": 500, "hard": 800},
    "other": {"target": 500, "hard": 800},
}


@dataclass(frozen=True)
class FunctionRecord:
    path: str
    qualname: str
    lineno: int
    line_count: int

    @property
    def key(self) -> str:
        return f"{self.path}:{self.qualname}"


@dataclass(frozen=True)
class FileRecord:
    path: str
    category: str
    line_count: int
    functions: tuple[FunctionRecord, ...]


@dataclass(frozen=True)
class GateFinding:
    severity: str
    code: str
    path: str
    message: str
    current: int
    limit: int
    qualname: str | None = None


@dataclass(frozen=True)
class GateResult:
    files_scanned: int
    failures: tuple[GateFinding, ...]
    warnings: tuple[GateFinding, ...]
    inventory: tuple[FileRecord, ...]

    @property
    def ok(self) -> bool:
        return not self.failures


def _as_posix(path: Path) -> str:
    return path.as_posix()


def _categorize(path: Path) -> str:
    parts = path.parts
    normalized = path.as_posix()
    if "api" in parts and "routes" in parts:
        return "route"
    if "api" in parts:
        return "api"
    if "/models/" in normalized or "/schemas/" in normalized:
        return "model_schema"
    if "parsers" in parts:
        return "parser"
    if "database" in parts:
        return "database"
    if parts and parts[0] == "core":
        if path.name == "db.py":
            return "database"
    if "utils" in parts:
        return "utility"
    if "core" in parts:
        return "core_service"
    return "other"


def _find_database_layout_violations(repo_root: Path) -> tuple[GateFinding, ...]:
    core_root = repo_root / SOURCE_ROOT / "core"
    if not core_root.exists():
        return ()

    findings: list[GateFinding] = []
    for path in sorted(core_root.iterdir()):
        if not path.name.startswith("db_"):
            continue
        findings.append(
            GateFinding(
                severity="fail",
                code="legacy-db-root-artifact",
                path=_as_posix(path.relative_to(repo_root)),
                message="database implementation must live under src/bill_analyser/core/database",
                current=1,
                limit=0,
            )
        )

    for path in sorted((repo_root / SOURCE_ROOT).rglob("*.py")):
        rel_path = _as_posix(path.relative_to(repo_root))
        text = path.read_text(encoding="utf-8")
        if LEGACY_DB_IMPORT_PATTERN.search(text):
            findings.append(
                GateFinding(
                    severity="fail",
                    code="legacy-db-import",
                    path=rel_path,
                    message="import database implementation through bill_analyser.core.database or core.db",
                    current=1,
                    limit=0,
                )
            )

    return tuple(findings)


def _module_parts_for_file(path: Path, source_root: Path) -> tuple[str, ...]:
    rel_path = path.relative_to(source_root).with_suffix("")
    parts = rel_path.parts
    if parts and parts[-1] == "__init__":
        parts = parts[:-1]
    return ("bill_analyser", *parts)


def _package_parts_for_file(path: Path, source_root: Path) -> tuple[str, ...]:
    module_parts = _module_parts_for_file(path, source_root)
    if path.name == "__init__.py":
        return module_parts
    return module_parts[:-1]


def _is_legacy_ai_module(module_name: str) -> bool:
    return module_name in LEGACY_AI_MODULES or module_name.startswith(
        "bill_analyser.core.llm_learning_service."
    )


def _is_legacy_domain_module(module_name: str) -> bool:
    return module_name in LEGACY_DOMAIN_MODULES or any(
        module_name.startswith(prefix) for prefix in LEGACY_DOMAIN_MODULE_PREFIXES
    )


def _resolved_import_from_candidates(
    node: ast.ImportFrom,
    *,
    path: Path,
    source_root: Path,
) -> tuple[str, ...]:
    if node.level <= 0:
        module_parts = tuple((node.module or "").split(".")) if node.module else ()
    else:
        package_parts = _package_parts_for_file(path, source_root)
        parent_count = node.level - 1
        if parent_count > len(package_parts):
            module_parts = ()
        else:
            module_parts = package_parts[: len(package_parts) - parent_count]
        if node.module:
            module_parts = (*module_parts, *node.module.split("."))

    module_name = ".".join(part for part in module_parts if part)
    candidates = [module_name] if module_name else []
    for alias in node.names:
        if alias.name == "*":
            continue
        candidates.append(f"{module_name}.{alias.name}" if module_name else alias.name)
    return tuple(candidates)


def _imports_legacy_module(
    path: Path,
    source_root: Path,
    text: str,
    is_legacy_module: Any,
) -> bool:
    tree = ast.parse(text, filename=_as_posix(path.relative_to(source_root.parent.parent)))
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            if any(is_legacy_module(alias.name) for alias in node.names):
                return True
            continue
        if isinstance(node, ast.ImportFrom):
            candidates = _resolved_import_from_candidates(
                node,
                path=path,
                source_root=source_root,
            )
            if any(is_legacy_module(candidate) for candidate in candidates):
                return True
    return False


def _imports_legacy_ai_module(path: Path, source_root: Path, text: str) -> bool:
    return _imports_legacy_module(path, source_root, text, _is_legacy_ai_module)


def _imports_legacy_domain_module(path: Path, source_root: Path, text: str) -> bool:
    return _imports_legacy_module(path, source_root, text, _is_legacy_domain_module)


def _find_ai_layout_violations(repo_root: Path) -> tuple[GateFinding, ...]:
    core_root = repo_root / SOURCE_ROOT / "core"
    if not core_root.exists():
        return ()

    source_root = repo_root / SOURCE_ROOT
    findings: list[GateFinding] = []
    for path in sorted(core_root.iterdir()):
        if path.name not in AI_ROOT_ARTIFACTS:
            continue
        findings.append(
            GateFinding(
                severity="fail",
                code="legacy-ai-root-artifact",
                path=_as_posix(path.relative_to(repo_root)),
                message="LLM/OCR implementation must live under src/bill_analyser/core/ai",
                current=1,
                limit=0,
            )
        )

    for path in sorted((repo_root / SOURCE_ROOT).rglob("*.py")):
        rel_path = _as_posix(path.relative_to(repo_root))
        text = path.read_text(encoding="utf-8")
        if _imports_legacy_ai_module(path, source_root, text):
            findings.append(
                GateFinding(
                    severity="fail",
                    code="legacy-ai-import",
                    path=rel_path,
                    message="import LLM/OCR implementation through bill_analyser.core.ai",
                    current=1,
                    limit=0,
                )
            )

    return tuple(findings)


def _find_domain_layout_violations(repo_root: Path) -> tuple[GateFinding, ...]:
    core_root = repo_root / SOURCE_ROOT / "core"
    if not core_root.exists():
        return ()

    source_root = repo_root / SOURCE_ROOT
    findings: list[GateFinding] = []
    for path in sorted(core_root.iterdir()):
        if path.name not in DOMAIN_ROOT_ARTIFACTS:
            continue
        findings.append(
            GateFinding(
                severity="fail",
                code="legacy-domain-root-artifact",
                path=_as_posix(path.relative_to(repo_root)),
                message="bills/budgets/investment implementation must live under matching core domain packages",
                current=1,
                limit=0,
            )
        )

    for path in sorted(source_root.rglob("*.py")):
        rel_path = _as_posix(path.relative_to(repo_root))
        text = path.read_text(encoding="utf-8")
        if _imports_legacy_domain_module(path, source_root, text):
            findings.append(
                GateFinding(
                    severity="fail",
                    code="legacy-domain-import",
                    path=rel_path,
                    message="import bills/budgets/investment implementation through core domain packages",
                    current=1,
                    limit=0,
                )
            )

    return tuple(findings)


def _function_records(tree: ast.AST, rel_path: str) -> tuple[FunctionRecord, ...]:
    records: list[FunctionRecord] = []

    def visit(node: ast.AST, parents: tuple[str, ...] = ()) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(child, ast.ClassDef):
                visit(child, (*parents, child.name))
                continue
            if isinstance(child, (ast.FunctionDef, ast.AsyncFunctionDef)):
                end_lineno = getattr(child, "end_lineno", child.lineno)
                qualname = ".".join((*parents, child.name))
                records.append(
                    FunctionRecord(
                        path=rel_path,
                        qualname=qualname,
                        lineno=child.lineno,
                        line_count=end_lineno - child.lineno + 1,
                    )
                )
                visit(child, (*parents, child.name))
                continue
            visit(child, parents)

    visit(tree)
    return tuple(records)


def scan_backend(repo_root: Path) -> tuple[FileRecord, ...]:
    source_root = repo_root / SOURCE_ROOT
    records: list[FileRecord] = []
    for path in sorted(source_root.rglob("*.py")):
        text = path.read_text(encoding="utf-8")
        rel_path = _as_posix(path.relative_to(repo_root))
        tree = ast.parse(text, filename=rel_path)
        records.append(
            FileRecord(
                path=rel_path,
                category=_categorize(path.relative_to(source_root)),
                line_count=len(text.splitlines()),
                functions=_function_records(tree, rel_path),
            )
        )
    return tuple(records)


def load_baseline(path: Path | None = None) -> dict[str, Any]:
    baseline_path = path or DEFAULT_BASELINE
    if not baseline_path.exists():
        return {"files": {}, "functions": {}}
    with baseline_path.open("r", encoding="utf-8") as handle:
        loaded = json.load(handle)
    return {
        "files": loaded.get("files", {}),
        "functions": loaded.get("functions", {}),
    }


def _file_limit(category: str, key: str) -> int:
    return CATEGORY_LIMITS.get(category, CATEGORY_LIMITS["other"])[key]


def _evaluate_file(record: FileRecord, baseline: dict[str, Any]) -> list[GateFinding]:
    findings: list[GateFinding] = []
    limits = CATEGORY_LIMITS.get(record.category, CATEGORY_LIMITS["other"])
    baseline_record = baseline["files"].get(record.path)
    if record.line_count > limits["hard"]:
        if baseline_record is None:
            findings.append(
                GateFinding(
                    severity="fail",
                    code="file-hard-limit",
                    path=record.path,
                    message=f"{record.category} module exceeds hard line limit",
                    current=record.line_count,
                    limit=limits["hard"],
                )
            )
        elif record.line_count > int(baseline_record["line_count"]):
            findings.append(
                GateFinding(
                    severity="fail",
                    code="legacy-file-grew",
                    path=record.path,
                    message="legacy oversized module grew beyond baseline",
                    current=record.line_count,
                    limit=int(baseline_record["line_count"]),
                )
            )
    elif record.line_count > limits["target"]:
        findings.append(
            GateFinding(
                severity="warn",
                code="file-target-warning",
                path=record.path,
                message=f"{record.category} module exceeds target line count",
                current=record.line_count,
                limit=limits["target"],
            )
        )
    return findings


def _evaluate_function(function: FunctionRecord, baseline: dict[str, Any]) -> list[GateFinding]:
    baseline_record = baseline["functions"].get(function.key)
    if function.line_count > FUNCTION_HARD_LINES:
        if baseline_record is None:
            return [
                GateFinding(
                    severity="fail",
                    code="function-hard-limit",
                    path=function.path,
                    qualname=function.qualname,
                    message="function exceeds hard line limit",
                    current=function.line_count,
                    limit=FUNCTION_HARD_LINES,
                )
            ]
        if function.line_count > int(baseline_record["line_count"]):
            return [
                GateFinding(
                    severity="fail",
                    code="legacy-function-grew",
                    path=function.path,
                    qualname=function.qualname,
                    message="legacy oversized function grew beyond baseline",
                    current=function.line_count,
                    limit=int(baseline_record["line_count"]),
                )
            ]
    elif function.line_count > FUNCTION_TARGET_LINES:
        return [
            GateFinding(
                severity="warn",
                code="function-target-warning",
                path=function.path,
                qualname=function.qualname,
                message="function exceeds target line count",
                current=function.line_count,
                limit=FUNCTION_TARGET_LINES,
            )
        ]
    return []


def run_gate(repo_root: Path, baseline_path: Path | None = None) -> GateResult:
    baseline = load_baseline(baseline_path)
    inventory = scan_backend(repo_root)
    failures: list[GateFinding] = list(_find_database_layout_violations(repo_root))
    failures.extend(_find_ai_layout_violations(repo_root))
    failures.extend(_find_domain_layout_violations(repo_root))
    warnings: list[GateFinding] = []
    for record in inventory:
        for finding in _evaluate_file(record, baseline):
            (failures if finding.severity == "fail" else warnings).append(finding)
        for function in record.functions:
            for finding in _evaluate_function(function, baseline):
                (failures if finding.severity == "fail" else warnings).append(finding)
    return GateResult(
        files_scanned=len(inventory),
        failures=tuple(failures),
        warnings=tuple(warnings),
        inventory=tuple(sorted(inventory, key=lambda item: item.line_count, reverse=True)),
    )


def write_current_baseline(repo_root: Path, output_path: Path) -> None:
    inventory = scan_backend(repo_root)
    files: dict[str, dict[str, Any]] = {}
    functions: dict[str, dict[str, Any]] = {}
    for record in inventory:
        hard_limit = _file_limit(record.category, "hard")
        if record.line_count > hard_limit:
            files[record.path] = {
                "category": record.category,
                "line_count": record.line_count,
                "hard_limit": hard_limit,
                "reason": "legacy oversized module; refactor slices must shrink or remove this exception",
            }
        for function in record.functions:
            if function.line_count > FUNCTION_HARD_LINES:
                functions[function.key] = {
                    "path": function.path,
                    "qualname": function.qualname,
                    "line_count": function.line_count,
                    "hard_limit": FUNCTION_HARD_LINES,
                    "reason": "legacy oversized function; refactor slices must shrink or remove this exception",
                }
    payload = {
        "version": 1,
        "source_root": _as_posix(SOURCE_ROOT),
        "policy": {
            "category_limits": CATEGORY_LIMITS,
            "function_target_lines": FUNCTION_TARGET_LINES,
            "function_hard_lines": FUNCTION_HARD_LINES,
        },
        "files": dict(sorted(files.items())),
        "functions": dict(sorted(functions.items())),
    }
    output_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _result_payload(result: GateResult) -> dict[str, Any]:
    return {
        "ok": result.ok,
        "files_scanned": result.files_scanned,
        "failures": [asdict(item) for item in result.failures],
        "warnings": [asdict(item) for item in result.warnings],
        "top_files": [
            {"path": item.path, "category": item.category, "line_count": item.line_count}
            for item in result.inventory[:40]
        ],
    }


def _print_text_result(result: GateResult, include_inventory: bool) -> None:
    status = "PASS" if result.ok else "FAIL"
    print(f"backend-structure-gate: {status}")
    print(f"files_scanned: {result.files_scanned}")
    print(f"failures: {len(result.failures)}")
    for finding in result.failures[:80]:
        location = finding.path
        if finding.qualname:
            location = f"{location}:{finding.qualname}"
        print(f"FAIL {finding.code} {location} {finding.current}>{finding.limit} {finding.message}")
    print(f"warnings: {len(result.warnings)}")
    for finding in result.warnings[:40]:
        location = finding.path
        if finding.qualname:
            location = f"{location}:{finding.qualname}"
        print(f"WARN {finding.code} {location} {finding.current}>{finding.limit} {finding.message}")
    if include_inventory:
        print("top_files:")
        for record in result.inventory[:40]:
            print(f"{record.line_count:5d} {record.category:13s} {record.path}")


def main() -> int:
    parser = argparse.ArgumentParser(description="Check backend Python file/function size structure.")
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--baseline", type=Path, default=DEFAULT_BASELINE)
    parser.add_argument("--json", action="store_true", help="Print machine-readable result.")
    parser.add_argument("--inventory", action="store_true", help="Print largest files in text mode.")
    parser.add_argument(
        "--write-baseline",
        action="store_true",
        help="Regenerate the legacy violation baseline for the current checkout.",
    )
    args = parser.parse_args()

    repo_root = args.repo_root.resolve()
    baseline_path = args.baseline.resolve()
    if args.write_baseline:
        write_current_baseline(repo_root, baseline_path)
        print(f"wrote backend structure baseline: {baseline_path}")
        return 0

    result = run_gate(repo_root, baseline_path)
    if args.json:
        print(json.dumps(_result_payload(result), ensure_ascii=False, indent=2))
    else:
        _print_text_result(result, include_inventory=args.inventory)
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
