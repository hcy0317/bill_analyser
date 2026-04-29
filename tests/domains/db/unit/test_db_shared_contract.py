from __future__ import annotations

from dataclasses import FrozenInstanceError
from datetime import date
from typing import Any, cast

import pytest

from bill_analyser.core.db_shared import BudgetExecutionRequest, BudgetForecastRequest, DatabaseFacadeBase


class _ProbeFacade(DatabaseFacadeBase):
    """Minimal concrete probe for DatabaseFacadeBase contract stubs."""


def test_budget_request_dataclasses_preserve_defaults_and_are_frozen() -> None:
    """预算请求 dataclass 应暴露稳定默认值并保持不可变。"""
    execution_request = BudgetExecutionRequest()
    forecast_request = BudgetForecastRequest()

    assert execution_request == BudgetExecutionRequest(
        budget_type=3,
        period_type=None,
        start_date=None,
        end_date=None,
        budget_id=None,
        category_id=None,
        account_ids=(),
        tag_ids=(),
        user_id=1,
    )
    assert forecast_request == BudgetForecastRequest(
        budget_type=3,
        period_type="monthly",
        start_date=None,
        end_date=None,
        forecast_strategy="historical_average",
        history_periods=6,
        user_id=1,
    )

    with pytest.raises(FrozenInstanceError):
        execution_request.user_id = 9  # type: ignore[misc]
    with pytest.raises(FrozenInstanceError):
        forecast_request.period_type = "yearly"  # type: ignore[misc]


@pytest.mark.asyncio
async def test_database_facade_base_async_stubs_are_explicit_noops() -> None:
    """DatabaseFacadeBase 的 async 协议桩应显式返回 None。"""
    probe = _ProbeFacade()

    assert await probe.init_db() is None
    assert await probe._get_connection() is None
    assert await probe.sync_account_balance(1) is None
    assert await probe.sync_all_account_balances(user_id=2) is None
    assert await probe.get_all_accounts(user_id=3) is None
    assert await probe.get_all_categories(user_id=4) is None
    assert await probe.get_bill_by_id(5, user_id=6) is None
    assert await probe.get_category_by_id(7, user_id=8) is None
    assert await probe.get_enabled_recurring_templates(user_id=9) is None
    assert await probe.update_import_session_status(
        "session-id",
        "completed",
        total_parsed=1,
        total_preview=2,
        total_confirmed=3,
    ) is None
    assert await probe.get_preview_by_session("session-id", user_id=10, selected_only=True) is None
    assert await probe.get_import_annotation_samples("session-id", user_id=11) is None
    assert await probe.get_import_learning_corpus_samples(user_id=11, limit=5, offset=1) is None
    assert await probe._record_import_learning_rule_log(
        cast("Any", None),
        rule_id=12,
        user_id=13,
        action="promote",
        match_type="description",
        match_value="value",
        normalized_match_value="value",
        session_id="session-id",
        preview_id=14,
        payload={"scope": "pytest"},
    ) is None
    assert await probe.record_import_learning_feedback_event(
        "preview_accept",
        user_id=13,
        rule_id=12,
        preview_id=14,
        payload={"scope": "pytest"},
        conn=cast("Any", None),
    ) is None
    assert await probe.get_budget_by_id(15, user_id=16) is None


def test_database_facade_base_sync_and_class_stubs_are_explicit_noops() -> None:
    """DatabaseFacadeBase 的同步/class/static 协议桩应显式返回 None。"""
    probe = _ProbeFacade()

    assert probe._clear_cache() is None
    assert probe._clear_cache("cache-key") is None
    assert probe._is_cache_valid("cache-key") is None
    assert probe._calculate_hash({"amount": 1}) is None
    assert probe.build_recurring_candidates_for_bill_data(
        {"id": 1},
        [{"id": 2}],
        linked_recurring_id=3,
        tolerance_days=4,
    ) is None
    assert probe._parse_date_value("2026-04-07") is None
    assert probe._get_next_recurring_occurrence_after({"id": 1}, date(2026, 4, 7), max_search_days=7) is None

    assert _ProbeFacade._normalize_import_learning_text("  raw  ") is None
    assert _ProbeFacade.build_composite_match_hash("wechat", "shop", "desc", "支付宝") is None
    assert _ProbeFacade.build_composite_match_features("wechat", "shop", "desc", "支付宝") is None
    assert _ProbeFacade._current_budget_timestamp_text() is None
    assert _ProbeFacade._normalize_budget_query_end_date("2026-04-07") is None
    assert _ProbeFacade._build_budget_history_filter_summary(period_type="monthly", user_id=1) is None
