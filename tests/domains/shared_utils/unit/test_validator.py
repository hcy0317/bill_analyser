from __future__ import annotations

from datetime import datetime
from typing import Any

import pytest

from bill_analyser.utils import validator as validator_module
from bill_analyser.utils.validator import BillValidator


@pytest.fixture
def bill_validator() -> BillValidator:
    """Provide a fresh validator instance for each test."""
    return BillValidator()


def test_validate_bill_reports_missing_required_fields(bill_validator: BillValidator) -> None:
    """缺失必填字段时应直接返回缺失列表。"""
    is_valid, errors = bill_validator.validate_bill({
        "date": None,
        "type": "支出",
        "amount": 10,
    })

    assert is_valid is False
    assert "缺少必需字段: date" in errors
    assert "缺少必需字段: counterparty" in errors
    assert "缺少必需字段: description" in errors


@pytest.mark.parametrize(
    "date_text",
    [
        "2025-01-02 03:04:05",
        "2025-01-02",
        "2025/01/02 03:04:05",
        "2025/01/02",
        "2025年01月02日 03:04:05",
        "2025年01月02日",
    ],
)
def test_validate_date_accepts_supported_formats(bill_validator: BillValidator, date_text: str) -> None:
    """日期校验应支持模块声明的所有格式。"""
    assert bill_validator._validate_date(date_text) == (True, None)
    assert bill_validator._validate_date(datetime(2025, 1, 2, 3, 4, 5)) == (True, None)


def test_validate_helpers_reject_invalid_types_and_out_of_range_values(bill_validator: BillValidator) -> None:
    """字段级 helper 应覆盖各自的错误分支。"""
    assert bill_validator._validate_date(123)[0] is False
    assert bill_validator._validate_type(123)[0] is False
    assert bill_validator._validate_type("未知类型")[0] is False
    assert bill_validator._validate_amount("¥1,234.50") == (True, None)
    assert bill_validator._validate_amount("1000000000")[0] is False
    assert bill_validator._validate_amount("-1000000000")[0] is False
    assert bill_validator._validate_amount("oops")[0] is False
    assert bill_validator._validate_counterparty("") == (True, None)
    assert bill_validator._validate_counterparty("x" * 201)[0] is False
    assert bill_validator._validate_counterparty(123)[0] is False
    assert bill_validator._validate_description("账单备注") == (True, None)
    assert bill_validator._validate_description("x" * 501)[0] is False
    assert bill_validator._validate_description(["bad"])[0] is False



def test_validate_bill_collects_all_field_errors(bill_validator: BillValidator) -> None:
    """validate_bill 应累计每个字段的错误，而不是遇到一个就停止。"""
    is_valid, errors = bill_validator.validate_bill({
        "date": "2025-13-40",
        "type": "未知类型",
        "amount": "oops",
        "counterparty": 123,
        "description": {"bad": True},
    })

    assert is_valid is False
    assert len(errors) == 5
    assert any(error.startswith("日期格式无效") for error in errors)
    assert any(error.startswith("交易类型无效") for error in errors)
    assert any(error.startswith("金额格式无效") for error in errors)
    assert any(error.startswith("对方信息必须是字符串") for error in errors)
    assert any(error.startswith("描述必须是字符串") for error in errors)



def test_validate_bills_partitions_invalid_entries_without_mutating_source(bill_validator: BillValidator) -> None:
    """批量验证应把错误信息附加到副本，而不是修改原始账单。"""
    valid_bill = {
        "date": "2025-01-02 03:04:05",
        "type": "收入",
        "amount": "88.80",
        "counterparty": "公司",
        "description": "工资",
    }
    invalid_bill = {
        "date": "bad-date",
        "type": "支出",
        "amount": "18.80",
        "counterparty": "商户",
        "description": "午饭",
    }

    valid_bills, invalid_bills = bill_validator.validate_bills([valid_bill, invalid_bill])

    assert valid_bills == [valid_bill]
    assert len(invalid_bills) == 1
    assert invalid_bills[0]["_index"] == 1
    assert invalid_bills[0]["_validation_errors"]
    assert "_validation_errors" not in invalid_bill
    assert "_index" not in invalid_bill



def test_normalize_bill_normalizes_supported_fields_without_mutating_input(bill_validator: BillValidator) -> None:
    """规范化应转换日期与金额，并修剪字符串字段。"""
    original_bill = {
        "date": "2025/01/02",
        "amount": "¥1,234.50",
        "type": " 支出 ",
        "counterparty": " 商户 ",
        "description": " 午饭 ",
    }

    normalized = bill_validator.normalize_bill(original_bill)

    assert normalized["date"] == "2025-01-02 00:00:00"
    assert normalized["amount"] == 1234.5
    assert normalized["type"] == "支出"
    assert normalized["counterparty"] == "商户"
    assert normalized["description"] == "午饭"
    assert original_bill["date"] == "2025/01/02"
    assert original_bill["amount"] == "¥1,234.50"



def test_normalize_bill_leaves_unparseable_fields_unchanged(bill_validator: BillValidator) -> None:
    """无法解析的日期和金额应保留原值。"""
    original_bill = {
        "date": "not-a-date",
        "amount": "not-a-number",
        "type": "收入",
        "counterparty": "平台",
        "description": "退款",
    }

    normalized = bill_validator.normalize_bill(original_bill)

    assert normalized["date"] == "not-a-date"
    assert normalized["amount"] == "not-a-number"



def test_module_level_helpers_delegate_to_global_validator(monkeypatch: pytest.MonkeyPatch) -> None:
    """模块级快捷函数应委托给全局验证器实例。"""

    class FakeValidator:
        def __init__(self) -> None:
            self.calls: list[tuple[str, Any]] = []

        def validate_bill(self, bill: dict[str, Any]) -> tuple[bool, list[str]]:
            self.calls.append(("validate_bill", bill))
            return True, ["single"]

        def validate_bills(self, bills: list[dict[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
            self.calls.append(("validate_bills", bills))
            return bills, []

        def normalize_bill(self, bill: dict[str, Any]) -> dict[str, Any]:
            self.calls.append(("normalize_bill", bill))
            return {"normalized": True}

    fake_validator = FakeValidator()
    monkeypatch.setattr(validator_module, "_validator", fake_validator)

    bill = {"description": "测试"}
    bills = [bill]

    assert validator_module.validate_bill(bill) == (True, ["single"])
    assert validator_module.validate_bills(bills) == (bills, [])
    assert validator_module.normalize_bill(bill) == {"normalized": True}
    assert fake_validator.calls == [
        ("validate_bill", bill),
        ("validate_bills", bills),
        ("normalize_bill", bill),
    ]
