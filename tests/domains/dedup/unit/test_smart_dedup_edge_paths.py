from __future__ import annotations

from datetime import UTC, datetime
from typing import Any

import pandas as pd
import pytest

from bill_analyser.core import smart_dedup as smart_dedup_module
from bill_analyser.core.smart_dedup import SmartDeduplicationEngine


class FakeDedupDB:
    """Minimal async DB stub for smart dedup edge-path tests."""

    def __init__(
        self,
        existing_bills: list[dict[str, Any]] | None = None,
        query_error: Exception | None = None,
        update_error: Exception | None = None,
    ) -> None:
        self.existing_bills = existing_bills or []
        self.query_error = query_error
        self.update_error = update_error
        self.updated_bills: list[tuple[int, dict[str, Any], int]] = []

    async def get_bills_by_date_range(self, start_date: str, end_date: str, user_id: int = 1) -> list[dict[str, Any]]:
        del start_date, end_date, user_id
        if self.query_error is not None:
            raise self.query_error
        return self.existing_bills

    async def update_bill(self, bill_id: int, payload: dict[str, Any], user_id: int = 1) -> None:
        self.updated_bills.append((bill_id, payload, user_id))
        if self.update_error is not None:
            raise self.update_error


def make_bill(**overrides: Any) -> dict[str, Any]:
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


def test_vectorized_helpers_cover_datetime_fallback_source_fallback_and_exact_skip() -> None:
    """低层 helper 应覆盖 fallback 解析、source 回退和 exact skip 路径。"""
    engine = SmartDeduplicationEngine()

    fallback_dataframe = engine._bills_to_dataframe(
        [make_bill(date="2025-01-02", _parser_id="wechat", source_account_id="wallet-a")]
    )
    assert str(fallback_dataframe.loc[0, "datetime"]).startswith("2025-01-02")

    invalid_dataframe = engine._bills_to_dataframe(
        [make_bill(date="bad-date", _parser_id="abc", source_account_id="bank-a", _removed=True)]
    )
    empty_pairs = engine._find_time_close_pairs_vectorized(invalid_dataframe, fallback_dataframe)
    assert empty_pairs.empty is True

    assert engine._merge_field_values("", "保留你") == "保留你"
    assert engine._merge_field_values("保留我", "") == "保留我"
    assert engine._merge_field_values("支付宝|微信", "支付宝(银行卡)|微信") == "支付宝(银行卡) | 微信"
    assert engine._get_source_type(make_bill(_parser_id="", source="cmbc", source_account_id="shadow")) == "cmbc"

    removed_duplicate = make_bill(counterparty="同一商户", description="同一描述")
    active_duplicate = make_bill(
        counterparty="同一商户",
        description="同一描述",
        _parser_id="abc",
        source_account_id="abc",
    )
    duplicate_hash = engine._generate_bill_hash(removed_duplicate)
    removed_duplicate["_dedup_id"] = duplicate_hash
    active_duplicate["_dedup_id"] = duplicate_hash
    removed_duplicate["_removed"] = True
    active_duplicate["_removed"] = False
    assert engine._find_exact_duplicates([removed_duplicate, active_duplicate]) == []


def test_platform_bank_duplicates_cover_empty_dataframe_amount_mismatch_removed_skip_and_template_merge(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """平台/银行去重应覆盖空 DataFrame、金额不匹配与模板合并路径。"""
    engine = SmartDeduplicationEngine()

    monkeypatch.setattr(engine, "_bills_to_dataframe", lambda bills: pd.DataFrame())
    assert engine._find_platform_bank_duplicates(
        [
            make_bill(_parser_id="alipay", source_account_id="wallet-a"),
            make_bill(_parser_id="abc", source_account_id="bank-a"),
        ]
    ) == []

    monkeypatch.undo()
    invalid_date_groups = engine._find_platform_bank_duplicates(
        [
            make_bill(date="bad-date", _parser_id="alipay", source_account_id="wallet-a"),
            make_bill(date="bad-date", _parser_id="abc", source_account_id="bank-a"),
        ]
    )
    assert invalid_date_groups == []

    mismatch_engine = SmartDeduplicationEngine()
    monkeypatch.setattr(
        mismatch_engine,
        "_find_time_close_pairs_vectorized",
        lambda *_args, **_kwargs: pd.DataFrame({"_idx_1": [0], "_idx_2": [0]}),
    )
    mismatch_groups = mismatch_engine._find_platform_bank_duplicates(
        [
            make_bill(date="2025-01-02 10:00:00", amount=-10.0, _parser_id="alipay", source_account_id="wallet-a"),
            make_bill(date="2025-01-02 10:00:10", amount=-11.0, _parser_id="abc", source_account_id="bank-a"),
        ]
    )
    assert mismatch_groups == []

    shared_bank = make_bill(
        date="2025-01-02 09:15:05",
        amount=-66.0,
        source_account_id="bank-shared",
        _parser_id="abc",
    )
    skip_groups = engine._find_platform_bank_duplicates(
        [
            make_bill(date="2025-01-02 09:15:00", amount=-66.0, source_account_id="wallet-a", _parser_id="alipay"),
            make_bill(date="2025-01-02 09:15:02", amount=-66.0, source_account_id="wallet-b", _parser_id="wechat"),
            shared_bank,
        ]
    )
    assert len(skip_groups) == 1

    platform_bill = make_bill(
        date="2025-01-02 09:30:00",
        amount=-88.0,
        counterparty="蚂蚁基金",
        description="基金购买",
        payment_method="余额宝",
        source_account_id="wallet-a",
        _parser_id="alipay",
        _template_id=48,
        _merged_template_ids=[70],
    )
    shared_bank_bill = make_bill(
        date="2025-01-02 09:30:10",
        amount=-88.0,
        counterparty="蚂蚁（杭州）基金销售有限公司",
        description="基金",
        payment_method="农业银行储蓄卡",
        source_account_id="bank-a",
        _parser_id="abc",
        _template_id=72,
        _merged_template_ids=[71],
    )

    groups = engine._find_platform_bank_duplicates([platform_bill, shared_bank_bill, shared_bank_bill])

    assert len(groups) == 1
    assert platform_bill["_merged_template_ids"] == [72, 70, 72, 71]
    assert shared_bank_bill["_removed"] is True



def test_similar_duplicates_cover_invalid_dates_matched_skip_and_template_accumulation() -> None:
    """类似账单去重应覆盖坏日期跳过、matched skip 与 merged template 叠加。"""
    engine = SmartDeduplicationEngine()

    bad_date_groups = engine._find_similar_duplicates(
        [
            make_bill(date="bad-date", source_account_id="wechat", _parser_id=""),
            make_bill(date="2025-01-02 10:00:05", source_account_id="alipay", _parser_id=""),
        ]
    )
    assert bad_date_groups == []

    primary_bill = make_bill(
        date="2025-01-02 12:00:00",
        amount=-30.0,
        counterparty="麦当劳旗舰店",
        payment_method="微信支付",
        description="工作日午餐",
        source_account_id="wechat",
        _parser_id="",
        _template_id=10,
        _merged_template_ids=[81],
    )
    secondary_bill = make_bill(
        date="2025-01-02 12:00:12",
        amount=-30.0,
        counterparty="麦当劳旗舰店(陆家嘴)",
        payment_method="微信支付-银行卡",
        description="午餐",
        source_account_id="alipay",
        _parser_id="",
        _template_id=83,
        _merged_template_ids=[82],
    )
    third_bill = make_bill(
        date="2025-01-02 12:00:15",
        amount=-30.0,
        counterparty="麦当劳旗舰店(浦东)",
        payment_method="微信支付-零钱",
        description="午餐拼单",
        source_account_id="abc",
        _parser_id="",
        _template_id=84,
    )

    groups = engine._find_similar_duplicates([primary_bill, secondary_bill, third_bill])

    assert len(groups) == 1
    assert groups[0].keep_bill is primary_bill
    assert primary_bill["_merged_template_ids"] == [83, 81, 83, 82]
    assert secondary_bill["_removed"] is True


def test_similar_duplicates_cover_controlled_rejection_paths(monkeypatch: pytest.MonkeyPatch) -> None:
    """类似账单去重应覆盖 dt2 缺失、金额不符、符号相反与 removed-object skip。"""
    unstable_engine = SmartDeduplicationEngine()
    unstable_calls: dict[str, int] = {"unstable": 0}

    def _unstable_parse(raw_value: Any):
        text = str(raw_value)
        if text == "stable":
            return datetime(2025, 1, 2, 10, 0, 0, tzinfo=UTC)
        if text == "unstable":
            unstable_calls[text] += 1
            if unstable_calls[text] == 1:
                return datetime(2025, 1, 2, 10, 0, 5, tzinfo=UTC)
            return None
        return None

    monkeypatch.setattr(unstable_engine, "_parse_datetime", _unstable_parse)
    assert unstable_engine._find_similar_duplicates(
        [
            make_bill(date="stable", amount=-20.0, source_account_id="wechat", _parser_id=""),
            make_bill(date="unstable", amount=-20.0, source_account_id="alipay", _parser_id=""),
        ]
    ) == []

    original_float = getattr(smart_dedup_module, "float", float)

    diff_engine = SmartDeduplicationEngine()
    monkeypatch.setattr(
        diff_engine,
        "_parse_datetime",
        lambda _raw: datetime(2025, 1, 2, 10, 0, 0, tzinfo=UTC),
    )
    diff_counts = {"left": 0, "right": 0}

    def _diff_float(raw_value: Any) -> float:
        text = str(raw_value)
        if text == "left":
            diff_counts[text] += 1
            return 10.0
        if text == "right":
            diff_counts[text] += 1
            return 10.0 if diff_counts[text] == 1 else 12.0
        return original_float(raw_value)

    monkeypatch.setattr(smart_dedup_module, "float", _diff_float, raising=False)
    assert diff_engine._find_similar_duplicates(
        [
            make_bill(date="stable", amount="left", source_account_id="wechat", _parser_id=""),
            make_bill(date="stable", amount="right", source_account_id="alipay", _parser_id=""),
        ]
    ) == []

    sign_engine = SmartDeduplicationEngine()
    monkeypatch.setattr(
        sign_engine,
        "_parse_datetime",
        lambda _raw: datetime(2025, 1, 2, 10, 0, 0, tzinfo=UTC),
    )
    sign_counts = {"left": 0, "right": 0}

    def _sign_float(raw_value: Any) -> float:
        text = str(raw_value)
        if text == "left":
            sign_counts[text] += 1
            return 10.0
        if text == "right":
            sign_counts[text] += 1
            return 10.0 if sign_counts[text] == 1 else -10.0
        return original_float(raw_value)

    monkeypatch.setattr(smart_dedup_module, "float", _sign_float, raising=False)
    assert sign_engine._find_similar_duplicates(
        [
            make_bill(date="stable", amount="left", source_account_id="wechat", _parser_id=""),
            make_bill(date="stable", amount="right", source_account_id="alipay", _parser_id=""),
        ]
    ) == []

    monkeypatch.setattr(smart_dedup_module, "float", original_float, raising=False)

    shared_secondary = make_bill(
        date="2025-01-02 12:00:05",
        amount=-20.0,
        counterparty="共享次账单",
        payment_method="共享支付方式",
        description="共享描述",
        source_account_id="abc",
        _parser_id="",
    )
    removed_skip_engine = SmartDeduplicationEngine()
    groups = removed_skip_engine._find_similar_duplicates(
        [
            make_bill(date="2025-01-02 12:00:00", amount=-20.0, source_account_id="wechat", _parser_id=""),
            make_bill(date="2025-01-02 12:00:01", amount=-20.0, source_account_id="wechat", _parser_id=""),
            shared_secondary,
            shared_secondary,
        ]
    )
    assert len(groups) == 1
    assert shared_secondary["_removed"] is True



def test_split_bills_cover_matched_candidate_invalid_date_opposite_direction_and_multi_source_rejection() -> None:
    """分账单识别应覆盖 matched candidate、坏日期、方向不符和多来源拒绝。"""
    engine = SmartDeduplicationEngine()

    total_bill = make_bill(date="2025-01-02 18:00:00", amount=-30.0, source_account_id="alipay", _parser_id="alipay")
    sibling_total = make_bill(date="2025-01-02 18:00:01", amount=-15.0, source_account_id="alipay", _parser_id="alipay")
    split_a = make_bill(date="2025-01-02 18:00:05", amount=-10.0, source_account_id="abc", _parser_id="abc")
    split_b = make_bill(date="2025-01-02 18:00:10", amount=-20.0, source_account_id="abc", _parser_id="abc")
    invalid_date_split = make_bill(date="bad-date", amount=-5.0, source_account_id="abc", _parser_id="abc")
    opposite_split = make_bill(date="2025-01-02 18:00:12", amount=5.0, source_account_id="abc", _parser_id="abc")

    groups = engine._find_split_bills([total_bill, sibling_total, split_a, split_b, invalid_date_split, opposite_split])
    assert len(groups) == 1
    assert total_bill["_removed"] is True

    multi_source_groups = engine._find_split_bills(
        [
            make_bill(date="2025-01-02 19:00:00", amount=-30.0, source_account_id="wechat", _parser_id="wechat"),
            make_bill(date="2025-01-02 19:00:05", amount=-10.0, source_account_id="abc", _parser_id="abc"),
            make_bill(date="2025-01-02 19:00:10", amount=-20.0, source_account_id="cmbc", _parser_id="cmbc"),
        ]
    )
    assert multi_source_groups == []



def test_transfer_pairs_cover_dataframe_empty_source_filter_amount_mismatch_and_removed_object_paths(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """转账配对应覆盖 DataFrame 空、来源过滤、金额不匹配、matched skip 与 removed object。"""
    engine = SmartDeduplicationEngine()

    assert engine._find_transfer_pairs(
        [
            make_bill(date="bad-date", amount=-100.0, source_account_id="wallet-a", _parser_id="alipay"),
            make_bill(date="bad-date", amount=100.0, source_account_id="bank-a", _parser_id="abc", type="收入"),
        ]
    ) == []

    blank_source_pairs = engine._find_transfer_pairs(
        [
            make_bill(date="2025-01-02 08:00:00", amount=-100.0, source_account_id="", _parser_id=""),
            make_bill(date="2025-01-02 08:00:10", amount=100.0, source_account_id="", _parser_id="", type="收入"),
        ]
    )
    assert blank_source_pairs == []

    mismatch_engine = SmartDeduplicationEngine()
    monkeypatch.setattr(
        mismatch_engine,
        "_find_time_close_pairs_vectorized",
        lambda *_args, **_kwargs: pd.DataFrame({"_idx_1": [0], "_idx_2": [1]}),
    )
    assert mismatch_engine._find_transfer_pairs(
        [
            make_bill(
                date="2025-01-02 08:00:00",
                amount=100.0,
                type="收入",
                source_account_id="wallet-a",
                _parser_id="alipay",
            ),
            make_bill(date="2025-01-02 08:00:05", amount=-120.0, source_account_id="bank-a", _parser_id="abc"),
        ]
    ) == []

    empty_merge_engine = SmartDeduplicationEngine()
    monkeypatch.setattr(
        empty_merge_engine,
        "_find_time_close_pairs_vectorized",
        lambda *_args, **_kwargs: pd.DataFrame({"_idx_1": [99], "_idx_2": [99]}),
    )
    assert empty_merge_engine._find_transfer_pairs(
        [
            make_bill(
                date="2025-01-02 08:00:00",
                amount=100.0,
                type="收入",
                source_account_id="wallet-a",
                _parser_id="alipay",
            ),
            make_bill(date="2025-01-02 08:00:05", amount=-100.0, source_account_id="bank-a", _parser_id="abc"),
        ]
    ) == []

    repeated_positive = make_bill(
        date="2025-01-02 08:00:10",
        amount=100.0,
        type="收入",
        source_account_id="wallet-a",
        _parser_id="alipay",
        _template_id=501,
    )
    transfer_engine = SmartDeduplicationEngine()
    monkeypatch.setattr(
        transfer_engine,
        "_find_time_close_pairs_vectorized",
        lambda *_args, **_kwargs: pd.DataFrame({"_idx_1": [0, 1, 1], "_idx_2": [2, 2, 3]}),
    )
    pairs = transfer_engine._find_transfer_pairs(
        [
            repeated_positive,
            repeated_positive,
            make_bill(date="2025-01-02 08:00:00", amount=-100.0, source_account_id="bank-a", _parser_id="abc"),
            make_bill(date="2025-01-02 08:00:05", amount=-100.0, source_account_id="cmbc", _parser_id="cmbc"),
        ]
    )
    assert len(pairs) == 1
    assert repeated_positive["_removed"] is True


@pytest.mark.asyncio
async def test_cross_batch_transfer_pairs_cover_no_dates_skip_paths_positive_branch_and_update_warning() -> None:
    """跨批次转账应覆盖 no-dates、skip 分支、正金额分支与 update warning。"""
    engine = SmartDeduplicationEngine()

    assert await engine._find_cross_batch_transfer_pairs([], FakeDedupDB()) == []
    assert await engine._find_cross_batch_transfer_pairs([make_bill(date="bad-date")], FakeDedupDB()) == []
    assert await engine._find_cross_batch_transfer_pairs(
        [make_bill(date="2025-01-02 10:00:00", amount=100.0, type="收入")],
        FakeDedupDB(existing_bills=[]),
    ) == []

    db = FakeDedupDB(
        existing_bills=[
            {
                "id": 2,
                "date": "2025-01-02 invalid",
                "amount": -100.0,
                "source_account_id": "bank-a",
                "counterparty": "invalid-date",
                "description": "invalid-date",
            },
            {
                "id": 3,
                "date": "2025-01-02 09:30:00",
                "amount": -100.0,
                "source_account_id": "bank-a",
                "counterparty": "too-far",
                "description": "too-far",
            },
            {
                "id": 4,
                "date": "2025-01-02 10:00:10",
                "amount": -100.0,
                "source_account_id": "alipay",
                "counterparty": "same-source",
                "description": "same-source",
            },
            {
                "id": 1,
                "date": "2025-01-02 10:00:05",
                "amount": -100.0,
                "source_account_id": "abc",
                "counterparty": "match-me",
                "description": "match-me",
            },
        ],
        update_error=RuntimeError("cannot update db bill"),
    )
    positive_bill = make_bill(
        date="2025-01-02 10:00:00",
        amount=100.0,
        type="收入",
        source_account_id="alipay",
        _parser_id="alipay",
    )
    already_transfer_bill = make_bill(
        date="2025-01-02 10:00:00",
        amount=100.0,
        type="收入",
        source_account_id="wechat",
        _parser_id="wechat",
        _dedup_type="transfer",
    )
    invalid_date_bill = make_bill(
        date="bad-date",
        amount=100.0,
        type="收入",
        source_account_id="wechat",
        _parser_id="wechat",
    )
    second_positive_bill = make_bill(
        date="2025-01-02 10:00:00",
        amount=100.0,
        type="收入",
        source_account_id="alipay",
        _parser_id="alipay",
    )

    pairs = await engine._find_cross_batch_transfer_pairs(
        [already_transfer_bill, invalid_date_bill, positive_bill, second_positive_bill],
        db,
        user_id=7,
    )

    assert len(pairs) == 1
    assert pairs[0][0] is positive_bill
    assert positive_bill["_dedup_type"] == "transfer_cross_batch"
    assert positive_bill["_cross_batch_db_id"] == 1
    assert db.updated_bills == [(1, {"type": "转账"}, 7)]


@pytest.mark.asyncio
async def test_database_duplicates_cover_empty_date_removed_and_empty_description_fallback() -> None:
    """数据库重复检测应覆盖空输入、坏日期、removed skip、时间/金额拒绝与空描述 fallback。"""
    engine = SmartDeduplicationEngine()

    assert await engine._find_database_duplicates([], FakeDedupDB(), user_id=1) == []
    assert await engine._find_database_duplicates([make_bill(date="bad-date")], FakeDedupDB(), user_id=1) == []
    assert await engine._find_database_duplicates(
        [make_bill(date="2025-01-02 10:00:00")],
        FakeDedupDB(existing_bills=[]),
        user_id=1,
    ) == []

    db = FakeDedupDB(
        existing_bills=[
            {
                "id": 11,
                "date": "2025-01-02 invalid",
                "amount": -50.0,
                "source_account_id": "cmbc",
                "counterparty": "坏日期",
                "description": "坏日期",
            },
            {
                "id": 12,
                "date": "2025-01-02 10:20:00",
                "amount": -50.0,
                "source_account_id": "cmbc",
                "counterparty": "时间过远",
                "description": "时间过远",
            },
            {
                "id": 13,
                "date": "2025-01-02 10:00:05",
                "amount": 50.0,
                "source_account_id": "cmbc",
                "counterparty": "金额不匹配",
                "description": "金额不匹配",
            },
            {
                "id": 14,
                "date": "2025-01-02 10:00:05",
                "amount": -50.0,
                "source_account_id": "cmbc",
                "counterparty": "",
                "description": "",
            },
        ]
    )
    removed_bill = make_bill(date="2025-01-02 10:00:00", _removed=True)
    invalid_bill = make_bill(date="2025-01-02 invalid")
    valid_bill = make_bill(
        date="2025-01-02 10:00:00",
        amount=-50.0,
        source_account_id="abc",
        _parser_id="abc",
        counterparty="",
        description="",
    )

    groups = await engine._find_database_duplicates([removed_bill, invalid_bill, valid_bill], db, user_id=5)

    assert len(groups) == 1
    assert valid_bill["_removed"] is True
    assert valid_bill["_duplicate_of_db_id"] == 14
