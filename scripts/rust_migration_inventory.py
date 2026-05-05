"""Generate the Python-to-Rust backend migration inventory baseline.

S0 is intentionally read-only for runtime code: it records every Python backend
file, gives it an initial migration domain, and keeps ``verified_dead`` empty
until a later slice produces direct evidence.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any


SOURCE_ROOT = Path("src/bill_analyser")
INVENTORY_DOC = Path("docs/rust-backend-migration-inventory.md")
PLAN_DOC = Path("docs/rust-backend-migration-plan.md")

EXCLUDED_RUST_DIRS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
}

FACADE_PATHS = {
    "src/bill_analyser/__init__.py",
    "src/bill_analyser/api/__init__.py",
    "src/bill_analyser/api/app.py",
    "src/bill_analyser/core/__init__.py",
    "src/bill_analyser/core/db.py",
    "src/bill_analyser/core/report.py",
    "src/bill_analyser/import_contracts/__init__.py",
    "src/bill_analyser/parsers/__init__.py",
    "src/bill_analyser/parsers/parser_tags.py",
    "src/bill_analyser/utils/__init__.py",
}

PACKAGE_IMPLEMENTATION_LINE_THRESHOLD = 80

ROUTE_DOMAIN_BY_SEGMENT = {
    "accounts": "accounts",
    "auth": "auth-security",
    "backup": "backup-operations",
    "bills": "bills-import",
    "budgets": "budgets",
    "categories": "classification-rules",
    "llm": "ai-learning-llm",
    "matching": "matching-reconciliation",
    "statistics": "statistics-reporting",
}

ROUTE_DOMAIN_BY_FILE = {
    "calendar.py": "recurring-calendar",
    "category_rules.py": "classification-rules",
    "encryption.py": "auth-security",
    "insights.py": "statistics-reporting",
    "learning.py": "ai-learning-llm",
    "networth.py": "statistics-reporting",
    "receipt_ocr.py": "ai-ocr",
    "recurring.py": "recurring-calendar",
    "request_context_helpers.py": "api-runtime-shell",
    "rules.py": "classification-rules",
    "settings_bundle.py": "settings-bundle",
    "tags.py": "tags-templates",
    "templates.py": "tags-templates",
}

DATABASE_DOMAIN_BY_SEGMENT = {
    "accounts": "accounts",
    "audit_backup": "backup-operations",
    "bills": "bills-import",
    "budgets": "budgets",
    "categories": "classification-rules",
    "category_rules": "classification-rules",
    "imports": "bills-import",
    "llm": "ai-learning-llm",
    "matching": "matching-reconciliation",
    "reconciliation": "matching-reconciliation",
    "recurring_suggestions": "recurring-calendar",
    "settings_bundle": "settings-bundle",
    "tags": "tags-templates",
    "templates": "tags-templates",
    "users": "auth-security",
}

CORE_DOMAIN_BY_SEGMENT = {
    "ai": "ai-learning-llm",
    "bills": "bills-import",
    "budgets": "budgets",
    "category_engine": "classification-rules",
    "database": "database-facade",
    "exchange_rate_providers": "statistics-reporting",
    "import_learning": "bills-import",
    "investment": "matching-reconciliation",
    "matching": "matching-reconciliation",
    "smart_dedup": "smart-dedup",
}

CORE_DOMAIN_BY_FILE = {
    "analyzer.py": "statistics-reporting",
    "bill_date_utils.py": "shared-primitives",
    "constants.py": "shared-primitives",
    "db.py": "database-facade",
    "default_category_seed.py": "classification-rules",
    "recurring_detection.py": "recurring-calendar",
    "report.py": "statistics-reporting",
    "sync.py": "sync-runtime",
}

UTILS_DOMAIN_BY_SEGMENT = {
    "charting": "statistics-reporting",
}

UTILS_DOMAIN_BY_FILE = {
    "charts.py": "statistics-reporting",
    "config.py": "api-runtime-shell",
    "constants.py": "shared-primitives",
    "currency.py": "shared-primitives",
    "logger.py": "api-runtime-shell",
    "report_export.py": "statistics-reporting",
    "validator.py": "shared-primitives",
}


@dataclass(frozen=True)
class PythonFileRecord:
    path: str
    domain: str
    role: str
    initial_status: str
    line_count: int


@dataclass(frozen=True)
class RustMigrationInventory:
    summary: dict[str, int]
    python_files: tuple[PythonFileRecord, ...]
    rust_files: tuple[str, ...]

    def to_dict(self) -> dict[str, Any]:
        return {
            "summary": dict(self.summary),
            "python_files": [asdict(record) for record in self.python_files],
            "rust_files": list(self.rust_files),
        }


def _as_repo_path(path: Path, repo_root: Path) -> str:
    return path.relative_to(repo_root).as_posix()


def _read_line_count(path: Path) -> int:
    return len(path.read_text(encoding="utf-8").splitlines())


def _path_parts(rel_path: str) -> tuple[str, ...]:
    return tuple(rel_path.split("/"))


def _role_for_path(rel_path: str) -> str:
    parts = _path_parts(rel_path)
    if rel_path.endswith("/__init__.py") or rel_path.endswith("__init__.py"):
        return "package-marker"
    if rel_path == "src/bill_analyser/api/app.py":
        return "api-shell"
    if "/api/adapters/" in rel_path:
        return "api-adapter"
    if "/api/middleware/" in rel_path:
        return "api-middleware"
    if "/api/config/" in rel_path:
        return "api-config"
    if "/api/routes/" in rel_path:
        return "api-route"
    if "/core/database/schema/" in rel_path:
        return "database-schema"
    if "/core/database/" in rel_path:
        return "database-access"
    if "/parsers/" in rel_path:
        return "import-parser"
    if "/import_contracts/" in rel_path:
        return "import-contract"
    if "/utils/" in rel_path:
        return "utility"
    if len(parts) > 2 and parts[2] == "core":
        return "core-service"
    return "backend-module"


def _domain_for_api_route(rel_path: str) -> str:
    route_suffix = rel_path.removeprefix("src/bill_analyser/api/routes/")
    first_segment = route_suffix.split("/", 1)[0]
    if first_segment in ROUTE_DOMAIN_BY_SEGMENT:
        return ROUTE_DOMAIN_BY_SEGMENT[first_segment]
    return ROUTE_DOMAIN_BY_FILE.get(first_segment, "api-runtime-shell")


def _domain_for_database(rel_path: str) -> str:
    db_suffix = rel_path.removeprefix("src/bill_analyser/core/database/")
    first_segment = db_suffix.split("/", 1)[0]
    if first_segment == "schema":
        return "database-schema"
    if first_segment in {"runtime.py", "shared.py", "time.py", "__init__.py", "encryption.py"}:
        if first_segment == "encryption.py":
            return "auth-security"
        return "database-facade"
    return DATABASE_DOMAIN_BY_SEGMENT.get(first_segment, "database-facade")


def _domain_for_core(rel_path: str) -> str:
    core_suffix = rel_path.removeprefix("src/bill_analyser/core/")
    first_segment = core_suffix.split("/", 1)[0]
    if first_segment in CORE_DOMAIN_BY_SEGMENT:
        if first_segment == "ai" and "/ocr/" in rel_path:
            return "ai-ocr"
        return CORE_DOMAIN_BY_SEGMENT[first_segment]
    return CORE_DOMAIN_BY_FILE.get(first_segment, "shared-primitives")


def _domain_for_utils(rel_path: str) -> str:
    utils_suffix = rel_path.removeprefix("src/bill_analyser/utils/")
    first_segment = utils_suffix.split("/", 1)[0]
    if first_segment in UTILS_DOMAIN_BY_SEGMENT:
        return UTILS_DOMAIN_BY_SEGMENT[first_segment]
    return UTILS_DOMAIN_BY_FILE.get(first_segment, "shared-primitives")


def _domain_for_path(rel_path: str) -> str:
    if rel_path in {"src/bill_analyser/__init__.py", "src/bill_analyser/constants.py"}:
        return "shared-primitives"
    if rel_path == "src/bill_analyser/api/__init__.py":
        return "api-runtime-shell"
    if rel_path == "src/bill_analyser/api/app.py":
        return "api-runtime-shell"
    if "/api/adapters/" in rel_path:
        return "api-contract-adapters"
    if "/api/middleware/" in rel_path or "/api/config/" in rel_path:
        return "api-runtime-shell"
    if "/api/routes/" in rel_path:
        return _domain_for_api_route(rel_path)
    if "/core/database/" in rel_path:
        return _domain_for_database(rel_path)
    if "/core/" in rel_path:
        return _domain_for_core(rel_path)
    if "/import_contracts/" in rel_path:
        return "import-contracts"
    if "/parsers/" in rel_path:
        return "import-parsers"
    if "/utils/" in rel_path:
        return _domain_for_utils(rel_path)
    return "shared-primitives"


def _initial_status_for_path(rel_path: str, role: str) -> str:
    if rel_path in FACADE_PATHS or role == "package-marker":
        return "facade"
    if role in {"api-shell", "api-adapter", "api-config", "api-middleware", "import-contract"}:
        return "facade"
    return "port"


def _python_files(repo_root: Path) -> tuple[PythonFileRecord, ...]:
    source_root = repo_root / SOURCE_ROOT
    records: list[PythonFileRecord] = []
    for path in sorted(source_root.rglob("*.py")):
        rel_path = _as_repo_path(path, repo_root)
        line_count = _read_line_count(path)
        role = _role_for_path(rel_path)
        if role == "package-marker" and line_count > PACKAGE_IMPLEMENTATION_LINE_THRESHOLD:
            role = "package-implementation"
        records.append(
            PythonFileRecord(
                path=rel_path,
                domain=_domain_for_path(rel_path),
                role=role,
                initial_status=_initial_status_for_path(rel_path, role),
                line_count=line_count,
            )
        )
    return tuple(records)


def _is_excluded_rust_path(path: Path, repo_root: Path) -> bool:
    rel_parts = path.relative_to(repo_root).parts
    return any(part in EXCLUDED_RUST_DIRS for part in rel_parts)


def _rust_files(repo_root: Path) -> tuple[str, ...]:
    return tuple(
        sorted(
            _as_repo_path(path, repo_root)
            for path in repo_root.rglob("*.rs")
            if path.is_file() and not _is_excluded_rust_path(path, repo_root)
        )
    )


def build_inventory(repo_root: Path | str = Path.cwd()) -> RustMigrationInventory:
    root = Path(repo_root).resolve()
    python_files = _python_files(root)
    rust_files = _rust_files(root)
    status_counts = Counter(record.initial_status for record in python_files)
    domain_counts = Counter(record.domain for record in python_files)
    summary = {
        "python_backend_files": len(python_files),
        "rust_backend_files": len(rust_files),
        "verified_dead_files": status_counts.get("verified_dead", 0),
        "migration_domains": len(domain_counts),
        "port_files": status_counts.get("port", 0),
        "facade_files": status_counts.get("facade", 0),
        "deferred_files": status_counts.get("deferred", 0),
    }
    return RustMigrationInventory(summary=summary, python_files=python_files, rust_files=rust_files)


def _domain_rows(inventory: RustMigrationInventory) -> list[tuple[str, int, int, int]]:
    counts: dict[str, Counter[str]] = {}
    for record in inventory.python_files:
        counts.setdefault(record.domain, Counter())[record.initial_status] += 1
    return [
        (domain, counter.get("port", 0), counter.get("facade", 0), counter.get("deferred", 0))
        for domain, counter in sorted(counts.items())
    ]


def render_inventory_markdown(inventory: RustMigrationInventory) -> str:
    lines = [
        "# Rust Backend Migration Inventory",
        "",
        "This S0 baseline is generated from a deterministic filesystem scan.",
        "It does not change runtime behavior and does not mark any Python business file as dead.",
        "",
        "## Summary",
        "",
        f"- Python backend files: {inventory.summary['python_backend_files']}",
        f"- Rust backend files: {inventory.summary['rust_backend_files']}",
        f"- Migration domains: {inventory.summary['migration_domains']}",
        f"- Files marked port: {inventory.summary['port_files']}",
        f"- Files marked facade: {inventory.summary['facade_files']}",
        f"- Files marked deferred: {inventory.summary['deferred_files']}",
        f"- Verified dead files: {inventory.summary['verified_dead_files']}",
        "",
        "## Domain Counts",
        "",
        "| Domain | Port | Facade | Deferred |",
        "| --- | ---: | ---: | ---: |",
    ]
    for domain, port_count, facade_count, deferred_count in _domain_rows(inventory):
        lines.append(f"| {domain} | {port_count} | {facade_count} | {deferred_count} |")

    lines.extend(
        [
            "",
            "## Python File Matrix",
            "",
            "| Path | Migration domain | Role | Initial status | Lines |",
            "| --- | --- | --- | --- | ---: |",
        ]
    )
    for record in inventory.python_files:
        lines.append(
            f"| {record.path} | {record.domain} | {record.role} | "
            f"{record.initial_status} | {record.line_count} |"
        )

    lines.extend(
        [
            "",
            "## Rust Backend Files",
            "",
        ]
    )
    if inventory.rust_files:
        for path in inventory.rust_files:
            lines.append(f"- {path}")
    else:
        lines.append("- None detected in S0.")

    lines.extend(
        [
            "",
            "## Verified Dead Baseline",
            "",
            "No Python backend file is marked `verified_dead` in S0.",
            "Later slices may use this inventory as the comparison surface, but any deletion requires direct evidence.",
        ]
    )
    return "\n".join(lines) + "\n"


def render_plan_markdown(inventory: RustMigrationInventory) -> str:
    lines = [
        "# Rust Backend Migration Plan Baseline",
        "",
        "S0 establishes the auditable contract used by later Python-to-Rust migration slices.",
        "S0 does not add a Rust runtime, does not change Flask route behavior, and does not delete Python code.",
        "",
        "## Preservation Rules",
        "",
        "- Every Python backend file remains preserved unless a later slice proves verified-dead with direct evidence.",
        "- Flask REST remains the runtime shell until a later slice introduces and verifies a Rust implementation boundary.",
        "- `/api/v1/*` is not revived as a runtime chain.",
        "- Business behavior, amount units, time semantics, DB isolation, and response envelopes remain unchanged.",
        "",
        "## Initial Migration Surface",
        "",
        f"- Python backend files to track: {inventory.summary['python_backend_files']}",
        f"- Current Rust backend files: {inventory.summary['rust_backend_files']}",
        f"- Initial verified-dead files: {inventory.summary['verified_dead_files']}",
        "",
        "## Domain Review Baseline",
        "",
        "| Domain | Port | Facade | Deferred |",
        "| --- | ---: | ---: | ---: |",
    ]
    for domain, port_count, facade_count, deferred_count in _domain_rows(inventory):
        lines.append(f"| {domain} | {port_count} | {facade_count} | {deferred_count} |")

    lines.extend(
        [
            "",
            "## Review Contract",
            "",
            "- Code-bug reviews compare changed inventory tooling and docs against this deterministic scan.",
            "- Feature-gap reviews use the Python File Matrix to prove each backend item is ported, facade-only, deferred with reason, or verified-dead with evidence.",
            "- A later slice cannot claim a domain complete while any file in that domain is unmapped.",
        ]
    )
    return "\n".join(lines) + "\n"


def write_docs(repo_root: Path | str = Path.cwd()) -> RustMigrationInventory:
    root = Path(repo_root).resolve()
    inventory = build_inventory(root)
    (root / INVENTORY_DOC).write_text(render_inventory_markdown(inventory), encoding="utf-8")
    (root / PLAN_DOC).write_text(render_plan_markdown(inventory), encoding="utf-8")
    return inventory


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate Rust backend migration inventory docs.")
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--write-docs", action="store_true", help="Write migration inventory and plan docs.")
    parser.add_argument("--json", action="store_true", help="Print the inventory as JSON.")
    args = parser.parse_args()

    inventory = write_docs(args.repo_root) if args.write_docs else build_inventory(args.repo_root)
    if args.json:
        print(json.dumps(inventory.to_dict(), ensure_ascii=False, indent=2))
    elif not args.write_docs:
        print(render_inventory_markdown(inventory), end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
