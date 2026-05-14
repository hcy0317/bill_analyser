from __future__ import annotations

import asyncio
from datetime import datetime



def _run(coro):
    return asyncio.run(coro)



def _current_user_id(client, auth_headers, db_instance) -> int:
    profile_response = client.get("/api/profile", headers=auth_headers)
    assert profile_response.status_code == 200, profile_response.get_data(as_text=True)
    username = ((profile_response.get_json() or {}).get("result") or {}).get("username")
    user = _run(db_instance.get_user_by_username(username))
    assert user is not None
    return int(user["id"])


def _count_bills(db_instance, user_id: int) -> int:
    _, total = _run(db_instance.query_bills(page=1, page_size=1000, user_id=user_id))
    return int(total)



def _create_import_learning_rule(db_instance, user_id: int, match_value: str) -> int:
    async def _insert() -> int:
        conn = await db_instance._get_connection()  # pylint: disable=protected-access
        now = datetime.now().isoformat()
        normalized_value = db_instance._normalize_import_learning_text(match_value)  # pylint: disable=protected-access
        cursor = await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id,
                learned_source_account_id, learned_destination_account_id,
                enabled, source_session_id, source_preview_id,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?)
            """,
            (
                user_id,
                "description",
                match_value,
                normalized_value,
                "支出",
                None,
                None,
                None,
                "pytest-session",
                None,
                now,
                now,
            ),
        )
        await conn.commit()
        return int(cursor.lastrowid)

    return _run(_insert())



def test_get_bills_flask_sidecar_route_is_deleted(client, auth_headers) -> None:
    """账单列表 Flask route shell 已删除，CRUD 合同由 Rust runtime 覆盖。"""
    response = client.get(
        "/api/bills/",
        query_string={
            "page": 1,
            "page_size": 20,
            "accountIds": "1",
            "categoryIds": "1",
            "keyword": "域测试命中",
            "min_time": int(datetime(2026, 3, 1, 0, 0, 0).timestamp() * 1000),
            "max_time": int(datetime(2026, 3, 31, 23, 59, 59).timestamp() * 1000),
        },
        headers=auth_headers,
    )

    assert response.status_code in (404, 405), response.get_data(as_text=True)
    payload = response.get_json() or {}
    assert payload["success"] is False



def test_create_bill_flask_sidecar_route_is_deleted_without_persisting(client, auth_headers, db_instance) -> None:
    """账单创建 Flask route shell 已删除，sidecar 不应写入账单。"""
    user_id = _current_user_id(client, auth_headers, db_instance)
    bill_count_before = _count_bills(db_instance, user_id)

    response = client.post(
        "/api/bills/",
        json={
            "type": 3,
            "categoryId": "0",
            "time": int(datetime(2026, 3, 8, 9, 30, 0).timestamp() * 1000),
            "utcOffset": 480,
            "sourceAccountId": "not-an-int",
            "destinationAccountId": "0",
            "sourceAmount": 1280,
            "destinationAmount": 1280,
            "hideAmount": False,
            "tagIds": [],
            "comment": "非法账户ID不应持久化",
        },
        headers=auth_headers,
    )

    assert response.status_code in (404, 405)
    payload = response.get_json() or {}
    assert payload["success"] is False
    assert payload["error"] in {"Not Found", "Method Not Allowed"}
    assert _count_bills(db_instance, user_id) == bill_count_before



def test_import_learning_rule_routes_support_list_update_and_delete(client, auth_headers, db_instance) -> None:
    """长期学习规则路由应支持列出、禁用和删除。"""
    user_id = _current_user_id(client, auth_headers, db_instance)
    rule_id = _create_import_learning_rule(db_instance, user_id, "域测试学习规则")

    list_response = client.get(
        "/api/bills/import/learning-rules?page=1&pageSize=10",
        headers=auth_headers,
    )
    assert list_response.status_code == 200, list_response.get_data(as_text=True)
    assert list_response.headers["Cache-Control"] == "no-store, no-cache, must-revalidate, max-age=0"
    list_payload = list_response.get_json() or {}
    assert list_payload["success"] is True
    matched_rule = next(item for item in list_payload["result"] if item["id"] == rule_id)
    assert matched_rule["matchValue"] == "域测试学习规则"
    assert matched_rule["enabled"] is True

    update_response = client.put(
        f"/api/bills/import/learning-rules/{rule_id}",
        json={"enabled": False},
        headers=auth_headers,
    )
    assert update_response.status_code == 200, update_response.get_data(as_text=True)
    assert (update_response.get_json() or {}) == {"success": True, "result": True}

    enabled_rules = _run(db_instance.get_import_learning_rules(user_id=user_id, enabled_only=True, limit=20))
    assert all(int(rule["id"]) != rule_id for rule in enabled_rules)

    delete_response = client.delete(
        f"/api/bills/import/learning-rules/{rule_id}",
        headers=auth_headers,
    )
    assert delete_response.status_code == 200, delete_response.get_data(as_text=True)
    assert (delete_response.get_json() or {}) == {"success": True, "result": True}

    missing_delete_response = client.delete(
        f"/api/bills/import/learning-rules/{rule_id}",
        headers=auth_headers,
    )
    assert missing_delete_response.status_code == 404
    missing_payload = missing_delete_response.get_json() or {}
    assert missing_payload["success"] is False
    assert missing_payload["error"] == "Rule not found"
