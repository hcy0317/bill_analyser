from __future__ import annotations

from bill_analyser.import_contracts.preview_selection import (
    coerce_preview_selected,
    preview_update_is_selected,
)


def test_preview_selection_contract_normalizes_supported_false_and_true_values() -> None:
    assert coerce_preview_selected(None, default=False) is False
    assert coerce_preview_selected(True) is True
    assert coerce_preview_selected(0) is False
    assert coerce_preview_selected(1) is True
    assert coerce_preview_selected("") is True
    assert coerce_preview_selected("false") is False
    assert coerce_preview_selected("0") is False
    assert coerce_preview_selected("yes") is True
    assert coerce_preview_selected("custom") is True


def test_preview_selection_contract_reads_first_supported_update_key() -> None:
    assert preview_update_is_selected({"isSelected": False}) is False
    assert preview_update_is_selected({"is_selected": "false"}) is False
    assert preview_update_is_selected({"preview_selected": 0}) is False
    assert preview_update_is_selected({"selected": True, "isSelected": False}) is True
    assert preview_update_is_selected({}, default=False) is False
