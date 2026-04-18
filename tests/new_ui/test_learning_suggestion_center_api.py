"""独立学习建议中心 REST API 契约测试。"""

from __future__ import annotations

import asyncio
import time

from tests.new_ui.test_bills_api import (
    _build_isolated_auth_headers,
    _get_current_user_id,
)


def _create_session_and_preview(user_id: int, *, suffix: str = "") -> str:
    """创建导入会话 + preview + annotation，返回 session_id。"""
    from src.api.app import db

    session_id = f"pytest-learn-center-{suffix}-{int(time.time() * 1000)}"

    async def _setup() -> None:
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        conn = await db._get_connection()
        now = "2025-06-01T00:00:00Z"
        await conn.execute(
            """
            INSERT INTO bills_preview (
                session_id, user_id,
                preview_date, preview_amount, preview_description,
                preview_counterparty, preview_payment_method, preview_parser_id,
                preview_type, preview_main_category, preview_sub_category,
                created_at
            ) VALUES (?, ?, '2025-01-15', 100, ?, ?, ?, ?, ?, '餐饮', '', ?)
            """,
            (session_id, user_id, f"描述{suffix}", f"商户{suffix}", "微信支付", "wechat", "支出", now),
        )
        async with conn.execute("SELECT last_insert_rowid() AS id") as cur:
            row = await cur.fetchone()
            preview_id = int(row["id"] if isinstance(row, dict) else row[0])
        await db.save_import_annotation_samples(
            session_id,
            [{"preview_id": preview_id, "annotated_type": "支出", "annotated_category_id": 1}],
            user_id=user_id,
        )

    asyncio.run(_setup())
    return session_id


def test_learning_suggestions_generate_and_list(client) -> None:
    """POST /api/learning/suggestions/generate + GET /api/learning/suggestions"""
    headers = _build_isolated_auth_headers(client, "learn_center")
    user_id = _get_current_user_id(client, headers)
    _create_session_and_preview(user_id, suffix="gen")

    resp = client.post("/api/learning/suggestions/generate", headers=headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert body["success"] is True
    assert body["data"]["total_annotations"] >= 1

    resp = client.get("/api/learning/suggestions?status=pending", headers=headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert body["success"] is True
    assert "items" in body["data"]
    assert "total" in body["data"]


def test_learning_suggestions_accept_creates_rule(client) -> None:
    """POST /api/learning/suggestions/<id>/accept 应将建议提升为规则。"""
    headers = _build_isolated_auth_headers(client, "learn_center_accept")
    user_id = _get_current_user_id(client, headers)
    _create_session_and_preview(user_id, suffix="accept")

    client.post("/api/learning/suggestions/generate", headers=headers)

    resp = client.get("/api/learning/suggestions?status=pending", headers=headers)
    items = resp.get_json()["data"]["items"]
    if not items:
        return

    suggestion_id = items[0]["id"]
    resp = client.post(f"/api/learning/suggestions/{suggestion_id}/accept", headers=headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert body["success"] is True
    assert body["data"]["status"] == "accepted"
    assert body["data"]["rule_id"] is not None


def test_learning_suggestions_reject(client) -> None:
    """POST /api/learning/suggestions/<id>/reject 应标记为 rejected。"""
    headers = _build_isolated_auth_headers(client, "learn_center_reject")
    user_id = _get_current_user_id(client, headers)
    _create_session_and_preview(user_id, suffix="reject")

    client.post("/api/learning/suggestions/generate", headers=headers)

    resp = client.get("/api/learning/suggestions?status=pending", headers=headers)
    items = resp.get_json()["data"]["items"]
    if not items:
        return

    suggestion_id = items[0]["id"]
    resp = client.post(f"/api/learning/suggestions/{suggestion_id}/reject", headers=headers)
    assert resp.status_code == 200
    assert resp.get_json()["success"] is True


def test_learning_suggestions_not_found_returns_404(client) -> None:
    """不存在的建议 ID 应返回 404。"""
    headers = _build_isolated_auth_headers(client, "learn_center_404")
    resp = client.post("/api/learning/suggestions/999999/accept", headers=headers)
    assert resp.status_code == 404


def test_learning_rules_list(client) -> None:
    """GET /api/learning/rules 应返回规则列表。"""
    headers = _build_isolated_auth_headers(client, "learn_center_rules")
    resp = client.get("/api/learning/rules", headers=headers)
    assert resp.status_code == 200
    body = resp.get_json()
    assert body["success"] is True
    assert "items" in body["data"]
    assert "total" in body["data"]
