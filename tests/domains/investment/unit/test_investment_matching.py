from __future__ import annotations

import pytest

from bill_analyser.core.bills import BillService
from bill_analyser.core.investment.matching import (
    classify_investment_pnl_change,
    extract_investment_profile,
    is_ordinary_bank_interest_income,
    score_investment_candidate,
)


class FakeAccountDb:
    async def get_account_alias_mapping(self, user_id: int = 1):  # pylint: disable=unused-argument
        return {
            "招商银行": 1,
            "招行": 1,
            "建设银行": 2,
            "建行": 2,
        }

    async def get_all_accounts(self, user_id: int = 1):  # pylint: disable=unused-argument
        return [
            {"id": 1, "name": "招商银行"},
            {"id": 2, "name": "建设银行"},
        ]

    async def get_historical_source_account_suggestion(
        self,
        **kwargs,  # pylint: disable=unused-argument
    ):
        return None

    async def get_historical_destination_account_suggestion(
        self,
        **kwargs,  # pylint: disable=unused-argument
    ):
        return None


def test_investment_matching_scores_platform_product_and_summary_positive() -> None:
    """Known platform + named product + investment action should produce a strong, explainable signal."""
    candidate = score_investment_candidate(
        {
            "type": "支出",
            "counterparty": "蚂蚁财富",
            "payment_method": "支付宝",
            "description": "黄金ETF 自动定投扣款",
            "main_category": "",
            "sub_category": "",
            "original_category": "基金申购",
        }
    )

    assert candidate is not None
    assert candidate["score"] >= 0.65
    assert candidate["platform"] == "蚂蚁财富"
    assert "ETF" in candidate["product"] or "黄金" in candidate["product"]
    assert "platform:蚂蚁财富" in candidate["reason"]


def test_investment_matching_rejects_generic_service_fee_false_positive() -> None:
    """Generic platform/product words in a service-fee summary should not become an investment signal."""
    candidate = score_investment_candidate(
        {
            "type": "支出",
            "counterparty": "基金销售平台",
            "payment_method": "证券开户",
            "description": "账户服务费",
            "main_category": "",
            "sub_category": "",
            "original_category": "基金服务",
        }
    )

    assert candidate is None


def test_investment_matching_keeps_formal_investment_pair_with_specific_platform_action() -> None:
    """Formal investment bills may lack a named product but still be eligible for manual pairing."""
    candidate = score_investment_candidate(
        {
            "type": "投资",
            "counterparty": "pytest matching api",
            "payment_method": "银行卡",
            "description": "蚂蚁财富 手工投资配对 买入",
            "main_category": "转账",
            "sub_category": "历史后配对",
        },
        allow_existing_investment=True,
    )

    assert candidate is not None
    assert candidate["platform"] == "蚂蚁财富"
    assert "type:investment" in candidate["reason"]


def test_investment_profile_does_not_expose_generic_service_fee_as_product() -> None:
    """Profile text should stay specific enough for hover details and avoid generic fee labels."""
    profile = extract_investment_profile("基金销售平台 账户服务费")

    assert profile == {"platform": "基金销售平台", "product": ""}


def test_investment_matching_is_deterministic_for_specific_profile() -> None:
    """Repeated scoring should keep stable score/reason/profile for the same evidence."""
    bill = {
        "type": "支出",
        "counterparty": "天天基金",
        "payment_method": "银行卡",
        "description": "买入 沪深300ETF 申购 扣款",
        "main_category": "",
        "sub_category": "",
        "original_category": "基金申购",
    }

    first_candidate = score_investment_candidate(bill)
    second_candidate = score_investment_candidate(dict(bill))

    assert first_candidate == second_candidate
    assert first_candidate is not None
    assert first_candidate["score"] == 0.92
    assert first_candidate["platform"] == "天天基金"
    assert first_candidate["product"] == "沪深300ETF"
    assert first_candidate["reason"] == "platform:天天基金, product:沪深300ETF/基金/ETF"


def test_investment_pnl_change_detects_gain_loss_and_rejects_generic_income() -> None:
    """投资收益/分红/亏损应形成盈亏变化信号，但普通工资收益不能误报。"""
    gain_signal = classify_investment_pnl_change(
        {
            "type": "投资",
            "counterparty": "天天基金",
            "payment_method": "银行卡",
            "description": "沪深300ETF 分红发放",
            "main_category": "投资理财",
            "sub_category": "基金",
        }
    )
    loss_signal = classify_investment_pnl_change(
        {
            "type": "投资",
            "counterparty": "蚂蚁财富",
            "payment_method": "支付宝",
            "description": "黄金ETF 亏损调整",
            "main_category": "投资理财",
            "sub_category": "基金",
        }
    )

    assert gain_signal is not None
    assert gain_signal["label"] == "盈亏变化"
    assert gain_signal["direction"] == "gain"
    assert "盈亏变化:收益" in gain_signal["reason"]
    assert loss_signal is not None
    assert loss_signal["direction"] == "loss"
    assert "盈亏变化:亏损" in loss_signal["reason"]
    assert classify_investment_pnl_change({"type": "收入", "description": "工资收益"}) is None


def test_bank_settlement_interest_is_income_not_investment_signal() -> None:
    """普通银行结息/利息入账不应进入投资信号或投资盈亏识别。"""
    bill = {
        "type": "收入",
        "amount": 1.23,
        "counterparty": "工商银行",
        "payment_method": "工商银行储蓄卡",
        "description": "账户结息 利息入账",
        "main_category": "",
        "sub_category": "",
        "original_category": "银行结息",
    }

    assert is_ordinary_bank_interest_income(bill) is True
    assert classify_investment_pnl_change(bill) is None
    assert score_investment_candidate(bill) is None


def test_investment_pnl_rows_do_not_score_as_pair_candidates() -> None:
    """投资收益/亏损流水只作为普通收入/支出处理，不再作为投资配对候选。"""
    gain_bill = {
        "type": "投资",
        "counterparty": "天天基金",
        "payment_method": "银行卡",
        "description": "沪深300ETF 分红发放",
        "main_category": "投资理财",
        "sub_category": "基金",
    }
    loss_bill = {
        "type": "投资",
        "counterparty": "蚂蚁财富",
        "payment_method": "支付宝",
        "description": "黄金ETF 亏损调整",
        "main_category": "投资理财",
        "sub_category": "基金",
    }

    assert classify_investment_pnl_change(gain_bill) is not None
    assert classify_investment_pnl_change(loss_bill) is not None
    assert score_investment_candidate(gain_bill, allow_existing_investment=True) is None
    assert score_investment_candidate(loss_bill, allow_existing_investment=True) is None


@pytest.mark.asyncio
async def test_transfer_destination_account_hint_feeds_account_matching() -> None:
    """转账配对得到的去处账户 ID/名称应参与目标账户自动匹配。"""
    service = BillService(db=FakeAccountDb())
    bill = {
        "type": "转账",
        "source_account_id": 1,
        "payment_method": "招商银行",
        "counterparty": "转出",
        "description": "转账到建行",
        "_destination_account_id": 2,
        "_destination_account_name": "建设银行",
        "_destination_parser_id": "ccb",
        "_destination_payment_method": "建设银行储蓄卡",
        "_destination_counterparty": "本人",
    }

    matched_bills = await service._match_accounts([bill])

    assert matched_bills[0]["source_account_id"] == 1
    assert matched_bills[0]["destination_account_id"] == 2
