"""Core business schema index creation."""

from __future__ import annotations

# pylint: disable=line-too-long
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import aiosqlite


class SchemaCoreIndexesMixin:
    async def _init_core_business_indexes(self, conn: aiosqlite.Connection) -> None:
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_user ON bills(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_date ON bills(date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_type ON bills(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_category ON bills(main_category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_batch ON bills(batch_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bills_hash ON bills(hash)")
        await conn.execute("CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_categories_user ON categories(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_user ON accounts(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_accounts_hidden ON accounts(hidden)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_types_user ON account_types(user_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_types_type ON account_types(type)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_account_transfers_from ON account_transfers(from_account_id)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_transfers_to ON account_transfers(to_account_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_account_transfers_date ON account_transfers(transfer_date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_tags_name ON tags(name)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bill_tags_bill ON bill_tags(bill_id)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_bill_tags_tag ON bill_tags(tag_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_left ON bill_pair_links(user_id, left_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_right ON bill_pair_links(user_id, right_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_links_user_type ON bill_pair_links(user_id, pair_type)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_feedback_user_candidate "
            "ON bill_pair_feedback(user_id, candidate_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_pair_feedback_user_created_at "
            "ON bill_pair_feedback(user_id, created_at DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_transfer_pair_suppressions_user_left "
            "ON bill_transfer_pair_suppressions(user_id, left_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_transfer_pair_suppressions_user_right "
            "ON bill_transfer_pair_suppressions(user_id, right_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_investment_pair_suppressions_user_left "
            "ON bill_investment_pair_suppressions(user_id, left_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_investment_pair_suppressions_user_right "
            "ON bill_investment_pair_suppressions(user_id, right_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_learning_rule_suppressions_user_bill "
            "ON bill_learning_rule_suppressions(user_id, bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_learning_rule_suppressions_user_rule "
            "ON bill_learning_rule_suppressions(user_id, rule_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_status "
            "ON bill_reconciliation_candidates(user_id, family, status)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_session "
            "ON bill_reconciliation_candidates(user_id, session_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_preview "
            "ON bill_reconciliation_candidates(user_id, preview_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_reconciliation_candidates_user_existing "
            "ON bill_reconciliation_candidates(user_id, existing_bill_id)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_merge_groups_user_family "
            "ON bill_merge_groups(user_id, family, status)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_merge_members_group_type "
            "ON bill_merge_members(group_id, member_type)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_bill_merge_events_user_group "
            "ON bill_merge_events(user_id, group_id, created_at DESC)"
        )
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_period ON budgets(period_type)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category, sub_category)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budgets_dates ON budgets(start_date, end_date)")
        await conn.execute("CREATE INDEX IF NOT EXISTS idx_budget_history_budget ON budget_history(budget_id)")
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_budget_history_period ON budget_history(period_start, period_end)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_currencies "
            "ON user_exchange_rates(from_currency, to_currency)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rates_date ON user_exchange_rates(effective_date DESC)"
        )
        await conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_exchange_rate_sources_enabled ON exchange_rate_sources(enabled, priority)"
        )
