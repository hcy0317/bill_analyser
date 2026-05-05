from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[4]
CORE_SRC = ROOT / "crates" / "bill-analyser-core" / "src"


def _rust_sources_under(relative: str) -> list[Path]:
    return sorted((CORE_SRC / relative).rglob("*.rs"))


def test_shared_primitives_modules_are_exported_from_core_crate() -> None:
    lib_rs = (CORE_SRC / "lib.rs").read_text(encoding="utf-8")

    assert "pub mod primitives;" in lib_rs
    assert "pub mod adapters;" in lib_rs
    assert "pub use primitives::" in lib_rs


def test_financial_primitives_and_adapters_do_not_use_float_types() -> None:
    sources = _rust_sources_under("primitives") + _rust_sources_under("adapters")
    assert sources, "S2 must add shared Rust primitive and adapter modules"

    float_type = re.compile(r"\bf(?:32|64)\b")
    offenders = {
        str(source.relative_to(ROOT)): float_type.findall(source.read_text(encoding="utf-8"))
        for source in sources
        if float_type.search(source.read_text(encoding="utf-8"))
    }
    assert offenders == {}


def test_migration_plan_records_s2_python_to_rust_mapping() -> None:
    plan_text = (ROOT / "docs" / "rust-backend-migration-plan.md").read_text(encoding="utf-8")

    assert "## S2 Shared Primitives Mapping" in plan_text
    assert "src/bill_analyser/utils/currency.py" in plan_text
    assert "crates/bill-analyser-core/src/primitives/money.rs" in plan_text
    assert "src/bill_analyser/core/bill_date_utils.py" in plan_text
    assert "crates/bill-analyser-core/src/primitives/date_time.rs" in plan_text
    assert "src/bill_analyser/api/adapters/transaction_adapter.py" in plan_text
