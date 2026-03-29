from __future__ import annotations
# pyright: reportPrivateUsage=false, reportUnusedVariable=false, reportUnusedImport=false

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
        }
        self.accounts = [
            {"id": 1, "name": "默认账户"},
            {"id": 2, "name": "招商银行卡"},
            {"id": 3, "name": "现金"},
            {"id": 4, "name": "支付宝"},
            {"id": 5, "name": "理财账户"},
            {"id": 6, "name": "转入账户"},
        ]
        self.history_source: dict[str, Any] | None = None
        self.history_destination: dict[str, Any] | None = None
        self.updated_categories: list[tuple[int, dict[str, Any], int]] = []
        self.import_rules: list[dict[str, Any]] = []
        self.rule_usage_updates: list[tuple[list[int], int]] = []
        self.saved_annotation_samples: list[tuple[str, list[dict[str, Any]], int]] = []
        self.promoted_annotation_sessions: list[tuple[str, list[int] | None, int]] = []
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
    return BillService(db=cast(Any, fake_db or FakeBillServiceDB()))



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


@pytest.mark.asyncio
async def test_detect_cash_transfers_and_type_category_consistency() -> None:
    """存取转账检测和类型/分类一致性修复应覆盖收入支出两种方向。"""
    fake_db = FakeBillServiceDB()
    service = _make_service(fake_db)
    service.category_engine = cast(Any, FakeCategoryEngine())

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
    assert matched[7]["source_account_id"] is None


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
