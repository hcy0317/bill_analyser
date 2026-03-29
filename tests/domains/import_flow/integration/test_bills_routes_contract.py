from __future__ import annotations

import asyncio
from datetime import datetime
from typing import Any



def _run(coro):
    return asyncio.run(coro)



def _current_user_id(client, auth_headers, db_instance) -> int:
    profile_response = client.get("/api/profile", headers=auth_headers)
    assert profile_response.status_code == 200, profile_response.get_data(as_text=True)
    username = ((profile_response.get_json() or {}).get("result") or {}).get("username")
    user = _run(db_instance.get_user_by_username(username))
    assert user is not None
    return int(user["id"])



def _create_account(db_instance, user_id: int, name: str) -> int:
    return _run(
        db_instance.create_account(
            {
                "name": name,
                "type": 1,
                "category": "asset",
                "currency": "CNY",
                "icon": "",
                "color": "",
                "balance": 0.0,
                "initial_balance": 0.0,
                "hidden": False,
                "display_order": 0,
                "comment": "",
                "aliases": [],
            },
            user_id=user_id,
        )
    )



def _create_category(db_instance, user_id: int, *, main_category: str, sub_category: str = "") -> int:
    return _run(
        db_instance.create_category(
            {
                "type": 3,
                "main_category": main_category,
                "sub_category": sub_category,
                "description": "",
                "priority": 0,
                "keywords": "",
                "hidden": False,
                "icon": "",
                "color": "",
            },
            user_id=user_id,
        )
    )



def _create_bill(db_instance, user_id: int, **overrides: Any) -> int:
    payload = {
        "date": "2026-03-05 12:00:00",
        "type": "支出",
        "amount": -28.5,
        "counterparty": "域测试商户",
        "description": "域测试描述",
        "payment_method": "支付宝",
        "main_category": "域测试餐饮",
        "sub_category": "午餐",
        "source_account_id": 0,
        "destination_account_id": 0,
        "destination_amount": 0.0,
    }
    payload.update(overrides)
    bill_id = _run(db_instance.create_bill(payload, user_id=user_id))
    assert bill_id is not None
    return int(bill_id)



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



def test_get_bills_combines_account_category_keyword_and_time_filters(client, auth_headers, db_instance) -> None:
    """账单列表应联合应用账户、分类、关键词与时间筛选。"""
    user_id = _current_user_id(client, auth_headers, db_instance)
    matched_account_id = _create_account(db_instance, user_id, "域测试账户-命中")
    other_account_id = _create_account(db_instance, user_id, "域测试账户-排除")
    matched_category_id = _create_category(
        db_instance,
        user_id,
        main_category="域测试餐饮",
        sub_category="午餐",
    )
    _create_category(db_instance, user_id, main_category="域测试交通", sub_category="地铁")

    _create_bill(
        db_instance,
        user_id,
        date="2026-03-05 12:00:00",
        description="域测试命中账单",
        main_category="域测试餐饮",
        sub_category="午餐",
        source_account_id=matched_account_id,
    )
    _create_bill(
        db_instance,
        user_id,
        date="2026-03-06 12:00:00",
        description="域测试错误分类",
        main_category="域测试交通",
        sub_category="地铁",
        source_account_id=matched_account_id,
    )
    _create_bill(
        db_instance,
        user_id,
        date="2026-03-07 12:00:00",
        description="域测试错误账户",
        main_category="域测试餐饮",
        sub_category="午餐",
        source_account_id=other_account_id,
    )

    response = client.get(
        "/api/bills/",
        query_string={
            "page": 1,
            "page_size": 20,
            "accountIds": str(matched_account_id),
            "categoryIds": str(matched_category_id),
            "keyword": "域测试命中",
            "min_time": int(datetime(2026, 3, 1, 0, 0, 0).timestamp() * 1000),
            "max_time": int(datetime(2026, 3, 31, 23, 59, 59).timestamp() * 1000),
        },
        headers=auth_headers,
    )

    assert response.status_code == 200, response.get_data(as_text=True)
    payload = response.get_json() or {}
    assert payload["success"] is True
    assert payload["result"]["totalCount"] == 1
    assert len(payload["result"]["items"]) == 1
    assert payload["result"]["items"][0]["comment"] == "域测试命中账单"



def test_create_bill_invalid_source_account_id_returns_400_without_persisting(client, auth_headers, db_instance) -> None:
    """单条创建接口遇到非法账户 ID 时应返回 400，且不写入账单。"""
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

    assert response.status_code == 400
    payload = response.get_json() or {}
    assert payload["success"] is False
    assert "invalid literal for int()" in payload["error"]
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
