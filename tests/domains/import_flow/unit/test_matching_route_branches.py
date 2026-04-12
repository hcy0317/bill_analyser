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

    def __init__(self) -> None:
        self.import_session_result: dict[str, Any] | None = {"session_id": "session-1", "user_id": 9}

    async def get_import_session(self, session_id: str, user_id: int = 1) -> dict[str, Any] | None:
        _ = (session_id, user_id)
        return dict(self.import_session_result) if self.import_session_result else None


class FakeMatchingService:
    """Async service stub for matching route branch tests."""

    def __init__(self) -> None:
        self.calls: list[tuple[str, int]] = []
        self.bill_candidate_calls: list[tuple[int, int]] = []
        self.matching_pairs_calls: list[int] = []
        self.accept_candidate_calls: list[tuple[str, dict[str, Any], int]] = []
        self.reject_candidate_calls: list[tuple[str, dict[str, Any], int]] = []
        self.manual_pair_calls: list[tuple[int, int, int]] = []
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

    async def create_manual_transfer_pair(
        self,
        bill_id: int,
        candidate_bill_id: int,
        user_id: int = 1,
    ) -> dict[str, Any]:
        self.manual_pair_calls.append((bill_id, candidate_bill_id, user_id))
        return dict(self.manual_pair_result)

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
        json={"billId": 11, "candidateBillId": 12},
    ):
        _set_request_user_id(7)
        payload = create_manual_pair_route().get_json() or {}
        assert payload["success"] is True
        assert payload["data"]["pair"]["leftBillId"] == 11
        assert payload["data"]["pair"]["rightBillId"] == 12
        assert service.manual_pair_calls == [(11, 12, 7)]

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
        "error": "Candidate family not supported",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:recurring/accept",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:recurring"))
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


def test_matching_candidate_reject_route_dispatches_preview_transfer_and_preserves_boundary(
    matching_route_app: Flask,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """generic reject route 应分发 preview transfer / recurring / historical transfer，并保留其他 family 边界。"""
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
        "candidate_id": "preview:1:recurring",
        "action": "reject",
        "preview_id": 1,
        "session_id": "session-1",
        "preview": [{"id": 1, "matching": {"recurring": {"id": None, "candidate_count": 2}}}],
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/preview:1:recurring/reject",
        method="POST",
        json={"expectedState": {"sessionId": "session-1", "previewType": "支出", "categoryId": 10, "recurringId": 9}},
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
            {"expectedState": {"sessionId": "session-1", "previewType": "支出", "categoryId": 10, "recurringId": 9}},
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
        "success": False,
        "error": "Candidate family not supported",
        "status_code": 400,
    }
    with matching_route_app.test_request_context(
        "/api/matching/candidates/bill:11:investment:12/reject",
        method="POST",
        json={},
    ):
        _set_request_user_id(7)
        response, status = _unwrap_response(route("bill:11:investment:12"))
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
        "/api/matching/candidates/preview:1:investment/reject",
        method="POST",
        json={},
    ):
        _set_request_user_id(9)
        response, status = _unwrap_response(route("preview:1:investment"))
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
