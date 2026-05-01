"""Investment and bank-interest normalization helpers used by import and matching flows."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    build_user_investment_keyword_settings,
    classify_investment_pnl_change,
    clean_investment_product_name,
    extract_investment_profile,
    is_ordinary_bank_interest_income,
    score_investment_candidate,
)

class InvestmentHelpersMixin:
    """Investment and bank-interest normalization helpers used by import and matching flows."""

    @staticmethod
    def _clear_investment_signal_fields(bill: dict[str, Any]) -> None:
        """Remove transient investment signal fields from a bill-like object."""
        for key in (
            "_investment_hint",
            "_investment_candidate_score",
            "_investment_candidate_reason",
            "_investment_platform",
            "_investment_product",
            "_investment_signal_type",
            "_investment_pnl_direction",
        ):
            bill.pop(key, None)

    def _apply_investment_pnl_type_override(
        self,
        bill: dict[str, Any],
        pnl_signal: dict[str, Any],
    ) -> None:
        """Classify investment gain/loss rows as ordinary income/expense."""
        direction = str(pnl_signal.get("direction", "") or "")
        bill["type"] = "支出" if direction == "loss" else "收入"
        bill["_suppress_investment_signal"] = True
        bill.pop("destination_account_id", None)
        bill.pop("destination_amount", None)
        self._clear_investment_signal_fields(bill)

    def _apply_ordinary_bank_interest_type_override(
        self,
        bill: dict[str, Any],
        *,
        keyword_config: dict[str, list[str]] | None = None,
    ) -> bool:
        """Keep ordinary bank settlement/interest rows on the income path."""
        if not is_ordinary_bank_interest_income(bill, keyword_config=keyword_config):
            return False

        bill["type"] = "收入"
        bill["_suppress_investment_signal"] = True
        bill.pop("destination_account_id", None)
        bill.pop("destination_amount", None)
        self._clear_investment_signal_fields(bill)
        return True

    async def _normalize_non_pair_investment_balance_changes(
        self,
        bills: list[dict[str, Any]],
        *,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        """Normalize bank-interest and investment PnL rows before category matching."""
        if not bills:
            return bills

        keyword_config = await self._get_investment_keyword_config(user_id)
        normalized_count = 0
        for bill in bills:
            if self._apply_ordinary_bank_interest_type_override(
                bill,
                keyword_config=keyword_config,
            ):
                normalized_count += 1
                continue

            pnl_signal = self._classify_investment_pnl_change(
                bill,
                keyword_config=keyword_config,
            )
            if pnl_signal:
                self._apply_investment_pnl_type_override(bill, pnl_signal)
                normalized_count += 1

        if normalized_count:
            self.logger.info(
                "[投资盈亏归一] 已将 %d 条结息/盈亏流水转为普通收入/支出",
                normalized_count,
            )
        return bills

    async def _detect_investment_candidates(
        self, bills: list[dict[str, Any]], user_id: int = 1
    ) -> list[dict[str, Any]]:
        """基于平台/产品关键词识别投资账单。

        导入预览运行态的投资分类已收口到 canonical category_rules。
        保留这个 helper 仅作为兼容层，避免旧调用方继续在分类后触发
        关键字投资检测链路。
        """
        _ = user_id
        if not bills:
            return bills

        cleaned_count = 0
        for bill in bills:
            removed_any = False
            for field in (
                "_investment_hint",
                "_investment_candidate_score",
                "_investment_candidate_reason",
                "_investment_platform",
                "_investment_product",
            ):
                if field in bill:
                    bill.pop(field, None)
                    removed_any = True
            if removed_any:
                cleaned_count += 1

        self.logger.info(
            "[投资分类收口] 已跳过旧 preview 投资检测，清理 %d 条兼容字段",
            cleaned_count,
        )
        return bills

    async def _get_investment_keyword_config(self, user_id: int = 1) -> dict[str, list[str]]:
        """获取用户有效的投资识别关键词配置。"""
        if not hasattr(self.db, "get_user_by_id"):
            return build_user_investment_keyword_settings(None)
        user = await self.db.get_user_by_id(user_id)
        return build_user_investment_keyword_settings(user)

    def _score_investment_candidate(
        self,
        bill: dict[str, Any],
        allow_existing_investment: bool = False,
        keyword_config: dict[str, list[str]] | None = None,
    ) -> dict[str, Any] | None:
        """为单条账单计算投资候选分数。"""
        return score_investment_candidate(
            bill,
            allow_existing_investment=allow_existing_investment,
            keyword_config=keyword_config,
        )

    def _classify_investment_pnl_change(
        self,
        bill: dict[str, Any],
        keyword_config: dict[str, list[str]] | None = None,
    ) -> dict[str, Any] | None:
        """识别投资收益/分红/亏损这类同账户盈亏变化信号。"""
        return classify_investment_pnl_change(bill, keyword_config=keyword_config)

    def _extract_investment_profile(
        self,
        text: str,
        keyword_config: dict[str, list[str]] | None = None,
    ) -> dict[str, str]:
        """提取投资平台与产品归一信息。"""
        return extract_investment_profile(text, keyword_config=keyword_config)

    def _clean_investment_product_name(self, product: str, platform: str = "") -> str:
        """清理提取出的投资产品名，移除平台前缀与交易动作后缀。"""
        return clean_investment_product_name(product, platform=platform)
