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

    async def get_matching_session_candidates(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        self.calls.append((session_id, user_id))
        return dict(self.result)


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
