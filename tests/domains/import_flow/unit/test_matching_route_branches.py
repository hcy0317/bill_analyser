from __future__ import annotations

import asyncio
from typing import Any, cast

import pytest
from flask import Flask

from bill_analyser.api.routes import matching as matching_module

matching_module = cast("Any", matching_module)


@pytest.fixture(name="matching_route_app")
def matching_route_app_fixture() -> Flask:
    """Create a minimal Flask app for direct matching route tests."""
    app = Flask(__name__)
    app.config["TESTING"] = True
    return app


class FakeLoop:
    """Minimal event-loop adapter for direct route invocation tests."""

    def run_until_complete(self, coroutine: Any) -> Any:
        return asyncio.run(coroutine)

    def close(self) -> None:
        return None


class FakeMatchingDB:
    """Async DB stub for matching route branch tests."""

    _UNSET = object()

    def __init__(self) -> None:
        self.import_session_result: dict[str, Any] | None = {"session_id": "session-1", "user_id": 9}
        self.pairing_investment_settings_result: dict[str, Any] | None = {
            "user_id": 9,
            "import_learning_enabled": True,
            "investment_platform_keywords": ["蚂蚁财富"],
            "investment_product_keywords": ["基金"],
            "investment_exclude_keywords": ["还款"],
        }
        self.update_pairing_investment_settings_calls: list[dict[str, Any]] = []

    async def get_import_session(self, session_id: str, user_id: int = 1) -> dict[str, Any] | None:
        _ = (session_id, user_id)
        return dict(self.import_session_result) if self.import_session_result else None

    async def get_pairing_investment_settings(self, user_id: int = 1) -> dict[str, Any] | None:
        _ = user_id
        return dict(self.pairing_investment_settings_result) if self.pairing_investment_settings_result else None

    async def update_pairing_investment_settings(
        self,
        *,
        user_id: int = 1,
        import_learning_enabled: Any = _UNSET,
        investment_platform_keywords: Any = _UNSET,
        investment_product_keywords: Any = _UNSET,
        investment_exclude_keywords: Any = _UNSET,
    ) -> dict[str, Any] | None:
        self.update_pairing_investment_settings_calls.append(
            {
                "user_id": user_id,
                "import_learning_enabled": import_learning_enabled,
                "investment_platform_keywords": investment_platform_keywords,
                "investment_product_keywords": investment_product_keywords,
                "investment_exclude_keywords": investment_exclude_keywords,
            }
        )
        if self.pairing_investment_settings_result is None:
            return None

        next_settings = dict(self.pairing_investment_settings_result)
        if import_learning_enabled is not self._UNSET:
            next_settings["import_learning_enabled"] = bool(import_learning_enabled)
        if investment_platform_keywords is not self._UNSET:
            next_settings["investment_platform_keywords"] = list(investment_platform_keywords)
        if investment_product_keywords is not self._UNSET:
            next_settings["investment_product_keywords"] = list(investment_product_keywords)
        if investment_exclude_keywords is not self._UNSET:
            next_settings["investment_exclude_keywords"] = list(investment_exclude_keywords)
        self.pairing_investment_settings_result = next_settings
        return dict(next_settings)


class FakeMatchingService:
    """Async service stub for matching route branch tests."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, int]] = []
        self.bill_candidate_calls: list[tuple[int, int]] = []
        self.reconcile_history_calls: list[tuple[list[int], int, list[str] | None]] = []
        self.matching_pairs_calls: list[int] = []
        self.accept_candidate_calls: list[tuple[str, dict[str, Any], int]] = []
        self.reject_candidate_calls: list[tuple[str, dict[str, Any], int]] = []
        self.clear_candidate_calls: list[tuple[str, dict[str, Any], int]] = []
        self.manual_pair_calls: list[tuple[int, int, int]] = []
        self.manual_investment_pair_calls: list[tuple[int, int, int]] = []
        self.deleted_pair_calls: list[tuple[int, int]] = []
        self.result = {
            "session_id": "session-1",
            "summary": {
                "preview_count": 1,
                "candidate_count": 1,
                "counts_by_kind": {
                    "transfer": 1,
                    "investment": 0,
                    "learning": 0,
                    "recurring": 0,
                },
            },
            "candidates": [{"candidate_id": "preview:1:transfer", "kind": "transfer"}],
        }
        self.bill_candidate_result = {
            "success": True,
            "bill_id": 11,
            "linked_pair": None,
            "candidates": [
                {
                    "candidate_id": "bill:11:transfer:12",
                    "bill_id": 12,
                    "score": 0.95,
                    "level": "high",
                    "reason": "same_amount|opposite_sign",
                }
            ],
        }
        self.reconcile_history_result = {
            "success": True,
            "summary": {
                "bill_count": 2,
                "candidate_count": 1,
                "linked_pair_count": 1,
            },
            "results": [
                {
                    "bill_id": 11,
                    "linked_pair": None,
                    "candidates": [
                        {
                            "candidate_id": "bill:11:transfer:12",
                            "bill_id": 12,
                            "score": 0.95,
                            "level": "high",
                            "reason": "same_amount|opposite_sign",
                        }
                    ],
                },
                {
                    "bill_id": 21,
                    "linked_pair": {
                        "id": 3,
                        "pair_type": "transfer",
                        "source": "manual",
                        "left_bill_id": 21,
                        "right_bill_id": 22,
                        "other_bill_id": 22,
                    },
                    "candidates": [],
                },
            ],
        }
        self.matching_pairs_result = {
            "success": True,
            "pairs": [
                {
                    "id": 3,
                    "pair_type": "transfer",
                    "source": "manual",
                    "left_bill_id": 11,
                    "right_bill_id": 12,
                    "created_at": "2026-07-18T10:05:00",
                    "updated_at": "2026-07-18T10:05:00",
                    "left_bill": {
                        "id": 11,
                        "date": "2026-07-18 10:00:00",
                        "type": "支出",
                        "amount": -41.0,
                        "counterparty": "pytest pair left",
                        "description": "pair list left bill",
                        "payment_method": "银行卡",
                        "main_category": "转账",
                        "sub_category": "历史后配对",
                        "source_account_id": 21,
                        "destination_account_id": 0,
                    },
                    "right_bill": {
                        "id": 12,
                        "date": "2026-07-18 10:03:00",
                        "type": "收入",
                        "amount": 41.0,
                        "counterparty": "pytest pair right",
                        "description": "pair list right bill",
                        "payment_method": "银行卡",
                        "main_category": "转账",
                        "sub_category": "历史后配对",
                        "source_account_id": 22,
                        "destination_account_id": 0,
                    },
                }
            ],
        }
        self.accept_candidate_result = {
            "success": True,
            "candidate_id": "preview:1:transfer",
            "action": "accept",
            "preview_id": 1,
            "session_id": "session-1",
            "preview": [{"id": 1, "matching": {"transfer": {"review_status": "accepted"}}}],
        }
        self.reject_candidate_result = {
            "success": True,
            "candidate_id": "preview:1:transfer",
            "action": "reject",
            "preview_id": 1,
            "session_id": "session-1",
            "preview": [{"id": 1, "matching": {"transfer": {"review_status": "rejected"}}}],
        }
        self.clear_candidate_result = {
            "success": True,
            "candidate_id": "preview:1:learning",
            "action": "clear",
            "preview_id": 1,
            "session_id": "session-1",
            "preview": [{"id": 1, "matching": {"learning": {"review_status": "pending"}}}],
        }
        self.manual_pair_result = {
            "success": True,
            "pair": {
                "id": 3,
                "pair_type": "transfer",
                "source": "manual",
                "left_bill_id": 11,
                "right_bill_id": 12,
            },
        }
        self.manual_investment_pair_result = {
            "success": True,
            "pair": {
                "id": 91,
                "pair_type": "investment",
                "source": "manual",
                "left_bill_id": 21,
                "right_bill_id": 22,
            },
        }
        self.delete_pair_result = {
            "success": True,
            "pair": {
                "id": 3,
                "pair_type": "transfer",
                "source": "manual",
                "left_bill_id": 11,
                "right_bill_id": 12,
            },
        }

    async def get_matching_session_candidates(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        self.calls.append((session_id, user_id))
        return dict(self.result)

    async def get_matching_bill_candidates(self, bill_id: int, user_id: int = 1) -> dict[str, Any]:
        self.bill_candidate_calls.append((bill_id, user_id))
        return dict(self.bill_candidate_result)

    async def reconcile_matching_history(self, bill_ids: list[int], user_id: int = 1, families: list[str] | None = None) -> dict[str, Any]:
        self.reconcile_history_calls.append((list(bill_ids), user_id, families))
        return {
            "success": bool(self.reconcile_history_result.get("success", False)),
            "summary": dict(self.reconcile_history_result.get("summary", {})),
            "results": [dict(item) for item in self.reconcile_history_result.get("results", [])],
            **({"error": self.reconcile_history_result.get("error")} if "error" in self.reconcile_history_result else {}),
            **({"status_code": self.reconcile_history_result.get("status_code")} if "status_code" in self.reconcile_history_result else {}),
        }

    async def get_matching_pairs(self, user_id: int = 1) -> dict[str, Any]:
        self.matching_pairs_calls.append(user_id)
        return {
            "success": True,
            "pairs": [dict(pair) for pair in self.matching_pairs_result.get("pairs", [])],
        }

    async def _accept_matching_candidate(
        self,
        candidate_id: str,
        payload: dict[str, Any],
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.accept_candidate_calls.append((candidate_id, dict(payload), user_id))
        result = dict(self.accept_candidate_result)
        if isinstance(result.get("pair"), dict):
            result["pair"] = dict(result["pair"])
        if isinstance(result.get("preview"), list):
            result["preview"] = list(result["preview"])
        return result

    async def _reject_matching_candidate(
        self,
        candidate_id: str,
        payload: dict[str, Any],
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.reject_candidate_calls.append((candidate_id, dict(payload), user_id))
        result = dict(self.reject_candidate_result)
        if isinstance(result.get("pair"), dict):
            result["pair"] = dict(result["pair"])
        if isinstance(result.get("preview"), list):
            result["preview"] = list(result["preview"])
        return result

    async def _clear_matching_candidate(
        self,
        candidate_id: str,
        payload: dict[str, Any],
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.clear_candidate_calls.append((candidate_id, dict(payload), user_id))
        result = dict(self.clear_candidate_result)
        if isinstance(result.get("pair"), dict):
            result["pair"] = dict(result["pair"])
        if isinstance(result.get("preview"), list):
            result["preview"] = list(result["preview"])
        return result

    async def create_manual_transfer_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.manual_pair_calls.append((bill_id, candidate_bill_id, user_id))
        return dict(self.manual_pair_result)

    async def create_manual_investment_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.manual_investment_pair_calls.append((bill_id, candidate_bill_id, user_id))
        return dict(self.manual_investment_pair_result)

    async def delete_manual_transfer_pair(self, pair_id: int, user_id: int = 1) -> dict[str, Any]:
        self.deleted_pair_calls.append((pair_id, user_id))
        return dict(self.delete_pair_result)


def _install_fake_loop(monkeypatch: pytest.MonkeyPatch, loop: FakeLoop) -> None:
    monkeypatch.setattr(matching_module.asyncio, "new_event_loop", lambda: loop)
    monkeypatch.setattr(matching_module.asyncio, "set_event_loop", lambda _loop: None)


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
    request_obj = cast("Any", matching_module.request)
    request_obj.user_id = user_id


def test_matching_route_returns_candidates_404_and_500(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """matching route 应覆盖成功、session 不存在与异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.get_matching_session_candidates)

    with matching_route_app.test_request_context("/api/matching/sessions/session-1/candidates", method="GET"):
        _set_request_user_id(9)
        payload = route("session-1").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["session_id"] == "session-1"
        assert service.calls == [("session-1", 9)]

    db.import_session_result = None
    with matching_route_app.test_request_context("/api/matching/sessions/missing/candidates", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("missing"))
        assert status == 404
        assert response.get_json()["error"] == "Import session not found"

    async def raise_runtime_error(_session_id: str, user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("matching boom")

    db.import_session_result = {"session_id": "session-1", "user_id": 9}
    monkeypatch.setattr(service, "get_matching_session_candidates", raise_runtime_error)
    with matching_route_app.test_request_context("/api/matching/sessions/session-1/candidates", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("session-1"))
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_investment_settings_route_covers_get_put_and_validation(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """pairing investment settings route 应覆盖 GET/PUT 成功、404、校验与异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.manage_matching_investment_settings)

    with matching_route_app.test_request_context("/api/matching/investment-settings", method="GET"):
        _set_request_user_id(9)
        payload = route().get_json() or {}
        assert payload == {
            "success": True,
            "data": {
                "importLearningEnabled": True,
                "investmentPlatformKeywords": ["蚂蚁财富"],
                "investmentProductKeywords": ["基金"],
                "investmentExcludeKeywords": ["还款"],
            },
        }

    with matching_route_app.test_request_context(
        "/api/matching/investment-settings",
        method="PUT",
        json={
            "importLearningEnabled": False,
            "investmentPlatformKeywords": ["京东金融"],
            "investmentProductKeywords": ["ETF"],
            "investmentExcludeKeywords": ["账单"],
        },
    ):
        _set_request_user_id(9)
        payload = route().get_json() or {}
        assert payload == {
            "success": True,
            "data": {
                "importLearningEnabled": False,
                "investmentPlatformKeywords": ["京东金融"],
                "investmentProductKeywords": ["ETF"],
                "investmentExcludeKeywords": ["账单"],
            },
        }
        assert db.update_pairing_investment_settings_calls[-1] == {
            "user_id": 9,
            "import_learning_enabled": False,
            "investment_platform_keywords": ["京东金融"],
            "investment_product_keywords": ["ETF"],
            "investment_exclude_keywords": ["账单"],
        }

    with matching_route_app.test_request_context(
        "/api/matching/investment-settings",
        method="PUT",
        json={"importLearningEnabled": "yes"},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    with matching_route_app.test_request_context(
        "/api/matching/investment-settings",
        method="PUT",
        json=["invalid"],
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    db.pairing_investment_settings_result = None
    with matching_route_app.test_request_context("/api/matching/investment-settings", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "User not found"

    async def raise_matching_settings_error(user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("matching settings boom")

    monkeypatch.setattr(db, "get_pairing_investment_settings", raise_matching_settings_error)
    with matching_route_app.test_request_context("/api/matching/investment-settings", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_bill_routes_cover_candidates_and_manual_pair_branches(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """历史正式账单 matching 路由应覆盖 GET 候选与 POST 手工配对分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    get_candidates_route = _unwrap_all(matching_module.get_matching_bill_candidates)
    create_manual_pair_route = _unwrap_all(matching_module.create_manual_pair)

    with matching_route_app.test_request_context("/api/matching/bills/11/candidates", method="GET"):
        _set_request_user_id(9)
        payload = get_candidates_route(11).get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["billId"] == 11
        assert payload["data"]["linkedPair"] is None
        assert payload["data"]["candidates"][0]["candidateId"] == "bill:11:transfer:12"
        assert payload["data"]["candidates"][0]["billId"] == 12
        assert service.bill_candidate_calls == [(11, 9)]

    service.bill_candidate_result = {"success": False, "error": "Bill not found", "status_code": 404}
    with matching_route_app.test_request_context("/api/matching/bills/999/candidates", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(get_candidates_route(999))
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    async def raise_bill_candidates_error(_bill_id: int, user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("bill candidates boom")

    monkeypatch.setattr(service, "get_matching_bill_candidates", raise_bill_candidates_error)
    with matching_route_app.test_request_context("/api/matching/bills/11/candidates", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(get_candidates_route(11))
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"

    monkeypatch.setattr(service, "get_matching_bill_candidates", FakeMatchingService().get_matching_bill_candidates)
    with matching_route_app.test_request_context("/api/matching/manual-pair", method="POST", json={}):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 400
        assert response.get_json()["error"] == "billId and candidateBillId are required"

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 11},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 400
        assert response.get_json()["error"] == "billId and candidateBillId must be different"

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 12, "pairType": "crypto"},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid pairType"

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 12, "pairType": False},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid pairType"

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": True, "candidateBillId": 12},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 12.5},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 12},
    ):
        _set_request_user_id(7)
        payload = create_manual_pair_route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["pair"]["leftBillId"] == 11
        assert payload["data"]["pair"]["rightBillId"] == 12
        assert service.manual_pair_calls == [(11, 12, 7)]

    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 21, "candidateBillId": 22, "pairType": "investment"},
    ):
        _set_request_user_id(8)
        payload = create_manual_pair_route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["pair"]["pairType"] == "investment"
        assert payload["data"]["pair"]["leftBillId"] == 21
        assert payload["data"]["pair"]["rightBillId"] == 22
        assert service.manual_investment_pair_calls == [(21, 22, 8)]

    service.manual_pair_result = {
        "success": False,
        "error": "Bills already belong to an existing transfer pair",
        "status_code": 409,
    }
    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 12},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 409
        assert response.get_json()["error"] == "Bills already belong to an existing transfer pair"

    async def raise_manual_pair_error(_bill_id: int, _candidate_bill_id: int, user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("manual pair boom")

    monkeypatch.setattr(service, "create_manual_transfer_pair", raise_manual_pair_error)
    with matching_route_app.test_request_context(
        "/api/matching/manual-pair",
        method="POST",
        json={"billId": 11, "candidateBillId": 12},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(create_manual_pair_route())
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_pair_delete_route_covers_success_404_and_500(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """历史正式账单手工配对删除路由应覆盖成功、404 与异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    delete_pair_route = _unwrap_all(matching_module.delete_manual_pair)

    with matching_route_app.test_request_context("/api/matching/pairs/3", method="DELETE"):
        _set_request_user_id(9)
        payload = delete_pair_route(3).get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["pair"]["id"] == 3
        assert service.deleted_pair_calls == [(3, 9)]

    service.delete_pair_result = {
        "success": False,
        "error": "Pair not found",
        "status_code": 404,
    }
    with matching_route_app.test_request_context("/api/matching/pairs/999", method="DELETE"):
        _set_request_user_id(9)
        response, status = _unwrap_response(delete_pair_route(999))
        assert status == 404
        assert response.get_json()["error"] == "Pair not found"

    async def raise_delete_pair_error(_pair_id: int, user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("delete pair boom")

    monkeypatch.setattr(service, "delete_manual_transfer_pair", raise_delete_pair_error)
    with matching_route_app.test_request_context("/api/matching/pairs/3", method="DELETE"):
        _set_request_user_id(9)
        response, status = _unwrap_response(delete_pair_route(3))
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_pairs_route_returns_serialized_pairs_empty_and_500(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """pair 列表路由应覆盖成功、空列表和异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    list_pairs_route = _unwrap_all(matching_module.get_matching_pairs)

    with matching_route_app.test_request_context("/api/matching/pairs", method="GET"):
        _set_request_user_id(9)
        payload = list_pairs_route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["pairs"][0]["id"] == 3
        assert payload["data"]["pairs"][0]["pairType"] == "transfer"
        assert payload["data"]["pairs"][0]["leftBill"]["paymentMethod"] == "银行卡"
        assert payload["data"]["pairs"][0]["rightBill"]["description"] == "pair list right bill"
        assert service.matching_pairs_calls == [9]

    service.matching_pairs_result = {"success": True, "pairs": []}
    with matching_route_app.test_request_context("/api/matching/pairs", method="GET"):
        _set_request_user_id(9)
        payload = list_pairs_route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"] == {"pairs": []}

    async def raise_matching_pairs_error(user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("matching pairs boom")

    monkeypatch.setattr(service, "get_matching_pairs", raise_matching_pairs_error)
    with matching_route_app.test_request_context("/api/matching/pairs", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(list_pairs_route())
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_candidates_route_requires_exactly_one_selector_and_dispatches_branches(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """统一 candidates 入口应要求 selector 二选一，并分发到 session/bill 分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.get_matching_candidates)

    with matching_route_app.test_request_context("/api/matching/candidates", method="GET"):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Exactly one of sessionId or billId is required"

    with matching_route_app.test_request_context(
        "/api/matching/candidates?sessionId=session-1&billId=11",
        method="GET",
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Exactly one of sessionId or billId is required"

    with matching_route_app.test_request_context(
        "/api/matching/candidates?sessionId=session-1",
        method="GET",
    ):
        _set_request_user_id(9)
        payload = route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["session_id"] == "session-1"
        assert service.calls == [("session-1", 9)]

    with matching_route_app.test_request_context(
        "/api/matching/candidates?billId=11",
        method="GET",
    ):
        _set_request_user_id(9)
        payload = route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["billId"] == 11
        assert payload["data"]["candidates"][0]["candidateId"] == "bill:11:transfer:12"
        assert service.bill_candidate_calls == [(11, 9)]


def test_matching_candidates_route_preserves_validation_404_and_500(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """统一 candidates 入口应保持 selector 校验、404 与 500 语义。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.get_matching_candidates)

    with matching_route_app.test_request_context(
        "/api/matching/candidates?billId=abc",
        method="GET",
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid billId"

    db.import_session_result = None
    with matching_route_app.test_request_context(
        "/api/matching/candidates?sessionId=missing",
        method="GET",
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "Import session not found"

    service.bill_candidate_result = {"success": False, "error": "Bill not found", "status_code": 404}
    with matching_route_app.test_request_context(
        "/api/matching/candidates?billId=999",
        method="GET",
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    async def raise_session_error(_session_id: str, user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("matching candidates session boom")

    db.import_session_result = {"session_id": "session-1", "user_id": 9}
    monkeypatch.setattr(service, "get_matching_session_candidates", raise_session_error)
    with matching_route_app.test_request_context(
        "/api/matching/candidates?sessionId=session-1",
        method="GET",
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_reconcile_history_route_validates_and_dispatches(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """reconcile-history route 应校验 billIds[]，并分发到 service 批量历史调和入口。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.reconcile_matching_history)

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json=[11],
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "billIds is required"

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": []},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "billIds must be a non-empty list"

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [11, "abc"]},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid billIds"

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [True]},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 400
        assert response.get_json()["error"] == "Invalid billIds"

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [11, 21, 11]},
    ):
        _set_request_user_id(9)
        payload = route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["summary"] == {
            "billCount": 2,
            "candidateCount": 1,
            "linkedPairCount": 1,
        }
        assert [item["billId"] for item in payload["data"]["results"]] == [11, 21]
        assert payload["data"]["results"][0]["candidates"][0]["candidateId"] == "bill:11:transfer:12"
        assert payload["data"]["results"][1]["linkedPair"]["otherBillId"] == 22
        assert service.reconcile_history_calls == [([11, 21], 9, None)]


def test_matching_reconcile_history_route_preserves_service_errors_and_500(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """reconcile-history route 应保留 service 错误状态码，并覆盖异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.reconcile_matching_history)

    service.reconcile_history_result = {
        "success": False,
        "error": "Bill not found",
        "status_code": 404,
        "results": [],
        "summary": {},
    }
    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [11]},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 404
        assert response.get_json()["error"] == "Bill not found"

    async def raise_reconcile_error(_bill_ids: list[int], user_id: int = 1, families: list[str] | None = None) -> dict[str, Any]:
        _ = user_id
        _ = families
        raise RuntimeError("reconcile history boom")

    monkeypatch.setattr(service, "reconcile_matching_history", raise_reconcile_error)
    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [11]},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_reconcile_history_route_passes_families_parameter(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """reconcile-history route 应将 families 参数正确传递给 service。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.reconcile_matching_history)

    service.reconcile_history_result = {
        "success": True,
        "summary": {"billCount": 1, "candidateCount": 0, "linkedPairCount": 0},
        "results": [{"billId": 11, "linkedPair": None, "candidates": []}],
    }

    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [11], "families": ["transfer", "investment"]},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route())
        assert status == 200
        assert service.reconcile_history_calls == [([11], 9, ["transfer", "investment"])]

    service.reconcile_history_calls.clear()
    with matching_route_app.test_request_context(
        "/api/matching/reconcile-history",
        method="POST",
        json={"billIds": [11]},
    ):
        _set_request_user_id(9)
        _unwrap_response(route())
        assert service.reconcile_history_calls == [([11], 9, None)]


def test_matching_candidate_accept_route_dispatches_preview_and_bill_transfer_candidates(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic accept route 应把 preview/bill transfer candidate 分发到 service，并序列化返回。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.accept_matching_candidate)

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/accept",
        method="POST",
        json={"expectedState": {"sessionId": "session-1", "reviewStatus": "pending"}},
    ):
        _set_request_user_id(9)
        payload = route("preview:1:transfer").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:transfer"
        assert payload["data"]["action"] == "accept"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert service.accept_candidate_calls == [
            (
                "preview:1:transfer",
                {"expectedState": {"sessionId": "session-1", "reviewStatus": "pending"}},
                9,
            )
        ]

    service.accept_candidate_result = {
        "success": True,
        "candidate_id": "bill:11:transfer:12",
        "action": "accept",
        "pair": {
            "id": 3,
            "pair_type": "transfer",
            "source": "manual",
            "left_bill_id": 11,
            "right_bill_id": 12,
        },
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/bill:11:transfer:12/accept",
        method="POST",
        json={},
    ):
        _set_request_user_id(7)
        payload = route("bill:11:transfer:12").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "bill:11:transfer:12"
        assert payload["data"]["pair"]["leftBillId"] == 11
        assert payload["data"]["pair"]["rightBillId"] == 12
        assert service.accept_candidate_calls[-1] == ("bill:11:transfer:12", {}, 7)


def test_matching_candidate_accept_route_dispatches_preview_recurring_candidate(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic accept route 应分发 preview recurring candidate，并保留 recurring payload。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.accept_matching_candidate)
    service.accept_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:recurring",
        "action": "accept",
        "preview_id": 1,
        "session_id": "session-1",
        "recurring_id": 9,
        "preview": [
            {
                "id": 1,
                "preview_recurring_id": 9,
                "matching": {"recurring": {"id": 9, "candidate_count": 2}},
            }
        ],
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:recurring/accept",
        method="POST",
        json={
            "recurringId": 9,
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": 10,
                "recurringId": None,
            },
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:recurring").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:recurring"
        assert payload["data"]["action"] == "accept"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert payload["data"]["recurringId"] == 9
        assert service.accept_candidate_calls == [
            (
                "preview:1:recurring",
                {
                    "recurringId": 9,
                    "expectedState": {
                        "sessionId": "session-1",
                        "reviewStatus": "pending",
                        "previewType": "支出",
                        "categoryId": 10,
                        "recurringId": None,
                    },
                },
                9,
            )
        ]


def test_matching_candidate_accept_route_dispatches_preview_investment_candidate(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic accept route 应分发 preview investment candidate，并返回最小审查状态。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.accept_matching_candidate)
    service.accept_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:investment",
        "action": "accept",
        "preview_id": 1,
        "session_id": "session-1",
        "review_status": "accepted",
        "suppressed": False,
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:investment/accept",
        method="POST",
        json={
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "pending",
                "previewType": "投资",
                "categoryId": 10,
                "recurringId": None,
            },
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:investment").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:investment"
        assert payload["data"]["action"] == "accept"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert payload["data"]["reviewStatus"] == "accepted"
        assert payload["data"]["suppressed"] is False
        assert "preview" not in payload["data"]
        assert service.accept_candidate_calls == [
            (
                "preview:1:investment",
                {
                    "expectedState": {
                        "sessionId": "session-1",
                        "reviewStatus": "pending",
                        "previewType": "投资",
                        "categoryId": 10,
                        "recurringId": None,
                    },
                },
                9,
            )
        ]


def test_matching_candidate_accept_route_dispatches_preview_learning_with_rule_id(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic accept route 应分发 preview learning candidate，并携带 ruleId。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.accept_matching_candidate)
    service.accept_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:learning",
        "action": "accept",
        "preview_id": 1,
        "session_id": "session-1",
        "preview": [
            {
                "id": 1,
                "preview_type": "收入",
                "matching": {"learning": {"rule_id": 42, "review_status": "accepted", "suppressed": False}},
            }
        ],
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/accept",
        method="POST",
        json={
            "ruleId": 42,
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": None,
                "recurringId": None,
            },
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:learning").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:learning"
        assert payload["data"]["action"] == "accept"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert payload["data"]["preview"][0]["matching"]["learning"]["review_status"] == "accepted"
        assert service.accept_candidate_calls == [
            (
                "preview:1:learning",
                {
                    "ruleId": 42,
                    "expectedState": {
                        "sessionId": "session-1",
                        "reviewStatus": "pending",
                        "previewType": "支出",
                        "categoryId": None,
                        "recurringId": None,
                    },
                },
                9,
            )
        ]


def test_matching_candidate_accept_route_dispatches_historical_learning_candidate(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic accept route 应分发 historical learning candidate，并序列化更新后的 bill。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.accept_matching_candidate)
    service.accept_candidate_result = {
        "success": True,
        "candidate_id": "bill:11:learning:21:r1",
        "action": "accept",
        "bill": {
            "id": 11,
            "date": "2026-07-24 16:00:00",
            "type": "收入",
            "amount": -72.5,
            "counterparty": "pytest learning bill",
            "description": "accepted historical learning bill",
            "payment_method": "银行卡",
            "main_category": "餐饮",
            "sub_category": "午餐",
            "source_account_id": 31,
            "destination_account_id": 32,
        },
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/bill:11:learning:21:r1/accept",
        method="POST",
        json={},
    ):
        _set_request_user_id(7)
        payload = route("bill:11:learning:21:r1").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "bill:11:learning:21:r1"
        assert payload["data"]["action"] == "accept"
        assert payload["data"]["bill"]["id"] == 11
        assert payload["data"]["bill"]["type"] == "收入"
        assert payload["data"]["bill"]["mainCategory"] == "餐饮"
        assert payload["data"]["bill"]["destinationAccountId"] == 32
        assert service.accept_candidate_calls == [("bill:11:learning:21:r1", {}, 7)]


def test_matching_candidate_accept_route_rejects_invalid_request_and_preserves_errors(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic accept route 应覆盖非法请求、业务错误与异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.accept_matching_candidate)

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/accept",
        method="POST",
        json=["accept"],
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:transfer"))
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    service.accept_candidate_result = {
        "success": False,
        "error": "Missing recurringId",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:recurring/accept",
        method="POST",
        json={"expectedState": {"sessionId": "session-1", "reviewStatus": "pending"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:recurring"))
        assert status == 400
        assert response.get_json()["error"] == "Missing recurringId"

    service.accept_candidate_result = {
        "success": False,
        "error": "Missing ruleId",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/accept",
        method="POST",
        json={"expectedState": {"sessionId": "session-1", "reviewStatus": "pending"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:learning"))
        assert status == 400
        assert response.get_json()["error"] == "Missing ruleId"

    service.accept_candidate_result = {
        "success": False,
        "error": "Candidate family not supported",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:mystery/accept",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:mystery"))
        assert status == 400
        assert response.get_json()["error"] == "Candidate family not supported"

    service.accept_candidate_result = {
        "success": False,
        "error": "Preview state changed, please refresh",
        "status_code": 409,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/accept",
        method="POST",
        json={"expectedState": {"sessionId": "session-1"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:transfer"))
        assert status == 409
        assert response.get_json()["error"] == "Preview state changed, please refresh"

    async def raise_accept_error(_candidate_id: str, _payload: dict[str, Any], user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("accept candidate boom")

    monkeypatch.setattr(service, "_accept_matching_candidate", raise_accept_error)
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/accept",
        method="POST",
        json={"expectedState": {"sessionId": "session-1"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:transfer"))
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_candidate_reject_route_dispatches_supported_candidates_and_preserves_boundary(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic reject route 应分发 preview transfer / investment / learning / recurring / historical transfer，并保留其他 family 边界。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.reject_matching_candidate)

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/reject",
        method="POST",
        json={"expectedState": {"sessionId": "session-1", "reviewStatus": "pending"}},
    ):
        _set_request_user_id(9)
        payload = route("preview:1:transfer").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:transfer"
        assert payload["data"]["action"] == "reject"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert service.reject_candidate_calls == [
            (
                "preview:1:transfer",
                {"expectedState": {"sessionId": "session-1", "reviewStatus": "pending"}},
                9,
            )
        ]

    service.reject_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:investment",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-1",
        "review_status": "rejected",
        "suppressed": True,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:investment/reject",
        method="POST",
        json={
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "pending",
                "previewType": "投资",
                "categoryId": 10,
                "recurringId": None,
            }
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:investment").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:investment"
        assert payload["data"]["action"] == "reject"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert payload["data"]["reviewStatus"] == "rejected"
        assert payload["data"]["suppressed"] is True
        assert "preview" not in payload["data"]
        assert service.reject_candidate_calls[-1] == (
            "preview:1:investment",
            {
                "expectedState": {
                    "sessionId": "session-1",
                    "reviewStatus": "pending",
                    "previewType": "投资",
                    "categoryId": 10,
                    "recurringId": None,
                }
            },
            9,
        )

    service.reject_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:learning",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-1",
        "preview": [
            {
                "id": 1,
                "matching": {
                    "learning": {
                        "review_status": "rejected",
                        "suppressed": True,
                    }
                },
            }
        ],
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/reject",
        method="POST",
        json={
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": None,
                "recurringId": None,
            }
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:learning").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:learning"
        assert payload["data"]["action"] == "reject"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert service.reject_candidate_calls[-1] == (
            "preview:1:learning",
            {
                "expectedState": {
                    "sessionId": "session-1",
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": None,
                    "recurringId": None,
                }
            },
            9,
        )

    service.reject_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:recurring",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-1",
        "preview": [
            {
                "id": 1,
                "matching": {"recurring": {"id": None, "candidate_count": 2}},
            }
        ],
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:recurring/reject",
        method="POST",
        json={
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "pending",
                "previewType": "支出",
                "categoryId": 10,
                "recurringId": 9,
            }
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:recurring").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:recurring"
        assert payload["data"]["action"] == "reject"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert service.reject_candidate_calls[-1] == (
            "preview:1:recurring",
            {
                "expectedState": {
                    "sessionId": "session-1",
                    "reviewStatus": "pending",
                    "previewType": "支出",
                    "categoryId": 10,
                    "recurringId": 9,
                }
            },
            9,
        )

    service.reject_candidate_result = {
        "success": True,
        "candidate_id": "bill:11:transfer:12",
        "action": "reject",
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/bill:11:transfer:12/reject",
        method="POST",
        json={},
    ):
        _set_request_user_id(7)
        payload = route("bill:11:transfer:12").get_json() or {}
        assert payload["success"] is True
        assert payload["data"] == {
            "candidateId": "bill:11:transfer:12",
            "action": "reject",
        }
        assert service.reject_candidate_calls[-1] == ("bill:11:transfer:12", {}, 7)

    service.reject_candidate_result = {
        "success": True,
        "candidate_id": "bill:11:learning:21:r1",
        "action": "reject",
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/bill:11:learning:21:r1/reject",
        method="POST",
        json={},
    ):
        _set_request_user_id(7)
        payload = route("bill:11:learning:21:r1").get_json() or {}
        assert payload["success"] is True
        assert payload["data"] == {
            "candidateId": "bill:11:learning:21:r1",
            "action": "reject",
        }
        assert service.reject_candidate_calls[-1] == ("bill:11:learning:21:r1", {}, 7)

    service.reject_candidate_result = {
        "success": False,
        "error": "Candidate family not supported",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:mystery/reject",
        method="POST",
        json={},
    ):
        _set_request_user_id(7)
        response, status = _unwrap_response(route("preview:1:mystery"))
        assert status == 400
        assert response.get_json()["error"] == "Candidate family not supported"


def test_matching_candidate_reject_route_rejects_invalid_request_and_preserves_errors(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic reject route 应覆盖非法请求、业务错误与异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.reject_matching_candidate)

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/reject",
        method="POST",
        json=["reject"],
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:transfer"))
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    service.reject_candidate_result = {
        "success": False,
        "error": "Candidate family not supported",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:mystery/reject",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:mystery"))
        assert status == 400
        assert response.get_json()["error"] == "Candidate family not supported"

    service.reject_candidate_result = {
        "success": False,
        "error": "Preview state changed, please refresh",
        "status_code": 409,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/reject",
        method="POST",
        json={"expectedState": {"sessionId": "session-1"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:transfer"))
        assert status == 409
        assert response.get_json()["error"] == "Preview state changed, please refresh"

    async def raise_reject_error(_candidate_id: str, _payload: dict[str, Any], user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("reject candidate boom")

    monkeypatch.setattr(service, "_reject_matching_candidate", raise_reject_error)
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:transfer/reject",
        method="POST",
        json={"expectedState": {"sessionId": "session-1"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:transfer"))
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"


def test_matching_candidate_clear_route_dispatches_preview_learning(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic clear route 应分发 preview learning candidate，并返回刷新后的 preview。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.clear_matching_candidate)
    service.clear_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:learning",
        "action": "clear",
        "preview_id": 1,
        "session_id": "session-1",
        "preview": [
            {
                "id": 1,
                "matching": {"learning": {"review_status": "pending", "suppressed": False}},
            }
        ],
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/clear",
        method="POST",
        json={
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "accepted",
                "previewType": "支出",
                "categoryId": None,
                "recurringId": None,
                "sourceAccountId": 12,
                "destinationAccountId": 18,
            }
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:learning").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:learning"
        assert payload["data"]["action"] == "clear"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert payload["data"]["preview"][0]["matching"]["learning"]["review_status"] == "pending"
        assert service.clear_candidate_calls == [
            (
                "preview:1:learning",
                {
                    "expectedState": {
                        "sessionId": "session-1",
                        "reviewStatus": "accepted",
                        "previewType": "支出",
                        "categoryId": None,
                        "recurringId": None,
                        "sourceAccountId": 12,
                        "destinationAccountId": 18,
                    }
                },
                9,
            )
        ]


def test_matching_candidate_clear_route_dispatches_preview_investment(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic clear route 应分发 preview investment candidate，并返回最小审查状态。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.clear_matching_candidate)
    service.clear_candidate_result = {
        "success": True,
        "candidate_id": "preview:1:investment",
        "action": "clear",
        "preview_id": 1,
        "session_id": "session-1",
        "review_status": "pending",
        "suppressed": False,
    }

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:investment/clear",
        method="POST",
        json={
            "expectedState": {
                "sessionId": "session-1",
                "reviewStatus": "accepted",
                "previewType": "投资",
                "categoryId": None,
                "recurringId": None,
            }
        },
    ):
        _set_request_user_id(9)
        payload = route("preview:1:investment").get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["candidateId"] == "preview:1:investment"
        assert payload["data"]["action"] == "clear"
        assert payload["data"]["previewId"] == 1
        assert payload["data"]["sessionId"] == "session-1"
        assert payload["data"]["reviewStatus"] == "pending"
        assert payload["data"]["suppressed"] is False
        assert "preview" not in payload["data"]
        assert service.clear_candidate_calls == [
            (
                "preview:1:investment",
                {
                    "expectedState": {
                        "sessionId": "session-1",
                        "reviewStatus": "accepted",
                        "previewType": "投资",
                        "categoryId": None,
                        "recurringId": None,
                    }
                },
                9,
            )
        ]


def test_matching_candidate_clear_route_rejects_invalid_request_and_unsupported_family(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic clear route 应覆盖非法请求、业务错误与异常分支。"""
    db = FakeMatchingDB()
    service = FakeMatchingService()
    loop = FakeLoop()
    _install_fake_loop(monkeypatch, loop)
    monkeypatch.setattr(matching_module, "get_app_context", lambda: (db, service))

    route = _unwrap_all(matching_module.clear_matching_candidate)

    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/clear",
        method="POST",
        json=["clear"],
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:learning"))
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    service.clear_candidate_result = {
        "success": False,
        "error": "Invalid request",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/clear",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:learning"))
        assert status == 400
        assert response.get_json()["error"] == "Invalid request"

    service.clear_candidate_result = {
        "success": False,
        "error": "Candidate family not supported",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:mystery/clear",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:mystery"))
        assert status == 400
        assert response.get_json()["error"] == "Candidate family not supported"

    service.clear_candidate_result = {
        "success": False,
        "error": "Preview state changed, please refresh",
        "status_code": 409,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/clear",
        method="POST",
        json={"expectedState": {"sessionId": "session-1"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:learning"))
        assert status == 409
        assert response.get_json()["error"] == "Preview state changed, please refresh"

    async def raise_clear_error(_candidate_id: str, _payload: dict[str, Any], user_id: int = 1) -> dict[str, Any]:
        _ = user_id
        raise RuntimeError("clear candidate boom")

    monkeypatch.setattr(service, "_clear_matching_candidate", raise_clear_error)
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:learning/clear",
        method="POST",
        json={"expectedState": {"sessionId": "session-1"}},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:learning"))
        assert status == 500
        assert response.get_json()["error"] == "Internal Server Error"
