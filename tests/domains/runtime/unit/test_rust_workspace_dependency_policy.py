from __future__ import annotations

from pathlib import Path

from scripts import check_rust_workspace_dependencies


def test_workspace_dependency_policy_matches_current_crate_graph() -> None:
    repo_root = Path(__file__).resolve().parents[4]

    graph = check_rust_workspace_dependencies.load_workspace_graph(repo_root)
    problems = check_rust_workspace_dependencies.validate_workspace_graph(graph)

    assert graph == {
        "bill-analyser-core": set(),
        "bill-analyser-db": {"bill-analyser-core", "bill-analyser-parsers"},
        "bill-analyser-http": {
            "bill-analyser-core",
            "bill-analyser-db",
            "bill-analyser-parsers",
        },
        "bill-analyser-parsers": {"bill-analyser-core"},
    }
    assert not problems
