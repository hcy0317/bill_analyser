from __future__ import annotations

import pytest

from bill_analyser.api.routes import budgets as budgets_route


def test_budget_route_parses_csv_and_json_integer_lists() -> None:
    """预算路由 helper 应解析整数列表并拒绝坏值。"""
    assert budgets_route._parse_csv_int_list("1, 2, ,3", "account_ids") == [1, 2, 3]
    assert budgets_route._parse_csv_int_list("", "account_ids") is None
    assert budgets_route._parse_json_int_list([1, " 2 ", ""], "tag_ids") == [1, 2]
    assert budgets_route._parse_json_int_list([], "tag_ids") == []

    with pytest.raises(ValueError, match="account_ids"):
        budgets_route._parse_csv_int_list("1,foo", "account_ids")

    with pytest.raises(ValueError, match="expected JSON array"):
        budgets_route._parse_json_int_list("12", "tag_ids")

    with pytest.raises(ValueError, match="tag_ids"):
        budgets_route._parse_json_int_list(["bad"], "tag_ids")


def test_budget_route_validates_import_payload_and_period_args() -> None:
    """预算导入校验与周期参数校验应覆盖缺字段与非法取值分支。"""
    with pytest.raises(ValueError, match="index 1"):
        budgets_route._validate_import_budget_item("bad-item", 1)

    budgets_route._validate_import_budget_item(
        {
            "category": "餐饮",
            "period_type": "monthly",
            "amount": 88.0,
            "start_date": "2026-01-01",
            "end_date": "2026-01-31",
        },
        0,
    )

    with pytest.raises(ValueError, match="category"):
        budgets_route._validate_import_budget_item(
            {
                "period_type": "monthly",
                "amount": 88.0,
                "start_date": "2026-01-01",
            },
            2,
        )

    with pytest.raises(ValueError, match="amount"):
        budgets_route._validate_import_budget_item(
            {
                "category": "餐饮",
                "period_type": "monthly",
                "start_date": "2026-01-01",
            },
            3,
        )

    with pytest.raises(ValueError, match="period_type"):
        budgets_route._validate_budget_period_args("decade")

    with pytest.raises(ValueError, match="month"):
        budgets_route._validate_budget_period_args("monthly", month=13)

    with pytest.raises(ValueError, match="months_history"):
        budgets_route._validate_budget_period_args("monthly", months_history=0)


def test_budget_route_resolves_explicit_and_calendar_period_ranges() -> None:
    """预算路由应优先使用显式区间，并正确展开季度/年度范围。"""
    assert budgets_route._resolve_budget_period_range(
        "monthly",
        start_date="2026-05-10",
        end_date="2026-05-20",
    ) == ("2026-05-10", "2026-05-20")

    assert budgets_route._resolve_budget_period_range(
        "quarterly",
        year=2026,
        quarter=2,
    ) == ("2026-04-01", "2026-06-30")

    assert budgets_route._resolve_budget_period_range(
        "yearly",
        year=2026,
    ) == ("2026-01-01", "2026-12-31")

    with pytest.raises(ValueError, match="start_date"):
        budgets_route._resolve_budget_period_range(
            "monthly",
            start_date="2026-05-20",
            end_date="2026-05-10",
        )
