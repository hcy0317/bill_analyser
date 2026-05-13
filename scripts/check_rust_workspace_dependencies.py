from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


EXPECTED_INTERNAL_GRAPH = {
    "bill-analyser-core": set(),
    "bill-analyser-db": {"bill-analyser-core", "bill-analyser-parsers"},
    "bill-analyser-http": {
        "bill-analyser-core",
        "bill-analyser-db",
        "bill-analyser-parsers",
    },
    "bill-analyser-parsers": {"bill-analyser-core"},
}


def load_workspace_graph(repo_root: Path) -> dict[str, set[str]]:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    payload = json.loads(result.stdout)
    workspace_member_ids = set(payload["workspace_members"])
    workspace_packages = [
        package for package in payload["packages"] if package["id"] in workspace_member_ids
    ]
    workspace_names = {package["name"] for package in workspace_packages}

    graph: dict[str, set[str]] = {}
    for package in workspace_packages:
        internal_deps = {
            dependency["name"]
            for dependency in package.get("dependencies", [])
            if dependency["name"] in workspace_names
        }
        graph[package["name"]] = internal_deps
    return graph


def validate_workspace_graph(graph: dict[str, set[str]]) -> list[str]:
    problems: list[str] = []
    missing_nodes = sorted(set(EXPECTED_INTERNAL_GRAPH) - set(graph))
    if missing_nodes:
        problems.append(f"missing workspace crates: {', '.join(missing_nodes)}")

    extra_nodes = sorted(set(graph) - set(EXPECTED_INTERNAL_GRAPH))
    if extra_nodes:
        problems.append(
            "unexpected workspace crates without policy: " + ", ".join(extra_nodes)
        )

    for crate_name, expected_deps in EXPECTED_INTERNAL_GRAPH.items():
        actual_deps = graph.get(crate_name, set())
        if actual_deps != expected_deps:
            problems.append(
                f"{crate_name}: expected {sorted(expected_deps)}, got {sorted(actual_deps)}"
            )
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Check Rust workspace dependency layering for migration governance."
    )
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    graph = load_workspace_graph(args.repo_root.resolve())
    problems = validate_workspace_graph(graph)
    payload = {
        "expected_internal_graph": {
            crate_name: sorted(deps) for crate_name, deps in EXPECTED_INTERNAL_GRAPH.items()
        },
        "actual_internal_graph": {
            crate_name: sorted(deps) for crate_name, deps in sorted(graph.items())
        },
        "ok": not problems,
        "problems": problems,
    }

    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(json.dumps(payload, ensure_ascii=False, indent=2))

    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main())
