from __future__ import annotations

from unittest.mock import Mock

import pytest

from bill_analyser.core.database.bills import DatabaseBillsMixin


class _ProbeBillsMixin(DatabaseBillsMixin):
    """Minimal probe for db_bills helper methods."""

    def __init__(self) -> None:
        self.logger = Mock()


@pytest.mark.parametrize(
    ("amount_filter", "expected_condition", "expected_params"),
    [
        ("eq:1", ["amount = ?"], [1.0]),
        ("ne:2", ["amount != ?"], [2.0]),
        ("gt:3", ["amount > ?"], [3.0]),
        ("lt:3.5", ["amount < ?"], [3.5]),
        ("gte:4", ["amount >= ?"], [4.0]),
        ("lte:5", ["amount <= ?"], [5.0]),
        ("between:6:7", ["amount BETWEEN ? AND ?"], [6.0, 7.0]),
    ],
)
def test_apply_amount_filter_covers_supported_dsl_matrix(
    amount_filter: str,
    expected_condition: list[str],
    expected_params: list[float],
) -> None:
    """金额过滤 DSL 应覆盖所有支持的操作符分支。"""
    probe = _ProbeBillsMixin()
    conditions: list[str] = []
    params: list[object] = []

    probe._apply_amount_filter(conditions, params, amount_filter)

    assert conditions == expected_condition
    assert params == expected_params
    probe.logger.warning.assert_not_called()
    probe.logger.error.assert_not_called()


def test_apply_amount_filter_ignores_short_unknown_and_invalid_filters() -> None:
    """金额过滤 DSL 在短格式、未知类型和非法数值下应走安全分支。"""
    probe = _ProbeBillsMixin()

    conditions: list[str] = []
    params: list[object] = []
    probe._apply_amount_filter(conditions, params, "broken")
    assert conditions == []
    assert params == []

    probe._apply_amount_filter(conditions, params, "wat:1")
    assert conditions == []
    assert params == []
    probe.logger.warning.assert_called_once_with("未知的金额过滤器类型: %s", "wat")

    invalid_conditions: list[str] = []
    invalid_params: list[object] = []
    probe._apply_amount_filter(invalid_conditions, invalid_params, "eq:not-a-number")
    assert invalid_conditions == []
    assert invalid_params == []
    probe.logger.error.assert_called_once()


def test_build_bill_filter_conditions_returns_empty_for_missing_filters() -> None:
    """筛选构造器在 filters 为空时应直接返回空条件。"""
    probe = _ProbeBillsMixin()

    assert probe._build_bill_filter_conditions(None, []) == []
    assert probe._build_bill_filter_conditions({}, []) == []


def test_build_bill_filter_conditions_covers_id_batch_text_and_main_only_category_filters() -> None:
    """筛选构造器应拼出 id、batch/text 与仅主分类条件。"""
    probe = _ProbeBillsMixin()
    params: list[object] = []

    conditions = probe._build_bill_filter_conditions(
        {
            "id": 11,
            "batch_id": "batch-001",
            "counterparty": "奶茶店",
            "description": "工作日下午茶",
            "categories": [{"main": "餐饮"}],
        },
        params,
    )

    assert conditions == [
        "id = ?",
        "batch_id = ?",
        "counterparty LIKE ?",
        "description LIKE ?",
        "((main_category = ?))",
    ]
    assert params == [11, "batch-001", "%奶茶店%", "%工作日下午茶%", "餐饮"]


def test_build_bill_filter_conditions_keeps_date_range_and_mixed_category_paths() -> None:
    """筛选构造器应兼容 date_from/date_to 与 mixed category 路径。"""
    probe = _ProbeBillsMixin()
    params: list[object] = []

    conditions = probe._build_bill_filter_conditions(
        {
            "date_from": "2026-04-01",
            "date_to": "2026-04-30",
            "categories": [{"main": "餐饮", "sub": "午餐"}, {"main": "交通"}],
        },
        params,
    )

    assert conditions == [
        "date >= ?",
        "date <= ?",
        "((main_category = ? AND sub_category = ?) OR (main_category = ?))",
    ]
    assert params == ["2026-04-01", "2026-04-30", "餐饮", "午餐", "交通"]


def test_build_bill_filter_conditions_covers_type_category_tag_and_amount_bounds() -> None:
    """筛选构造器应覆盖 type/main/sub、tag_ids 与 min/max amount 条件。"""
    probe = _ProbeBillsMixin()
    params: list[object] = []

    conditions = probe._build_bill_filter_conditions(
        {
            "type": "支出",
            "main_category": "餐饮",
            "sub_category": "早餐",
            "tag_ids": [3, 7],
            "min_amount": -20,
            "max_amount": -5,
        },
        params,
    )

    assert conditions == [
        "type = ?",
        "main_category = ?",
        "sub_category = ?",
        "id IN (SELECT bill_id FROM bill_tags WHERE tag_id IN (?,?))",
        "amount >= ?",
        "amount <= ?",
    ]
    assert params == ["支出", "餐饮", "早餐", 3, 7, -20.0, -5.0]
