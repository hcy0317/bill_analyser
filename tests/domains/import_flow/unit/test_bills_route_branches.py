from __future__ import annotations

import asyncio
import io
from typing import TYPE_CHECKING, Any, cast

if TYPE_CHECKING:
    from pathlib import Path

import pytest
from flask import Flask

from bill_analyser.api.routes import bills as bills_module

bills_module = cast("Any", bills_module)


@pytest.fixture(name="bills_route_app")
def bills_route_app_fixture() -> Flask:
    """Create a minimal Flask app for direct bills route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeLoop:
    """Minimal event-loop adapter for direct route invocation tests."""

    def __init__(self) -> None:
        self.closed = False

    def run_until_complete(self, coroutine: Any) -> Any:
        return asyncio.run(coroutine)

    def close(self) -> None:
        self.closed = True


class FakeBillsDB:
    """Async DB stub for bills route branch tests."""

    def __init__(self) -> None:
        self.import_configs = [
            {
                "id": 7,
                "name": "CSV 模板",
                "file_format": "csv",
                "description": "desc",
                "description_summary": "summary",
                "field_mappings": {"date": "交易时间"},
                "date_format": "%Y-%m-%d",
                "encoding": "utf-8",
                "delimiter": ",",
                "skip_rows": 1,
                "has_header": True,
                "custom_rules": {"a": 1},
                "sample_headers": ["交易时间", "金额"],
                "header_signature": "sig",
                "is_default": True,
                "default_recommendation": False,
                "use_count": 8,
                "last_used_at": "2026-03-30T10:00:00",
                "created_at": "2026-03-30T09:00:00",
                "updated_at": "2026-03-30T10:00:00",
            }
        ]
        self.matching_config: dict[str, Any] | None = None
        self.saved_payloads: list[tuple[dict[str, Any], int]] = []
        self.delete_import_config_result = True
        self.batch_update_result = 2
        self.batch_delete_result = 2
        self.bill_lookup: dict[int, Any] = {
            1: {"source_account_id": 11, "destination_account_id": 22},
            2: {"source_account_id": 33, "destination_account_id": 0},
        }
        self.deleted_bill_ids: list[int] = []
        self.synced_accounts: list[int] = []
        self.raise_sync_for: set[int] = set()
        self.categories = [
            {"id": 10, "main_category": "餐饮", "sub_category": "早餐"},
            {"id": 11, "main_category": "学习", "sub_category": "课本"},
        ]
        self.accounts = [
            {"id": 1, "name": "支付宝", "aliases": "支付宝钱包", "comment": ""},
            {"id": 2, "name": "招商银行", "aliases": "", "comment": "招商"},
        ]
        self.learning_rules = [
            {
                "id": 1,
                "match_type": "description",
                "match_value": "早餐",
                "match_features_json": '{"merchant": "美团"}',
                "learned_type": "支出",
                "learned_category_id": 10,
                "learned_source_account_id": 1,
                "learned_destination_account_id": 2,
                "enabled": 1,
                "applied_count": 3,
                "created_at": "2026-03-30T09:00:00",
                "updated_at": "2026-03-30T10:00:00",
                "last_applied_at": "2026-03-30T11:00:00",
            },
            {
                "id": 2,
                "match_type": "counterparty",
                "match_value": "书店",
                "match_features_json": "{bad-json}",
                "learned_type": "支出",
                "learned_category_id": None,
                "learned_source_account_id": None,
                "learned_destination_account_id": None,
                "enabled": 0,
                "applied_count": 0,
                "created_at": "",
                "updated_at": "",
                "last_applied_at": "",
            },
        ]
        self.learning_rule_total_count = 2
        self.set_learning_rule_enabled_result = True
        self.delete_learning_rule_result = True
        self.learning_rule_count_calls: list[tuple[int, bool]] = []
        self.learning_rule_query_calls: list[tuple[int, bool, int | None, int]] = []
        self.learning_rule_update_calls: list[tuple[int, bool, int]] = []
        self.learning_rule_delete_calls: list[tuple[int, int]] = []
        self.query_bills_result: tuple[list[dict[str, Any]], int] = ([{"id": 1, "description": "早餐"}], 1)
        self.tags_for_bill: dict[int, list[dict[str, Any]]] = {1: [{"id": 7, "name": "早餐"}]}
        self.recurring_candidates_result: dict[str, Any] = {
            "bill": {"id": 1},
            "linked_recurring_id": 8,
            "linked_recurring_name": "每月早餐",
            "candidates": [{"id": 8, "name": "每月早餐"}],
        }
        self.bind_recurring_result: dict[str, Any] | None = {"recurringId": 8}
        self.unbind_recurring_result = True
        self.created_bill: dict[str, Any] | None = {"id": 123, "description": "新账单"}
        self.updated_bill: dict[str, Any] | None = {"id": 1, "source_account_id": 11, "destination_account_id": 22}
        self.delete_bill_result = True
        self.update_bill_result = True
        self.category_by_id: dict[int, dict[str, Any]] = {10: {"main_category": "餐饮", "sub_category": "早餐"}}
        self.import_session_result: dict[str, Any] | None = {
            "session_id": "sess-1",
            "status": "parsed",
            "created_at": "2026-03-30 10:00:00",
            "parsed_count": 2,
            "preview_count": 1,
            "file_paths": "a.csv;b.csv",
        }
        self.clear_session_result = True
        self.preview_rows = [
            {
                "id": 1,
                "preview_date": "2026-03-01 08:00:00",
                "preview_type": "支出",
                "preview_amount": 12.34,
                "preview_destination_amount": 0.0,
                "category_id": 10,
                "preview_main_category": "餐饮",
                "preview_sub_category": "早餐",
                "preview_source_account_id": 1,
                "preview_destination_account_id": 2,
                "preview_counterparty": "美团",
                "preview_payment_method": "支付宝",
                "preview_description": "早餐",
                "is_selected": 1,
            },
            {
                "id": 2,
                "preview_date": "2026-03-02 08:00:00",
                "preview_type": "收入",
                "preview_amount": 88.0,
                "preview_destination_amount": 0.0,
                "category_id": None,
                "preview_main_category": "",
                "preview_sub_category": "",
                "preview_source_account_id": None,
                "preview_destination_account_id": None,
                "preview_counterparty": "公司",
                "preview_payment_method": "银行卡",
                "preview_description": "工资",
                "is_selected": 0,
            },
        ]
        self.preview_recurring_result: dict[str, Any] = {
            "preview": {"id": 1},
            "linked_recurring_id": 9,
            "candidates": [{"id": 9, "name": "早餐模板"}],
        }
        self.update_preview_result = True

    async def get_import_configs(
        self,
        *,
        user_id: int,
        file_format: str | None = None,
        limit: int = 100,
    ) -> list[dict[str, Any]]:
        _ = (user_id, file_format, limit)
        return [dict(item) for item in self.import_configs]

    async def save_import_config(self, payload: dict[str, Any], *, user_id: int) -> int:
        self.saved_payloads.append((dict(payload), user_id))
        return 101

    async def find_matching_import_config(
        self,
        *,
        file_format: str,
        headers: list[str],
        user_id: int,
    ) -> dict[str, Any] | None:
        _ = (file_format, headers, user_id)
        return dict(self.matching_config) if self.matching_config else None

    async def delete_import_config(self, config_id: int, *, user_id: int) -> bool:
        _ = (config_id, user_id)
        return self.delete_import_config_result

    async def batch_update_bills(self, ids: list[int], updates: dict[str, Any], *, user_id: int) -> int:
        _ = (ids, updates, user_id)
        return self.batch_update_result

    async def get_bill_by_id(self, bill_id: int, *, user_id: int) -> dict[str, Any] | None:
        _ = user_id
        bill = self.bill_lookup.get(int(bill_id))
        if isinstance(bill, Exception):
            raise bill
        if bill is None:
            return None
        return dict(bill)

    async def batch_delete_bills(self, ids: list[int], *, user_id: int) -> int:
        _ = user_id
        self.deleted_bill_ids = [int(item) for item in ids]
        return self.batch_delete_result

    async def sync_account_balance(self, account_id: int) -> None:
        if account_id in self.raise_sync_for:
            raise RuntimeError("sync boom")
        self.synced_accounts.append(account_id)

    async def query_bills(
        self,
        *,
        page: int,
        page_size: int,
        filters: dict[str, Any],
        user_id: int,
    ) -> tuple[list[dict[str, Any]], int]:
        _ = (page, page_size, filters, user_id)
        bills, total = self.query_bills_result
        return [dict(item) for item in bills], total

    async def get_tags_for_bill(self, bill_id: int, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.tags_for_bill.get(bill_id, [])]

    async def get_recurring_candidates_for_bill(
        self,
        bill_id: int,
        *,
        user_id: int,
        tolerance_days: int,
    ) -> dict[str, Any]:
        _ = (bill_id, user_id, tolerance_days)
        return dict(self.recurring_candidates_result)

    async def bind_bill_to_recurring(self, bill_id: int, recurring_id: int, *, user_id: int) -> dict[str, Any] | None:
        _ = (bill_id, recurring_id, user_id)
        if self.bind_recurring_result is None:
            return None
        return dict(self.bind_recurring_result)

    async def unbind_bill_from_recurring(self, bill_id: int, *, user_id: int) -> bool:
        _ = (bill_id, user_id)
        return self.unbind_recurring_result

    async def create_bill(self, payload: dict[str, Any], *, user_id: int) -> int:
        _ = (payload, user_id)
        return int((self.created_bill or {}).get("id", 0))

    async def add_tags_to_bill(self, bill_id: int, tag_ids: list[int], *, user_id: int) -> None:
        _ = (bill_id, tag_ids, user_id)

    async def update_bill(self, bill_id: int, payload: dict[str, Any], *, user_id: int) -> bool:
        _ = (bill_id, payload, user_id)
        return self.update_bill_result

    async def update_bill_tags(self, bill_id: int, tag_ids: list[int], *, user_id: int) -> None:
        _ = (bill_id, tag_ids, user_id)

    async def delete_bill(self, bill_id: int, *, user_id: int) -> bool:
        _ = (bill_id, user_id)
        return self.delete_bill_result

    async def get_category_by_id(self, category_id: int, *, user_id: int) -> dict[str, Any] | None:
        _ = user_id
        category = self.category_by_id.get(category_id)
        return dict(category) if category else None

    async def get_account_by_id(self, account_id: int, *, user_id: int) -> dict[str, Any] | None:
        _ = user_id
        for account in self.accounts:
            if int(account.get("id", 0)) == int(account_id):
                return {"id": account_id, "name": account["name"], "initial_balance": 20.0}
        return None

    async def get_all_categories(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.categories]

    async def get_all_accounts(self, *, user_id: int) -> list[dict[str, Any]]:
        _ = user_id
        return [dict(item) for item in self.accounts]

    async def get_account_mappings(self) -> dict[str, Any]:
        return {"id_to_name": {int(item["id"]): item["name"] for item in self.accounts}}

    async def get_category_mappings(self) -> dict[str, Any]:
        return {"id_to_category": {int(item["id"]): dict(item) for item in self.categories}}

    async def count_import_learning_rules(self, *, user_id: int, enabled_only: bool) -> int:
        self.learning_rule_count_calls.append((user_id, enabled_only))
        return self.learning_rule_total_count

    async def get_import_learning_rules(
        self,
        *,
        user_id: int,
        enabled_only: bool,
        limit: int | None,
        offset: int,
    ) -> list[dict[str, Any]]:
        self.learning_rule_query_calls.append((user_id, enabled_only, limit, offset))
        return [dict(item) for item in self.learning_rules]

    async def set_import_learning_rule_enabled(self, rule_id: int, enabled: bool, *, user_id: int) -> bool:
        self.learning_rule_update_calls.append((rule_id, enabled, user_id))
        return self.set_learning_rule_enabled_result

    async def delete_import_learning_rule(self, rule_id: int, *, user_id: int) -> bool:
        self.learning_rule_delete_calls.append((rule_id, user_id))
        return self.delete_learning_rule_result

    async def reset_session_preview_selection(self, session_id: str) -> int:
        _ = session_id
        return 0

    async def update_preview_bills_batch(
        self,
        session_id: str,
        preview_updates: list[dict[str, Any]],
        user_id: int,
    ) -> None:
        _ = (session_id, preview_updates, user_id)

    async def get_import_session(self, session_id: str, user_id: int) -> dict[str, Any] | None:
        _ = (session_id, user_id)
        return dict(self.import_session_result) if self.import_session_result else None

    async def clear_session_data(self, session_id: str, user_id: int) -> bool:
        _ = (session_id, user_id)
        return self.clear_session_result

    async def get_preview_by_session(self, session_id: str, user_id: int) -> list[dict[str, Any]]:
        _ = (session_id, user_id)
        return [dict(item) for item in self.preview_rows]

    async def get_recurring_candidates_for_preview(
        self,
        preview_id: int,
        *,
        user_id: int,
        tolerance_days: int,
    ) -> dict[str, Any]:
        _ = (preview_id, user_id, tolerance_days)
        return dict(self.preview_recurring_result)

    async def update_preview_bill(self, preview_id: int, updates: dict[str, Any], user_id: int) -> bool:
        _ = (preview_id, updates, user_id)
        return self.update_preview_result


class FakeBillsCategoryEngine:
    """Category-engine stub for bills route branch tests."""

    def __init__(self) -> None:
        self.loaded_with: list[tuple[Any, int]] = []

    async def load_rules_from_db(self, db: Any, *, user_id: int) -> None:
        self.loaded_with.append((db, user_id))

    def match_category(self, bill: dict[str, Any]) -> tuple[str | None, str | None]:
        description = str(bill.get("description") or "")
        if description == "无分类":
            return None, None
        if description == "未知分类":
            return "未知", ""
        return "餐饮", "早餐"


class FakeBillsAdapter:
    """Transaction-adapter stub for bills route branch tests."""

    def __init__(self) -> None:
        self.frontend_to_backend_result = (
            {"type": "支出", "amount": 12.34, "source_account_id": 11, "destination_account_id": 22},
            {"category_id": "10", "tag_ids": [7], "source_account_id": 11},
        )

    def frontend_to_backend(self, _frontend_data: dict[str, Any]) -> tuple[dict[str, Any], dict[str, Any]]:
        backend_data, metadata = self.frontend_to_backend_result
        return dict(backend_data), dict(metadata)

    async def backend_list_to_frontend(
        self,
        bills: list[dict[str, Any]],
        total: int,
        page: int,
        page_size: int,
    ) -> dict[str, Any]:
        return {
            "success": True,
            "result": {"items": [dict(item) for item in bills], "totalCount": total, "page": page, "pageSize": page_size},
        }

    async def backend_to_frontend(self, bill: dict[str, Any], *args: Any, **kwargs: Any) -> dict[str, Any]:
        tags = kwargs.get("tags") or []
        _ = args
        payload = dict(bill)
        payload["tags"] = [dict(item) for item in tags]
        payload.setdefault("time", bill.get("time", bill.get("sort_time", bill.get("id", 0))))
        return payload


class FakeBillsService:
    """Async service stub for bills route branch tests."""

    def __init__(self) -> None:
        self.confirm_calls: list[tuple[list[dict[str, Any]], int]] = []
        self.keyword_calls: list[tuple[str, str | None, str, int]] = []
        self.refresh_calls: list[tuple[Any, int]] = []
        self.reclassify_calls: list[tuple[str, list[dict[str, Any]], int]] = []
        self.preview_calls: list[str] = []
        self.promote_calls: list[tuple[str, list[dict[str, Any]], int]] = []
        self.confirm_result: dict[str, Any] = {"success": True, "inserted": 2}
        self.quick_add_result = True
        self.refresh_result: dict[str, Any] = {"success": True, "categorized": 3}
        self.reclassify_result: dict[str, Any] = {
            "success": True,
            "total": 2,
            "categorized": 1,
            "account_matched": 1,
            "session_samples_saved": 1,
            "annotation_applied": 1,
        }
        self.preview_result: list[dict[str, Any]] = [{"id": 1, "description": "早餐"}]
        self.promote_result: dict[str, Any] = {"promotedCount": 2}
        self.stage1_result: dict[str, Any] = {
            "success": True,
            "session_id": "sess-1",
            "total_parsed": 2,
            "file_results": [],
            "errors": [],
        }
        self.stage2_result: dict[str, Any] = {
            "success": True,
            "template_count": 3,
            "preview_count": 2,
            "dedup_stats": {"similar": 1},
            "match_stats": {"matched": 1},
        }
        self.stage3_result: dict[str, Any] = {"success": True, "imported_count": 2, "skipped_count": 0, "errors": []}
        self.preview_result: list[dict[str, Any]] = [{"id": 1, "description": "早餐"}]
        self.stage1_calls: list[tuple[list[str], str, int]] = []
        self.stage2_calls: list[tuple[str, int]] = []
        self.stage3_calls: list[tuple[str, int, Any]] = []
        self.validator = type("Validator", (), {"validate_bills": staticmethod(lambda bills: (bills, []))})()
        self.db = type(
            "DBBridge",
            (),
            {
                "insert_parser_templates": staticmethod(lambda session_id, bills, parser_id, user_id: asyncio.sleep(0, result=len(bills)))
            },
        )()

    async def import_preview_confirmed(self, bills: list[dict[str, Any]], *, user_id: int) -> dict[str, Any]:
        self.confirm_calls.append(([dict(item) for item in bills], user_id))
        return dict(self.confirm_result)

    async def add_category_keyword(
        self,
        main_category: str,
        sub_category: str | None,
        keyword: str,
        *,
        user_id: int,
    ) -> bool:
        self.keyword_calls.append((main_category, sub_category, keyword, user_id))
        return self.quick_add_result

    async def refresh_category_for_bills(self, bill_ids: Any, *, user_id: int) -> dict[str, Any]:
        self.refresh_calls.append((bill_ids, user_id))
        return dict(self.refresh_result)

    async def reclassify_preview_bills(
        self,
        session_id: str,
        *,
        preview_updates: list[dict[str, Any]],
        user_id: int,
    ) -> dict[str, Any]:
        self.reclassify_calls.append((session_id, [dict(item) for item in preview_updates], user_id))
        return dict(self.reclassify_result)

    async def get_import_preview(self, session_id: str, selected_only: bool = True) -> list[dict[str, Any]]:
        _ = selected_only
        self.preview_calls.append(session_id)
        return [dict(item) for item in self.preview_result]

    async def promote_session_annotations_to_learning(
        self,
        session_id: str,
        *,
        preview_updates: list[dict[str, Any]],
        user_id: int,
    ) -> dict[str, Any]:
        self.promote_calls.append((session_id, [dict(item) for item in preview_updates], user_id))
        return dict(self.promote_result)

    async def import_stage1_parse(self, file_paths: list[str], session_id: str, user_id: int) -> dict[str, Any]:
        self.stage1_calls.append((list(file_paths), session_id, user_id))
        return dict(self.stage1_result)

    async def import_stage2_dedup(self, session_id: str, user_id: int) -> dict[str, Any]:
        self.stage2_calls.append((session_id, user_id))
        return dict(self.stage2_result)

    async def import_stage3_confirm(self, session_id: str, user_id: int, selected_ids: Any) -> dict[str, Any]:
        copied_selected = list(selected_ids) if isinstance(selected_ids, list) else selected_ids
        self.stage3_calls.append((session_id, user_id, copied_selected))
        return dict(self.stage3_result)


def _install_fake_loop(monkeypatch: pytest.MonkeyPatch, loop: FakeLoop) -> None:
    monkeypatch.setattr(bills_module.asyncio, "new_event_loop", lambda: loop)
    monkeypatch.setattr(bills_module.asyncio, "set_event_loop", lambda _loop: None)


def _unwrap_response(result: Any) -> tuple[Any, int]:
    if isinstance(result, tuple):
        response, status = result
        return response, status
    return result, result.status_code


def _unwrap_all(func: Any) -> Any:
    first = getattr(func, "__wrapped__", None)
    if first is None:
        return func

    second = getattr(first, "__wrapped__", None)
    return second or first


def _set_request_user_id(user_id: int = 9) -> None:
    request_obj = cast("Any", bills_module.request)
    request_obj.user_id = user_id


def test_bills_misc_routes_cover_parser_and_import_config_branches(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """轻量 bills 路由应覆盖 parser 与导入配置的校验、成功和异常分支。"""
    db = FakeBillsDB()
    service = FakeBillsService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, None))

    parsers_route = _unwrap_all(bills_module.get_available_parsers)
    list_configs_route = _unwrap_all(bills_module.list_import_configs)
    save_config_route = _unwrap_all(bills_module.save_import_config)
    match_config_route = _unwrap_all(bills_module.match_import_config)
    suggest_config_route = _unwrap_all(bills_module.suggest_import_config)
    delete_config_route = _unwrap_all(bills_module.delete_import_config)

    with bills_route_app.test_request_context("/api/bills/import/parsers", method="GET"):
        payload = parsers_route().get_json() or {}
        assert payload["success"] is True
        parser_ids = [item["id"] for item in payload["result"]]
        assert parser_ids[:3] == ["auto", "wechat", "alipay"]

    with bills_route_app.test_request_context("/api/bills/import/configs?file_format=csv&limit=5", method="GET"):
        _set_request_user_id()
        payload = list_configs_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"][0]["fileFormat"] == "csv"
        assert payload["result"][0]["sampleHeaders"] == ["交易时间", "金额"]

    with bills_route_app.test_request_context("/api/bills/import/configs", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(save_config_route())
        assert status == 400
        assert response.get_json()["error"] == "name is required"

    with bills_route_app.test_request_context(
        "/api/bills/import/configs",
        method="POST",
        json={"name": "模板"},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(save_config_route())
        assert status == 400
        assert response.get_json()["error"] == "fileFormat is required"

    with bills_route_app.test_request_context(
        "/api/bills/import/configs",
        method="POST",
        json={"name": "模板", "fileFormat": "csv"},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(save_config_route())
        assert status == 400
        assert response.get_json()["error"] == "fieldMappings is required"

    with bills_route_app.test_request_context(
        "/api/bills/import/configs",
        method="POST",
        json={
            "id": 7,
            "name": "模板",
            "fileFormat": "csv",
            "fieldMappings": {"date": "交易时间"},
            "sampleHeaders": ["交易时间"],
            "isDefault": True,
        },
    ):
        _set_request_user_id(3)
        response, status = _unwrap_response(save_config_route())
        assert status == 201
        assert response.get_json()["result"] == {"id": 101}
        assert db.saved_payloads[-1][0]["file_format"] == "csv"
        assert db.saved_payloads[-1][1] == 3

    async def _raise_value_error(*_args: Any, **_kwargs: Any) -> int:
        raise ValueError("bad config")

    monkeypatch.setattr(db, "save_import_config", _raise_value_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/configs",
        method="POST",
        json={"name": "模板", "fileFormat": "csv", "fieldMappings": {"date": "交易时间"}},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(save_config_route())
        assert status == 400
        assert response.get_json()["error"] == "bad config"

    async def _raise_runtime_error(*_args: Any, **_kwargs: Any) -> int:
        raise RuntimeError("save boom")

    monkeypatch.setattr(db, "save_import_config", _raise_runtime_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/configs",
        method="POST",
        json={"name": "模板", "fileFormat": "csv", "fieldMappings": {"date": "交易时间"}},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(save_config_route())
        assert status == 500
        assert response.get_json()["error"] == "save boom"

    with bills_route_app.test_request_context("/api/bills/import/configs/match", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(match_config_route())
        assert status == 400
        assert response.get_json()["error"] == "fileFormat is required"

    with bills_route_app.test_request_context(
        "/api/bills/import/configs/match",
        method="POST",
        json={"fileFormat": "csv", "headers": []},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(match_config_route())
        assert status == 400
        assert response.get_json()["error"] == "headers is required"

    db.matching_config = None
    with bills_route_app.test_request_context(
        "/api/bills/import/configs/match",
        method="POST",
        json={"fileFormat": "csv", "headers": ["交易时间"]},
    ):
        _set_request_user_id()
        payload = match_config_route().get_json() or {}
        assert payload == {"success": True, "result": None}

    db.matching_config = {
        "id": 8,
        "name": "命中模板",
        "file_format": "csv",
        "description": "desc",
        "description_summary": "summary",
        "field_mappings": {"date": "交易时间"},
        "date_format": "%Y-%m-%d",
        "encoding": "utf-8",
        "delimiter": ",",
        "skip_rows": 0,
        "has_header": True,
        "custom_rules": {},
        "sample_headers": ["交易时间"],
        "default_recommendation": True,
        "match_score": 0.9,
        "match_reason": "keyword",
        "matched_header_count": 1,
    }
    with bills_route_app.test_request_context(
        "/api/bills/import/configs/match",
        method="POST",
        json={"fileFormat": "csv", "headers": ["交易时间"]},
    ):
        _set_request_user_id()
        payload = match_config_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["id"] == 8
        assert payload["result"]["defaultRecommendation"] is True

    async def _raise_match_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("match boom")

    monkeypatch.setattr(db, "find_matching_import_config", _raise_match_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/configs/match",
        method="POST",
        json={"fileFormat": "csv", "headers": ["交易时间"]},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(match_config_route())
        assert status == 500
        assert response.get_json()["error"] == "match boom"

    with bills_route_app.test_request_context("/api/bills/import/configs/suggest", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(suggest_config_route())
        assert status == 400
        assert response.get_json()["error"] == "fileFormat is required"

    with bills_route_app.test_request_context(
        "/api/bills/import/configs/suggest",
        method="POST",
        json={"fileFormat": "csv", "headers": []},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(suggest_config_route())
        assert status == 400
        assert response.get_json()["error"] == "headers is required"

    monkeypatch.setattr(
        bills_module,
        "_build_import_mapping_suggestion",
        lambda headers, configs, sample_rows: {"headers": headers, "count": len(configs), "sampleRows": sample_rows},
    )
    monkeypatch.setattr(db, "get_import_configs", FakeBillsDB().get_import_configs)
    with bills_route_app.test_request_context(
        "/api/bills/import/configs/suggest",
        method="POST",
        json={"fileFormat": "csv", "headers": ["交易时间"], "sampleRows": [["2026-03-01"]]},
    ):
        _set_request_user_id(5)
        payload = suggest_config_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["headers"] == ["交易时间"]
        assert payload["result"]["count"] == 1

    async def _raise_suggest_error(*_args: Any, **_kwargs: Any) -> list[dict[str, Any]]:
        raise RuntimeError("suggest boom")

    monkeypatch.setattr(db, "get_import_configs", _raise_suggest_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/configs/suggest",
        method="POST",
        json={"fileFormat": "csv", "headers": ["交易时间"]},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(suggest_config_route())
        assert status == 500
        assert response.get_json()["error"] == "suggest boom"

    db.delete_import_config_result = False
    with bills_route_app.test_request_context("/api/bills/import/configs/7", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(delete_config_route(7))
        assert status == 404
        assert response.get_json()["error"] == "Config not found"

    db.delete_import_config_result = True
    with bills_route_app.test_request_context("/api/bills/import/configs/7", method="DELETE"):
        _set_request_user_id()
        payload = delete_config_route(7).get_json() or {}
        assert payload == {"success": True, "result": True}

    async def _raise_delete_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("delete boom")

    monkeypatch.setattr(db, "delete_import_config", _raise_delete_error)
    with bills_route_app.test_request_context("/api/bills/import/configs/7", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(delete_config_route(7))
        assert status == 500
        assert response.get_json()["error"] == "delete boom"


def test_bills_preview_confirm_keyword_refresh_and_batch_routes_cover_lightweight_branches(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """预览、确认导入、分类刷新和批量操作路由应覆盖主要轻量分支。"""
    db = FakeBillsDB()
    service = FakeBillsService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, None))
    monkeypatch.setattr(bills_module, "UPLOAD_FOLDER", tmp_path)

    preview_route = _unwrap_all(bills_module.preview_import_file)
    confirm_route = _unwrap_all(bills_module.confirm_import)
    quick_add_route = _unwrap_all(bills_module.quick_add_category_keyword)
    refresh_route = _unwrap_all(bills_module.refresh_bill_categories)
    batch_update_route = _unwrap_all(bills_module.batch_update_bills)
    batch_delete_route = _unwrap_all(bills_module.batch_delete_bills)

    with bills_route_app.test_request_context("/api/bills/import/preview", method="POST", data={}):
        response, status = _unwrap_response(preview_route())
        assert status == 400
        assert response.get_json()["error"] == "No file or temp_path provided"

    outside_path = tmp_path.parent / "escape.csv"
    with bills_route_app.test_request_context(
        "/api/bills/import/preview",
        method="POST",
        data={"temp_path": str(outside_path)},
    ):
        response, status = _unwrap_response(preview_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid temp_path"

    missing_path = tmp_path / "missing.csv"
    with bills_route_app.test_request_context(
        "/api/bills/import/preview",
        method="POST",
        data={"temp_path": str(missing_path)},
    ):
        response, status = _unwrap_response(preview_route())
        assert status == 404
        assert response.get_json()["error"] == "Temp file not found"

    existing_path = tmp_path / "preview.csv"
    existing_path.write_text("raw", encoding="utf-8")
    monkeypatch.setattr(
        bills_module,
        "_load_generic_import_rows",
        lambda *_args, **_kwargs: ([ ["说明"], ["交易时间", "金额"], ["2026-03-01", "12.34"] ], "utf-8", ","),
    )
    monkeypatch.setattr(
        bills_module,
        "_trim_generic_import_rows_to_header",
        lambda rows: (rows[1:], 1),
    )
    with bills_route_app.test_request_context(
        "/api/bills/import/preview",
        method="POST",
        data={"temp_path": str(existing_path)},
    ):
        payload = preview_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["headers"] == ["交易时间", "金额"]
        assert payload["result"]["totalRows"] == 1
        assert payload["result"]["detectedHeaderRow"] == 1

    with bills_route_app.test_request_context(
        "/api/bills/import/preview",
        method="POST",
        data={"file": (io.BytesIO(b"abc"), "bad.exe")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(preview_route())
        assert status == 400
        assert "File type not allowed" in response.get_json()["error"]

    def _raise_preview_error(*_args: Any, **_kwargs: Any) -> Any:
        raise RuntimeError("preview boom")

    monkeypatch.setattr(bills_module, "_load_generic_import_rows", _raise_preview_error)
    preview_bytes = "交易时间,金额\n2026-03-01,12.34\n".encode()
    with bills_route_app.test_request_context(
        "/api/bills/import/preview",
        method="POST",
        data={"file": (io.BytesIO(preview_bytes), "good.csv")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(preview_route())
        assert status == 500
        assert response.get_json()["error"] == "preview boom"

    with bills_route_app.test_request_context("/api/bills/import/confirm", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(confirm_route())
        assert status == 400
        assert response.get_json()["error"] == "bills is required"

    with bills_route_app.test_request_context(
        "/api/bills/import/confirm",
        method="POST",
        json={"bills": [{"description": "早餐"}]},
    ):
        _set_request_user_id(6)
        payload = confirm_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["inserted"] == 2
        assert service.confirm_calls[-1][1] == 6

    async def _raise_confirm_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("confirm boom")

    monkeypatch.setattr(service, "import_preview_confirmed", _raise_confirm_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/confirm",
        method="POST",
        json={"bills": [{"description": "早餐"}]},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(confirm_route())
        assert status == 500
        assert response.get_json()["error"] == "confirm boom"

    with bills_route_app.test_request_context(
        "/api/bills/category/quick-add-keyword",
        method="POST",
        data="null",
        content_type="application/json",
    ):
        response, status = _unwrap_response(quick_add_route())
        assert status == 400
        assert response.get_json()["error"] == "Request body is required"

    with bills_route_app.test_request_context(
        "/api/bills/category/quick-add-keyword",
        method="POST",
        json={"main_category": "餐饮"},
    ):
        response, status = _unwrap_response(quick_add_route())
        assert status == 400
        assert response.get_json()["error"] == "main_category and keyword are required"

    service.quick_add_result = True
    with bills_route_app.test_request_context(
        "/api/bills/category/quick-add-keyword",
        method="POST",
        json={"main_category": "餐饮", "sub_category": "早餐", "keyword": "美团"},
    ):
        _set_request_user_id(4)
        payload = quick_add_route().get_json() or {}
        assert payload == {"success": True, "message": "Keyword added successfully"}
        assert service.keyword_calls[-1] == ("餐饮", "早餐", "美团", 4)

    service.quick_add_result = False
    with bills_route_app.test_request_context(
        "/api/bills/category/quick-add-keyword",
        method="POST",
        json={"main_category": "餐饮", "keyword": "美团"},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(quick_add_route())
        assert status == 400
        assert response.get_json()["error"] == "Failed to add keyword"

    async def _raise_quick_add_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("keyword boom")

    monkeypatch.setattr(service, "add_category_keyword", _raise_quick_add_error)
    with bills_route_app.test_request_context(
        "/api/bills/category/quick-add-keyword",
        method="POST",
        json={"main_category": "餐饮", "keyword": "美团"},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(quick_add_route())
        assert status == 500
        assert response.get_json()["error"] == "keyword boom"

    service.refresh_result = {"success": True, "categorized": 5}
    with bills_route_app.test_request_context(
        "/api/bills/category/refresh",
        method="POST",
        json={"bill_ids": [1, 2]},
    ):
        _set_request_user_id(8)
        payload = refresh_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["categorized"] == 5
        assert service.refresh_calls[-1] == ([1, 2], 8)

    async def _raise_refresh_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("refresh boom")

    monkeypatch.setattr(service, "refresh_category_for_bills", _raise_refresh_error)
    with bills_route_app.test_request_context(
        "/api/bills/category/refresh",
        method="POST",
        json={"bill_ids": [1]},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(refresh_route())
        assert status == 500
        assert response.get_json()["error"] == "refresh boom"

    with bills_route_app.test_request_context("/api/bills/batch/update", method="PUT", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(batch_update_route())
        assert status == 400
        assert response.get_json()["error"] == "ids and updates are required"

    with bills_route_app.test_request_context(
        "/api/bills/batch/update",
        method="PUT",
        json={"ids": [1, 2], "updates": {"description": "updated"}},
    ):
        _set_request_user_id()
        payload = batch_update_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["updated_count"] == 2

    async def _raise_batch_update_error(*_args: Any, **_kwargs: Any) -> int:
        raise RuntimeError("batch update boom")

    monkeypatch.setattr(db, "batch_update_bills", _raise_batch_update_error)
    with bills_route_app.test_request_context(
        "/api/bills/batch/update",
        method="PUT",
        json={"ids": [1], "updates": {"description": "updated"}},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(batch_update_route())
        assert status == 500
        assert response.get_json()["error"] == "batch update boom"

    with bills_route_app.test_request_context("/api/bills/batch/delete", method="DELETE", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(batch_delete_route())
        assert status == 400
        assert response.get_json()["error"] == "ids are required"

    db.bill_lookup = {
        1: {"source_account_id": 11, "destination_account_id": 22},
        2: RuntimeError("lookup boom"),
        3: {"source_account_id": 33, "destination_account_id": 0},
    }
    db.raise_sync_for = {22}
    with bills_route_app.test_request_context(
        "/api/bills/batch/delete",
        method="DELETE",
        json={"ids": [1, 2, 3]},
    ):
        _set_request_user_id()
        payload = batch_delete_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["deleted_count"] == 2
        assert db.deleted_bill_ids == [1, 2, 3]
        assert sorted(db.synced_accounts) == [11, 33]

    async def _raise_batch_delete_error(*_args: Any, **_kwargs: Any) -> int:
        raise RuntimeError("batch delete boom")

    monkeypatch.setattr(db, "batch_delete_bills", _raise_batch_delete_error)
    with bills_route_app.test_request_context(
        "/api/bills/batch/delete",
        method="DELETE",
        json={"ids": [1]},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(batch_delete_route())
        assert status == 500
        assert response.get_json()["error"] == "batch delete boom"


def test_parse_import_file_covers_early_validation_branches(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """parse_import_file 应覆盖缺文件、空文件名和非法扩展名等早期校验。"""
    monkeypatch.setattr(bills_module, "UPLOAD_FOLDER", tmp_path)
    parse_route = _unwrap_all(bills_module.parse_import_file)

    with bills_route_app.test_request_context("/api/bills/parse_import", method="POST", data={}):
        response, status = _unwrap_response(parse_route())
        assert status == 400
        assert response.get_json()["error"] == "No file provided"

    with bills_route_app.test_request_context(
        "/api/bills/parse_import",
        method="POST",
        data={"file": (io.BytesIO(b"abc"), "")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(parse_route())
        assert status == 400
        assert response.get_json()["error"] == "No file selected"

    with bills_route_app.test_request_context(
        "/api/bills/parse_import",
        method="POST",
        data={"file": (io.BytesIO(b"abc"), "bad.exe")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(parse_route())
        assert status == 400
        assert "File type not allowed" in response.get_json()["error"]


def test_bills_reclassify_and_learning_routes_cover_remaining_midweight_branches(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """reclassify、长期学习与学习规则路由应覆盖主要成功/失败分支。"""
    db = FakeBillsDB()
    service = FakeBillsService()
    category_engine = FakeBillsCategoryEngine()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)

    reclassify_transactions_route = _unwrap_all(bills_module.reclassify_transactions)
    reclassify_preview_route = _unwrap_all(bills_module.reclassify_preview_session)
    promote_route = _unwrap_all(bills_module.promote_import_learning)
    list_rules_route = _unwrap_all(bills_module.list_import_learning_rules)
    update_rule_route = _unwrap_all(bills_module.update_import_learning_rule)
    delete_rule_route = _unwrap_all(bills_module.delete_import_learning_rule)

    with bills_route_app.test_request_context("/api/bills/import/reclassify", method="POST", json={}):
        response, status = _unwrap_response(reclassify_transactions_route())
        assert status == 400
        assert response.get_json()["error"] == "缺少transactions字段"

    with bills_route_app.test_request_context(
        "/api/bills/import/reclassify",
        method="POST",
        json={"transactions": {}},
    ):
        response, status = _unwrap_response(reclassify_transactions_route())
        assert status == 400
        assert response.get_json()["error"] == "transactions必须是数组"

    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, category_engine))
    with bills_route_app.test_request_context(
        "/api/bills/import/reclassify",
        method="POST",
        json={
            "transactions": [
                {
                    "description": "早餐",
                    "counterparty": "美团",
                    "amount": "12.34",
                    "type": "支出",
                    "originalSourceAccountName": "支付宝钱包",
                    "originalDestinationAccountName": "招商",
                },
                {
                    "description": "无分类",
                    "amount": "1.00",
                },
                {
                    "description": "未知分类",
                    "amount": "2.00",
                },
            ]
        },
    ):
        _set_request_user_id(9)
        payload = reclassify_transactions_route().get_json() or {}
        assert payload["success"] is True
        assert category_engine.loaded_with
        assert category_engine.loaded_with[0][1] == 9
        assert payload["result"][0]["categoryId"] == "10"
        assert payload["result"][0]["sourceAccountId"] == "1"
        assert payload["result"][0]["destinationAccountId"] == "2"
        assert payload["result"][1]["categoryId"] == ""
        assert payload["result"][1]["categoryName"] == ""
        assert payload["result"][2]["categoryId"] == ""
        assert payload["result"][2]["categoryName"] == "未知"

    async def _raise_reclassify_accounts(*_args: Any, **_kwargs: Any) -> list[dict[str, Any]]:
        raise RuntimeError("reclassify accounts boom")

    monkeypatch.setattr(db, "get_all_accounts", _raise_reclassify_accounts)
    with bills_route_app.test_request_context(
        "/api/bills/import/reclassify",
        method="POST",
        json={"transactions": [{"description": "早餐", "amount": 1}]},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(reclassify_transactions_route())
        assert status == 500
        assert response.get_json()["error"] == "reclassify accounts boom"

    monkeypatch.setattr(db, "get_all_accounts", FakeBillsDB().get_all_accounts)
    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, category_engine))

    service.reclassify_result = {
        "success": True,
        "total": 3,
        "categorized": 2,
        "account_matched": 2,
        "session_samples_saved": 1,
        "annotation_applied": 1,
    }
    service.preview_result = [{"id": 1}, {"id": 2}]
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/reclassify/sess-1",
        method="POST",
        json={"preview_updates": [{"id": 1, "selected": True}]},
    ):
        _set_request_user_id(5)
        payload = reclassify_preview_route("sess-1").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["session_id"] == "sess-1"
        assert payload["data"]["preview"] == [{"id": 1}, {"id": 2}]
        assert service.reclassify_calls[-1] == ("sess-1", [{"id": 1, "selected": True}], 5)
        assert service.preview_calls[-1] == "sess-1"

    service.reclassify_result = {"success": False, "errors": ["bad preview"]}
    with bills_route_app.test_request_context("/api/bills/import/v2/reclassify/sess-2", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(reclassify_preview_route("sess-2"))
        assert status == 500
        assert response.get_json()["error"] == "bad preview"

    service.reclassify_result = {"success": False}
    with bills_route_app.test_request_context("/api/bills/import/v2/reclassify/sess-3", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(reclassify_preview_route("sess-3"))
        assert status == 500
        assert response.get_json()["error"] == "重新分类失败"

    async def _raise_preview_reclassify(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("preview reclassify boom")

    monkeypatch.setattr(service, "reclassify_preview_bills", _raise_preview_reclassify)
    with bills_route_app.test_request_context("/api/bills/import/v2/reclassify/sess-4", method="POST", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(reclassify_preview_route("sess-4"))
        assert status == 500
        assert response.get_json()["error"] == "preview reclassify boom"

    monkeypatch.setattr(service, "reclassify_preview_bills", FakeBillsService().reclassify_preview_bills)
    service.promote_result = {"promotedCount": 4}
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/learning/sess-5/promote",
        method="POST",
        json={"preview_updates": [{"id": 9}]},
    ):
        _set_request_user_id(7)
        payload = promote_route("sess-5").get_json() or {}
        assert payload == {"success": True, "data": {"promotedCount": 4}}
        assert service.promote_calls[-1] == ("sess-5", [{"id": 9}], 7)

    async def _raise_promote_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("promote boom")

    monkeypatch.setattr(service, "promote_session_annotations_to_learning", _raise_promote_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/learning/sess-6/promote",
        method="POST",
        json={},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(promote_route("sess-6"))
        assert status == 500
        assert response.get_json()["error"] == "promote boom"

    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, category_engine))
    with bills_route_app.test_request_context(
        "/api/bills/import/learning-rules?page=99&pageSize=0&enabledOnly=true",
        method="GET",
    ):
        _set_request_user_id(4)
        response = list_rules_route()
        payload = response.get_json() or {}
        assert payload["success"] is True
        assert payload["page"] == 1
        assert payload["pageSize"] == 100
        assert payload["totalCount"] == 2
        assert payload["totalPages"] == 1
        assert payload["result"][0]["matchFeatures"] == {"merchant": "美团"}
        assert payload["result"][0]["learnedCategoryName"] == "餐饮/早餐"
        assert payload["result"][0]["learnedSourceAccountName"] == "支付宝"
        assert payload["result"][0]["learnedDestinationAccountName"] == "招商银行"
        assert payload["result"][1]["matchFeatures"] == {}
        assert response.headers["Cache-Control"] == "no-store, no-cache, must-revalidate, max-age=0"
        assert db.learning_rule_count_calls[-1] == (4, True)
        assert db.learning_rule_query_calls[-1] == (4, True, 100, 9800)

    async def _raise_count_error(*_args: Any, **_kwargs: Any) -> int:
        raise RuntimeError("list rules boom")

    monkeypatch.setattr(db, "count_import_learning_rules", _raise_count_error)
    with bills_route_app.test_request_context("/api/bills/import/learning-rules", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(list_rules_route())
        assert status == 500
        assert response.get_json()["error"] == "list rules boom"

    with bills_route_app.test_request_context(
        "/api/bills/import/learning-rules/1",
        method="PUT",
        json={},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(update_rule_route(1))
        assert status == 400
        assert response.get_json()["error"] == "enabled is required"

    monkeypatch.setattr(db, "count_import_learning_rules", FakeBillsDB().count_import_learning_rules)
    db.set_learning_rule_enabled_result = False
    with bills_route_app.test_request_context(
        "/api/bills/import/learning-rules/1",
        method="PUT",
        json={"enabled": True},
    ):
        _set_request_user_id(2)
        response, status = _unwrap_response(update_rule_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Rule not found"
        assert db.learning_rule_update_calls[-1] == (1, True, 2)

    db.set_learning_rule_enabled_result = True
    with bills_route_app.test_request_context(
        "/api/bills/import/learning-rules/1",
        method="PUT",
        json={"enabled": False},
    ):
        _set_request_user_id(2)
        payload = update_rule_route(1).get_json() or {}
        assert payload == {"success": True, "result": True}

    async def _raise_update_rule_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("update rule boom")

    monkeypatch.setattr(db, "set_import_learning_rule_enabled", _raise_update_rule_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/learning-rules/1",
        method="PUT",
        json={"enabled": True},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(update_rule_route(1))
        assert status == 500
        assert response.get_json()["error"] == "update rule boom"

    db.delete_learning_rule_result = False
    with bills_route_app.test_request_context("/api/bills/import/learning-rules/1", method="DELETE"):
        _set_request_user_id(2)
        response, status = _unwrap_response(delete_rule_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Rule not found"
        assert db.learning_rule_delete_calls[-1] == (1, 2)

    db.delete_learning_rule_result = True
    with bills_route_app.test_request_context("/api/bills/import/learning-rules/1", method="DELETE"):
        _set_request_user_id(2)
        payload = delete_rule_route(1).get_json() or {}
        assert payload == {"success": True, "result": True}

    async def _raise_delete_rule_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("delete rule boom")

    monkeypatch.setattr(db, "delete_import_learning_rule", _raise_delete_rule_error)
    with bills_route_app.test_request_context("/api/bills/import/learning-rules/1", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(delete_rule_route(1))
        assert status == 500
        assert response.get_json()["error"] == "delete rule boom"


def test_bills_picture_monthly_recurring_and_single_item_routes_cover_more_branches(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """图片、按月查询、定时交易和单条 CRUD 路由应覆盖关键轻中量分支。"""
    db = FakeBillsDB()
    service = FakeBillsService()
    category_engine = FakeBillsCategoryEngine()
    adapter = FakeBillsAdapter()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(bills_module, "UPLOAD_FOLDER", tmp_path)
    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, category_engine))
    monkeypatch.setattr(bills_module, "get_app_context_with_adapter", lambda user_id=None: (db, service, category_engine, adapter))
    monkeypatch.setattr(bills_module, "_apply_common_transaction_filters", lambda *args, **kwargs: asyncio.sleep(0, result=None))

    upload_picture_route = _unwrap_all(bills_module.upload_transaction_picture_rest)
    remove_picture_route = _unwrap_all(bills_module.remove_unused_transaction_picture_rest)
    by_month_route = _unwrap_all(bills_module.get_bills_rest_by_month)
    get_bill_route = _unwrap_all(bills_module.get_bill)
    recurring_candidates_route = _unwrap_all(bills_module.get_bill_recurring_candidates)
    bind_recurring_route = _unwrap_all(bills_module.bind_bill_recurring_match)
    unbind_recurring_route = _unwrap_all(bills_module.unbind_bill_recurring_match)
    get_bill_by_query_route = _unwrap_all(bills_module.get_bill_by_query)
    create_bill_route = _unwrap_all(bills_module.create_bill)
    update_bill_route = _unwrap_all(bills_module.update_bill)
    delete_bill_route = _unwrap_all(bills_module.delete_bill)

    with bills_route_app.test_request_context("/api/bills/pictures", method="POST", data={}):
        response, status = _unwrap_response(upload_picture_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing picture file"

    with bills_route_app.test_request_context(
        "/api/bills/pictures",
        method="POST",
        data={"picture": (io.BytesIO(b"abc"), "")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(upload_picture_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid picture file"

    with bills_route_app.test_request_context(
        "/api/bills/pictures",
        method="POST",
        data={"picture": (io.BytesIO(b"abc"), "bad.exe")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(upload_picture_route())
        assert status == 400
        assert "Picture type not allowed" in response.get_json()["error"]

    monkeypatch.setattr(bills_module, "_build_picture_data_url", lambda _path: "data:image/png;base64,abc")
    with bills_route_app.test_request_context(
        "/api/bills/pictures",
        method="POST",
        data={"picture": (io.BytesIO(b"abc"), "ok.png")},
        content_type="multipart/form-data",
    ):
        _set_request_user_id()
        payload = upload_picture_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["pictureId"].endswith(".png")
        assert payload["result"]["originalUrl"] == "data:image/png;base64,abc"

    with bills_route_app.test_request_context("/api/bills/pictures/unused", method="POST", json={}):
        response, status = _unwrap_response(remove_picture_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing picture id"

    removable = tmp_path / "remove.png"
    removable.write_bytes(b"abc")
    with bills_route_app.test_request_context(
        "/api/bills/pictures/unused",
        method="POST",
        json={"id": "remove.png"},
    ):
        _set_request_user_id()
        payload = remove_picture_route().get_json() or {}
        assert payload == {"success": True, "result": True}
        assert not removable.exists()

    with bills_route_app.test_request_context("/api/bills/by-month?year=2026&month=3&type=2", method="GET"):
        _set_request_user_id(3)
        payload = by_month_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["items"][0]["description"] == "早餐"

    with bills_route_app.test_request_context("/api/bills/by-month?year=2026&month=12&type=9", method="GET"):
        _set_request_user_id()
        payload = by_month_route().get_json() or {}
        assert payload["success"] is True

    async def _raise_query_bills(*_args: Any, **_kwargs: Any) -> tuple[list[dict[str, Any]], int]:
        raise RuntimeError("month boom")

    monkeypatch.setattr(db, "query_bills", _raise_query_bills)
    with bills_route_app.test_request_context("/api/bills/by-month?year=2026&month=3", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(by_month_route())
        assert status == 500
        assert response.get_json()["error"] == "month boom"

    monkeypatch.setattr(db, "query_bills", FakeBillsDB().query_bills)
    db.bill_lookup[1] = {"id": 1, "source_account_id": 11, "destination_account_id": 22, "description": "账单详情"}
    with bills_route_app.test_request_context("/api/bills/1", method="GET"):
        _set_request_user_id()
        payload = get_bill_route(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["id"] == 1

    db.bill_lookup[1] = None
    with bills_route_app.test_request_context("/api/bills/1", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(get_bill_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    db.bill_lookup[1] = RuntimeError("detail boom")
    with bills_route_app.test_request_context("/api/bills/1", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(get_bill_route(1))
        assert status == 500
        assert response.get_json()["error"] == "detail boom"

    db.bill_lookup[1] = {"id": 1, "source_account_id": 11, "destination_account_id": 22, "description": "账单详情"}
    with bills_route_app.test_request_context("/api/bills/1/recurring-candidates?toleranceDays=99", method="GET"):
        _set_request_user_id()
        payload = recurring_candidates_route(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["billId"] == 1
        assert payload["result"]["linkedRecurringId"] == 8

    db.recurring_candidates_result = {"bill": None}
    with bills_route_app.test_request_context("/api/bills/1/recurring-candidates", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(recurring_candidates_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    async def _raise_candidates_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("candidates boom")

    monkeypatch.setattr(db, "get_recurring_candidates_for_bill", _raise_candidates_error)
    with bills_route_app.test_request_context("/api/bills/1/recurring-candidates", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(recurring_candidates_route(1))
        assert status == 500
        assert response.get_json()["error"] == "candidates boom"

    monkeypatch.setattr(db, "get_recurring_candidates_for_bill", FakeBillsDB().get_recurring_candidates_for_bill)
    with bills_route_app.test_request_context("/api/bills/1/recurring-match", method="PUT", json={}):
        _set_request_user_id()
        response, status = _unwrap_response(bind_recurring_route(1))
        assert status == 400
        assert response.get_json()["error"] == "Missing recurringId"

    db.bind_recurring_result = {"recurringId": 88}
    with bills_route_app.test_request_context(
        "/api/bills/1/recurring-match",
        method="PUT",
        json={"recurringId": 88},
    ):
        _set_request_user_id()
        payload = bind_recurring_route(1).get_json() or {}
        assert payload == {"success": True, "result": {"recurringId": 88}}

    db.bind_recurring_result = None
    with bills_route_app.test_request_context(
        "/api/bills/1/recurring-match",
        method="PUT",
        json={"recurringId": 88},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(bind_recurring_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill or recurring template not found"

    async def _raise_bind_error(*_args: Any, **_kwargs: Any) -> dict[str, Any] | None:
        raise RuntimeError("bind boom")

    monkeypatch.setattr(db, "bind_bill_to_recurring", _raise_bind_error)
    with bills_route_app.test_request_context(
        "/api/bills/1/recurring-match",
        method="PUT",
        json={"recurringId": 88},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(bind_recurring_route(1))
        assert status == 500
        assert response.get_json()["error"] == "bind boom"

    monkeypatch.setattr(db, "bind_bill_to_recurring", FakeBillsDB().bind_bill_to_recurring)
    db.unbind_recurring_result = True
    with bills_route_app.test_request_context("/api/bills/1/recurring-match", method="DELETE"):
        _set_request_user_id()
        payload = unbind_recurring_route(1).get_json() or {}
        assert payload == {"success": True, "result": True}

    db.unbind_recurring_result = False
    with bills_route_app.test_request_context("/api/bills/1/recurring-match", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(unbind_recurring_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    async def _raise_unbind_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("unbind boom")

    monkeypatch.setattr(db, "unbind_bill_from_recurring", _raise_unbind_error)
    with bills_route_app.test_request_context("/api/bills/1/recurring-match", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(unbind_recurring_route(1))
        assert status == 500
        assert response.get_json()["error"] == "unbind boom"

    monkeypatch.setattr(db, "unbind_bill_from_recurring", FakeBillsDB().unbind_bill_from_recurring)
    with bills_route_app.test_request_context("/api/bills/get", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(get_bill_by_query_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing id parameter"

    db.bill_lookup[1] = {"id": 1, "source_account_id": 11, "destination_account_id": 22, "description": "账单详情"}
    with bills_route_app.test_request_context("/api/bills/get?id=1", method="GET"):
        _set_request_user_id()
        payload = get_bill_by_query_route().get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["id"] == 1

    with bills_route_app.test_request_context("/api/bills/", method="POST", data="null", content_type="application/json"):
        _set_request_user_id()
        response, status = _unwrap_response(create_bill_route())
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    monkeypatch.setattr(bills_module, "_prepare_backend_bill_for_create", lambda *args, **kwargs: ({"source_account_id": 11, "destination_account_id": 22}, {"tag_ids": [7]}))
    monkeypatch.setattr(bills_module, "_create_bill_and_build_response", lambda *args, **kwargs: (123, {"id": 123, "description": "新账单"}))
    with bills_route_app.test_request_context("/api/bills/", method="POST", json={"comment": "早餐"}):
        _set_request_user_id()
        response, status = _unwrap_response(create_bill_route())
        assert status == 201
        assert response.get_json()["result"]["id"] == 123

    def _raise_prepare_value_error(*_args: Any, **_kwargs: Any) -> Any:
        raise ValueError("create bad input")

    monkeypatch.setattr(bills_module, "_prepare_backend_bill_for_create", _raise_prepare_value_error)
    with bills_route_app.test_request_context("/api/bills/", method="POST", json={"comment": "早餐"}):
        _set_request_user_id()
        response, status = _unwrap_response(create_bill_route())
        assert status == 400
        assert response.get_json()["error"] == "create bad input"

    def _raise_prepare_runtime_error(*_args: Any, **_kwargs: Any) -> Any:
        raise RuntimeError("create boom")

    monkeypatch.setattr(bills_module, "_prepare_backend_bill_for_create", _raise_prepare_runtime_error)
    with bills_route_app.test_request_context("/api/bills/", method="POST", json={"comment": "早餐"}):
        _set_request_user_id()
        response, status = _unwrap_response(create_bill_route())
        assert status == 500
        assert response.get_json()["error"] == "create boom"

    with bills_route_app.test_request_context("/api/bills/1", method="PUT", data="null", content_type="application/json"):
        _set_request_user_id()
        response, status = _unwrap_response(update_bill_route(1))
        assert status == 400
        assert response.get_json()["error"] == "No data provided"

    db.bill_lookup[1] = None
    with bills_route_app.test_request_context("/api/bills/1", method="PUT", json={"description": "x"}):
        _set_request_user_id()
        response, status = _unwrap_response(update_bill_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    db.bill_lookup[1] = {"id": 1, "source_account_id": 11, "destination_account_id": 22, "description": "old"}
    db.update_bill_result = False
    with bills_route_app.test_request_context("/api/bills/1", method="PUT", json={"description": "x"}):
        _set_request_user_id()
        response, status = _unwrap_response(update_bill_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found or update failed"

    db.update_bill_result = True
    with bills_route_app.test_request_context(
        "/api/bills/1",
        method="PUT",
        json={"sourceAmount": 1234, "sourceAccountId": "11", "categoryId": "10"},
    ):
        _set_request_user_id()
        payload = update_bill_route(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["id"] == 1

    db.bill_lookup[1] = RuntimeError("update boom")
    with bills_route_app.test_request_context("/api/bills/1", method="PUT", json={"description": "x"}):
        _set_request_user_id()
        response, status = _unwrap_response(update_bill_route(1))
        assert status == 500
        assert response.get_json()["error"] == "update boom"

    db.bill_lookup[1] = None
    with bills_route_app.test_request_context("/api/bills/1", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(delete_bill_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    db.bill_lookup[1] = {"id": 1, "source_account_id": 11, "destination_account_id": 22}
    db.delete_bill_result = True
    with bills_route_app.test_request_context("/api/bills/1", method="DELETE"):
        _set_request_user_id()
        payload = delete_bill_route(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"] is True

    db.delete_bill_result = False
    with bills_route_app.test_request_context("/api/bills/1", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(delete_bill_route(1))
        assert status == 500
        assert response.get_json()["error"] == "Failed to delete bill"

    db.bill_lookup[1] = RuntimeError("delete boom")
    with bills_route_app.test_request_context("/api/bills/1", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(delete_bill_route(1))
        assert status == 500
        assert response.get_json()["error"] == "delete boom"


def test_bills_import_stage_routes_cover_validation_and_lightweight_success_paths(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """三阶段导入轻量路由应覆盖参数校验和主要轻量成功/失败路径。"""
    db = FakeBillsDB()
    service = FakeBillsService()
    category_engine = FakeBillsCategoryEngine()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(bills_module, "UPLOAD_FOLDER", tmp_path)
    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, category_engine))

    stage1_route = _unwrap_all(bills_module.import_stage1_parse)
    parse_generic_route = _unwrap_all(bills_module.import_parse_generic_into_session)
    stage2_route = _unwrap_all(bills_module.import_stage2_dedup)
    stage3_route = _unwrap_all(bills_module.import_stage3_confirm)

    with bills_route_app.test_request_context("/api/bills/import/v2/parse", method="POST", data={}):
        response, status = _unwrap_response(stage1_route())
        assert status == 400
        assert response.get_json()["error"] == "No files provided"

    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse",
        method="POST",
        data={"file": (io.BytesIO(b"abc"), "")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(stage1_route())
        assert status == 400
        assert response.get_json()["error"] == "No files selected"

    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse",
        method="POST",
        data={"file": (io.BytesIO(b"abc"), "bad.exe")},
        content_type="multipart/form-data",
    ):
        response, status = _unwrap_response(stage1_route())
        assert status == 400
        assert response.get_json()["error"] == "No valid files to process"

    service.stage1_result = {
        "success": True,
        "session_id": "sess-stage1",
        "total_parsed": 1,
        "file_results": [],
        "errors": [],
    }
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse",
        method="POST",
        data={"file": (io.BytesIO(b"a,b\n1,2\n"), "ok.csv")},
        content_type="multipart/form-data",
    ):
        _set_request_user_id(4)
        payload = stage1_route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["session_id"] == "sess-stage1"
        assert payload["data"]["parsed_count"] == 1
        assert service.stage1_calls[-1][2] == 4

    temp_file = tmp_path / "generic.csv"
    temp_file.write_text("交易时间,金额\n2026-03-01,12.34\n", encoding="utf-8")
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"temp_path": str(temp_file)},
    ):
        response, status = _unwrap_response(parse_generic_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing session_id"

    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"session_id": "sess"},
    ):
        response, status = _unwrap_response(parse_generic_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing temp_path"

    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"session_id": "sess", "temp_path": str(tmp_path.parent / 'escape.csv')},
    ):
        response, status = _unwrap_response(parse_generic_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid file path"

    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"session_id": "sess", "temp_path": str(tmp_path / 'missing.csv')},
    ):
        response, status = _unwrap_response(parse_generic_route())
        assert status == 404
        assert response.get_json()["error"] == "Temp file not found"

    monkeypatch.setattr(bills_module, "_parse_import_file_with_column_mapping", lambda *args, **kwargs: ([], "utf-8", ","))
    empty_temp = tmp_path / "empty_generic.csv"
    empty_temp.write_text("交易时间,金额\n", encoding="utf-8")
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"session_id": "sess-empty", "temp_path": str(empty_temp)},
    ):
        payload = parse_generic_route().get_json() or {}
        assert payload == {"success": True, "data": {"parsed_count": 0}}
        assert not empty_temp.exists()

    valid_temp = tmp_path / "valid_generic.csv"
    valid_temp.write_text("交易时间,金额\n2026-03-01,12.34\n", encoding="utf-8")
    monkeypatch.setattr(
        bills_module,
        "_parse_import_file_with_column_mapping",
        lambda *args, **kwargs: ([{"trade_time": "2026-03-01 08:00:00", "amount": 12.34}], "utf-8", ","),
    )
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"session_id": "sess-generic", "temp_path": str(valid_temp)},
    ):
        _set_request_user_id(5)
        payload = parse_generic_route().get_json() or {}
        assert payload == {"success": True, "data": {"parsed_count": 1}}
        assert not valid_temp.exists()

    broken_temp = tmp_path / "broken_generic.csv"
    broken_temp.write_text("交易时间,金额\n2026-03-01,12.34\n", encoding="utf-8")

    def _raise_generic_error(*_args: Any, **_kwargs: Any) -> Any:
        raise RuntimeError("generic boom")

    monkeypatch.setattr(bills_module, "_parse_import_file_with_column_mapping", _raise_generic_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/parse_generic",
        method="POST",
        json={"session_id": "sess-error", "temp_path": str(broken_temp)},
    ):
        response, status = _unwrap_response(parse_generic_route())
        assert status == 500
        assert response.get_json()["error"] == "generic boom"

    with bills_route_app.test_request_context("/api/bills/import/v2/dedup", method="POST", json={}):
        response, status = _unwrap_response(stage2_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing session_id"

    service.stage2_result = {
        "success": True,
        "template_count": 5,
        "preview_count": 3,
        "dedup_stats": {"similar": 2},
        "match_stats": {"matched": 2},
    }
    service.preview_result = [{"id": 1}, {"id": 2}, {"id": 3}]
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/dedup",
        method="POST",
        json={"session_id": "sess-dedup"},
    ):
        _set_request_user_id(6)
        response, status = _unwrap_response(stage2_route())
        payload = response.get_json() or {}
        assert status == 200
        assert payload["success"] is True
        assert payload["data"]["after_dedup"] == 3
        assert payload["data"]["preview"] == [{"id": 1}, {"id": 2}, {"id": 3}]
        assert service.stage2_calls[-1] == ("sess-dedup", 6)

    async def _raise_stage2_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("stage2 boom")

    monkeypatch.setattr(service, "import_stage2_dedup", _raise_stage2_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/dedup",
        method="POST",
        json={"session_id": "sess-dedup"},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(stage2_route())
        assert status == 500
        assert response.get_json()["error"] == "stage2 boom"

    with bills_route_app.test_request_context("/api/bills/import/v2/confirm", method="POST", json={}):
        response, status = _unwrap_response(stage3_route())
        assert status == 400
        assert response.get_json()["error"] == "Missing session_id"

    monkeypatch.setattr(service, "import_stage2_dedup", service.__class__.import_stage2_dedup.__get__(service, service.__class__))
    monkeypatch.setattr(service, "import_stage3_confirm", service.__class__.import_stage3_confirm.__get__(service, service.__class__))

    async def _reset_selection(session_id: str) -> int:
        assert session_id == "sess-confirm"
        return 2

    async def _update_preview_bills_batch(session_id: str, preview_updates: list[dict[str, Any]], user_id: int) -> None:
        assert session_id == "sess-confirm"
        assert user_id == 7
        assert preview_updates[0]["id"] == 1

    monkeypatch.setattr(db, "reset_session_preview_selection", _reset_selection)
    monkeypatch.setattr(db, "update_preview_bills_batch", _update_preview_bills_batch)
    service.stage3_result = {"success": True, "imported_count": 1, "skipped_count": 1, "errors": []}
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/confirm",
        method="POST",
        json={
            "session_id": "sess-confirm",
            "preview_updates": [
                {"id": 1, "selected": True},
                {"id": 2, "selected": False},
            ],
        },
    ):
        _set_request_user_id(7)
        response, status = _unwrap_response(stage3_route())
        payload = response.get_json() or {}
        assert status == 200
        assert payload == {"success": True, "data": {"imported_count": 1, "skipped_count": 1, "errors": []}}
        assert service.stage3_calls[-1] == ("sess-confirm", 7, [1])

    async def _raise_stage3_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("stage3 boom")

    monkeypatch.setattr(service, "import_stage3_confirm", _raise_stage3_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/confirm",
        method="POST",
        json={"session_id": "sess-confirm"},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(stage3_route())
        assert status == 500
        assert response.get_json()["error"] == "stage3 boom"


def test_bills_import_session_and_preview_routes_cover_lookup_paging_and_update_branches(
    bills_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """导入会话与预览相关路由应覆盖查询、分页、候选和更新分支。"""
    db = FakeBillsDB()
    service = FakeBillsService()
    category_engine = FakeBillsCategoryEngine()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(bills_module, "get_app_context", lambda: (db, service, category_engine))

    get_session_route = _unwrap_all(bills_module.get_import_session)
    cancel_session_route = _unwrap_all(bills_module.cancel_import_session)
    preview_route = _unwrap_all(bills_module.get_import_preview)
    preview_candidates_route = _unwrap_all(bills_module.get_preview_recurring_candidates)
    update_preview_route = _unwrap_all(bills_module.update_preview_bill)

    with bills_route_app.test_request_context("/api/bills/import/v2/session/sess-1", method="GET"):
        _set_request_user_id(3)
        payload = get_session_route("sess-1").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["session_id"] == "sess-1"
        assert payload["data"]["file_paths"] == "a.csv;b.csv"

    db.import_session_result = None
    with bills_route_app.test_request_context("/api/bills/import/v2/session/sess-2", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(get_session_route("sess-2"))
        assert status == 404
        assert response.get_json()["error"] == "Session not found or expired"

    async def _raise_get_session_error(*_args: Any, **_kwargs: Any) -> dict[str, Any] | None:
        raise RuntimeError("session boom")

    monkeypatch.setattr(db, "get_import_session", _raise_get_session_error)
    with bills_route_app.test_request_context("/api/bills/import/v2/session/sess-3", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(get_session_route("sess-3"))
        assert status == 500
        assert response.get_json()["error"] == "session boom"

    monkeypatch.setattr(db, "get_import_session", FakeBillsDB().get_import_session)
    db.clear_session_result = True
    with bills_route_app.test_request_context("/api/bills/import/v2/session/sess-4", method="DELETE"):
        _set_request_user_id(4)
        payload = cancel_session_route("sess-4").get_json() or {}
        assert payload == {"success": True, "message": "Session cleared"}

    db.clear_session_result = False
    with bills_route_app.test_request_context("/api/bills/import/v2/session/sess-5", method="DELETE"):
        _set_request_user_id()
        payload = cancel_session_route("sess-5").get_json() or {}
        assert payload == {"success": False, "message": "Session not found"}

    async def _raise_clear_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("clear session boom")

    monkeypatch.setattr(db, "clear_session_data", _raise_clear_error)
    with bills_route_app.test_request_context("/api/bills/import/v2/session/sess-6", method="DELETE"):
        _set_request_user_id()
        response, status = _unwrap_response(cancel_session_route("sess-6"))
        assert status == 500
        assert response.get_json()["error"] == "clear session boom"

    monkeypatch.setattr(db, "clear_session_data", FakeBillsDB().clear_session_data)
    with bills_route_app.test_request_context("/api/bills/import/v2/preview/sess-7?page=2&page_size=1", method="GET"):
        _set_request_user_id(5)
        payload = preview_route("sess-7").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["total"] == 2
        assert payload["data"]["page"] == 2
        assert payload["data"]["page_size"] == 1
        assert payload["data"]["preview"][0]["id"] == 2
        assert payload["data"]["preview"][0]["amount"] == 8800
        assert payload["data"]["preview"][0]["isSelected"] is False

    async def _raise_preview_list_error(*_args: Any, **_kwargs: Any) -> list[dict[str, Any]]:
        raise RuntimeError("preview list boom")

    monkeypatch.setattr(db, "get_preview_by_session", _raise_preview_list_error)
    with bills_route_app.test_request_context("/api/bills/import/v2/preview/sess-8", method="GET"):
        _set_request_user_id()
        response, status = _unwrap_response(preview_route("sess-8"))
        assert status == 500
        assert response.get_json()["error"] == "preview list boom"

    monkeypatch.setattr(db, "get_preview_by_session", FakeBillsDB().get_preview_by_session)
    db.preview_recurring_result = {"preview": {"id": 1}, "linked_recurring_id": 9, "candidates": [{"id": 9}]}
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview-item/1/recurring-candidates?toleranceDays=99",
        method="GET",
    ):
        _set_request_user_id()
        payload = preview_candidates_route(1).get_json() or {}
        assert payload["success"] is True
        assert payload["result"]["previewId"] == 1
        assert payload["result"]["linkedRecurringId"] == 9

    db.preview_recurring_result = {"preview": None}
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview-item/1/recurring-candidates",
        method="GET",
    ):
        _set_request_user_id()
        response, status = _unwrap_response(preview_candidates_route(1))
        assert status == 404
        assert response.get_json()["error"] == "Preview bill not found"

    async def _raise_preview_candidates_error(*_args: Any, **_kwargs: Any) -> dict[str, Any]:
        raise RuntimeError("preview candidates boom")

    monkeypatch.setattr(db, "get_recurring_candidates_for_preview", _raise_preview_candidates_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview-item/1/recurring-candidates",
        method="GET",
    ):
        _set_request_user_id()
        response, status = _unwrap_response(preview_candidates_route(1))
        assert status == 500
        assert response.get_json()["error"] == "preview candidates boom"

    monkeypatch.setattr(db, "get_recurring_candidates_for_preview", FakeBillsDB().get_recurring_candidates_for_preview)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview/sess-9/update",
        method="PUT",
        json={},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(update_preview_route("sess-9"))
        assert status == 400
        assert response.get_json()["error"] == "Missing bill id"

    db.update_preview_result = True
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview/sess-10/update",
        method="PUT",
        json={
            "id": 1,
            "type": "支出",
            "amount": 1234,
            "destinationAmount": 0,
            "mainCategory": "餐饮",
            "subCategory": "早餐",
            "sourceAccountId": "1",
            "destinationAccountId": "2",
            "counterparty": "美团",
            "paymentMethod": "支付宝",
            "description": "早餐",
            "isSelected": False,
        },
    ):
        _set_request_user_id(8)
        payload = update_preview_route("sess-10").get_json() or {}
        assert payload == {"success": True}

    db.update_preview_result = False
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview/sess-11/update",
        method="PUT",
        json={"id": 1},
    ):
        _set_request_user_id()
        payload = update_preview_route("sess-11").get_json() or {}
        assert payload == {"success": False}

    async def _raise_update_preview_error(*_args: Any, **_kwargs: Any) -> bool:
        raise RuntimeError("update preview boom")

    monkeypatch.setattr(db, "update_preview_bill", _raise_update_preview_error)
    with bills_route_app.test_request_context(
        "/api/bills/import/v2/preview/sess-12/update",
        method="PUT",
        json={"id": 1},
    ):
        _set_request_user_id()
        response, status = _unwrap_response(update_preview_route("sess-12"))
        assert status == 500
        assert response.get_json()["error"] == "update preview boom"
