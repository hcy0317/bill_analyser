from bill_analyser.core.db import Database


def test_budget_category_helpers_resolve_types_and_fallback_visuals() -> None:
    """预算分类 helper 应正确归一化子分类、回退预算类型并补齐一级分类视觉信息。"""
    db = Database(":memory:")
    categories = [
        {
            "id": 11,
            "type": 3,
            "main_category": "预算测试分类",
            "sub_category": "",
            "icon": "",
            "color": "",
        },
        {
            "id": 12,
            "type": 3,
            "main_category": "预算测试分类",
            "sub_category": "午餐",
            "icon": "fork-spoon",
            "color": "#ffaa00",
        },
        {
            "id": 13,
            "type": 5,
            "main_category": "预算测试分类",
            "sub_category": "",
            "icon": "chart-line",
            "color": "#00aa88",
        },
    ]

    category_context = db._build_budget_category_context(categories)

    assert db._normalize_budget_sub_category(None) == ""
    assert db._normalize_budget_sub_category("   ") == ""
    assert db._normalize_budget_sub_category(" 午餐 ") == "午餐"

    assert db._resolve_budget_category_type(
        "预算测试分类",
        "午餐",
        category_context,
        preferred_type=5,
    ) == 3
    assert db._resolve_budget_category_type(
        "预算测试分类",
        "",
        category_context,
        preferred_type=5,
    ) == 5

    primary_info = db._resolve_budget_category_info(
        "预算测试分类",
        "",
        category_context,
        3,
    )
    assert primary_info is not None
    assert primary_info["id"] == 11
    assert primary_info["icon"] == "fork-spoon"
    assert primary_info["color"] == "#ffaa00"

    sub_category_info = db._resolve_budget_category_info(
        "预算测试分类",
        "午餐",
        category_context,
        3,
    )
    assert sub_category_info is not None
    assert sub_category_info["id"] == 12
    assert sub_category_info["icon"] == "fork-spoon"

    assert db._resolve_budget_category_info("不存在分类", "", category_context, 3) is None


def test_budget_category_type_falls_back_to_preferred_type_when_context_missing() -> None:
    """预算类型推导在缺少分类上下文时应保留 legacy preferred_type 回退。"""
    db = Database(":memory:")
    category_context = db._build_budget_category_context(
        [
            {
                "id": 21,
                "type": 3,
                "main_category": "餐饮",
                "sub_category": "",
                "icon": "fork-spoon",
                "color": "#ffaa00",
            }
        ]
    )

    assert db._resolve_budget_category_type(
        "不存在分类",
        "",
        category_context,
        preferred_type=5,
    ) == 5



def test_budget_date_helpers_normalize_end_date_and_expand_period_windows() -> None:
    """预算日期 helper 应扩展结束日边界、历史窗口，并生成稳定周期 key。"""
    assert Database._normalize_budget_query_end_date(None) is None
    assert Database._normalize_budget_query_end_date("2026-03-31") == "2026-03-31 23:59:59"
    assert Database._normalize_budget_query_end_date("2026-03-31 08:00:00") == "2026-03-31 08:00:00"

    assert Database._expand_forecast_history_window(
        "quarterly",
        "2026-04-01",
        "2026-06-30",
        3,
    ) == ("2025-10-01", "2026-06-30")
    assert Database._expand_forecast_history_window(
        "monthly",
        "2026-03-01",
        "2026-03-31",
        1,
    ) == ("2026-03-01", "2026-03-31")

    assert Database._build_forecast_period_key("daily", "2026-03-11") == "2026-03-11"
    assert Database._build_forecast_period_key("weekly", "2026-03-10") == "2026-10"
    assert Database._build_forecast_period_key("quarterly", "2026-04-01") == "2026-Q2"
    assert Database._build_forecast_period_key("yearly", "2026-01-01") == "2026"
    assert Database._build_forecast_period_key("monthly", "2026-03-01") == "2026-03"

    assert Database._iter_budget_history_period_ranges(
        "weekly",
        "2026-03-10",
        "2026-03-18",
    ) == [
        {"start_date": "2026-03-09", "end_date": "2026-03-15"},
        {"start_date": "2026-03-16", "end_date": "2026-03-22"},
    ]
    assert Database._budget_overlaps_period("2026-03-01", "2026-03-31", "2026-03-10", "2026-03-18") is True
    assert Database._budget_overlaps_period("2026-04-01", "2026-04-30", "2026-03-10", "2026-03-18") is False
