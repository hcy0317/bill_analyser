"""用户报告的相似账单去重回归。"""

from difflib import SequenceMatcher

from bill_analyser.core.smart_dedup import SmartDeduplicationEngine


def _similarity(counterparty_a: str | None, counterparty_b: str | None) -> float:
    left = str(counterparty_a or "").strip().lower()
    right = str(counterparty_b or "").strip().lower()
    return SequenceMatcher(None, left, right).ratio()


def test_reported_similarity_scenario_keeps_same_source_bills() -> None:
    """同来源账单即使文本相似，也不应被错误去重。"""
    engine = SmartDeduplicationEngine()
    test_bills = [
        {
            "date": "2025-08-20 17:30:46",
            "amount": -1860.00,
            "type": "支出",
            "source_account_id": "abc",
            "counterparty": "微信支付微信转账",
            "description": "UA0820563404958312微信支付-微信转账 | 微信支付微信转账",
        },
        {
            "date": "2025-08-20 17:30:46",
            "amount": -1860.00,
            "type": "支出",
            "source_account_id": "abc",
            "counterparty": "陈红梅",
            "description": "转账备注:差旅费转账 | 陈红梅 | 农业银行储蓄卡(4071) | 转账",
        },
        {
            "date": "2025-08-20 09:23:53",
            "amount": -50.00,
            "type": "支出",
            "source_account_id": "abc",
            "counterparty": "蚂蚁（杭州）基金销售有限公司",
            "description": "NA2025082061879040930531090310104蚂蚁（杭州）基金销售有限公司",
        },
        {
            "date": "2025-08-20 09:23:52",
            "amount": -50.00,
            "type": "支出",
            "source_account_id": "abc",
            "counterparty": "蚂蚁财富-蚂蚁（杭州）基金销售有限公司",
            "description": "蚂蚁财富-南方红利低波50ETF联接A-买入 | 中国农业银行储蓄卡(4071) | 投资理财",
            "original_category": "投资理财",
        },
    ]

    result = engine.process(test_bills)

    assert result.original_count == 4
    assert len(result.kept_bills) == 4
    assert result.removed_count == 0
    assert result.duplicate_groups == []
    assert result.transfer_pairs == []
    assert result.split_groups == []
    assert _similarity("微信支付微信转账", "陈红梅") < _similarity(
        "蚂蚁（杭州）基金销售有限公司",
        "蚂蚁财富-蚂蚁（杭州）基金销售有限公司",
    )
