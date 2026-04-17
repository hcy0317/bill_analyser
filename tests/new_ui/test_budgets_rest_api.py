"""预算 REST 收口回归测试。"""

# pylint: disable=too-many-lines

import asyncio
import time
from datetime import date, timedelta

import pytest

from tests.user_cleanup_support import register_test_user_for_cleanup

# pylint: disable=line-too-long,too-many-locals,too-many-arguments
# pylint: disable=import-outside-toplevel,import-error,no-name-in-module


@pytest.fixture(name="auth_headers")
def _auth_headers_fixture(client):
    """获取认证请求头。"""
    username = f"test_budgets_{int(time.time() * 1000)}"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]
    register_response = client.post("/api/auth/register", json={
        "username": username,
        "email": f"{username}@example.com",
        "password": "Test123456!",
        "nickname": username
    })
    assert register_response.status_code in [200, 201], (
        f"注册失败: {register_response.status_code}, {register_response.get_data(as_text=True)}"
    )
    register_test_user_for_cleanup(db, username)

    login_response = client.post("/api/auth/login", json={
        "loginName": username,
        "password": "Test123456!"
    })

    assert login_response.status_code == 200, (
        f"登录失败: {login_response.status_code}, {login_response.get_data(as_text=True)}"
    )
    data = login_response.get_json() or {}
    token = (data.get("result") or {}).get("token")
    assert token, f"登录响应缺少token: {data}"
    return {"Authorization": f"Bearer {token}"}


def _get_current_user_id(client, auth_headers):
    """获取当前测试用户 ID。"""
    profile_response = client.get("/api/profile", headers=auth_headers)
    assert profile_response.status_code == 200
    username = profile_response.get_json()["result"]["username"]

    from bill_analyser.api.app import db

    async def _find_user_id():
        user = await db.get_user_by_username(username)
        assert user is not None
        return int(user["id"])

    return asyncio.run(_find_user_id())


def _create_budget_support_category(user_id, main_category, sub_category):
    """为预算测试创建主/子分类。"""
    from bill_analyser.api.app import db

    async def _create():
        parent_category_id = await db.create_category({
            "type": 3,
            "main_category": main_category,
            "sub_category": "",
            "description": main_category,
            "priority": 0,
            "keywords": "",
            "hidden": False,
            "icon": "folder",
            "color": "#ffaa00",
        }, user_id=user_id)
        assert parent_category_id is not None
        if not sub_category:
            return int(parent_category_id)

        sub_category_id = await db.create_category({
            "type": 3,
            "main_category": main_category,
            "sub_category": sub_category,
            "description": sub_category,
            "priority": 0,
            "keywords": "",
            "hidden": False,
            "icon": "tag",
            "color": "#ffaa00",
        }, user_id=user_id)
        assert sub_category_id is not None
        return int(sub_category_id)

    return asyncio.run(_create())


def _create_budget_support_sub_category(user_id, main_category, sub_category):
    """为预算测试创建额外子分类，避免重复创建父分类。"""
    from bill_analyser.api.app import db

    async def _create():
        sub_category_id = await db.create_category({
            "type": 3,
            "main_category": main_category,
            "sub_category": sub_category,
            "description": sub_category,
            "priority": 0,
            "keywords": "",
            "hidden": False,
            "icon": "tag",
            "color": "#ffaa00",
        }, user_id=user_id)
        assert sub_category_id is not None
        return int(sub_category_id)

    return asyncio.run(_create())


def _create_budget_support_tag(user_id, name):
    """为预算测试创建标签。"""
    from bill_analyser.api.app import db

    async def _create():
        return int(await db.create_tag({
            "name": name,
            "color": "#44aa44",
            "icon": "tag",
            "hidden": False,
        }, user_id=user_id))

    return asyncio.run(_create())


def _insert_budget_support_bill(user_id, *, main_category, sub_category, amount, description, date_text, tag_ids):
    """直接写入预算测试账单并绑定标签。"""
    from bill_analyser.api.app import db

    async def _create():
        bill_id = await db.create_bill(
            {
                "date": date_text,
                "type": "支出",
                "amount": amount,
                "counterparty": "pytest预算商户",
                "description": description,
                "payment_method": "pytest预算账户",
                "main_category": main_category,
                "sub_category": sub_category,
                "source_account_id": 0,
                "destination_account_id": 0,
                "destination_amount": 0.0,
            },
            user_id=user_id,
        )
        assert bill_id is not None
        if tag_ids:
            assert await db.add_tags_to_bill(int(bill_id), tag_ids, user_id=user_id) is True
        return int(bill_id)

    return asyncio.run(_create())


def test_budget_rest_crud_and_analysis(client, auth_headers):
    """预算主链应全部走 REST 接口。"""
    unique_name = f"REST预算-{int(time.time())}"

    create_response = client.post("/api/budgets/", json={
        "name": unique_name,
        "category": "REST测试分类",
        "sub_category": "REST测试子分类",
        "period_type": "monthly",
        "amount": 123.45,
        "start_date": "2026-01-01",
        "end_date": "2026-12-31",
        "alert_threshold": 75,
        "enabled": True
    }, headers=auth_headers)
    assert create_response.status_code == 201
    create_data = create_response.get_json()
    assert create_data["success"] is True
    budget_id = create_data["result"]["id"]

    detail_response = client.get(f"/api/budgets/{budget_id}", headers=auth_headers)
    assert detail_response.status_code == 200
    detail_data = detail_response.get_json()
    assert detail_data["success"] is True
    assert detail_data["result"]["name"] == unique_name

    update_response = client.put(f"/api/budgets/{budget_id}", json={
        "name": unique_name + "-更新",
        "category": "REST测试分类",
        "sub_category": "REST测试子分类",
        "period_type": "monthly",
        "amount": 222.22,
        "start_date": "2026-01-01",
        "end_date": "2026-12-31",
        "alert_threshold": 80,
        "enabled": True
    }, headers=auth_headers)
    assert update_response.status_code == 200
    update_data = update_response.get_json()
    assert update_data["success"] is True

    execution_response = client.get("/api/budgets/execution?budget_type=3&period_type=monthly", headers=auth_headers)
    assert execution_response.status_code == 200
    execution_data = execution_response.get_json()
    assert execution_data["success"] is True
    assert "items" in execution_data["result"]
    assert "summary" in execution_data["result"]

    forecast_response = client.get("/api/budgets/forecast?budget_type=3&period_type=monthly", headers=auth_headers)
    assert forecast_response.status_code == 200
    forecast_data = forecast_response.get_json()
    assert forecast_data["success"] is True
    assert "items" in forecast_data["result"]

    export_response = client.get("/api/budgets/export", headers=auth_headers)
    assert export_response.status_code == 200
    export_data = export_response.get_json()
    assert export_data["success"] is True
    assert isinstance(export_data["result"], list)

    import_name = f"REST导入预算-{int(time.time())}"
    import_response = client.post("/api/budgets/import", json=[{
        "name": import_name,
        "category": "REST导入分类",
        "sub_category": "",
        "period_type": "monthly",
        "amount": 88.88,
        "start_date": "2026-02-01",
        "end_date": "2026-02-28",
        "alert_threshold": 70,
        "enabled": True
    }], headers=auth_headers)
    assert import_response.status_code == 200
    import_data = import_response.get_json()
    assert import_data["success"] is True
    assert import_data["result"]["created"] >= 1

    delete_response = client.delete(f"/api/budgets/{budget_id}", headers=auth_headers)
    assert delete_response.status_code == 200
    delete_data = delete_response.get_json()
    assert delete_data["success"] is True


def test_legacy_budget_v1_routes_removed(client, auth_headers):
    """预算旧 v1 兼容端点应已移除。"""
    legacy_paths = [
        ("GET", "/api/v1/budgets/list.json"),
        ("GET", "/api/v1/budgets/execution.json"),
        ("GET", "/api/v1/budgets/forecast.json"),
        ("GET", "/api/v1/budgets/export.json"),
        ("POST", "/api/v1/budgets/add.json"),
        ("POST", "/api/v1/budgets/modify.json"),
        ("POST", "/api/v1/budgets/delete.json"),
        ("POST", "/api/v1/budgets/import.json")
    ]

    for method, path in legacy_paths:
        if method == "GET":
            response = client.get(path, headers=auth_headers)
        else:
            response = client.post(path, json={}, headers=auth_headers)
        assert response.status_code == 404, path


def test_budget_history_snapshot_and_query(client, auth_headers):
    """预算历史快照应可通过 REST 端点创建并查询。"""
    unique_name = f"REST预算快照-{int(time.time())}"

    create_response = client.post("/api/budgets/", json={
        "name": unique_name,
        "category": "REST快照分类",
        "sub_category": "",
        "period_type": "monthly",
        "amount": 345.67,
        "start_date": "2026-01-01",
        "end_date": "2026-12-31",
        "alert_threshold": 80,
        "enabled": True
    }, headers=auth_headers)
    assert create_response.status_code == 201
    budget_id = create_response.get_json()["result"]["id"]

    snapshot_response = client.post("/api/budgets/history/snapshot", json={
        "budget_type": 3,
        "period_type": "monthly",
        "year": 2026,
        "month": 1,
        "budget_id": budget_id
    }, headers=auth_headers)
    assert snapshot_response.status_code == 200
    snapshot_data = snapshot_response.get_json()
    assert snapshot_data["success"] is True
    assert snapshot_data["result"]["created_count"] >= 1
    assert snapshot_data["result"]["period_start"] == "2026-01-01"
    assert snapshot_data["result"]["period_end"] == "2026-01-31"

    history_response = client.get(
        f"/api/budgets/history?budget_type=3&period_type=monthly&year=2026&month=1&budget_id={budget_id}",
        headers=auth_headers
    )
    assert history_response.status_code == 200
    history_data = history_response.get_json()
    assert history_data["success"] is True
    assert history_data["result"]["summary"]["count"] >= 1
    assert history_data["result"]["summary"]["period_start"] == "2026-01-01"
    assert history_data["result"]["summary"]["period_end"] == "2026-01-31"
    assert any(int(item["budget_id"]) == int(budget_id) for item in history_data["result"]["items"])

    delete_response = client.delete(f"/api/budgets/{budget_id}", headers=auth_headers)
    assert delete_response.status_code == 200
    assert delete_response.get_json()["success"] is True


def test_budget_forecast_strategy_params(client, auth_headers):
    """预算预测应支持策略参数与历史周期参数。"""
    response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=monthly&forecast_strategy=moving_average&months_history=3",
        headers=auth_headers
    )
    assert response.status_code == 200
    data = response.get_json()
    assert data["success"] is True
    assert data["result"]["summary"]["forecast_strategy"] == "moving_average"
    assert data["result"]["summary"]["history_periods"] == 3
    assert data["result"]["period_start"]
    assert data["result"]["period_end"]
    assert "daysElapsed" in data["result"]
    assert "daysRemaining" in data["result"]
    assert "avg_backtest_mape" in data["result"]["summary"]

    forecast_items = data["result"]["items"]
    if forecast_items:
        item = forecast_items[0]
        assert "backtest_mape" in item
        assert "confidence" in item
        assert item["confidence"] in ["high", "medium", "low"]


def test_budget_primary_secondary_rules(client, auth_headers):
    """一级/二级预算应遵循自动补父预算、总额下限与级联删除规则。"""
    category_name = f"层级预算分类-{int(time.time())}"
    period_payload = {
        "category": category_name,
        "period_type": "monthly",
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": True
    }

    create_secondary_a = client.post("/api/budgets/", json={
        "name": "二级预算-A",
        **period_payload,
        "sub_category": "子分类A",
        "amount": 100.0
    }, headers=auth_headers)
    assert create_secondary_a.status_code == 201

    first_list = client.get("/api/budgets/", query_string={"category": category_name}, headers=auth_headers)
    assert first_list.status_code == 200
    first_items = first_list.get_json()["result"]
    assert len(first_items) == 2

    primary_budget = next(item for item in first_items if not item.get("sub_category"))
    assert primary_budget["amount"] == 100.0

    update_primary = client.put(f"/api/budgets/{primary_budget['id']}", json={
        "name": primary_budget.get("name", ""),
        **period_payload,
        "sub_category": "",
        "amount": 180.0
    }, headers=auth_headers)
    assert update_primary.status_code == 200

    create_secondary_b = client.post("/api/budgets/", json={
        "name": "二级预算-B",
        **period_payload,
        "sub_category": "子分类B",
        "amount": 30.0
    }, headers=auth_headers)
    assert create_secondary_b.status_code == 201

    second_list = client.get("/api/budgets/", query_string={"category": category_name}, headers=auth_headers)
    second_items = second_list.get_json()["result"]
    updated_primary = next(item for item in second_items if int(item["id"]) == int(primary_budget["id"]))
    assert updated_primary["amount"] == 180.0

    create_secondary_c = client.post("/api/budgets/", json={
        "name": "二级预算-C",
        **period_payload,
        "sub_category": "子分类C",
        "amount": 70.0
    }, headers=auth_headers)
    assert create_secondary_c.status_code == 201

    third_list = client.get("/api/budgets/", query_string={"category": category_name}, headers=auth_headers)
    third_items = third_list.get_json()["result"]
    synced_primary = next(item for item in third_items if int(item["id"]) == int(primary_budget["id"]))
    assert synced_primary["amount"] == 200.0

    delete_primary = client.delete(f"/api/budgets/{primary_budget['id']}", headers=auth_headers)
    assert delete_primary.status_code == 200
    assert delete_primary.get_json()["success"] is True

    final_list = client.get("/api/budgets/", query_string={"category": category_name}, headers=auth_headers)
    final_items = final_list.get_json()["result"]
    assert final_items == []

def test_budget_execution_summary_avoids_double_counting_synced_primary_budget(client, auth_headers):
    """预算执行 summary 不应把自动同步的一级预算与子预算重复累计。"""
    category_name = f"REST执行汇总分类-{int(time.time())}"
    user_id = _get_current_user_id(client, auth_headers)
    parent_category_id = _create_budget_support_category(user_id, category_name, "")
    _create_budget_support_sub_category(user_id, category_name, "子分类A")

    create_secondary = client.post(
        "/api/budgets/",
        json={
            "name": "执行汇总子预算",
            "category": category_name,
            "sub_category": "子分类A",
            "period_type": "monthly",
            "amount": 100.0,
            "start_date": "2026-03-01",
            "end_date": "2026-03-31",
            "alert_threshold": 80,
            "enabled": True,
        },
        headers=auth_headers,
    )
    assert create_secondary.status_code == 201

    _insert_budget_support_bill(
        user_id,
        main_category=category_name,
        sub_category="子分类A",
        amount=-60.0,
        description="执行汇总账单",
        date_text="2026-03-12 12:00:00",
        tag_ids=[],
    )

    execution_response = client.get(
        (
            "/api/budgets/execution?budget_type=3&period_type=monthly"
            "&start_date=2026-03-01&end_date=2026-03-31"
            f"&category_id={parent_category_id}"
        ),
        headers=auth_headers,
    )
    assert execution_response.status_code == 200
    execution_data = execution_response.get_json()
    assert execution_data["success"] is True
    assert len(execution_data["result"]["items"]) == 2
    assert execution_data["result"]["summary"] == {
        "total_budget": 100.0,
        "total_spent": 60.0,
        "total_remaining": 40.0,
        "overall_execution_rate": 60.0,
        "count": 1,
    }

def test_budget_route_validation_and_not_found_branches(client, auth_headers):  # pylint: disable=too-many-statements
    """预算路由应覆盖空请求、缺字段、404 与非法导入格式分支。"""
    no_data_response = client.post(
        "/api/budgets/",
        data="null",
        content_type="application/json",
        headers=auth_headers,
    )
    assert no_data_response.status_code == 400
    assert no_data_response.get_json()["error"] == "No data provided"

    missing_period_type_response = client.post("/api/budgets/", json={
        "name": "缺字段预算",
        "category": "缺字段分类",
        "amount": 10.0,
        "start_date": "2026-01-01"
    }, headers=auth_headers)
    assert missing_period_type_response.status_code == 400
    assert missing_period_type_response.get_json()["error"] == "Missing required field: period_type"

    missing_category_response = client.post("/api/budgets/", json={
        "name": "缺分类预算",
        "period_type": "monthly",
        "amount": 10.0,
        "start_date": "2026-01-01"
    }, headers=auth_headers)
    assert missing_category_response.status_code == 400
    assert missing_category_response.get_json()["error"] == "Missing required field: category"

    get_missing_response = client.get("/api/budgets/999999", headers=auth_headers)
    assert get_missing_response.status_code == 404
    assert get_missing_response.get_json()["error"] == "Budget not found"

    update_missing_response = client.put("/api/budgets/999999", json={
        "name": "不存在预算",
        "amount": 99.0,
        "period_type": "monthly",
        "start_date": "2026-01-01"
    }, headers=auth_headers)
    assert update_missing_response.status_code == 404
    assert update_missing_response.get_json()["error"] == "Budget not found"

    delete_missing_response = client.delete("/api/budgets/999999", headers=auth_headers)
    assert delete_missing_response.status_code == 404
    assert delete_missing_response.get_json()["error"] == "Budget not found"

    invalid_import_response = client.post("/api/budgets/import", json={
        "name": "不是数组"
    }, headers=auth_headers)
    assert invalid_import_response.status_code == 400
    assert invalid_import_response.get_json()["error"] == "Invalid data format. Expected array of budgets."

    invalid_execution_budget_id = client.get(
        "/api/budgets/execution?budget_id=abc",
        headers=auth_headers,
    )
    assert invalid_execution_budget_id.status_code == 400
    assert "budget_id" in invalid_execution_budget_id.get_json()["error"]

    invalid_history_tag_ids = client.get(
        "/api/budgets/history?tag_ids=1,foo",
        headers=auth_headers,
    )
    assert invalid_history_tag_ids.status_code == 400
    assert "tag_ids" in invalid_history_tag_ids.get_json()["error"]

    invalid_period_type_response = client.get(
        "/api/budgets/execution?period_type=decade",
        headers=auth_headers,
    )
    assert invalid_period_type_response.status_code == 400
    assert "period_type" in invalid_period_type_response.get_json()["error"]

    invalid_month_response = client.get(
        "/api/budgets/execution?period_type=monthly&month=13",
        headers=auth_headers,
    )
    assert invalid_month_response.status_code == 400
    assert "month" in invalid_month_response.get_json()["error"]

    invalid_quarter_response = client.get(
        "/api/budgets/history?period_type=quarterly&quarter=5",
        headers=auth_headers,
    )
    assert invalid_quarter_response.status_code == 400
    assert "quarter" in invalid_quarter_response.get_json()["error"]

    invalid_months_history_response = client.get(
        "/api/budgets/forecast?period_type=monthly&months_history=0",
        headers=auth_headers,
    )
    assert invalid_months_history_response.status_code == 400
    assert "months_history" in invalid_months_history_response.get_json()["error"]

    invalid_create_period_type_response = client.post("/api/budgets/", json={
        "name": "非法周期预算",
        "category": "非法周期分类",
        "period_type": "decade",
        "amount": 10.0,
        "start_date": "2026-01-01"
    }, headers=auth_headers)
    assert invalid_create_period_type_response.status_code == 400
    assert "period_type" in invalid_create_period_type_response.get_json()["error"]

    invalid_create_shape_response = client.post(
        "/api/budgets/",
        json=[1],
        headers=auth_headers,
    )
    assert invalid_create_shape_response.status_code == 400
    assert "Expected object" in invalid_create_shape_response.get_json()["error"]

    invalid_date_range_response = client.get(
        "/api/budgets/execution?start_date=2026-05-10&end_date=2026-05-01&period_type=monthly",
        headers=auth_headers,
    )
    assert invalid_date_range_response.status_code == 400
    assert "start_date" in invalid_date_range_response.get_json()["error"]

    valid_budget_for_update = client.post("/api/budgets/", json={
        "name": "待更新预算",
        "category": "待更新分类",
        "period_type": "monthly",
        "amount": 10.0,
        "start_date": "2026-01-01"
    }, headers=auth_headers)
    assert valid_budget_for_update.status_code == 201
    valid_budget_id = valid_budget_for_update.get_json()["result"]["id"]

    invalid_update_period_type_response = client.put(f"/api/budgets/{valid_budget_id}", json={
        "period_type": "decade",
    }, headers=auth_headers)
    assert invalid_update_period_type_response.status_code == 400
    assert "period_type" in invalid_update_period_type_response.get_json()["error"]

    invalid_update_shape_response = client.put(
        f"/api/budgets/{valid_budget_id}",
        json=[1],
        headers=auth_headers,
    )
    assert invalid_update_shape_response.status_code == 400
    assert "Expected object" in invalid_update_shape_response.get_json()["error"]

    invalid_import_period_type_response = client.post("/api/budgets/import", json=[{
        "name": "非法导入预算",
        "category": "非法导入分类",
        "period_type": "decade",
        "amount": 10.0,
        "start_date": "2026-01-01"
    }], headers=auth_headers)
    assert invalid_import_period_type_response.status_code == 400
    assert "period_type" in invalid_import_period_type_response.get_json()["error"]

    invalid_snapshot_body_response = client.post(
        "/api/budgets/history/snapshot",
        json=[],
        headers=auth_headers,
    )
    assert invalid_snapshot_body_response.status_code == 400
    assert "Expected object" in invalid_snapshot_body_response.get_json()["error"]

    invalid_snapshot_tag_ids_response = client.post(
        "/api/budgets/history/snapshot",
        json={"tag_ids": "12"},
        headers=auth_headers,
    )
    assert invalid_snapshot_tag_ids_response.status_code == 400
    assert "tag_ids" in invalid_snapshot_tag_ids_response.get_json()["error"]

    invalid_snapshot_null_response = client.post(
        "/api/budgets/history/snapshot",
        data="null",
        content_type="application/json",
        headers=auth_headers,
    )
    assert invalid_snapshot_null_response.status_code == 400
    assert "Expected object" in invalid_snapshot_null_response.get_json()["error"]

    invalid_snapshot_empty_body_response = client.post(
        "/api/budgets/history/snapshot",
        data="",
        content_type="application/json",
        headers=auth_headers,
    )
    assert invalid_snapshot_empty_body_response.status_code == 400
    assert "Expected object" in invalid_snapshot_empty_body_response.get_json()["error"]

    invalid_snapshot_malformed_body_response = client.post(
        "/api/budgets/history/snapshot",
        data='{"budget_type":',
        content_type="application/json",
        headers=auth_headers,
    )
    assert invalid_snapshot_malformed_body_response.status_code == 400
    assert "Expected object" in invalid_snapshot_malformed_body_response.get_json()["error"]

    missing_category_response = client.get(
        "/api/budgets/execution?budget_type=3&period_type=monthly&category_id=999999",
        headers=auth_headers,
    )
    assert missing_category_response.status_code == 200
    assert missing_category_response.get_json()["result"]["items"] == []


@pytest.mark.parametrize(
    ("method", "path", "payload"),
    [
        ("GET", "/api/budgets/execution?period_type=", None),
        ("GET", "/api/budgets/forecast?period_type=", None),
        ("GET", "/api/budgets/history?period_type=", None),
        ("POST", "/api/budgets/history/snapshot", {"period_type": ""}),
    ],
)
def test_budget_routes_reject_blank_period_type(client, auth_headers, method, path, payload):
    """空字符串 period_type 不应被静默归一为 monthly。"""
    if method == "GET":
        response = client.get(path, headers=auth_headers)
    else:
        response = client.post(path, json=payload, headers=auth_headers)

    assert response.status_code == 400
    assert "period_type" in response.get_json()["error"]


def test_budget_route_period_resolution_and_forecast_day_branches(client, auth_headers):
    """预算路由应覆盖显式区间、季度/年度解析以及 forecast 的过去/未来天数分支。"""
    explicit_execution_response = client.get(
        "/api/budgets/execution?budget_type=3&period_type=monthly&start_date=2026-05-10&end_date=2026-05-20",
        headers=auth_headers,
    )
    assert explicit_execution_response.status_code == 200
    explicit_execution_data = explicit_execution_response.get_json()
    assert explicit_execution_data["success"] is True
    assert explicit_execution_data["result"]["period_start"] == "2026-05-10"
    assert explicit_execution_data["result"]["period_end"] == "2026-05-20"

    quarterly_history_response = client.get(
        "/api/budgets/history?budget_type=3&period_type=quarterly&year=2026&quarter=2",
        headers=auth_headers,
    )
    assert quarterly_history_response.status_code == 200
    quarterly_history_data = quarterly_history_response.get_json()
    assert quarterly_history_data["success"] is True
    assert quarterly_history_data["result"]["summary"]["period_start"] == "2026-04-01"
    assert quarterly_history_data["result"]["summary"]["period_end"] == "2026-06-30"

    yearly_execution_response = client.get(
        "/api/budgets/execution?budget_type=3&period_type=yearly&year=2026",
        headers=auth_headers,
    )
    assert yearly_execution_response.status_code == 200
    yearly_execution_data = yearly_execution_response.get_json()
    assert yearly_execution_data["success"] is True
    assert yearly_execution_data["result"]["period_start"] == "2026-01-01"
    assert yearly_execution_data["result"]["period_end"] == "2026-12-31"

    future_forecast_response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=monthly&start_date=2099-01-01&end_date=2099-01-31",
        headers=auth_headers,
    )
    assert future_forecast_response.status_code == 200
    future_forecast_data = future_forecast_response.get_json()
    assert future_forecast_data["success"] is True
    assert future_forecast_data["result"]["daysElapsed"] == 0
    assert future_forecast_data["result"]["daysRemaining"] > 0

    past_forecast_response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=monthly&start_date=2000-01-01&end_date=2000-01-31",
        headers=auth_headers,
    )
    assert past_forecast_response.status_code == 200
    past_forecast_data = past_forecast_response.get_json()
    assert past_forecast_data["success"] is True
    assert past_forecast_data["result"]["daysElapsed"] == 31
    assert past_forecast_data["result"]["daysRemaining"] == 0

    today = date.today()
    expected_week_start = today - timedelta(days=today.weekday())
    expected_week_end = expected_week_start + timedelta(days=6)

    daily_execution_response = client.get(
        "/api/budgets/execution?budget_type=3&period_type=daily",
        headers=auth_headers,
    )
    assert daily_execution_response.status_code == 200
    daily_execution_data = daily_execution_response.get_json()
    assert daily_execution_data["result"]["period_start"] == today.strftime("%Y-%m-%d")
    assert daily_execution_data["result"]["period_end"] == today.strftime("%Y-%m-%d")

    weekly_execution_response = client.get(
        "/api/budgets/execution?budget_type=3&period_type=weekly",
        headers=auth_headers,
    )
    assert weekly_execution_response.status_code == 200
    weekly_execution_data = weekly_execution_response.get_json()
    assert weekly_execution_data["result"]["period_start"] == expected_week_start.strftime("%Y-%m-%d")
    assert weekly_execution_data["result"]["period_end"] == expected_week_end.strftime("%Y-%m-%d")

    quarterly_forecast_response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=quarterly&year=2026&quarter=2",
        headers=auth_headers,
    )
    assert quarterly_forecast_response.status_code == 200
    quarterly_forecast_data = quarterly_forecast_response.get_json()
    assert quarterly_forecast_data["success"] is True
    assert quarterly_forecast_data["result"]["period_start"] == "2026-04-01"
    assert quarterly_forecast_data["result"]["period_end"] == "2026-06-30"


def test_budget_write_path_validation_closes_partial_update_and_import_shape_gaps(client, auth_headers):
    """更新与导入写路径应拦住单侧日期越界、坏 JSON 与缺字段请求。"""
    create_response = client.post("/api/budgets/", json={
        "name": "写路径校验预算",
        "category": "写路径校验分类",
        "period_type": "monthly",
        "amount": 10.0,
        "start_date": "2026-01-01",
        "end_date": "2026-01-31"
    }, headers=auth_headers)
    assert create_response.status_code == 201
    budget_id = create_response.get_json()["result"]["id"]

    invalid_end_only_response = client.put(
        f"/api/budgets/{budget_id}",
        json={"end_date": "2025-12-31"},
        headers=auth_headers,
    )
    assert invalid_end_only_response.status_code == 400
    assert "start_date" in invalid_end_only_response.get_json()["error"]

    invalid_start_only_response = client.put(
        f"/api/budgets/{budget_id}",
        json={"start_date": "2026-02-01"},
        headers=auth_headers,
    )
    assert invalid_start_only_response.status_code == 400
    assert "start_date" in invalid_start_only_response.get_json()["error"]

    malformed_import_response = client.post(
        "/api/budgets/import",
        data='[{"name":',
        content_type="application/json",
        headers=auth_headers,
    )
    assert malformed_import_response.status_code == 400
    assert malformed_import_response.get_json()["error"] == "Invalid data format. Expected array of budgets."

    missing_import_category_response = client.post("/api/budgets/import", json=[{
        "name": "缺分类导入预算",
        "period_type": "monthly",
        "amount": 10.0,
        "start_date": "2026-01-01"
    }], headers=auth_headers)
    assert missing_import_category_response.status_code == 400
    assert missing_import_category_response.get_json()["error"] == (
        "Missing required field at index 0: category"
    )


def test_budget_history_prefers_snapshot_and_execution_respects_tag_ids(client, auth_headers):
    """预算执行应应用 tag_ids，预算历史在命中快照时应优先返回落库快照。"""
    user_id = _get_current_user_id(client, auth_headers)
    unique_suffix = int(time.time() * 1000)
    main_category = f"REST快照标签分类-{unique_suffix}"
    sub_category = f"子分类-{unique_suffix}"
    sub_category_id = _create_budget_support_category(user_id, main_category, sub_category)
    matching_tag_id = _create_budget_support_tag(user_id, f"快照命中标签-{unique_suffix}")
    other_tag_id = _create_budget_support_tag(user_id, f"快照排除标签-{unique_suffix}")

    create_response = client.post("/api/budgets/", json={
        "name": f"REST快照预算-{unique_suffix}",
        "category": main_category,
        "sub_category": sub_category,
        "period_type": "monthly",
        "amount": 100.0,
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    assert create_response.status_code == 201
    budget_id = int(create_response.get_json()["result"]["id"])

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category=sub_category,
        amount=-25.0,
        description="快照前命中标签账单",
        date_text="2026-03-05 12:00:00",
        tag_ids=[matching_tag_id],
    )
    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category=sub_category,
        amount=-40.0,
        description="快照前排除标签账单",
        date_text="2026-03-06 12:00:00",
        tag_ids=[other_tag_id],
    )

    execution_response = client.get(
        f"/api/budgets/execution?budget_type=3&period_type=monthly&year=2026&month=3&"
        f"budget_id={budget_id}&category_id={sub_category_id}&tag_ids={matching_tag_id}",
        headers=auth_headers,
    )
    assert execution_response.status_code == 200
    execution_data = execution_response.get_json()
    assert execution_data["success"] is True
    assert len(execution_data["result"]["items"]) == 1
    assert execution_data["result"]["items"][0]["spent_amount"] == pytest.approx(25.0)

    snapshot_payload = {
        "budget_type": 3,
        "period_type": "monthly",
        "year": 2026,
        "month": 3,
        "budget_id": budget_id,
        "category_id": sub_category_id,
        "tag_ids": [matching_tag_id],
    }
    snapshot_response = client.post("/api/budgets/history/snapshot", json=snapshot_payload, headers=auth_headers)
    assert snapshot_response.status_code == 200
    snapshot_data = snapshot_response.get_json()
    assert snapshot_data["success"] is True
    assert snapshot_data["result"]["created_count"] == 1

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category=sub_category,
        amount=-90.0,
        description="快照后命中标签账单",
        date_text="2026-03-20 12:00:00",
        tag_ids=[matching_tag_id],
    )

    history_response = client.get(
        f"/api/budgets/history?budget_type=3&period_type=monthly&year=2026&month=3&"
        f"budget_id={budget_id}&category_id={sub_category_id}&tag_ids={matching_tag_id}",
        headers=auth_headers,
    )
    assert history_response.status_code == 200
    history_data = history_response.get_json()
    assert history_data["success"] is True
    assert len(history_data["result"]["items"]) == 1
    assert history_data["result"]["items"][0]["spent_amount"] == pytest.approx(25.0)
    assert history_data["result"]["items"][0]["status"] == "within_budget"

    refreshed_snapshot_response = client.post("/api/budgets/history/snapshot", json=snapshot_payload, headers=auth_headers)
    assert refreshed_snapshot_response.status_code == 200
    refreshed_history_response = client.get(
        f"/api/budgets/history?budget_type=3&period_type=monthly&year=2026&month=3&"
        f"budget_id={budget_id}&category_id={sub_category_id}&tag_ids={matching_tag_id}",
        headers=auth_headers,
    )
    refreshed_history_data = refreshed_history_response.get_json()
    assert refreshed_history_data["success"] is True
    assert refreshed_history_data["result"]["items"][0]["spent_amount"] == pytest.approx(115.0)
    assert refreshed_history_data["result"]["items"][0]["status"] == "over_budget"


def test_budget_execution_includes_end_date_daytime_entries(client, auth_headers):
    """预算执行接口应包含 end_date 当天 23:59:59 的账单。"""
    user_id = _get_current_user_id(client, auth_headers)
    unique_suffix = int(time.time() * 1000)
    main_category = f"REST结束日分类-{unique_suffix}"
    sub_category_id = _create_budget_support_category(user_id, main_category, "")

    create_response = client.post("/api/budgets/", json={
        "name": f"REST结束日预算-{unique_suffix}",
        "category": main_category,
        "sub_category": "",
        "period_type": "monthly",
        "amount": 100.0,
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    assert create_response.status_code == 201
    budget_id = int(create_response.get_json()["result"]["id"])

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-45.0,
        description="结束日深夜账单",
        date_text="2026-03-31 23:59:59",
        tag_ids=[],
    )

    execution_response = client.get(
        f"/api/budgets/execution?budget_type=3&period_type=monthly&year=2026&month=3&"
        f"budget_id={budget_id}&category_id={sub_category_id}",
        headers=auth_headers,
    )
    assert execution_response.status_code == 200
    execution_data = execution_response.get_json()
    assert execution_data["success"] is True
    assert len(execution_data["result"]["items"]) == 1
    assert execution_data["result"]["items"][0]["spent_amount"] == pytest.approx(45.0)


def test_budget_forecast_uses_resolved_period_range(client, auth_headers):
    """预算预测应使用路由解析后的 period_start/period_end 作为真实查询边界。"""
    user_id = _get_current_user_id(client, auth_headers)
    unique_suffix = int(time.time() * 1000)
    main_category = f"REST预测周期分类-{unique_suffix}"
    _create_budget_support_category(user_id, main_category, "")

    today = date.today()
    current_month_start = today.replace(day=1)
    previous_month_end = current_month_start - timedelta(days=1)
    previous_month_mid = previous_month_end.replace(day=min(previous_month_end.day, 15))

    create_response = client.post("/api/budgets/", json={
        "name": f"REST预测周期预算-{unique_suffix}",
        "category": main_category,
        "sub_category": "",
        "period_type": "monthly",
        "amount": 100.0,
        "start_date": current_month_start.strftime("%Y-%m-%d"),
        "end_date": today.strftime("%Y-%m-%d"),
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    assert create_response.status_code == 201

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-10.0,
        description="当月预算预测账单",
        date_text=f"{current_month_start.strftime('%Y-%m-%d')} 08:00:00",
        tag_ids=[],
    )
    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-90.0,
        description="上月预算预测账单",
        date_text=f"{previous_month_mid.strftime('%Y-%m-%d')} 08:00:00",
        tag_ids=[],
    )

    forecast_response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=monthly&months_history=6",
        headers=auth_headers,
    )
    assert forecast_response.status_code == 200
    forecast_data = forecast_response.get_json()
    assert forecast_data["success"] is True

    forecast_item = next(
        item
        for item in forecast_data["result"]["items"]
        if item["category"] == main_category
    )
    assert forecast_item["total_amount"] == pytest.approx(100.0)
    assert forecast_item["current_spent"] == pytest.approx(10.0)


def test_budget_quarterly_forecast_uses_quarter_scope(client, auth_headers):
    """季度预测应按季度聚合，不应误退化为年度聚合。"""
    user_id = _get_current_user_id(client, auth_headers)
    unique_suffix = int(time.time() * 1000)
    main_category = f"REST季度预测分类-{unique_suffix}"
    _create_budget_support_category(user_id, main_category, "")

    create_response = client.post("/api/budgets/", json={
        "name": f"REST季度预测预算-{unique_suffix}",
        "category": main_category,
        "sub_category": "",
        "period_type": "quarterly",
        "amount": 100.0,
        "start_date": "2026-01-01",
        "end_date": "2026-12-31",
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    assert create_response.status_code == 201

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-10.0,
        description="第一季度账单",
        date_text="2026-02-15 08:00:00",
        tag_ids=[],
    )
    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-40.0,
        description="第二季度账单",
        date_text="2026-05-10 08:00:00",
        tag_ids=[],
    )

    forecast_response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=quarterly&start_date=2026-04-01&end_date=2026-06-30",
        headers=auth_headers,
    )
    assert forecast_response.status_code == 200
    forecast_data = forecast_response.get_json()
    assert forecast_data["success"] is True

    forecast_item = next(
        item
        for item in forecast_data["result"]["items"]
        if item["category"] == main_category
    )
    assert forecast_item["total_amount"] == pytest.approx(50.0)
    assert forecast_item["current_spent"] == pytest.approx(40.0)


def test_budget_execution_filters_same_category_multi_period_budgets(client, auth_headers):
    """同分类多周期预算经由 REST 执行查询时，不应串到其他周期预算。"""
    user_id = _get_current_user_id(client, auth_headers)
    unique_suffix = int(time.time() * 1000)
    main_category = f"REST多周期预算分类-{unique_suffix}"
    category_id = _create_budget_support_category(user_id, main_category, "")

    monthly_response = client.post("/api/budgets/", json={
        "name": f"REST月预算-{unique_suffix}",
        "category": main_category,
        "sub_category": "",
        "period_type": "monthly",
        "amount": 100.0,
        "start_date": "2026-03-01",
        "end_date": "2026-03-31",
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    yearly_response = client.post("/api/budgets/", json={
        "name": f"REST年预算-{unique_suffix}",
        "category": main_category,
        "sub_category": "",
        "period_type": "yearly",
        "amount": 1200.0,
        "start_date": "2026-01-01",
        "end_date": "2026-12-31",
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    assert monthly_response.status_code == 201
    assert yearly_response.status_code == 201
    monthly_budget_id = int(monthly_response.get_json()["result"]["id"])
    yearly_budget_id = int(yearly_response.get_json()["result"]["id"])

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-30.0,
        description="多周期预算账单",
        date_text="2026-03-18 08:00:00",
        tag_ids=[],
    )

    monthly_execution = client.get(
        f"/api/budgets/execution?budget_type=3&period_type=monthly&year=2026&month=3&category_id={category_id}",
        headers=auth_headers,
    )
    yearly_execution = client.get(
        f"/api/budgets/execution?budget_type=3&period_type=yearly&year=2026&category_id={category_id}",
        headers=auth_headers,
    )
    assert monthly_execution.status_code == 200
    assert yearly_execution.status_code == 200

    monthly_items = monthly_execution.get_json()["result"]["items"]
    yearly_items = yearly_execution.get_json()["result"]["items"]
    assert [item["id"] for item in monthly_items] == [monthly_budget_id]
    assert [item["id"] for item in yearly_items] == [yearly_budget_id]


def test_budget_forecast_reports_zero_current_spent_when_current_period_has_no_bills(client, auth_headers):
    """当前周期无账单但历史窗口有数据时，current_spent 应保持为 0。"""
    user_id = _get_current_user_id(client, auth_headers)
    unique_suffix = int(time.time() * 1000)
    main_category = f"REST空当前期预测分类-{unique_suffix}"
    _create_budget_support_category(user_id, main_category, "")

    today = date.today()
    current_month_start = today.replace(day=1)
    previous_month_end = current_month_start - timedelta(days=1)
    previous_month_mid = previous_month_end.replace(day=min(previous_month_end.day, 15))

    create_response = client.post("/api/budgets/", json={
        "name": f"REST空当前期预测预算-{unique_suffix}",
        "category": main_category,
        "sub_category": "",
        "period_type": "monthly",
        "amount": 100.0,
        "start_date": current_month_start.strftime("%Y-%m-%d"),
        "end_date": today.strftime("%Y-%m-%d"),
        "alert_threshold": 80,
        "enabled": True,
    }, headers=auth_headers)
    assert create_response.status_code == 201

    _insert_budget_support_bill(
        user_id,
        main_category=main_category,
        sub_category="",
        amount=-90.0,
        description="仅历史周期账单",
        date_text=f"{previous_month_mid.strftime('%Y-%m-%d')} 08:00:00",
        tag_ids=[],
    )

    forecast_response = client.get(
        "/api/budgets/forecast?budget_type=3&period_type=monthly&months_history=6",
        headers=auth_headers,
    )
    assert forecast_response.status_code == 200
    forecast_data = forecast_response.get_json()
    assert forecast_data["success"] is True

    forecast_item = next(
        item
        for item in forecast_data["result"]["items"]
        if item["category"] == main_category
    )
    assert forecast_item["total_amount"] == pytest.approx(90.0)
    assert forecast_item["current_spent"] == pytest.approx(0.0)
