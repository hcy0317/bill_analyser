"""Shared request bundles and cross-mixin protocol-style stubs for Database."""

# pylint: disable=missing-function-docstring,line-too-long,too-many-instance-attributes,unused-argument,too-many-arguments,too-many-positional-arguments

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from datetime import date, datetime
    from pathlib import Path

    import aiosqlite

BudgetGroupKey = tuple[str, str, str, int]


@dataclass(frozen=True)
class BudgetExecutionRequest:
    """Immutable request bundle for budget execution, snapshot, and history flows."""

    budget_type: int = 3
    period_type: str | None = None
    start_date: str | None = None
    end_date: str | None = None
    budget_id: int | None = None
    category_id: int | None = None
    account_ids: tuple[int, ...] = ()
    tag_ids: tuple[int, ...] = ()
    user_id: int = 1


@dataclass(frozen=True)
class BudgetForecastRequest:
    """Immutable request bundle for budget forecast queries."""

    budget_type: int = 3
    period_type: str = "monthly"
    start_date: str | None = None
    end_date: str | None = None
    forecast_strategy: str = "historical_average"
    history_periods: int = 6
    user_id: int = 1


class DatabaseFacadeBase:  # pylint: disable=too-many-public-methods
    """Shared attribute and cross-domain method declarations for database mixins."""

    logger: Any
    db_path: Path
    _connection: aiosqlite.Connection | None
    batch_size: int
    _cache: dict[str, Any]
    _cache_expiry: dict[str, datetime]
    _cache_ttl: int

    async def init_db(self) -> None:
        ...

    async def _get_connection(self) -> aiosqlite.Connection:
        ...

    def _clear_cache(self, key: str | None = None) -> None:
        ...

    def _is_cache_valid(self, key: str) -> bool:
        ...

    def _calculate_hash(self, bill: dict[str, Any]) -> str:
        ...

    @staticmethod
    def _normalize_import_learning_text(raw_value: Any) -> str:
        ...

    async def sync_account_balance(self, account_id: int) -> bool:
        ...

    async def sync_all_account_balances(self, user_id: int = 1) -> dict[str, Any]:
        ...

    async def get_all_accounts(self, user_id: int = 1) -> list[dict[str, Any]]:
        ...

    async def get_all_categories(self, user_id: int = 1) -> list[dict[str, Any]]:
        ...

    async def get_bill_by_id(self, bill_id: int, user_id: int = 1) -> dict[str, Any] | None:
        ...

    async def get_category_by_id(self, category_id: int, user_id: int = 1) -> dict[str, Any] | None:
        ...

    async def get_enabled_recurring_templates(self, user_id: int = 1) -> list[dict[str, Any]]:
        ...

    def build_recurring_candidates_for_bill_data(
        self,
        bill: dict[str, Any],
        recurring_rows: list[dict[str, Any]],
        linked_recurring_id: Any = None,
        tolerance_days: int = 3,
    ) -> list[dict[str, Any]]:
        ...

    def _parse_date_value(self, raw_value: Any) -> date | None:
        ...

    def _get_next_recurring_occurrence_after(
        self,
        recurring: dict[str, Any],
        after_date: date,
        max_search_days: int = 370,
    ) -> date | None:
        ...

    async def update_import_session_status(
        self,
        session_id: str,
        status: str,
        total_parsed: int | None = None,
        total_preview: int | None = None,
        total_confirmed: int | None = None,
    ) -> bool:
        ...

    async def get_preview_by_session(self, session_id: str, selected_only: bool = False) -> list[dict[str, Any]]:
        ...

    async def get_import_annotation_samples(self, session_id: str, user_id: int = 1) -> list[dict[str, Any]]:
        ...

    @classmethod
    def build_composite_match_hash(
        cls,
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> str | None:
        ...

    @classmethod
    def build_composite_match_features(
        cls,
        parser_id: str,
        counterparty: str,
        description: str,
        payment_method: str,
    ) -> dict[str, str] | None:
        ...

    async def _record_import_learning_rule_log(
        self,
        conn: aiosqlite.Connection,
        *,
        rule_id: int | None,
        user_id: int,
        action: str,
        match_type: str,
        match_value: str,
        normalized_match_value: str,
        session_id: str | None = None,
        preview_id: int | None = None,
        payload: dict[str, Any] | None = None,
    ) -> None:
        ...

    async def get_budget_by_id(self, budget_id: int, user_id: int = 1) -> dict[str, Any] | None:
        ...

    @staticmethod
    def _current_budget_timestamp_text() -> str:
        ...

    @staticmethod
    def _normalize_budget_query_end_date(end_date: str | None) -> str | None:
        ...

    @staticmethod
    def _build_budget_history_filter_summary(
        budget_type: int | None = None,
        period_type: str | None = None,
        budget_id: int | None = None,
        category_id: int | None = None,
        account_ids: list[int] | None = None,
        tag_ids: list[int] | None = None,
    ) -> str:
        ...
