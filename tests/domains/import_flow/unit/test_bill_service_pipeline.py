from __future__ import annotations

import warnings
from types import SimpleNamespace
from typing import Any, cast

import pytest

from bill_analyser.core.bill_service import BillService
from bill_analyser.core.smart_dedup import DeduplicationType
from bill_analyser.utils.constants import TransactionType


class FakePipelineDB:
    """Minimal DB stub for import_bills pipeline tests."""

    def __init__(self) -> None:
        self.inserted_batches: list[tuple[list[dict[str, Any]], str, int]] = []

    async def insert_bills(self, bills: list[dict[str, Any]], batch_id: str, user_id: int = 1) -> int:
        self.inserted_batches.append((list(bills), batch_id, user_id))
        return len(bills)


class FakeLifecycleDB(FakePipelineDB):
    """DB stub that also tracks init/close lifecycle calls."""

    def __init__(self) -> None:
        super().__init__()
        self.init_calls = 0
        self.close_calls = 0

    async def init_db(self) -> None:
        self.init_calls += 1

    async def close(self) -> None:
        self.close_calls += 1


class FakeCanonicalCategoryRuleDB(FakePipelineDB):
    """DB stub exposing canonical category_rules for import classification."""

    async def get_category_rules(
        self,
        user_id: int = 1,
        category_id: int | None = None,
        enabled_only: bool = True,
        include_category_priority: bool = False,
    ) -> list[dict[str, Any]]:
        _ = (user_id, category_id, enabled_only, include_category_priority)
        return [
            {
                "id": 501,
                "category_id": 88,
                "main_category": "餐饮",
                "sub_category": "咖啡",
                "category_type": TransactionType.EXPENSE,
                "category_priority": 1,
                "priority": 1,
                "rule_expression": "OR={新规则咖啡}",
                "regex_enabled": 0,
            }
        ]


class FakeStageImportDB(FakePipelineDB):
    """DB stub for stage import flows."""

    def __init__(self) -> None:
        super().__init__()
        self.created_sessions: list[tuple[str, int, int]] = []
        self.parser_template_batches: list[tuple[str, list[dict[str, Any]], str | None, int]] = []
        self.updated_statuses: list[tuple[str, str, int]] = []
        self.unprocessed_templates: list[dict[str, Any]] = []
        self.recurring_templates: list[dict[str, Any]] = []
        self.preview_batches: list[tuple[str, list[dict[str, Any]], int]] = []
        self.updated_template_statuses: list[tuple[list[int], bool]] = []
        self.confirm_result: dict[str, Any] = {"confirmed_count": 0, "duplicate_count": 0, "errors": []}
        self.clear_result: dict[str, Any] = {"parser_count": 0, "preview_count": 0}

    async def create_import_session(self, session_id: str, user_id: int, file_count: int) -> None:
        self.created_sessions.append((session_id, user_id, file_count))

    async def insert_parser_templates(
        self,
        session_id: str,
        bills: list[dict[str, Any]],
        parser_type: str | None,
        user_id: int,
    ) -> int:
        self.parser_template_batches.append((session_id, list(bills), parser_type, user_id))
        return len(bills)

    async def update_import_session_status(
        self,
        session_id: str,
        status: str,
        total_parsed: int = 0,
        **extra_counts: Any,
    ) -> None:
        _ = extra_counts
        self.updated_statuses.append((session_id, status, total_parsed))

    async def get_unprocessed_templates_for_dedup(self, _session_id: str) -> list[dict[str, Any]]:
        return list(self.unprocessed_templates)

    async def get_enabled_recurring_templates(self, user_id: int = 1) -> list[dict[str, Any]]:
        _ = user_id
        return list(self.recurring_templates)

    def build_recurring_candidates_for_bill_data(
        self,
        _bill: dict[str, Any],
        _recurring_templates: list[dict[str, Any]],
        linked_recurring_id: Any = None,
        tolerance_days: int = 3,
    ) -> list[dict[str, Any]]:
        _ = (linked_recurring_id, tolerance_days)
        return []

    async def insert_preview_bills_batch(
        self,
        session_id: str,
        preview_list: list[dict[str, Any]],
        user_id: int = 1,
    ) -> int:
        self.preview_batches.append((session_id, list(preview_list), user_id))
        return len(preview_list)

    async def update_parser_template_status(self, template_ids: list[int], processed: bool = False) -> None:
        self.updated_template_statuses.append((list(template_ids), processed))

    async def confirm_preview_to_bills(self, _session_id: str, _user_id: int) -> dict[str, Any]:
        return dict(self.confirm_result)

    async def clear_session_data(self, _session_id: str) -> dict[str, Any]:
        return dict(self.clear_result)


class FakeMigratingCategoryRuleDB(FakeStageImportDB):
    """Stage DB stub that exposes old-keyword migration into canonical rules."""

    def __init__(self) -> None:
        super().__init__()
        self.rule_rows_available = False
        self.rule_query_user_ids: list[int] = []
        self.migration_user_ids: list[int] = []

    async def get_category_rules(
        self,
        user_id: int = 1,
        category_id: int | None = None,
        enabled_only: bool = True,
        include_category_priority: bool = False,
    ) -> list[dict[str, Any]]:
        _ = (category_id, enabled_only, include_category_priority)
        self.rule_query_user_ids.append(user_id)
        if not self.rule_rows_available:
            return []

        return [
            {
                "id": 610,
                "category_id": 91,
                "main_category": "餐饮",
                "sub_category": "早餐",
                "category_type": TransactionType.EXPENSE,
                "category_priority": 1,
                "priority": 1,
                "rule_expression": "OR={自动识别早餐}",
                "regex_enabled": 0,
            }
        ]

    async def migrate_keywords_to_rules(self, user_id: int = 1) -> dict[str, int]:
        self.migration_user_ids.append(user_id)
        self.rule_rows_available = True
        return {"migrated": 1, "skipped": 0}


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


@pytest.mark.asyncio
async def test_import_bills_preview_reloads_canonical_category_rules_for_same_user(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """同一 BillService 实例也应在导入预览前重读 canonical category_rules。"""
    fake_db = FakeCanonicalCategoryRuleDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True
    service.category_engine._initialized = True  # pylint: disable=protected-access
    service.category_engine._current_user_id = 1  # pylint: disable=protected-access
    service.category_engine.rules = []

    parsed_bills = [
        {
            "date": "2026-03-05 09:30:00",
            "type": "支出",
            "amount": -18.8,
            "description": "新规则咖啡 自动分类",
            "counterparty": "pytest 咖啡店",
            "payment_method": "支付宝",
            "main_category": "",
            "sub_category": "",
            "source_account_id": None,
        }
    ]

    monkeypatch.setattr(service.parser_factory, "parse", lambda _path, _parser=None: parsed_bills)
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

    async def match_accounts(bills: list[dict[str, Any]], user_id: int = 1):
        _ = user_id
        for bill in bills:
            bill["source_account_id"] = 3
        return bills

    monkeypatch.setattr(service.smart_dedup_engine, "process_with_db", fake_process_with_db)
    monkeypatch.setattr(service, "_apply_import_learning_rules", identity_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", identity_step)
    monkeypatch.setattr(service, "_match_accounts", match_accounts)
    monkeypatch.setattr(service, "_detect_cash_transfers", identity_step)

    result = await service.import_bills("preview.csv", parser_type="wechat", preview_only=True, user_id=1)

    assert result["success"] is True
    assert result["matched_count"] == 1
    assert result["preview"][0]["main_category"] == "餐饮"
    assert result["preview"][0]["sub_category"] == "咖啡"
    assert result["preview"][0]["is_matched"] is True


@pytest.mark.asyncio
async def test_reload_category_rules_migrates_legacy_keywords_when_canonical_rules_empty() -> None:
    """导入分类前 canonical 规则为空时，应先迁移旧关键词再重载规则。"""
    fake_db = FakeMigratingCategoryRuleDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True

    await service._reload_category_rules_for_import(user_id=7)  # pylint: disable=protected-access

    assert fake_db.migration_user_ids == [7]
    assert fake_db.rule_query_user_ids == [7, 7]
    assert len(service.category_engine.rules) == 1
    assert service.category_engine.rules[0]["main"] == "餐饮"
    assert service.category_engine.rules[0]["sub"] == "早餐"


@pytest.mark.asyncio
async def test_bill_service_lifecycle_and_import_early_return_paths(monkeypatch: pytest.MonkeyPatch) -> None:
    """初始化/上下文管理及 import_bills 的空解析、全无效和异常分支都应稳定返回。"""
    call_log: list[str] = []
    lifecycle_db = FakeLifecycleDB()

    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        service = BillService(db=cast(Any, lifecycle_db), deduplication_mode="legacy", use_smart_dedup=False)

    service.category_engine = cast(Any, FakePipelineCategoryEngine(call_log))

    async with service as entered:
        assert entered is service
        assert service._initialized is True

    assert lifecycle_db.init_calls == 1
    assert lifecycle_db.close_calls == 1
    assert call_log == ["load_rules"]
    assert len(caught) == 2

    fake_db = FakePipelineDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True
    service.category_engine = cast(Any, FakePipelineCategoryEngine([]))

    monkeypatch.setattr(service.parser_factory, "parse", lambda _path, _parser=None: [])
    empty_result = await service.import_bills("empty.csv", parser_type="wechat", preview_only=False, user_id=1)
    assert empty_result["success"] is False
    assert empty_result["errors"] == ["文件解析失败或无有效数据"]

    parsed_bills = [{"date": "2026-03-05 09:30:00", "type": "支出", "amount": -18.8}]
    monkeypatch.setattr(service.parser_factory, "parse", lambda _path, _parser=None: parsed_bills)
    monkeypatch.setattr(
        service.validator,
        "validate_bills",
        lambda bills: ([], [{"_index": 1, "_validation_errors": ["mock invalid"]}]),
    )
    invalid_result = await service.import_bills("invalid.csv", parser_type="wechat", preview_only=False, user_id=1)
    assert invalid_result["valid"] == 0
    assert invalid_result["invalid"] == 1
    assert invalid_result["errors"] == [{"index": 1, "errors": ["mock invalid"]}, "没有有效的账单数据"]

    def explode_parse(_path: str, _parser: object | None = None) -> list[dict[str, Any]]:
        raise RuntimeError("parse boom")

    monkeypatch.setattr(service.parser_factory, "parse", explode_parse)
    exception_result = await service.import_bills("boom.csv", parser_type="wechat", preview_only=False, user_id=1)
    assert exception_result["success"] is False
    assert exception_result["errors"] == ["parse boom"]


@pytest.mark.asyncio
async def test_import_stage1_parse_handles_mixed_file_outcomes(monkeypatch: pytest.MonkeyPatch) -> None:
    """阶段1 应覆盖成功写模板、空结果、识别失败和解析异常等混合分支。"""
    fake_db = FakeStageImportDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True

    parser_info_map = {
        "good.csv": {"id": "wechat"},
        "empty.csv": {"id": "alipay"},
        "boom.csv": {"id": "icbc"},
        "unknown.csv": None,
    }

    def fake_detect_parser(file_path: str):
        return parser_info_map[file_path]

    def fake_parse(file_path: str, _parser_type: str | None = None) -> list[dict[str, Any]]:
        if file_path == "boom.csv":
            raise RuntimeError("boom parser")
        if file_path == "empty.csv":
            return [{"date": "2026-03-05", "type": "支出", "amount": -1, "description": "invalid"}]
        if file_path == "good.csv":
            return [{"date": "2026-03-05", "type": "支出", "amount": -18.8, "description": "valid"}]
        return []

    def fake_validate_bills(bills: list[dict[str, Any]]):
        if bills and bills[0].get("description") == "valid":
            return list(bills), []
        return [], [{"_index": 0, "_validation_errors": ["bad preview bill"]}]

    monkeypatch.setattr(service.parser_factory, "detect_parser", fake_detect_parser)
    monkeypatch.setattr(service.parser_factory, "parse", fake_parse)
    monkeypatch.setattr(service.validator, "validate_bills", fake_validate_bills)

    result = await service.import_stage1_parse(
        ["good.csv", "empty.csv", "boom.csv", "unknown.csv"],
        session_id="session-stage1",
        user_id=7,
    )

    assert result["success"] is True
    assert result["total_parsed"] == 1
    assert set(result["failed_files"]) == {"boom.csv", "unknown.csv"}
    assert any("boom.csv: boom parser" in error for error in result["errors"])
    assert any("unknown.csv: 无法识别文件格式" in error for error in result["errors"])
    assert fake_db.created_sessions == [("session-stage1", 7, 4)]
    assert fake_db.parser_template_batches[0][0] == "session-stage1"
    assert fake_db.parser_template_batches[0][2] == "wechat"
    assert fake_db.updated_statuses == [("session-stage1", "deduping", 1)]


@pytest.mark.asyncio
async def test_import_stage2_dedup_handles_empty_templates_and_success_path(monkeypatch: pytest.MonkeyPatch) -> None:
    """阶段2 应覆盖无模板早退与成功生成预览两条主分支。"""
    fake_db = FakeStageImportDB()
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True
    service.category_engine = cast(Any, FakePipelineCategoryEngine([]))

    empty_result = await service.import_stage2_dedup("session-empty", user_id=1)
    assert empty_result["success"] is False
    assert empty_result["errors"] == ["没有待处理的解析数据"]
    assert fake_db.updated_statuses[-1] == ("session-empty", "failed", 0)

    fake_db.unprocessed_templates = [
        {
            "id": 11,
            "parser_date": "2026-03-05 09:30:00",
            "parser_amount": -18.8,
            "parser_type": "支出",
            "parser_description": "早餐",
            "parser_counterparty": "早餐铺",
            "parser_payment_method": "",
            "parser_original_type": "支出",
            "parser_original_category": "餐饮",
            "parser_account_id": 3,
            "parser_id": "alipay",
        }
    ]

    async def fake_process_with_db(bills: list[dict[str, Any]], _db: object, _user_id: int):
        kept_bill = dict(bills[0])
        kept_bill.update(
            {
                "main_category": "餐饮",
                "sub_category": "早餐",
                "source_account_id": 3,
                "_dedup_type": "transfer",
                "_destination_parser_id": "wechat",
            }
        )
        return SimpleNamespace(
            kept_bills=[kept_bill],
            original_count=1,
            removed_count=0,
            transfer_pairs=[],
            split_groups=[],
            duplicate_groups=[],
        )

    async def zero_learning(*_args: object, **_kwargs: object) -> int:
        return 0

    async def identity_step(bills: list[dict[str, Any]], user_id: int = 1):
        _ = user_id
        return bills

    monkeypatch.setattr(service.smart_dedup_engine, "process_with_db", fake_process_with_db)
    monkeypatch.setattr(service, "_apply_import_learning_rules", zero_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", identity_step)
    monkeypatch.setattr(service, "_match_accounts", identity_step)
    monkeypatch.setattr(service, "_detect_cash_transfers", identity_step)
    monkeypatch.setattr(service, "_validate_type_category_consistency", identity_step)

    success_result = await service.import_stage2_dedup("session-stage2", user_id=1)

    assert success_result["success"] is True
    assert success_result["template_count"] == 1
    assert success_result["preview_count"] == 1
    assert success_result["match_stats"] == {"category_matched": 1, "account_matched": 1, "total": 1}
    assert fake_db.preview_batches[0][0] == "session-stage2"
    assert fake_db.preview_batches[0][1][0]["preview_payment_method"] == "alipay"
    assert fake_db.preview_batches[0][1][0]["preview_amount"] == 18.8
    assert fake_db.preview_batches[0][1][0]["preview_parser_tags"] == [
        "parser:alipay",
        "channel:wallet",
        "parser:wechat",
    ]
    assert fake_db.updated_template_statuses == [([11], True)]
    assert fake_db.updated_statuses[-1] == ("session-stage2", "previewing", 0)


@pytest.mark.asyncio
async def test_import_stage2_dedup_migrates_rules_and_persists_preview_category(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """阶段2 应在旧关键词迁移后使用 canonical 规则分类并写入预览分类字段。"""
    fake_db = FakeMigratingCategoryRuleDB()
    fake_db.unprocessed_templates = [
        {
            "id": 21,
            "parser_date": "2026-03-06 08:15:00",
            "parser_amount": -12.5,
            "parser_type": "支出",
            "parser_description": "自动识别早餐 粥铺",
            "parser_counterparty": "pytest 早餐店",
            "parser_payment_method": "",
            "parser_original_type": "支出",
            "parser_original_category": "",
            "parser_account_id": 4,
            "parser_id": "wechat",
        }
    ]

    service = BillService(db=cast(Any, fake_db))
    service._initialized = True

    async def fake_process_with_db(bills: list[dict[str, Any]], _db: object, _user_id: int):
        return SimpleNamespace(
            kept_bills=[dict(bills[0])],
            original_count=1,
            removed_count=0,
            transfer_pairs=[],
            split_groups=[],
            duplicate_groups=[],
        )

    async def zero_learning(*_args: object, **_kwargs: object) -> int:
        return 0

    async def identity_step(bills: list[dict[str, Any]], user_id: int = 1):
        _ = user_id
        return bills

    monkeypatch.setattr(service.smart_dedup_engine, "process_with_db", fake_process_with_db)
    monkeypatch.setattr(service, "_apply_import_learning_rules", zero_learning)
    monkeypatch.setattr(service, "_detect_investment_candidates", identity_step)
    monkeypatch.setattr(service, "_match_accounts", identity_step)
    monkeypatch.setattr(service, "_detect_cash_transfers", identity_step)
    monkeypatch.setattr(service, "_validate_type_category_consistency", identity_step)

    result = await service.import_stage2_dedup("session-stage2-migrate", user_id=7)

    assert result["success"] is True
    assert result["match_stats"] == {"category_matched": 1, "account_matched": 1, "total": 1}
    assert fake_db.migration_user_ids == [7]
    assert fake_db.rule_query_user_ids == [7, 7]

    preview = fake_db.preview_batches[0][1][0]
    assert preview["preview_main_category"] == "餐饮"
    assert preview["preview_sub_category"] == "早餐"
    assert preview["preview_parser_tags"] == ["parser:wechat", "channel:wallet"]


@pytest.mark.asyncio
async def test_import_stage3_confirm_writes_result_and_cleans_session() -> None:
    """阶段3 应读取确认结果并清理临时数据。"""
    fake_db = FakeStageImportDB()
    fake_db.confirm_result = {"confirmed_count": 2, "duplicate_count": 1, "errors": ["duplicate preview"]}
    fake_db.clear_result = {"parser_count": 2, "preview_count": 3}
    service = BillService(db=cast(Any, fake_db))
    service._initialized = True

    result = await service.import_stage3_confirm("session-stage3", user_id=1, selected_ids=[1, 2])

    assert result == {
        "success": True,
        "session_id": "session-stage3",
        "confirmed_count": 0,
        "skipped_count": 0,
        "duplicate_count": 1,
        "errors": ["duplicate preview"],
        "imported_count": 2,
    }
