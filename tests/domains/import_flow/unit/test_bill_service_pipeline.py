from __future__ import annotations

from types import SimpleNamespace
from typing import Any, cast

import pytest

from bill_analyser.core.bill_service import BillService
from bill_analyser.core.smart_dedup import DeduplicationType


class FakePipelineDB:
    """Minimal DB stub for import_bills pipeline tests."""

    def __init__(self) -> None:
        self.inserted_batches: list[tuple[list[dict[str, Any]], str, int]] = []

    async def insert_bills(self, bills: list[dict[str, Any]], batch_id: str, user_id: int = 1) -> int:
        self.inserted_batches.append((list(bills), batch_id, user_id))
        return len(bills)


class FakePipelineCategoryEngine:
    """Category engine stub with call tracing hooks."""

    def __init__(self, call_log: list[str]) -> None:
        self.call_log = call_log
        self.is_initialized = False
        self.current_user_id = 0
        self.rules: list[dict[str, Any]] = []

    async def load_rules_from_db(self, _db: object, user_id: int = 1) -> None:
        self.call_log.append("load_rules")
        self.is_initialized = True
        self.current_user_id = user_id

    async def batch_match_categories(self, bills: list[dict[str, Any]], types: Any = None) -> list[dict[str, Any]]:
        _ = types
        self.call_log.append("categorize")
        return bills


@pytest.mark.asyncio
async def test_import_bills_runs_core_pipeline_in_expected_order(monkeypatch: pytest.MonkeyPatch) -> None:
    """导入主链应按解析→校验→去重→学习→分类→账户→写库顺序执行。"""
    call_log: list[str] = []
    fake_db = FakePipelineDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True
    service.category_engine = cast(Any, FakePipelineCategoryEngine(call_log))

    parsed_bills = [
        {
            "date": "2026-03-05 09:30:00",
            "type": "支出",
            "amount": -18.8,
            "description": "管线命中账单",
            "counterparty": "域测试早餐铺",
            "payment_method": "支付宝",
            "main_category": "域测试餐饮",
            "sub_category": "早餐",
            "source_account_id": 7,
        }
    ]

    monkeypatch.setattr(service.parser_factory, "detect_parser", lambda _path: {"id": "alipay"})
    monkeypatch.setattr(service.parser_factory, "parse", lambda _path, _parser=None: parsed_bills)
    monkeypatch.setattr(
        service.validator,
        "validate_bills",
        lambda bills: (list(bills), [{"_index": 9, "_validation_errors": ["mock invalid"]}]),
    )

    async def fake_process_with_db(valid_bills: list[dict[str, Any]], _db: object, user_id: int):
        call_log.append(f"dedup:{user_id}")
        return SimpleNamespace(
            kept_bills=list(valid_bills),
            original_count=len(valid_bills),
            removed_count=0,
            transfer_pairs=[],
            split_groups=[],
            duplicate_groups=[SimpleNamespace(type=DeduplicationType.DATABASE_DUPLICATE)],
        )

    async def fake_apply_learning(
        bills: list[dict[str, Any]],
        user_id: int = 1,
        type_only: bool = False,
        record_usage: bool = False,
    ) -> int:
        _ = bills
        call_log.append(f"learning:{type_only}:{record_usage}:{user_id}")
        return 1

    async def fake_detect_investment_candidates(bills: list[dict[str, Any]], user_id: int = 1):
        call_log.append(f"investment:{user_id}")
        return bills

    async def fake_match_accounts(bills: list[dict[str, Any]], user_id: int = 1):
        call_log.append(f"accounts:{user_id}")
        return bills

    async def fake_detect_cash_transfers(bills: list[dict[str, Any]], user_id: int = 1):
        call_log.append(f"cash:{user_id}")
        return bills

    monkeypatch.setattr(service.smart_dedup_engine, "process_with_db", fake_process_with_db)
    monkeypatch.setattr(service, "_apply_import_learning_rules", fake_apply_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", fake_detect_investment_candidates)
    monkeypatch.setattr(service, "_match_accounts", fake_match_accounts)
    monkeypatch.setattr(service, "_detect_cash_transfers", fake_detect_cash_transfers)

    result = await service.import_bills("demo.csv", parser_type="auto", preview_only=False, user_id=7)

    assert result["success"] is True
    assert result["detected_parser"] == "alipay"
    assert result["parser_type"] == "alipay"
    assert result["total"] == 1
    assert result["valid"] == 1
    assert result["invalid"] == 1
    assert result["inserted"] == 1
    assert result["duplicates"] == 0
    assert result["dedup_stats"]["database_duplicates"] == 1
    assert result["errors"] == [{"index": 9, "errors": ["mock invalid"]}]
    assert fake_db.inserted_batches[0][2] == 7
    assert call_log == [
        "dedup:7",
        "load_rules",
        "learning:True:False:7",
        "categorize",
        "investment:7",
        "accounts:7",
        "cash:7",
        "learning:False:True:7",
    ]


@pytest.mark.asyncio
async def test_import_bills_preview_mode_sorts_matched_items_before_unmatched(monkeypatch: pytest.MonkeyPatch) -> None:
    """预览模式应把“分类+账户都已匹配”的账单排到前面。"""
    fake_db = FakePipelineDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True
    service.category_engine = cast(Any, FakePipelineCategoryEngine([]))
    service.category_engine.is_initialized = True
    service.category_engine.current_user_id = 1

    preview_bills = [
        {
            "date": "2026-03-05 09:30:00",
            "type": "支出",
            "amount": -18.8,
            "description": "已匹配账单",
            "counterparty": "域测试早餐铺",
            "payment_method": "支付宝",
            "main_category": "域测试餐饮",
            "sub_category": "早餐",
            "source_account_id": 3,
        },
        {
            "date": "2026-03-06 09:30:00",
            "type": "支出",
            "amount": -28.8,
            "description": "未匹配账单",
            "counterparty": "域测试未分类",
            "payment_method": "现金",
            "main_category": "",
            "sub_category": "",
            "source_account_id": None,
        },
    ]

    monkeypatch.setattr(service.parser_factory, "detect_parser", lambda _path: {"id": "wechat"})
    monkeypatch.setattr(service.parser_factory, "parse", lambda _path, _parser=None: preview_bills)
    monkeypatch.setattr(service.validator, "validate_bills", lambda bills: (list(bills), []))

    async def fake_process_with_db(bills: list[dict[str, Any]], _db: object, _user_id: int):
        return SimpleNamespace(
            kept_bills=list(bills),
            original_count=len(bills),
            removed_count=0,
            transfer_pairs=[],
            split_groups=[],
            duplicate_groups=[],
        )

    monkeypatch.setattr(service.smart_dedup_engine, "process_with_db", fake_process_with_db)

    async def identity_learning(
        bills: list[dict[str, Any]],
        user_id: int = 1,
        type_only: bool = False,
        record_usage: bool = False,
    ) -> int:
        _ = (bills, user_id, type_only, record_usage)
        return 0

    async def identity_step(bills: list[dict[str, Any]], user_id: int = 1):
        _ = user_id
        return bills

    monkeypatch.setattr(service, "_apply_import_learning_rules", identity_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", identity_step)
    monkeypatch.setattr(service, "_match_accounts", identity_step)
    monkeypatch.setattr(service, "_detect_cash_transfers", identity_step)

    result = await service.import_bills("preview.csv", parser_type="auto", preview_only=True, user_id=1)

    assert result["success"] is True
    assert result["matched_count"] == 1
    assert result["unmatched_count"] == 1
    assert [item["description"] for item in result["preview"]] == ["已匹配账单", "未匹配账单"]
    assert result["preview"][0]["is_matched"] is True
    assert result["preview"][1]["is_matched"] is False
    assert fake_db.inserted_batches == []
