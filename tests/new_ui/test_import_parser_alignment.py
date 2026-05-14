"""Parser alignment coverage moved to Rust parser contracts after import route cutover."""

from __future__ import annotations

import pytest

pytestmark = pytest.mark.skip(
    reason=(
        "Python bills import route helpers were deleted; parser alignment is covered by "
        "crates/bill-analyser-parsers/tests/parser_contracts.rs and import_runtime_contract.rs."
    )
)


def test_parser_alignment_moved_to_rust_contracts() -> None:
    """Keep the historical test path as a pointer for repo health rules."""
