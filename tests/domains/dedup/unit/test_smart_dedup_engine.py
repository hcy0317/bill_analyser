from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

from typing import Any

import pandas as pd
import pytest

from bill_analyser.core import smart_dedup as smart_dedup_module
from bill_analyser.core.smart_dedup import DeduplicationType, SmartDeduplicationEngine


class FakeDedupDB:
    """Minimal async DB stub for smart dedup integration tests."""

    def __init__(
        self,
        existing_bills: list[dict[str, Any]] | None = None,
        query_error: Exception | None = None,
        update_error: Exception | None = None,
    ) -> None:
        self.existing_bills = existing_bills or []
        self.query_error = query_error
        self.update_error = update_error
        self.range_queries: list[tuple[str, str, int]] = []
        self.updated_bills: list[tuple[int, dict[str, Any], int]] = []

    async def get_bills_by_date_range(self, start_date: str, end_date: str, user_id: int = 1) -> list[dict[str, Any]]:
        self.range_queries.append((start_date, end_date, user_id))
        if self.query_error is not None:
            raise self.query_error
        return self.existing_bills

    async def update_bill(self, bill_id: int, payload: dict[str, Any], user_id: int = 1) -> None:
        self.updated_bills.append((bill_id, payload, user_id))
        if self.update_error is not None:
            raise self.update_error



def make_bill(**overrides: Any) -> dict[str, Any]:
    """Create a fresh bill payload for dedup tests."""
    bill = {
        "date": "2025-01-02 10:00:00",
        "amount": -50.0,
        "type": "支出",
        "counterparty": "测试商户",
        "payment_method": "默认支付方式",
        "description": "默认描述",
        "source_account_id": "wechat",
        "_parser_id": "wechat",
        "_template_id": 1,
    }
    bill.update(overrides)
    return bill



def test_low_level_helpers_cover_dataframe_datetime_amount_similarity_and_merge_rules() -> None:
    """低层 helper 应稳定处理日期、金额、相似度和字段合并。"""
    engine = SmartDeduplicationEngine()

    assert engine._parse_datetime("2025/01/02 10:00:00") is not None
    assert engine._parse_datetime("bad-date") is None
    dt_close_left = engine._parse_datetime("2025-01-02 10:00:00")
    dt_close_right = engine._parse_datetime("2025-01-02 10:00:25")
    dt_far_right = engine._parse_datetime("2025-01-02 10:01:00")
    assert dt_close_left is not None and dt_close_right is not None and dt_far_right is not None
    assert engine._time_close(dt_close_left, dt_close_right) is True
    assert engine._time_close(dt_close_left, dt_far_right) is False
    assert engine._amount_equal_same_direction(-50.0, -50.005) is True
    assert engine._amount_equal_same_direction(-50.0, 50.0) is False
    assert engine._amount_opposite(-50.0, 50.0) is True
    assert engine._amount_opposite(-50.0, -50.0) is False
    assert engine._calculate_similarity("麦当劳旗舰店", "麦当劳旗舰店") == 1.0
    assert engine._calculate_similarity("", "") == 1.0
    assert engine._calculate_similarity("", "麦当劳") == 0.0

    parser_source_bill = make_bill(source_account_id="account-1", _parser_id="alipay", source="wechat")
    legacy_source_bill = make_bill(source_account_id="abc", _parser_id="", source="")
    assert engine._get_source_type(parser_source_bill) == "alipay"
    assert engine._get_source_type(legacy_source_bill) == "abc"
    assert engine._get_source_priority("wechat") < engine._get_source_priority("unknown")

    assert engine._merge_field_values("麦当劳|肯德基", "肯德基|汉堡王") == "麦当劳 | 肯德基 | 汉堡王"
    assert engine._merge_field_values("交易备注", "交易备注-更完整") == "交易备注-更完整"

    primary_bill = make_bill(counterparty="麦当劳", payment_method="微信", description="午餐")
    secondary_bill = make_bill(
        counterparty="麦当劳旗舰店",
        payment_method="中国银行储蓄卡",
        description="工作日午餐",
        source_account_id="abc",
        _parser_id="abc",
        _template_id=99,
    )
    merged_bill = engine._merge_bill_fields(primary_bill, secondary_bill)
    assert merged_bill["counterparty"] == "麦当劳旗舰店"
    assert merged_bill["payment_method"] == "微信 | 中国银行储蓄卡"
    assert merged_bill["description"] == "工作日午餐"
    assert merged_bill["_merged_template_ids"] == [99]
    assert merged_bill["_merged_from"][0]["_template_id"] == 99

    dataframe = engine._bills_to_dataframe(
        [
            make_bill(date="2025-01-02 10:00:00", _parser_id="wechat", source_account_id="numeric-account"),
            make_bill(date="2025-01-02 10:00:20", _parser_id="", source_account_id="legacy-source"),
        ]
    )
    assert list(dataframe["_source_identifier"]) == ["wechat", "legacy-source"]
    close_pairs = engine._find_time_close_pairs_vectorized(dataframe.iloc[[0]], dataframe.iloc[[1]], tolerance_seconds=30)
    assert close_pairs.to_dict("records") == [{"_idx_1": 0, "_idx_2": 1}]
    assert engine._bills_to_dataframe([]).empty is True
    assert engine._find_time_close_pairs_vectorized(pd.DataFrame(), pd.DataFrame()).empty is True



def test_process_and_singleton_wrapper_handle_exact_duplicates() -> None:
    """process 和便捷 wrapper 应处理完全重复，并复用单例引擎。"""
    smart_dedup_module._engine = None

    duplicate_a = make_bill(counterparty="同一商户", description="同一描述")
    duplicate_b = make_bill(counterparty="同一商户", description="同一描述", source_account_id="abc", _parser_id="abc")

    result = smart_dedup_module.smart_deduplicate([duplicate_a, duplicate_b])

    assert result.original_count == 2
    assert result.removed_count == 1
    assert len(result.kept_bills) == 1
    assert result.duplicate_groups[0].type is DeduplicationType.EXACT
    assert "完全重复" in result.duplicate_groups[0].reason
    assert smart_dedup_module.get_dedup_engine() is smart_dedup_module.get_dedup_engine()



def test_process_marks_platform_bank_duplicates_and_merges_template_ids() -> None:
    """平台/银行重复应保留平台账单并合并模板信息。"""
    engine = SmartDeduplicationEngine()
    platform_bill = make_bill(
        date="2025-01-02 09:30:00",
        amount=-88.0,
        counterparty="蚂蚁基金",
        description="基金购买",
        payment_method="余额宝",
        source_account_id="wallet-a",
        _parser_id="alipay",
        _template_id=48,
    )
    bank_bill = make_bill(
        date="2025-01-02 09:30:10",
        amount=-88.0,
        counterparty="蚂蚁（杭州）基金销售有限公司",
        description="基金",
        payment_method="农业银行储蓄卡",
        source_account_id="bank-a",
        _parser_id="abc",
        _template_id=1,
    )

    result = engine.process([bank_bill, platform_bill])

    assert result.removed_count == 1
    assert len(result.kept_bills) == 1
    kept_bill = result.kept_bills[0]
    assert kept_bill["_parser_id"] == "alipay"
    assert kept_bill["_dedup_type"] == "platform_bank"
    assert 1 in kept_bill["_merged_template_ids"]
    assert result.duplicate_groups[0].type is DeduplicationType.PLATFORM_BANK



def test_platform_bank_sample_wins_before_transfer_for_opposite_sign_duplicate() -> None:
    """样例 platform_bank|555693|553498|553499|553498 不应被转账配对吞掉。"""
    engine = SmartDeduplicationEngine()
    platform_bill = make_bill(
        date="2025-01-02 09:30:00",
        amount=-88.88,
        counterparty="蚂蚁基金销售",
        description="基金交易 余额宝扣款",
        payment_method="支付宝",
        source_account_id="wallet-a",
        _parser_id="alipay",
        _template_id=555693,
    )
    bank_bill = make_bill(
        date="2025-01-02 09:30:08",
        amount=88.88,
        counterparty="蚂蚁基金销售",
        description="基金交易 余额宝扣款",
        payment_method="农业银行",
        source_account_id="bank-a",
        _parser_id="abc",
        _template_id=553498,
        _merged_template_ids=[553499, 553498],
    )

    result = engine.process([platform_bill, bank_bill])

    assert result.transfer_pairs == []
    assert result.removed_count == 1
    assert len(result.kept_bills) == 1
    kept_bill = result.kept_bills[0]
    assert kept_bill["_dedup_type"] == "platform_bank"
    assert kept_bill["type"] != "转账"
    assert result.duplicate_groups[0].type is DeduplicationType.PLATFORM_BANK
    assert {555693, 553498, 553499}.issubset({kept_bill["_template_id"], *kept_bill["_merged_template_ids"]})


def test_platform_bank_opposite_sign_with_transfer_intent_stays_transfer() -> None:
    """带提现/转账语义的异号平台银行账单仍应走转账配对。"""
    engine = SmartDeduplicationEngine()
    outgoing_bill = make_bill(
        date="2025-01-02 09:30:00",
        amount=-100.0,
        counterparty="本人银行卡",
        description="支付宝提现到银行卡",
        payment_method="支付宝",
        source_account_id="wallet-a",
        _parser_id="alipay",
        _template_id=101,
    )
    incoming_bill = make_bill(
        date="2025-01-02 09:30:05",
        amount=100.0,
        counterparty="支付宝提现",
        description="转入 本人银行卡",
        payment_method="工商银行",
        source_account_id="bank-a",
        _parser_id="icbc",
        _template_id=102,
    )

    result = engine.process([outgoing_bill, incoming_bill])

    assert len(result.transfer_pairs) == 1
    assert result.duplicate_groups == []
    assert result.kept_bills[0]["_dedup_type"] == "transfer"


def test_similar_duplicates_prefer_primary_source_and_merge_secondary_fields() -> None:
    """类似账单去重应保留优先级更高的来源，并合并次账单信息。"""
    engine = SmartDeduplicationEngine()
    bill_one = make_bill(
        date="2025-01-02 12:00:00",
        amount=-30.0,
        counterparty="麦当劳旗舰店",
        payment_method="微信支付",
        description="工作日午餐",
        source_account_id="wallet-a",
        _parser_id="",
        _template_id=10,
    )
    bill_two = make_bill(
        date="2025-01-02 12:00:12",
        amount=-30.0,
        counterparty="麦当劳旗舰店(陆家嘴)",
        payment_method="招商银行卡",
        description="午餐",
        source_account_id="wallet-b",
        _parser_id="",
        _template_id=11,
    )

    groups = engine._find_similar_duplicates([bill_one, bill_two])

    assert len(groups) == 1
    group = groups[0]
    assert group.type is DeduplicationType.SIMILAR
    assert group.keep_bill is bill_one
    assert group.remove_bills == [bill_two]
    assert bill_two["_removed"] is True
    assert bill_one["_dedup_type"] == "similar"
    assert 11 in bill_one["_merged_template_ids"]
    assert "相似度" in group.reason



def test_split_bills_remove_total_bill_and_keep_split_details() -> None:
    """总账单与分账单命中时，应移除总账单并保留分账明细。"""
    engine = SmartDeduplicationEngine()
    total_bill = make_bill(
        date="2025-01-02 18:00:00",
        amount=-30.0,
        description="聚合消费",
        source_account_id="alipay",
        _parser_id="alipay",
        _template_id=200,
    )
    split_a = make_bill(
        date="2025-01-02 18:00:05",
        amount=-10.0,
        description="分账A",
        source_account_id="abc",
        _parser_id="abc",
        _template_id=201,
    )
    split_b = make_bill(
        date="2025-01-02 18:00:10",
        amount=-20.0,
        description="分账B",
        source_account_id="abc",
        _parser_id="abc",
        _template_id=202,
    )

    split_groups = engine._find_split_bills([total_bill, split_a, split_b])

    assert len(split_groups) == 1
    assert total_bill["_removed"] is True
    assert split_a["_dedup_type"] == "split"
    assert split_b["_dedup_type"] == "split"
    assert 200 in split_a["_merged_template_ids"]
    assert split_groups[0]["total_amount"] == -30.0
    assert len(split_groups[0]["split_bills"]) == 2



def test_transfer_pairs_use_parser_identifiers_and_skip_same_source_pairs() -> None:
    """转账配对应基于来源标识配对，并跳过同来源账单。"""
    engine = SmartDeduplicationEngine()
    outgoing_bill = make_bill(
        date="2025-01-02 08:00:00",
        amount=-100.0,
        type="支出",
        source_account_id="wallet-a",
        _parser_id="alipay",
        description="转出",
        _template_id=301,
    )
    incoming_bill = make_bill(
        date="2025-01-02 08:00:20",
        amount=100.0,
        type="收入",
        source_account_id="bank-a",
        _parser_id="abc",
        description="转入",
        payment_method="农业银行卡",
        _template_id=302,
    )
    same_source_noise = make_bill(
        date="2025-01-02 08:00:10",
        amount=100.0,
        type="收入",
        source_account_id="wallet-a",
        _parser_id="alipay",
        description="同源噪音",
        _template_id=303,
    )

    transfer_pairs = engine._find_transfer_pairs([outgoing_bill, incoming_bill, same_source_noise])

    assert len(transfer_pairs) == 1
    outgoing, incoming = transfer_pairs[0]
    assert outgoing is outgoing_bill
    assert incoming is incoming_bill
    assert outgoing_bill["type"] == "转账"
    assert incoming_bill["type"] == "转账"
    assert incoming_bill["_removed"] is True
    assert outgoing_bill["_dedup_type"] == "transfer"
    assert outgoing_bill["_destination_parser_id"] == "abc"
    assert outgoing_bill["_merged_template_ids"] == [302]
    assert same_source_noise.get("_removed", False) is False


@pytest.mark.asyncio
async def test_process_with_db_marks_database_duplicates_and_cross_batch_transfers() -> None:
    """带数据库的去重流程应同时处理数据库重复和跨批次转账。"""
    engine = SmartDeduplicationEngine()
    new_duplicate_bill = make_bill(
        date="2025-01-02 09:00:01",
        amount=-50.0,
        counterparty="蚂蚁基金",
        description="基金购买",
        source_account_id="abc",
        _parser_id="abc",
    )
    new_transfer_bill = make_bill(
        date="2025-01-02 10:00:00",
        amount=-100.0,
        counterparty="外部转账",
        description="A侧流水",
        source_account_id="alipay",
        _parser_id="alipay",
        _template_id=401,
    )
    unique_bill = make_bill(
        date="2025-01-02 11:00:00",
        amount=-12.0,
        counterparty="便利店",
        description="早餐",
        source_account_id="wechat",
        _parser_id="wechat",
    )

    db = FakeDedupDB(
        existing_bills=[
            {
                "id": 1,
                "date": "2025-01-02 09:00:00",
                "amount": -50.0,
                "source_account_id": "alipay",
                "counterparty": "蚂蚁（杭州）基金销售有限公司",
                "description": "基金购买",
            },
            {
                "id": 2,
                "date": "2025-01-02 10:01:00",
                "amount": 100.0,
                "source_account_id": "abc",
                "counterparty": "银行入账",
                "description": "B侧记账",
            },
        ]
    )

    result = await engine.process_with_db([new_duplicate_bill, new_transfer_bill, unique_bill], db, user_id=9)

    assert result.original_count == 3
    assert result.removed_count == 1
    assert len(result.kept_bills) == 2
    assert any(group.type is DeduplicationType.DATABASE_DUPLICATE for group in result.duplicate_groups)
    assert new_duplicate_bill.get("_duplicate_of_db_id") == 1
    assert len(result.transfer_pairs) == 1
    assert new_transfer_bill["type"] == "转账"
    assert new_transfer_bill["_dedup_type"] == "transfer_cross_batch"
    assert new_transfer_bill["_cross_batch_db_id"] == 2
    assert db.updated_bills == [(2, {"type": "转账"}, 9)]
    assert db.range_queries


@pytest.mark.asyncio
async def test_database_helper_paths_return_empty_on_query_errors() -> None:
    """数据库查询失败时，两个 DB helper 都应安全降级为空结果。"""
    engine = SmartDeduplicationEngine()
    db = FakeDedupDB(query_error=RuntimeError("db unavailable"))
    bills = [make_bill(date="2025-01-02 10:00:00", amount=-25.0)]

    assert await engine._find_database_duplicates(bills, db, user_id=1) == []
    assert await engine._find_cross_batch_transfer_pairs(bills, db, user_id=1) == []
