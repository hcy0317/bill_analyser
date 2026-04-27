from __future__ import annotations

# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false
import json
from datetime import datetime
from typing import Any, cast

import pytest

from bill_analyser.core.bill_service import BillService
from bill_analyser.utils.constants import TransactionType


class FakeBillServiceDB:
    """Minimal async DB stub for BillService helper tests."""

    def __init__(self) -> None:
        self.user: dict[str, Any] | None = {
            "id": 1,
            "cash_account_id": 99,
            "cash_transfer_category_id": 10,
            "import_learning_enabled": 1,
        }
        self.category_by_id: dict[int, dict[str, Any] | None] = {
            10: {"main_category": "资金管理", "sub_category": "存取现金"},
            77: {"main_category": "餐饮", "sub_category": "早餐"},
        }
        self.alias_mapping = {
            "招商银行卡": 2,
            "现金": 3,
            "支付宝": 4,
            "理财账户": 5,
            "转入账户": 6,
            "微信": 7,
        }
        self.accounts = [
            {"id": 1, "name": "默认账户"},
            {"id": 2, "name": "招商银行卡"},
            {"id": 3, "name": "现金"},
            {"id": 4, "name": "支付宝"},
            {"id": 5, "name": "理财账户"},
            {"id": 6, "name": "转入账户"},
            {"id": 7, "name": "微信"},
        ]
        self.history_source: dict[str, Any] | None = None
        self.history_destination: dict[str, Any] | None = None
        self.updated_categories: list[tuple[int, dict[str, Any], int]] = []
        self.import_rules: list[dict[str, Any]] = []
        self.rule_usage_updates: list[tuple[list[int], int]] = []
        self.saved_annotation_samples: list[tuple[str, list[dict[str, Any]], int]] = []
        self.promoted_annotation_sessions: list[tuple[str, list[int] | None, int]] = []
        self.preview_rows: list[dict[str, Any]] = []
        self.annotation_samples: list[dict[str, Any]] = []
        self.preview_selection_updates: list[tuple[list[int], bool]] = []
        self.cleared_sessions: list[str] = []
        self.cleared_session_calls: list[tuple[str, int]] = []
        self.preview_query_calls: list[tuple[str, int, bool]] = []
        self.session_status_updates: list[tuple[str, str]] = []
        self.preview_classification_updates: list[list[dict[str, Any]]] = []
        self.category_by_name: dict[tuple[str, str], dict[str, Any]] = {
            ("餐饮", "早餐"): {"id": 77, "keywords": "早餐,豆浆"},
        }
        self.bills_by_id: dict[int, dict[str, Any]] = {
            1: {"id": 1, "type": "支出", "main_category": None, "sub_category": None},
            2: {"id": 2, "type": "支出", "main_category": "转账", "sub_category": "内部转账"},
        }
        self.bills_for_filters: list[dict[str, Any]] = []
        self.deleted_bill_ids: list[int] = []
        self.inserted_batches: list[tuple[list[dict[str, Any]], str, int]] = []
        self.closed = False

    async def get_user_by_id(self, _user_id: int) -> dict[str, Any] | None:
        return self.user

    async def init_db(self) -> None:
        return None

    async def get_category_by_id(
        self,
        category_id: int,
        _user_id: int | None = None,
        user_id: int | None = None,
    ) -> dict[str, Any] | None:
        _ = user_id
        return self.category_by_id.get(category_id)

    async def get_account_alias_mapping(self, _user_id: int) -> dict[str, int]:
        return self.alias_mapping

    async def get_all_accounts(self, user_id: int | None = None) -> list[dict[str, Any]]:
        _ = user_id
        return self.accounts

    async def get_account_by_id(self, account_id: int, user_id: int = 1) -> dict[str, Any] | None:
        _ = user_id
        return next((account for account in self.accounts if int(account.get("id") or 0) == account_id), None)

    async def get_historical_source_account_suggestion(self, **kwargs: Any) -> dict[str, Any] | None:
        if kwargs.get("description") == "历史命中":
            return self.history_source
        return None

    async def get_historical_destination_account_suggestion(self, **kwargs: Any) -> dict[str, Any] | None:
        if kwargs.get("bill_type") == "转账":
            return self.history_destination
        return None

    async def get_category_by_name(self, main_category: str, sub_category: str, user_id: int = 1) -> dict[str, Any] | None:
        _ = user_id
        return self.category_by_name.get((main_category, sub_category))

    async def update_category(self, category_id: int, payload: dict[str, Any], user_id: int = 1) -> bool:
        self.updated_categories.append((category_id, payload, user_id))
        return True

    async def get_bill_by_id(self, bill_id: int, user_id: int = 1) -> dict[str, Any] | None:
        _ = user_id
        return self.bills_by_id.get(bill_id)

    async def get_bills(
        self,
        filters: dict[str, Any] | None = None,
        limit: int | None = None,
        offset: int = 0,
        user_id: int = 1,
    ) -> list[dict[str, Any]]:
        _ = (filters, limit, offset, user_id)
        return list(self.bills_for_filters)

    async def update_bill(self, bill_id: int, payload: dict[str, Any], user_id: int = 1) -> bool:
        self.updated_categories.append((bill_id, payload, user_id))
        return True

    async def get_import_learning_rules(
        self,
        user_id: int = 1,
        enabled_only: bool = True,
        limit: int = 1000,
    ) -> list[dict[str, Any]]:
        _ = (user_id, enabled_only, limit)
        return list(self.import_rules)

    async def increment_import_learning_rule_usage(self, rule_ids: list[int], user_id: int = 1) -> None:
        self.rule_usage_updates.append((rule_ids, user_id))

    async def save_import_annotation_samples(
        self,
        session_id: str,
        preview_updates: list[dict[str, Any]],
        user_id: int = 1,
    ) -> int:
        self.saved_annotation_samples.append((session_id, preview_updates, user_id))
        return len(preview_updates)

    async def promote_import_annotation_samples_to_learning(
        self,
        session_id: str,
        preview_ids: list[int] | None = None,
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.promoted_annotation_sessions.append((session_id, preview_ids, user_id))
        return {"saved_rule_count": len(preview_ids or []), "skipped_count": 0}

    async def get_preview_by_session(
        self,
        session_id: str,
        user_id: int = 1,
        selected_only: bool = False,
    ) -> list[dict[str, Any]]:
        self.preview_query_calls.append((session_id, user_id, selected_only))
        previews = [
            preview
            for preview in self.preview_rows
            if preview.get("session_id") in (None, session_id)
            and preview.get("user_id") in (None, user_id)
        ]
        if not selected_only:
            return list(previews)
        return [preview for preview in previews if bool(preview.get("preview_selected", 1))]

    async def get_import_annotation_samples(self, _session_id: str, user_id: int = 1) -> list[dict[str, Any]]:
        _ = user_id
        return list(self.annotation_samples)

    async def update_preview_selection(self, preview_ids: list[int], selected: bool) -> int:
        self.preview_selection_updates.append((list(preview_ids), selected))
        return len(preview_ids)

    async def clear_session_data(self, session_id: str, user_id: int = 1) -> int:
        self.cleared_sessions.append(session_id)
        self.cleared_session_calls.append((session_id, user_id))
        return 3

    async def update_import_session_status(self, session_id: str, status: str, total_parsed: int = 0) -> None:
        _ = total_parsed
        self.session_status_updates.append((session_id, status))

    async def batch_update_preview_classification(self, updates: list[dict[str, Any]]) -> int:
        self.preview_classification_updates.append(list(updates))
        return len(updates)

    @staticmethod
    def build_composite_match_hash(
        parser_id: Any = "",
        counterparty: Any = "",
        description: Any = "",
        payment_method: Any = "",
    ) -> str:
        return (
            f"c={str(counterparty or '').strip()}|"
            f"d={str(description or '').strip()}|"
            f"p={str(parser_id or '').strip()}|"
            f"m={str(payment_method or '').strip()}"
        )

    @staticmethod
    def build_composite_match_features(
        parser_id: Any = "",
        counterparty: Any = "",
        description: Any = "",
        payment_method: Any = "",
    ) -> dict[str, str]:
        result: dict[str, str] = {}
        for key, value in {
            "parser_id": parser_id,
            "counterparty": counterparty,
            "description": description,
            "payment_method": payment_method,
        }.items():
            normalized = BillService._normalize_learning_text(value)
            if normalized:
                result[key] = normalized
        return result

    async def insert_bills(self, bills: list[dict[str, Any]], batch_id: str, user_id: int = 1) -> int:
        self.inserted_batches.append((list(bills), batch_id, user_id))
        return len(bills) - 1 if bills else 0

    async def delete_bill(self, bill_id: int, user_id: int = 1) -> bool:
        _ = user_id
        self.deleted_bill_ids.append(bill_id)
        return True

    async def deduplicate(self) -> int:
        return 3

    async def get_statistics(self) -> dict[str, Any]:
        return {"total": 99}

    async def close(self) -> None:
        self.closed = True


class FakeCategoryEngine:
    """Minimal category-engine stub for consistency tests."""

    def __init__(self) -> None:
        self.is_initialized = True
        self.rules = [{"type": TransactionType.TRANSFER, "main": "转账", "sub": "内部转账"}]
        self.current_user_id = 1



def _make_service(fake_db: FakeBillServiceDB | None = None) -> BillService:
    return BillService(db=cast("Any", fake_db or FakeBillServiceDB()))



def test_preview_helpers_and_learning_text_normalization() -> None:
    """预览转换与长期学习文本标准化应保持前端契约。"""
    service = _make_service()
    bill = {
        "date": datetime(2025, 1, 2, 9, 30, 0),
        "amount": 18.8,
        "description": "早餐",
        "counterparty": "便利店",
        "payment_method": "支付宝",
        "source_account_id": 4,
    }

    preview = service._bill_to_preview(bill)
    assert preview["time"] == "2025-01-02 09:30:00"
    assert preview["account"] == 4
    assert preview["paymentMethod"] == "支付宝"

    batch_preview = service._prepare_preview_data([bill])
    assert batch_preview[0]["time"] == "2025-01-02 09:30:00"
    assert BillService._normalize_learning_text("  早餐铺 | 早餐铺   ") == "早餐铺 | 早餐铺"
    assert BillService._normalize_learning_text(None) == ""


@pytest.mark.asyncio
async def test_investment_helpers_detect_profiles_candidates_and_keyword_config() -> None:
    """投资 helper 应识别平台/产品、过滤转账，并读取用户关键词配置。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)

    keyword_config = await service._get_investment_keyword_config(user_id=1)
    assert "platform_keywords" in keyword_config
    assert "product_keywords" in keyword_config

    profile = service._extract_investment_profile("蚂蚁财富 黄金ETF 自动定投", keyword_config=keyword_config)
    assert profile["platform"]
    assert "黄金" in profile["product"] or "ETF" in profile["product"]

    cleaned_product = service._clean_investment_product_name("支付宝-自动定投 黄金ETF 确认份额", platform="支付宝")
    assert cleaned_product == "黄金ETF"

    candidate = service._score_investment_candidate(
        {
            "type": "支出",
            "counterparty": "蚂蚁财富",
            "payment_method": "支付宝",
            "description": "黄金ETF 自动定投",
            "main_category": "",
            "sub_category": "",
            "original_category": "",
        },
        keyword_config=keyword_config,
    )
    assert candidate is not None
    assert candidate["score"] >= 0.55
    assert candidate["hint_text"]

    assert service._score_investment_candidate({"type": "转账", "description": "基金申购"}) is None
    assert service._score_investment_candidate({"type": "投资", "description": "基金申购"}) is None

    pnl_bills = [
        {
            "date": "2026-06-01 10:00:00",
            "type": "收入",
            "amount": 12.0,
            "counterparty": "天天基金",
            "payment_method": "支付宝",
            "description": "沪深300ETF 分红发放",
            "main_category": "投资理财",
            "sub_category": "基金",
        }
    ]
    detected_pnl = await service._detect_investment_candidates(pnl_bills, user_id=1)
    assert detected_pnl[0]["type"] == "收入"
    assert detected_pnl[0]["_suppress_investment_signal"] is True
    assert "_investment_candidate_reason" not in detected_pnl[0]

    matched_pnl = await service._match_accounts(detected_pnl, user_id=1)
    assert matched_pnl[0]["source_account_id"] == 4
    assert "destination_account_id" not in matched_pnl[0]


@pytest.mark.asyncio
async def test_detect_cash_transfers_and_type_category_consistency() -> None:
    """存取转账检测和类型/分类一致性修复应覆盖收入支出两种方向。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)
    service.category_engine = cast("Any", FakeCategoryEngine())

    bills = [
        {
            "date": "2025-01-02",
            "type": "收入",
            "amount": 100.0,
            "main_category": "资金管理",
            "sub_category": "存取现金",
            "source_account_id": 2,
        },
        {
            "date": "2025-01-03",
            "type": "支出",
            "amount": -80.0,
            "main_category": "资金管理",
            "sub_category": "存取现金",
            "source_account_id": 2,
        },
        {
            "date": "2025-01-04",
            "type": "支出",
            "amount": -20.0,
            "main_category": "转账",
            "sub_category": "内部转账",
            "source_account_id": 2,
        },
    ]

    converted = await service._detect_cash_transfers(bills, user_id=1)
    assert converted[0]["type"] == "转账"
    assert converted[0]["source_account_id"] == 99
    assert converted[0]["destination_account_id"] == 2
    assert converted[0]["destination_amount"] == 100.0
    assert converted[1]["type"] == "转账"
    assert converted[1]["source_account_id"] == 2
    assert converted[1]["destination_account_id"] == 99
    assert converted[1]["destination_amount"] == 80.0

    fixed = await service._validate_type_category_consistency(converted, user_id=1)
    assert fixed[2]["main_category"] == ""
    assert fixed[2]["sub_category"] == ""

    fake_db.user = None
    unchanged = await service._detect_cash_transfers([{"type": "支出", "main_category": "x", "sub_category": "y"}], user_id=1)
    assert unchanged[0]["type"] == "支出"


@pytest.mark.asyncio
async def test_match_accounts_uses_multiple_matching_paths_and_destination_rules() -> None:
    """账户匹配应覆盖已有 ID、payment_method、description、历史规则、parser_id 与目标账户匹配。"""
    fake_db = FakeBillServiceDB()
    fake_db.history_source = {"account_id": 4, "reasons": ["history-source"]}
    fake_db.history_destination = {"account_id": 6, "reasons": ["history-destination"]}
    service = _make_service(fake_db)

    bills = [
        {"type": "支出", "source_account_id": 1, "payment_method": "", "description": "", "counterparty": ""},
        {"type": "支出", "source_account_id": "unknown", "payment_method": "招商银行卡", "description": "", "counterparty": ""},
        {"type": "支出", "source_account_id": "unknown", "payment_method": "", "description": "今天取现到现金", "counterparty": ""},
        {"type": "支出", "source_account_id": "unknown", "payment_method": "", "description": "历史命中", "counterparty": "", "_parser_id": ""},
        {"type": "支出", "source_account_id": "unknown", "payment_method": "", "description": "", "counterparty": "", "_parser_id": "支付宝"},
        {"type": "投资", "source_account_id": 2, "payment_method": "", "description": "理财申购", "counterparty": "理财账户"},
        {
            "type": "转账",
            "source_account_id": 2,
            "payment_method": "",
            "description": "转账",
            "counterparty": "",
            "_destination_parser_id": "",
            "_destination_payment_method": "",
            "_destination_counterparty": "",
        },
        {
            "type": "转账",
            "source_account_id": 2,
            "payment_method": "",
            "description": "转账",
            "counterparty": "",
            "_transfer_pair_sources": [
                {
                    "role": "outgoing",
                    "parser_id": "cmbc",
                    "payment_method": "招商银行卡",
                    "counterparty": "",
                    "source_account_id": 2,
                    "account_name": "招商银行卡",
                    "tags": ["parser:cmbc", "channel:bank"],
                },
                {
                    "role": "incoming",
                    "parser_id": "wechat",
                    "payment_method": "",
                    "counterparty": "",
                    "source_account_id": None,
                    "account_name": "",
                    "tags": ["parser:wechat", "channel:wallet"],
                },
            ],
        },
        {"type": "支出", "source_account_id": "unknown", "payment_method": "", "description": "", "counterparty": "", "_parser_id": "", "main_category": ""},
    ]

    matched = await service._match_accounts(bills, user_id=1)

    assert matched[0]["source_account_id"] == 1
    assert matched[0]["account_name"] == "默认账户"
    assert matched[1]["source_account_id"] == 2
    assert matched[1]["account_name"] == "招商银行卡"
    assert matched[2]["source_account_id"] == 3
    assert matched[2]["account_name"] == "现金"
    assert matched[3]["source_account_id"] == 4
    assert matched[3]["account_name"] == "支付宝"
    assert matched[4]["source_account_id"] == 4
    assert matched[4]["account_name"] == "支付宝"
    assert matched[5]["destination_account_id"] == 5
    assert matched[6]["destination_account_id"] == 6
    assert matched[7]["destination_account_id"] == 7
    assert matched[8]["source_account_id"] is None


@pytest.mark.asyncio
async def test_add_category_keyword_updates_db_and_refreshes_category_rules(monkeypatch: pytest.MonkeyPatch) -> None:
    """添加分类关键词时应更新数据库并重新加载分类规则。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)
    service._initialized = True

    reloaded: list[int] = []

    async def fake_load_rules_from_db(_db: object, user_id: int = 1) -> None:
        reloaded.append(user_id)

    monkeypatch.setattr(service.category_engine, "load_rules_from_db", fake_load_rules_from_db)

    success = await service.add_category_keyword("餐饮", "早餐", "煎饼", user_id=1)
    assert success is True
    assert fake_db.updated_categories == [(77, {"keywords": "早餐,豆浆,煎饼"}, 1)]
    assert reloaded == [1]


@pytest.mark.asyncio
async def test_import_learning_helpers_and_promotion_cover_rule_replay_paths() -> None:
    """长期学习与会话提升 helper 应覆盖复合匹配、单字段回退和规则使用记录。"""
    fake_db = FakeBillServiceDB()
    fake_db.import_rules = [
        {
            "id": 1,
            "match_type": "composite",
            "composite_match_hash": fake_db.build_composite_match_hash(
                parser_id="wechat",
                counterparty="早餐铺",
                description="共同描述",
                payment_method="微信支付",
            ),
            "learned_type": "转账",
            "learned_category_id": 77,
            "learned_source_account_id": 2,
            "learned_destination_account_id": 3,
        },
        {
            "id": 2,
            "match_type": "description",
            "normalized_match_value": "午餐",
            "learned_type": "支出",
            "learned_category_id": 77,
            "learned_source_account_id": 4,
            "learned_destination_account_id": None,
        },
    ]
    service = _make_service(fake_db)

    bills = [
        {
            "_parser_id": "wechat",
            "counterparty": "早餐铺",
            "description": "共同描述",
            "payment_method": "微信支付",
            "type": "支出",
        },
        {
            "_parser_id": "alipay",
            "counterparty": "午餐店",
            "description": "午餐",
            "payment_method": "支付宝",
            "type": "支出",
        },
    ]

    applied = await service._apply_import_learning_rules(bills, user_id=1, type_only=False, record_usage=True)
    assert applied == 2
    assert bills[0]["type"] == "转账"
    assert bills[0]["main_category"] == "餐饮"
    assert bills[0]["source_account_id"] == 2
    assert bills[0]["destination_account_id"] == 3
    assert bills[1]["source_account_id"] == 4
    assert fake_db.rule_usage_updates == [([1, 2], 1)]

    rule_lookup = await service._build_session_annotation_rule_lookup(
        previews=[
            {
                "id": 11,
                "preview_parser_id": "wechat",
                "preview_counterparty": "早餐铺",
                "preview_description": "共同描述",
                "preview_payment_method": "微信支付",
            }
        ],
        annotation_samples=[
            {
                "preview_id": 11,
                "annotated_type": "转账",
                "annotated_category_id": 77,
                "annotated_source_account_id": 2,
                "annotated_destination_account_id": 3,
            }
        ],
        user_id=1,
    )
    assert ("description", "共同描述") in rule_lookup
    assert (
        "composite",
        fake_db.build_composite_match_hash(
            parser_id="wechat", counterparty="早餐铺", description="共同描述", payment_method="微信支付"
        ),
    ) in rule_lookup

    promote_result = await service.promote_session_annotations_to_learning(
        "session-1",
        preview_updates=[{"id": 11, "preview_type": "转账"}],
        user_id=1,
    )
    assert promote_result["success"] is True
    assert fake_db.saved_annotation_samples[0][0] == "session-1"
    assert fake_db.promoted_annotation_sessions[0] == ("session-1", [11], 1)


@pytest.mark.asyncio
async def test_promote_session_annotations_supports_explicit_preview_ids_without_resaving_samples() -> None:
    """长期学习提升应支持直接透传 preview_ids，而不重复保存会话标注样本。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)

    promote_result = await service.promote_session_annotations_to_learning(
        "session-explicit-preview-ids",
        preview_ids=[21, 22],
        user_id=3,
    )

    assert promote_result["success"] is True
    assert fake_db.saved_annotation_samples == []
    assert fake_db.promoted_annotation_sessions == [
        ("session-explicit-preview-ids", [21, 22], 3)
    ]


@pytest.mark.asyncio
async def test_promote_session_annotations_prefers_preview_updates_over_explicit_preview_ids() -> None:
    """当 preview_updates 与 preview_ids 同时提供时，应优先使用 preview_updates 派生的选择结果。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)

    promote_result = await service.promote_session_annotations_to_learning(
        "session-preview-updates-priority",
        preview_updates=[{"id": 11, "preview_type": "支出"}, {"id": 12, "preview_type": "收入"}],
        preview_ids=[99],
        user_id=5,
    )

    assert promote_result["success"] is True
    assert fake_db.saved_annotation_samples == [
        (
            "session-preview-updates-priority",
            [{"id": 11, "preview_type": "支出"}, {"id": 12, "preview_type": "收入"}],
            5,
        )
    ]
    assert fake_db.promoted_annotation_sessions == [
        ("session-preview-updates-priority", [11, 12], 5)
    ]


@pytest.mark.asyncio
async def test_promote_session_annotations_treats_explicit_empty_preview_ids_as_zero_selection() -> None:
    """显式传入空 preview_ids 时，应保持零提升，而不是回退成全量提升。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)

    promote_result = await service.promote_session_annotations_to_learning(
        "session-empty-preview-ids",
        preview_ids=[],
        user_id=6,
    )

    assert promote_result["success"] is True
    assert fake_db.promoted_annotation_sessions == [
        ("session-empty-preview-ids", [], 6)
    ]


@pytest.mark.asyncio
async def test_apply_import_learning_rules_ignores_stale_account_ids() -> None:
    """长期学习回放遇到已失效账户 ID 时，不应把悬空账户写回 bill。"""
    fake_db = FakeBillServiceDB()
    fake_db.import_rules = [
        {
            "id": 10,
            "match_type": "composite",
            "composite_match_hash": fake_db.build_composite_match_hash(
                parser_id="wechat",
                counterparty="早餐铺",
                description="共同描述",
                payment_method="微信支付",
            ),
            "learned_type": "收入",
            "learned_category_id": 77,
            "learned_source_account_id": 999999,
            "learned_destination_account_id": 999998,
        }
    ]
    service = _make_service(fake_db)

    bills = [
        {
            "_parser_id": "wechat",
            "counterparty": "早餐铺",
            "description": "共同描述",
            "payment_method": "微信支付",
            "type": "支出",
            "source_account_id": 5,
            "destination_account_id": 6,
        }
    ]

    applied = await service._apply_import_learning_rules(bills, user_id=1, type_only=False, record_usage=False)

    assert applied == 1
    assert bills[0]["type"] == "收入"
    assert bills[0]["main_category"] == "餐饮"
    assert bills[0]["sub_category"] == "早餐"
    assert bills[0]["source_account_id"] == 5
    assert bills[0]["destination_account_id"] == 6


@pytest.mark.asyncio
async def test_batch_import_preview_confirmed_and_simple_db_wrappers(monkeypatch: pytest.MonkeyPatch) -> None:
    """批量导入汇总、预览确认和轻量 DB wrapper 应按契约返回结果。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)
    service._initialized = True

    async def fake_import_bills(file_path: str, user_id: int = 1) -> dict[str, Any]:
        if file_path.endswith("ok.csv"):
            return {
                "success": True,
                "total": 3,
                "inserted": 2,
                "uncategorized": [{"file": file_path}],
            }
        return {"success": False, "total": 0, "inserted": 0, "uncategorized": []}

    monkeypatch.setattr(service, "import_bills", fake_import_bills)
    summary = await service.import_multiple_files(["ok.csv", "bad.csv"], user_id=1)
    assert summary["success_files"] == 1
    assert summary["failed_files"] == 1
    assert summary["inserted_bills"] == 2
    assert summary["uncategorized_bills"] == [{"file": "ok.csv"}]

    confirmed = await service.import_preview_confirmed([{"date": "2025-01-02", "amount": 10.0}], user_id=1)
    assert confirmed == {"success": True, "total": 1, "inserted": 0, "duplicates": 1, "errors": []}
    assert fake_db.inserted_batches

    updated = await service.batch_update_category([1, 2], "餐饮", "早餐", user_id=1)
    assert updated["success"] is True
    assert updated["updated"] == 2

    fake_db.bills_for_filters = [{"id": 1, "type": "支出", "main_category": None, "sub_category": None}]
    monkeypatch.setattr(service.category_engine, "match_category", lambda _bill: ("餐饮", "早餐"))
    refreshed = await service.refresh_category_for_bills(user_id=1)
    assert refreshed["categorized"] == 1

    assert await service.get_bills({"main_category": None}) == fake_db.bills_for_filters
    assert await service.delete_bill(1) is True
    assert await service.update_bill(1, {"main_category": "餐饮"}) is True
    assert await service.deduplicate() == 3
    assert await service.get_statistics() == {"total": 99}
    await service.close()
    assert fake_db.closed is True


@pytest.mark.asyncio
async def test_import_preview_selection_updates_and_cancel_session_helpers() -> None:
    """预览读取、选中状态更新和取消会话 helper 应返回前端期望字段。"""
    fake_db = FakeBillServiceDB()
    fake_db.preview_rows = [
        {
            "id": 1,
            "user_id": 1,
            "preview_date": "2025-01-02 08:30:00",
            "preview_type": "支出",
            "preview_amount": -12.3,
            "preview_destination_amount": 12.3,
            "preview_main_category": "资金管理",
            "preview_sub_category": "存取现金",
            "preview_source_account_id": 2,
            "preview_destination_account_id": 3,
            "preview_counterparty": "现金",
            "preview_payment_method": "民生银行卡",
            "preview_description": "ATM取现",
            "preview_parser_id": "cmbc",
            "preview_selected": 1,
            "dedup_type": "transfer",
        },
        {
            "id": 2,
            "user_id": 1,
            "preview_date": "2025-01-03 09:00:00",
            "preview_type": "投资",
            "preview_amount": -88.0,
            "preview_destination_amount": 0,
            "preview_main_category": "投资理财",
            "preview_sub_category": "基金",
            "preview_source_account_id": 4,
            "preview_destination_account_id": None,
            "preview_counterparty": "蚂蚁财富",
            "preview_payment_method": "支付宝",
            "preview_description": "黄金ETF 自动定投",
            "preview_parser_id": "alipay",
            "preview_selected": 0,
            "dedup_type": "",
        },
    ]
    fake_db.annotation_samples = [{"preview_id": 1}]
    service = _make_service(fake_db)

    preview_items = await service.get_import_preview("session-preview", selected_only=False)
    selected_items = await service.get_import_preview("session-preview", selected_only=True)
    updated_count = await service.update_preview_selections("session-preview", {1: True, 2: False})
    cancel_result = await service.cancel_import_session("session-preview")

    assert len(preview_items) == 2
    assert preview_items[0]["preview_is_manually_annotated"] is True
    assert preview_items[0]["suggested_preview_type"] == "转账"
    assert preview_items[0]["transfer_suggestion_score"] >= 0.55
    assert preview_items[1]["investment_signal_score"] >= 0.55
    assert preview_items[1]["investment_platform"]
    assert preview_items[1]["investment_product"]
    assert [item["id"] for item in selected_items] == [1]
    assert updated_count == 2
    assert fake_db.preview_selection_updates == [([1], True), ([2], False)]
    assert cancel_result == {"success": True, "session_id": "session-preview", "cleared": 3}
    assert fake_db.cleared_sessions == ["session-preview"]
    assert fake_db.cleared_session_calls == [("session-preview", 1)]
    assert fake_db.session_status_updates == [("session-preview", "cancelled")]


@pytest.mark.asyncio
async def test_get_import_preview_adds_matching_transfer_parser_and_annotation_groups() -> None:
    """导入预览应新增 matching 镜像结构，并保持现有平铺字段不变。"""
    fake_db = FakeBillServiceDB()
    fake_db.preview_rows = [
        {
            "id": 1,
            "session_id": "session-matching-transfer",
            "user_id": 1,
            "preview_date": "2025-01-02 08:30:00",
            "preview_type": "支出",
            "preview_amount": -12.3,
            "preview_destination_amount": 12.3,
            "preview_main_category": "转账",
            "preview_sub_category": "账户互转",
            "preview_source_account_id": 2,
            "preview_destination_account_id": 3,
            "preview_counterparty": "内部转账",
            "preview_payment_method": "民生银行卡",
            "preview_description": "转账到现金",
            "preview_parser_id": "cmbc",
            "preview_parser_tags": ["parser:cmbc", "channel:bank", "parser:wechat"],
            "preview_selected": 1,
            "dedup_type": "transfer",
            "dedup_source_ids": [101, 102],
        }
    ]
    fake_db.annotation_samples = [{"preview_id": 1}]
    service = _make_service(fake_db)

    preview_items = await service.get_import_preview("session-matching-transfer", selected_only=False)

    assert len(preview_items) == 1
    preview_item = preview_items[0]
    matching = preview_item["matching"]

    assert preview_item["suggested_preview_type"] == "转账"
    assert preview_item["transfer_suggestion_score"] >= 0.55
    assert matching["transfer"]["candidate_type"] == preview_item["suggested_preview_type"]
    assert matching["transfer"]["score"] == preview_item["transfer_suggestion_score"]
    assert matching["transfer"]["level"] == preview_item["transfer_suggestion_level"]
    assert matching["transfer"]["reason"] == preview_item["transfer_suggestion_reason"]
    assert matching["transfer"]["pair_order"] == "outgoing_first"
    assert matching["transfer"]["source_chain"] == [
        {
            "position": 0,
            "role": "outgoing",
            "parser_id": "cmbc",
            "parser_label": "民生银行",
            "label": "民生银行卡",
            "channel": "bank",
            "tags": ["parser:cmbc", "channel:bank"],
            "account_id": 2,
        },
        {
            "position": 1,
            "role": "incoming",
            "parser_id": "wechat",
            "parser_label": "微信",
            "label": "微信",
            "channel": "",
            "tags": ["parser:wechat"],
            "account_id": 3,
        },
    ]
    assert matching["dedup"] == {
        "type": "transfer",
        "source_ids": [101, 102],
        "source_count": 2,
        "source_labels": ["民生银行卡", "微信"],
        "sources": [
            {
                "position": 0,
                "role": "outgoing",
                "parser_id": "cmbc",
                "parser_label": "民生银行",
                "label": "民生银行卡",
                "channel": "bank",
                "tags": ["parser:cmbc", "channel:bank"],
                "account_id": 2,
            },
            {
                "position": 1,
                "role": "incoming",
                "parser_id": "wechat",
                "parser_label": "微信",
                "label": "微信",
                "channel": "",
                "tags": ["parser:wechat"],
                "account_id": 3,
            },
        ],
    }
    assert matching["parser"] == {
        "id": "cmbc",
        "tags": ["parser:cmbc", "channel:bank", "parser:wechat"],
        "source_chain": [
            {
                "position": 0,
                "role": "outgoing",
                "parser_id": "cmbc",
                "parser_label": "民生银行",
                "label": "民生银行卡",
                "channel": "bank",
                "tags": ["parser:cmbc", "channel:bank"],
                "account_id": 2,
            },
            {
                "position": 1,
                "role": "incoming",
                "parser_id": "wechat",
                "parser_label": "微信",
                "label": "微信",
                "channel": "",
                "tags": ["parser:wechat"],
                "account_id": 3,
            },
        ],
    }
    assert matching["annotation"] == {"is_manually_annotated": True}


@pytest.mark.asyncio
async def test_get_import_preview_adds_platform_duplicate_source_metadata() -> None:
    """平台-银行重复应返回稳定来源标签与计数，不依赖 preview 行反查。"""
    fake_db = FakeBillServiceDB()
    fake_db.preview_rows = [
        {
            "id": 3,
            "session_id": "session-matching-platform-bank",
            "user_id": 1,
            "preview_date": "2025-01-05 08:30:00",
            "preview_type": "支出",
            "preview_amount": 66.0,
            "preview_destination_amount": 0.0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": 4,
            "preview_destination_account_id": None,
            "preview_counterparty": "测试商户",
            "preview_payment_method": "支付宝",
            "preview_description": "平台银行重复",
            "preview_parser_id": "alipay",
            "preview_parser_tags": ["parser:alipay", "channel:wallet", "parser:cmbc", "channel:bank"],
            "preview_selected": 1,
            "dedup_type": "platform_bank",
            "dedup_source_ids": [301, 302],
        }
    ]
    service = _make_service(fake_db)

    preview_items = await service.get_import_preview("session-matching-platform-bank", selected_only=False)

    assert len(preview_items) == 1
    matching = preview_items[0]["matching"]
    assert matching["dedup"] == {
        "type": "platform_bank",
        "source_ids": [301, 302],
        "source_count": 2,
        "source_labels": ["支付宝", "民生银行"],
        "sources": [
            {
                "position": 0,
                "role": "kept",
                "parser_id": "alipay",
                "parser_label": "支付宝",
                "label": "支付宝",
                "channel": "wallet",
                "tags": ["parser:alipay", "channel:wallet"],
                "account_id": 4,
            },
            {
                "position": 1,
                "role": "duplicate",
                "parser_id": "cmbc",
                "parser_label": "民生银行",
                "label": "民生银行",
                "channel": "bank",
                "tags": ["parser:cmbc", "channel:bank"],
                "account_id": None,
            },
        ],
    }
    assert matching["parser"]["source_chain"] == matching["dedup"]["sources"]


@pytest.mark.asyncio
async def test_get_import_preview_adds_matching_investment_learning_and_recurring_groups(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """matching 应镜像投资、长期学习和周期候选字段，而不是替换原有平铺字段。"""
    fake_db = FakeBillServiceDB()
    fake_db.preview_rows = [
        {
            "id": 2,
            "session_id": "session-matching-investment",
            "user_id": 1,
            "preview_date": "2025-01-03 08:30:00",
            "preview_type": "投资",
            "preview_amount": 88.0,
            "preview_destination_amount": 88.0,
            "preview_main_category": "投资理财",
            "preview_sub_category": "基金",
            "preview_source_account_id": 4,
            "preview_destination_account_id": 5,
            "preview_counterparty": "蚂蚁财富",
            "preview_payment_method": "支付宝",
            "preview_description": "黄金ETF 自动定投",
            "preview_parser_id": "alipay",
            "preview_parser_tags": ["parser:alipay", "channel:wallet"],
            "preview_recurring_id": 9,
            "preview_recurring_name": "每月定投",
            "preview_recurring_candidate_count": 2,
            "preview_recurring_match_score": 0.91,
            "preview_recurring_match_reasons": "date|amount",
            "preview_recurring_matched_date": "2025-01-01",
            "preview_selected": 1,
            "dedup_type": "remaining",
            "dedup_source_ids": [201],
        }
    ]
    service = _make_service(fake_db)

    monkeypatch.setattr(
        service,
        "_build_investment_signal_from_preview",
        lambda *_args, **_kwargs: {
            "score": 0.81,
            "level": "high",
            "reason": "investment_keyword",
            "platform": "蚂蚁财富",
            "product": "黄金ETF",
        },
    )
    monkeypatch.setattr(
        service,
        "_build_learning_similarity_signal_from_preview",
        lambda *_args, **_kwargs: {
            "rule_id": 42,
            "score": 0.88,
            "level": "high",
            "reason": "parser_id:exact",
            "recommended_type": "投资",
            "summary": "投资 | 投资理财/基金 | 支付宝 → 理财账户",
        },
    )

    preview_items = await service.get_import_preview("session-matching-investment", selected_only=False)

    assert len(preview_items) == 1
    preview_item = preview_items[0]
    matching = preview_item["matching"]

    assert preview_item["investment_signal_score"] == 0.81
    assert matching["investment"] == {
        "score": 0.81,
        "level": "high",
        "reason": "investment_keyword",
        "platform": "蚂蚁财富",
        "product": "黄金ETF",
        "review_status": "pending",
        "suppressed": False,
    }
    assert preview_item["learning_recommendation_rule_id"] == 42
    assert matching["learning"] == {
        "rule_id": 42,
        "score": 0.88,
        "level": "high",
        "reason": "parser_id:exact",
        "recommended_type": "投资",
        "summary": "投资 | 投资理财/基金 | 支付宝 → 理财账户",
        "review_status": "pending",
        "suppressed": False,
    }
    assert matching["recurring"] == {
        "id": 9,
        "name": "每月定投",
        "candidate_count": 2,
        "match_score": 0.91,
        "match_reasons": "date|amount",
        "matched_date": "2025-01-01",
    }
    assert preview_item["preview_recurring_id"] == 9
    assert preview_item["preview_recurring_match_score"] == 0.91


@pytest.mark.asyncio
async def test_preview_reclassify_and_session_cleanup_respect_non_default_user_ids(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """非默认用户的 preview / reclassify / confirm / cancel 路径必须显式透传 user_id。"""
    fake_db = FakeBillServiceDB()
    fake_db.preview_rows = [
        {
            "id": 11,
            "session_id": "session-user-7",
            "user_id": 7,
            "preview_date": "2025-01-02 08:30:00",
            "preview_type": "支出",
            "preview_amount": 12.3,
            "preview_destination_amount": 0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
            "preview_counterparty": "早餐铺",
            "preview_payment_method": "支付宝",
            "preview_description": "共同描述",
            "preview_parser_id": "wechat",
            "preview_selected": 1,
            "dedup_type": "remaining",
        },
        {
            "id": 12,
            "session_id": "session-user-7",
            "user_id": 1,
            "preview_date": "2025-01-03 08:30:00",
            "preview_type": "支出",
            "preview_amount": 99.0,
            "preview_destination_amount": 0,
            "preview_main_category": "其他",
            "preview_sub_category": "其他",
            "preview_source_account_id": 2,
            "preview_destination_account_id": None,
            "preview_counterparty": "默认用户账单",
            "preview_payment_method": "银行卡",
            "preview_description": "不应读到",
            "preview_parser_id": "alipay",
            "preview_selected": 1,
            "dedup_type": "remaining",
        },
    ]
    fake_db.annotation_samples = [
        {
            "preview_id": 11,
            "annotated_type": "转账",
            "annotated_category_id": 10,
            "annotated_source_account_id": 2,
            "annotated_destination_account_id": 3,
        }
    ]
    service = _make_service(fake_db)
    service._initialized = True

    class ReclassifyCategoryEngine:
        def __init__(self) -> None:
            self.rules = [{"type": TransactionType.TRANSFER, "main": "资金管理", "sub": "存取现金"}]

        async def load_rules_from_db(self, _db: object, user_id: int = 1) -> None:
            _ = user_id

        async def batch_match_categories(self, bills: list[dict[str, Any]], types: Any = None) -> list[dict[str, Any]]:
            _ = types
            for bill in bills:
                bill["main_category"] = "资金管理"
                bill["sub_category"] = "存取现金"
                bill["source_account_id"] = 2
                bill["destination_account_id"] = 3
            return bills

    service.category_engine = cast("Any", ReclassifyCategoryEngine())

    async def zero_learning(*_args: object, **_kwargs: object) -> int:
        return 0

    async def identity_step(bills: list[dict[str, Any]], user_id: int = 1):
        _ = user_id
        return bills

    async def confirm_preview_to_bills(session_id: str, user_id: int) -> dict[str, Any]:
        assert session_id == "session-user-7"
        assert user_id == 7
        return {"confirmed_count": 1, "duplicate_count": 0, "errors": []}

    async def clear_session_data_for_confirm(session_id: str, user_id: int = 1) -> dict[str, int]:
        fake_db.cleared_sessions.append(session_id)
        fake_db.cleared_session_calls.append((session_id, user_id))
        return {"parser_count": 1, "preview_count": 1}

    monkeypatch.setattr(service, "_build_session_annotation_rule_lookup", zero_learning)
    monkeypatch.setattr(service, "_apply_session_annotation_learning_rules", lambda *args, **kwargs: 0)
    monkeypatch.setattr(service, "_apply_import_learning_rules", zero_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", identity_step)
    monkeypatch.setattr(service, "_match_accounts", identity_step)
    monkeypatch.setattr(service, "_detect_cash_transfers", identity_step)
    monkeypatch.setattr(fake_db, "confirm_preview_to_bills", confirm_preview_to_bills, raising=False)
    monkeypatch.setattr(fake_db, "clear_session_data", clear_session_data_for_confirm)

    preview_items = await service.get_import_preview("session-user-7", user_id=7)
    reclassify_result = await service.reclassify_preview_bills("session-user-7", user_id=7)
    confirm_result = await service.import_stage3_confirm("session-user-7", user_id=7, selected_ids=[11])
    cancel_result = await service.cancel_import_session("session-user-7", user_id=7)

    assert [item["id"] for item in preview_items] == [11]
    assert reclassify_result["success"] is True
    assert reclassify_result["total"] == 1
    assert reclassify_result["categorized"] == 1
    assert reclassify_result["account_matched"] == 1
    assert confirm_result["success"] is True
    assert confirm_result["imported_count"] == 1
    assert cancel_result == {
        "success": True,
        "session_id": "session-user-7",
        "cleared": {"parser_count": 1, "preview_count": 1},
    }
    assert fake_db.preview_query_calls == [
        ("session-user-7", 7, False),
        ("session-user-7", 7, False),
    ]
    assert fake_db.preview_classification_updates == [
        [
            {
                "id": 11,
                "preview_type": "转账",
                "preview_main_category": "资金管理",
                "preview_sub_category": "存取现金",
                "preview_source_account_id": 2,
                "preview_destination_account_id": 3,
            }
        ]
    ]
    assert fake_db.cleared_session_calls == [
        ("session-user-7", 7),
        ("session-user-7", 7),
    ]


@pytest.mark.asyncio
async def test_reclassify_preview_bills_replays_annotations_and_updates_preview_rows(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """重新分类应保存会话样本、回放人工标注并批量更新 preview 表。"""
    fake_db = FakeBillServiceDB()
    fake_db.preview_rows = [
        {
            "id": 11,
            "preview_date": "2025-01-02 08:30:00",
            "preview_type": "支出",
            "preview_amount": 12.3,
            "preview_counterparty": "早餐铺",
            "preview_payment_method": "支付宝",
            "preview_description": "共同描述",
            "preview_parser_id": "wechat",
            "dedup_type": "remaining",
            "preview_source_account_id": None,
            "preview_destination_account_id": None,
        }
    ]
    fake_db.annotation_samples = [
        {
            "preview_id": 11,
            "annotated_type": "转账",
            "annotated_category_id": 10,
            "annotated_source_account_id": 2,
            "annotated_destination_account_id": 3,
        }
    ]
    service = _make_service(fake_db)

    class ReclassifyCategoryEngine:
        def __init__(self) -> None:
            self.rules = [{"type": TransactionType.TRANSFER, "main": "资金管理", "sub": "存取现金"}]

        async def load_rules_from_db(self, _db: object, user_id: int = 1) -> None:
            _ = user_id

        async def batch_match_categories(self, bills: list[dict[str, Any]], types: Any = None) -> list[dict[str, Any]]:
            _ = types
            for bill in bills:
                bill.setdefault("main_category", "资金管理")
                bill.setdefault("sub_category", "存取现金")
            return bills

    service.category_engine = cast("Any", ReclassifyCategoryEngine())

    async def zero_learning(*_args: object, **_kwargs: object) -> int:
        return 0

    async def identity_step(bills: list[dict[str, Any]], user_id: int = 1):
        _ = user_id
        return bills

    async def empty_session_rule_lookup(*_args: object, **_kwargs: object) -> dict[str, Any]:
        return {}

    monkeypatch.setattr(service, "_build_session_annotation_rule_lookup", empty_session_rule_lookup)
    monkeypatch.setattr(service, "_apply_session_annotation_learning_rules", lambda *args, **kwargs: 0)
    monkeypatch.setattr(service, "_apply_import_learning_rules", zero_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", identity_step)
    monkeypatch.setattr(service, "_match_accounts", identity_step)
    monkeypatch.setattr(service, "_detect_cash_transfers", identity_step)

    result = await service.reclassify_preview_bills(
        "session-reclassify",
        preview_updates=[{"id": 11, "preview_type": "转账"}],
        user_id=1,
    )

    assert result["success"] is True
    assert result["total"] == 1
    assert result["session_samples_saved"] == 1
    assert result["annotation_applied"] == 1
    assert result["categorized"] == 1
    assert result["account_matched"] == 1
    assert fake_db.saved_annotation_samples[0][0] == "session-reclassify"
    assert fake_db.preview_classification_updates[0] == [
        {
            "id": 11,
            "preview_type": "转账",
            "preview_main_category": "资金管理",
            "preview_sub_category": "存取现金",
            "preview_source_account_id": 2,
            "preview_destination_account_id": 3,
        }
    ]


def test_learning_similarity_helpers_cover_deserialize_scoring_summary_and_signal() -> None:
    """长期学习相似度 helper 应覆盖特征反序列化、打分、摘要和预览推荐。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)

    assert service._deserialize_learning_match_features({"match_features_json": "{bad"}) == {}
    assert service._deserialize_learning_match_features({"match_features_json": "[1, 2]"}) == {}

    parsed_features = service._deserialize_learning_match_features(
        {
            "match_features_json": json.dumps(
                {
                    "parser_id": " wechat ",
                    "counterparty": " 早餐铺 ",
                    "description": "共同描述",
                    "payment_method": " 微信支付 ",
                },
                ensure_ascii=False,
            )
        }
    )
    assert parsed_features == {
        "parser_id": "wechat",
        "counterparty": "早餐铺",
        "description": "共同描述",
        "payment_method": "微信支付",
    }

    assert BillService._calculate_learning_feature_similarity("parser_id", "wechat", "alipay") == 0.0
    assert BillService._calculate_learning_feature_similarity("description", "共同描述扩展", "共同描述") == 0.92
    assert BillService._score_learning_rule_similarity({"parser_id": "wechat"}, {"parser_id": "wechat"}) is None

    score_payload = BillService._score_learning_rule_similarity(
        {
            "parser_id": "wechat",
            "counterparty": "早餐铺",
            "description": "共同描述",
            "payment_method": "微信支付",
        },
        {
            "parser_id": "wechat",
            "counterparty": "早餐铺",
            "description": "共同描述",
            "payment_method": "微信支付",
        },
    )
    assert score_payload is not None
    assert score_payload["score"] == 1.0

    summary = BillService._build_learning_rule_result_summary(
        {
            "learned_type": "支出",
            "learned_category_id": 77,
            "learned_source_account_id": 2,
            "learned_destination_account_id": 4,
        },
        {77: {"main_category": "餐饮", "sub_category": "早餐"}},
        {2: {"name": "招商银行卡"}, 4: {"name": "支付宝"}},
    )
    assert summary == "支出 | 餐饮/早餐 | 招商银行卡 → 支付宝"

    preview = {
        "id": 11,
        "preview_parser_id": "wechat",
        "preview_counterparty": "早餐铺",
        "preview_description": "共同描述",
        "preview_payment_method": "微信支付",
    }
    learning_rule = {
        "id": 1,
        "match_features_json": json.dumps(parsed_features, ensure_ascii=False),
        "composite_match_hash": "different-hash",
        "learned_type": "支出",
        "learned_category_id": 77,
        "learned_source_account_id": 2,
        "learned_destination_account_id": 4,
        "applied_count": 5,
    }
    signal = service._build_learning_similarity_signal_from_preview(
        preview,
        [learning_rule],
        categories_by_id={77: {"main_category": "餐饮", "sub_category": "早餐"}},
        accounts_by_id={2: {"name": "招商银行卡"}, 4: {"name": "支付宝"}},
    )
    assert signal["rule_id"] == 1
    assert signal["level"] == "high"
    assert signal["recommended_type"] == "支出"
    assert signal["summary"] == "支出 | 餐饮/早餐 | 招商银行卡 → 支付宝"

    learning_rule["composite_match_hash"] = fake_db.build_composite_match_hash(
        parser_id="wechat",
        counterparty="早餐铺",
        description="共同描述",
        payment_method="微信支付",
    )
    assert service._build_learning_similarity_signal_from_preview(
        preview,
        [learning_rule],
        categories_by_id={77: {"main_category": "餐饮", "sub_category": "早餐"}},
        accounts_by_id={2: {"name": "招商银行卡"}, 4: {"name": "支付宝"}},
    ) == {}


def test_transfer_and_investment_signals_cover_threshold_levels(monkeypatch: pytest.MonkeyPatch) -> None:
    """转账/投资信号应覆盖空结果、low、medium、high 三档阈值。"""
    service = _make_service()

    assert service._build_transfer_suggestion_from_preview({"preview_type": "转账"}) == {}

    low_signal = service._build_transfer_suggestion_from_preview(
        {
            "preview_type": "支出",
            "preview_source_account_id": 1,
            "preview_destination_account_id": 2,
            "preview_destination_amount": 0,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_counterparty": "转账对手",
            "preview_payment_method": "",
            "preview_description": "提现",
            "dedup_type": "",
        }
    )
    assert low_signal["level"] == "low"

    medium_signal = service._build_transfer_suggestion_from_preview(
        {
            "preview_type": "支出",
            "preview_source_account_id": 1,
            "preview_destination_account_id": 2,
            "preview_destination_amount": 10,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_counterparty": "普通账户",
            "preview_payment_method": "",
            "preview_description": "转账记录",
            "dedup_type": "",
        }
    )
    assert medium_signal["level"] == "medium"

    high_signal = service._build_transfer_suggestion_from_preview(
        {
            "preview_type": "支出",
            "preview_source_account_id": 1,
            "preview_destination_account_id": 2,
            "preview_destination_amount": 10,
            "preview_main_category": "",
            "preview_sub_category": "",
            "preview_counterparty": "普通账户",
            "preview_payment_method": "",
            "preview_description": "无关描述",
            "dedup_type": "transfer",
        }
    )
    assert high_signal["level"] == "high"

    assert service._build_investment_signal_from_preview({"preview_type": "支出"}) == {}

    monkeypatch.setattr(service, "_score_investment_candidate", lambda *_args, **_kwargs: None)
    assert service._build_investment_signal_from_preview({"preview_type": "投资"}) == {}

    monkeypatch.setattr(
        service,
        "_score_investment_candidate",
        lambda *_args, **_kwargs: {"score": 0.66, "reason": "candidate", "platform": "蚂蚁财富", "product": "黄金ETF"},
    )
    assert service._build_investment_signal_from_preview({"preview_type": "investment"})["level"] == "medium"

    monkeypatch.setattr(
        service,
        "_score_investment_candidate",
        lambda *_args, **_kwargs: {"score": 0.81, "reason": "candidate", "platform": "蚂蚁财富", "product": "黄金ETF"},
    )
    assert service._build_investment_signal_from_preview({"preview_type": "5"})["level"] == "high"

    pnl_signal = service._build_investment_signal_from_preview(
        {
            "preview_type": "投资",
            "preview_counterparty": "天天基金",
            "preview_payment_method": "银行卡",
            "preview_description": "沪深300ETF 分红发放",
            "preview_main_category": "投资理财",
            "preview_sub_category": "基金",
        }
    )
    assert pnl_signal == {}


def test_learning_similarity_signal_suppresses_ambiguous_candidates(monkeypatch: pytest.MonkeyPatch) -> None:
    """长期学习推荐在分差过小或候选过于接近时应抑制输出。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)
    preview = {
        "id": 21,
        "preview_parser_id": "wechat",
        "preview_counterparty": "早餐铺",
        "preview_description": "共同描述",
        "preview_payment_method": "微信支付",
    }
    rules = [
        {"id": 1, "match_features_json": "rule-1", "learned_type": "支出", "applied_count": 5},
        {"id": 2, "match_features_json": "rule-2", "learned_type": "支出", "applied_count": 4},
    ]

    monkeypatch.setattr(
        service,
        "_deserialize_learning_match_features",
        lambda _rule: {
            "parser_id": "wechat",
            "counterparty": "早餐铺",
            "description": "共同描述",
            "payment_method": "微信支付",
        },
    )
    score_payloads = iter(
        [
            {"score": 0.83, "matched_fields": ["counterparty", "description"], "reason_parts": ["counterparty:exact"]},
            {"score": 0.78, "matched_fields": ["counterparty", "description"], "reason_parts": ["counterparty:similar(0.90)"]},
        ]
    )
    monkeypatch.setattr(service, "_score_learning_rule_similarity", lambda *_args, **_kwargs: next(score_payloads))

    assert service._build_learning_similarity_signal_from_preview(
        preview,
        rules,
        categories_by_id={},
        accounts_by_id={},
    ) == {}
