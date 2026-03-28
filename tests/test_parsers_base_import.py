"""Parser base import regression tests."""

from __future__ import annotations

import sys
from importlib import import_module
from typing import get_type_hints


def test_importing_parsers_base_and_resolving_from_dict_type_hints_does_not_raise():
    """Importing parser base should not fail on StandardBill self annotations."""
    module_name = "bill_analyser.parsers.base"
    sys.modules.pop(module_name, None)

    base_module = import_module(module_name)
    standard_bill = base_module.StandardBill

    hints = get_type_hints(standard_bill.from_dict)

    assert hints["return"] is standard_bill

    bill = standard_bill.from_dict(
        {
            "date": "2026-03-28 00:00:00",
            "amount": "12.34",
            "type": "支出",
            "description": "早餐",
            "source_account_id": "wechat",
        }
    )

    assert isinstance(bill, standard_bill)
    assert bill.amount == 12.34
    assert bill.description == "早餐"