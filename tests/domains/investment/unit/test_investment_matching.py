from __future__ import annotations

from bill_analyser.core.investment_matching import (
    extract_investment_profile,
    score_investment_candidate,
)


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
