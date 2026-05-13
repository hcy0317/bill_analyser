"""
测试bills API的所有端点
"""
import asyncio
import json
import time
from datetime import datetime
from io import BytesIO

import pytest
from openpyxl import Workbook

from tests.user_cleanup_support import register_test_user_for_cleanup


def _pick_first_account(account_items):
    """递归获取第一个可用账户。"""
    if not account_items:
        return None

    first = account_items[0]
    sub_accounts = first.get("subAccounts") or []
    if sub_accounts:
        return _pick_first_account(sub_accounts)

    return first


def _pick_first_expense_category(category_groups):
    """获取第一个可用支出分类，优先子分类。"""
    expense_group = category_groups.get("3") or category_groups.get(3) or []
    if not expense_group:
        return None

    first = expense_group[0]
    sub_categories = first.get("subCategories") or []
    if sub_categories:
        return sub_categories[0]

    return first


def _create_account_via_db(client, auth_headers, payload):
    """Create an account directly in the DB after the Flask accounts route shell is removed."""
    from bill_analyser.api.adapters.account_adapter import AccountAdapter
    from bill_analyser.api.app import db

    adapter = AccountAdapter()
    user_id = _get_current_user_id(client, auth_headers)

    async def _create():
        backend_payload = adapter.frontend_to_backend(dict(payload))
        account_id = await db.create_account(backend_payload, user_id=user_id)
        account = await db.get_account_by_id(account_id, user_id=user_id)
        assert account is not None
        return adapter.backend_to_frontend(account)

    return asyncio.run(_create())


def _ensure_test_account(client, auth_headers):
    """确保存在可用账户。"""
    from bill_analyser.api.adapters.account_adapter import AccountAdapter
    from bill_analyser.api.app import db

    adapter = AccountAdapter()
    user_id = _get_current_user_id(client, auth_headers)

    async def _list_accounts():
        accounts = await db.get_all_accounts(user_id=user_id)
        return adapter.format_list_response(accounts or []).get("result") or []

    source_account = _pick_first_account(asyncio.run(_list_accounts()))

    if source_account:
        return source_account

    return _create_account_via_db(client, auth_headers, {
        "name": "pytest批量账户",
        "category": 1,
        "type": 1,
        "icon": "1",
        "color": "00ccff",
        "currency": "CNY",
        "balance": 0,
        "comment": "pytest 批量创建账单账户",
        "hidden": False,
        "aliases": []
    })


def _ensure_test_expense_category(client, auth_headers):
    """确保存在可用支出分类。"""
    category_response = client.get("/api/categories/", headers=auth_headers)
    assert category_response.status_code == 200
    category_data = json.loads(category_response.data)
    category = _pick_first_expense_category(category_data.get("result") or {})

    if category:
        return category

    create_response = client.post("/api/categories/", json={
        "name": "pytest批量分类",
        "parentId": "0",
        "type": 3,
        "comment": "pytest 批量创建账单分类",
        "displayOrder": 0,
        "visible": True,
        "keywords": ""
    }, headers=auth_headers)
    assert create_response.status_code == 200 or create_response.status_code == 201
    return create_response.get_json()["result"]


def _build_isolated_auth_headers(client, prefix: str) -> dict[str, str]:
    """为易受共享状态影响的用例创建独立用户。"""
    username = f"{prefix}_{int(time.time() * 1000)}"
    password = "Test123456!"
    from bill_analyser.api.app import app as flask_app

    db = flask_app.config["DB_INSTANCE"]

    register_response = client.post("/api/auth/register", json={
        "username": username,
        "email": f"{username}@example.com",
        "password": password,
        "nickname": username,
    })
    assert register_response.status_code in (200, 201), register_response.get_data(as_text=True)
    register_test_user_for_cleanup(db, username)

    login_response = client.post("/api/auth/login", json={
        "loginName": username,
        "password": password,
    })
    assert login_response.status_code == 200, login_response.get_data(as_text=True)

    result = (login_response.get_json() or {}).get("result") or {}
    token = result.get("token")
    assert token, login_response.get_data(as_text=True)
    return {"Authorization": f"Bearer {token}"}


def _create_test_recurring_template(client, auth_headers, *, name, account_id, category_id,
                                    amount_cents, start_date, frequency_type, frequency):
    """创建测试用定时交易模板。"""
    response = client.post("/api/templates/", json={
        "templateType": 2,
        "name": name,
        "type": 3,
        "categoryId": str(category_id),
        "sourceAccountId": str(account_id),
        "destinationAccountId": "0",
        "sourceAmount": amount_cents,
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [],
        "comment": "pytest recurring candidate",
        "scheduledFrequencyType": frequency_type,
        "scheduledFrequency": frequency,
        "scheduledStartDate": start_date,
        "scheduledEndDate": "",
        "utcOffset": 480
    }, headers=auth_headers)
    assert response.status_code == 201
    return response.get_json()["result"]


def _create_test_bill_for_recurring(client, auth_headers, *, account_id, category_id,
                                    amount_cents, bill_time_ms, comment):
    """创建用于定时账单匹配测试的账单。"""
    response = client.post(
        "/api/bills/batch",
        data=json.dumps({
            "clientSessionId": f"pytest-recurring-{comment}",
            "transactions": [{
                "type": 3,
                "categoryId": str(category_id),
                "time": bill_time_ms,
                "utcOffset": 480,
                "sourceAccountId": str(account_id),
                "destinationAccountId": "0",
                "sourceAmount": amount_cents,
                "destinationAmount": amount_cents,
                "hideAmount": False,
                "tagIds": [],
                "pictureIds": [],
                "comment": comment,
                "clientSessionId": f"pytest-recurring-row-{comment}"
            }]
        }),
        content_type="application/json",
        headers=auth_headers
    )
    assert response.status_code == 201
    return response.get_json()["result"]["items"][0]


def _create_test_import_session(session_id, parser_bills, parser_id="alipay", user_id=1):
    """创建用于三阶段导入测试的会话与解析模板。"""
    from bill_analyser.api.app import db

    async def _create():
        await db.create_import_session(session_id, user_id=user_id, file_count=1)
        await db.insert_parser_templates(session_id, parser_bills, parser_id=parser_id, user_id=user_id)
        await db.update_import_session_status(session_id, "parsing", total_parsed=len(parser_bills))

    asyncio.run(_create())


def _find_bill_by_comment(comment, user_id=1):
    """按备注查找最近写入的账单。"""
    from bill_analyser.api.app import db

    async def _find():
        conn = await db._get_connection()  # pylint: disable=protected-access
        async with conn.execute(
            """
            SELECT * FROM bills
            WHERE user_id = ? AND description = ?
            ORDER BY id DESC LIMIT 1
            """,
            (user_id, comment)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    return asyncio.run(_find())


def _find_recurring_by_id(recurring_id, user_id=1):
    """按ID查找定时模板。"""
    from bill_analyser.api.app import db

    async def _find():
        conn = await db._get_connection()  # pylint: disable=protected-access
        async with conn.execute(
            """
            SELECT * FROM recurring_bills
            WHERE user_id = ? AND id = ?
            LIMIT 1
            """,
            (user_id, recurring_id)
        ) as cursor:
            row = await cursor.fetchone()
            return dict(row) if row else None

    return asyncio.run(_find())


def _find_category_id_by_name(main_category, sub_category, user_id=1):
    """按主/子分类名称查找分类 ID。"""
    from bill_analyser.api.app import db

    async def _find():
        category = await db.get_category_by_name(main_category or "", sub_category or "", user_id=user_id)
        return int(category["id"]) if category and category.get("id") is not None else None

    return asyncio.run(_find())


def _get_current_user_id(client, auth_headers):
    """获取当前测试用户ID。"""
    profile_response = client.get("/api/profile", headers=auth_headers)
    assert profile_response.status_code == 200
    username = profile_response.get_json()["result"]["username"]

    from bill_analyser.api.app import db

    async def _find_user_id():
        user = await db.get_user_by_username(username)
        assert user is not None
        return int(user["id"])

    return asyncio.run(_find_user_id())


def _create_import_learning_rule(user_id, match_type, match_value, *, learned_type=None,
                                 learned_category_id=None, learned_source_account_id=None,
                                 learned_destination_account_id=None):
    """创建导入长期学习规则。"""
    from bill_analyser.api.app import db

    async def _create():
        conn = await db._get_connection()  # pylint: disable=protected-access
        now = datetime.now().isoformat()
        normalized_value = db._normalize_import_learning_text(match_value)  # pylint: disable=protected-access
        cursor = await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id,
                learned_source_account_id, learned_destination_account_id,
                enabled, source_session_id, source_preview_id,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?)
            ON CONFLICT(user_id, match_type, normalized_match_value) DO UPDATE SET
                learned_type = excluded.learned_type,
                learned_category_id = excluded.learned_category_id,
                learned_source_account_id = excluded.learned_source_account_id,
                learned_destination_account_id = excluded.learned_destination_account_id,
                enabled = 1,
                updated_at = excluded.updated_at
            """,
            (
                user_id,
                match_type,
                match_value,
                normalized_value,
                learned_type,
                learned_category_id,
                learned_source_account_id,
                learned_destination_account_id,
                "pytest-session",
                None,
                now,
                now
            )
        )
        await conn.commit()
        return int(cursor.lastrowid)

    return asyncio.run(_create())


def _create_composite_import_learning_rule(
    user_id,
    *,
    parser_id,
    counterparty,
    description,
    payment_method,
    learned_type,
    learned_category_id=None,
    learned_source_account_id=None,
    learned_destination_account_id=None
):
    """创建 composite 导入长期学习规则。"""
    from bill_analyser.api.app import db

    async def _create():
        conn = await db._get_connection()  # pylint: disable=protected-access
        now = datetime.now().isoformat()
        rule_hash = db.build_composite_match_hash(  # pylint: disable=protected-access
            parser_id=parser_id,
            counterparty=counterparty,
            description=description,
            payment_method=payment_method,
        )
        assert rule_hash is not None
        cursor = await conn.execute(
            """
            INSERT INTO import_learning_rules (
                user_id, match_type, match_value, normalized_match_value,
                learned_type, learned_category_id, learned_source_account_id, learned_destination_account_id,
                enabled, parser_id, composite_match_hash,
                match_features_json, applied_count, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?)
            """,
            (
                user_id,
                "composite",
                rule_hash,
                rule_hash,
                learned_type,
                learned_category_id,
                learned_source_account_id,
                learned_destination_account_id,
                parser_id,
                rule_hash,
                json.dumps(
                    {
                        "counterparty": counterparty,
                        "description": description,
                        "parser_id": parser_id,
                        "payment_method": payment_method,
                    },
                    ensure_ascii=False,
                ),
                6,
                now,
                now,
            )
        )
        await conn.commit()
        return int(cursor.lastrowid)

    return asyncio.run(_create())


@pytest.fixture(name="auth_headers")
def _auth_headers_fixture(client):
    """获取认证请求头"""
    username = f"test_bills_api_{int(time.time() * 1000)}"
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


class TestBillsAPI:
    """账单API测试类"""

    def test_import_learning_rules_supports_pagination_and_total_count(self, client, auth_headers):
        """测试导入学习规则列表支持分页和总数统计。"""
        user_id = _get_current_user_id(client, auth_headers)
        created_rule_ids = []
        prefix = f"pytest-learning-page-{int(time.time() * 1000)}"

        try:
            for index in range(3):
                rule_id = _create_import_learning_rule(
                    user_id,
                    "description",
                    f"{prefix}-{index}",
                    learned_type="支出"
                )
                created_rule_ids.append(rule_id)

            first_page_response = client.get(
                "/api/bills/import/learning-rules?page=1&pageSize=2",
                headers=auth_headers
            )
            assert first_page_response.status_code == 200
            first_page_data = first_page_response.get_json()
            assert first_page_data["success"] is True
            assert first_page_data["page"] == 1
            assert first_page_data["pageSize"] == 2
            assert first_page_data["totalCount"] >= 3
            assert first_page_data["totalPages"] >= 2
            assert len(first_page_data["result"]) == 2

            first_page_values = {item["matchValue"] for item in first_page_data["result"]}
            assert f"{prefix}-2" in first_page_values
            assert f"{prefix}-1" in first_page_values

            second_page_response = client.get(
                "/api/bills/import/learning-rules?page=2&pageSize=2",
                headers=auth_headers
            )
            assert second_page_response.status_code == 200
            second_page_data = second_page_response.get_json()
            assert second_page_data["success"] is True
            assert second_page_data["page"] == 2
            assert second_page_data["pageSize"] == 2
            assert second_page_data["totalCount"] >= 3

            second_page_values = {item["matchValue"] for item in second_page_data["result"]}
            assert f"{prefix}-0" in second_page_values
        finally:
            for rule_id in created_rule_ids:
                client.delete(f"/api/bills/import/learning-rules/{rule_id}", headers=auth_headers)

    def test_import_config_crud_and_match(self, client, auth_headers):
        """测试导入列映射模板 CRUD 与自动匹配。"""
        create_response = client.post("/api/bills/import/configs", json={
            "name": "pytest导入模板",
            "fileFormat": "csv_pytest_crud",
            "description": "pytest 导入模板描述",
            "fieldMappings": {
                "date": "交易时间",
                "type": "交易类型",
                "amount": "金额",
                "description": "摘要"
            },
            "sampleHeaders": ["交易时间", "交易类型", "金额", "摘要"],
            "delimiter": ",",
            "hasHeader": True,
            "customRules": {
                "source": "pytest"
            }
        }, headers=auth_headers)
        assert create_response.status_code == 201
        create_data = create_response.get_json()
        assert create_data["success"] is True
        config_id = create_data["result"]["id"]
        assert config_id

        list_response = client.get("/api/bills/import/configs?file_format=csv_pytest_crud", headers=auth_headers)
        assert list_response.status_code == 200
        list_data = list_response.get_json()
        assert list_data["success"] is True
        matched_items = [item for item in list_data["result"] if item["id"] == config_id]
        assert matched_items
        assert matched_items[0]["description"] == "pytest 导入模板描述"
        assert matched_items[0]["descriptionSummary"].startswith("映射: 时间->交易时间")

        match_response = client.post("/api/bills/import/configs/match", json={
            "fileFormat": "csv_pytest_crud",
            "headers": ["交易时间", "金额", "交易类型", "摘要", "额外列"]
        }, headers=auth_headers)
        assert match_response.status_code == 200
        match_data = match_response.get_json()
        assert match_data["success"] is True
        assert match_data["result"] is not None
        assert match_data["result"]["id"] == config_id
        assert match_data["result"]["description"] == "pytest 导入模板描述"
        assert match_data["result"]["descriptionSummary"].startswith("映射: 时间->交易时间")
        assert match_data["result"]["matchScore"] >= 0.6

        rename_response = client.post("/api/bills/import/configs", json={
            "id": config_id,
            "name": "pytest导入模板-已重命名",
            "fileFormat": "csv_pytest_crud",
            "fieldMappings": {
                "date": "交易时间",
                "type": "交易类型",
                "amount": "金额",
                "description": "摘要"
            },
            "sampleHeaders": ["交易时间", "交易类型", "金额", "摘要"],
            "delimiter": ",",
            "hasHeader": True
        }, headers=auth_headers)
        assert rename_response.status_code == 201
        rename_data = rename_response.get_json()
        assert rename_data["success"] is True
        assert rename_data["result"]["id"] == config_id

        renamed_list_response = client.get("/api/bills/import/configs?file_format=csv_pytest_crud", headers=auth_headers)
        renamed_list_data = renamed_list_response.get_json()
        renamed_items = [item for item in renamed_list_data["result"] if item["id"] == config_id]
        assert renamed_items
        assert renamed_items[0]["name"] == "pytest导入模板-已重命名"

        delete_response = client.delete(f"/api/bills/import/configs/{config_id}", headers=auth_headers)
        assert delete_response.status_code == 200
        delete_data = delete_response.get_json()
        assert delete_data["success"] is True
        assert delete_data["result"] is True

    def test_import_config_match_falls_back_to_default_template(self, client, auth_headers):
        """测试表头未命中时会回退到默认模板。"""
        create_response = client.post("/api/bills/import/configs", json={
            "name": "pytest默认模板",
            "fileFormat": "csv_pytest_fallback",
            "fieldMappings": {
                "date": "交易时间",
                "type": "交易类型",
                "amount": "金额",
                "description": "摘要"
            },
            "description": "默认导入模板",
            "sampleHeaders": ["交易时间", "交易类型", "金额", "摘要"],
            "delimiter": ",",
            "hasHeader": True,
            "isDefault": True
        }, headers=auth_headers)
        assert create_response.status_code == 201
        create_data = create_response.get_json()
        assert create_data["success"] is True
        config_id = create_data["result"]["id"]

        match_response = client.post("/api/bills/import/configs/match", json={
            "fileFormat": "csv_pytest_fallback",
            "headers": ["记账日期", "方向", "数值", "附注"]
        }, headers=auth_headers)
        assert match_response.status_code == 200
        match_data = match_response.get_json()
        assert match_data["success"] is True
        assert match_data["result"] is not None
        assert match_data["result"]["id"] == config_id
        assert match_data["result"]["description"] == "默认导入模板"
        assert match_data["result"]["descriptionSummary"].startswith("映射: 时间->交易时间")
        assert match_data["result"]["matchReason"] == "default_template_fallback"
        assert match_data["result"]["matchScore"] == 0
        assert match_data["result"]["matchedHeaderCount"] == 0

        delete_response = client.delete(f"/api/bills/import/configs/{config_id}", headers=auth_headers)
        assert delete_response.status_code == 200
        delete_data = delete_response.get_json()
        assert delete_data["success"] is True
        assert delete_data["result"] is True

    def test_import_config_list_marks_recommended_default_when_missing(self, client, auth_headers):
        """测试没有默认模板时会标记推荐的默认模板候选。"""
        first_response = client.post("/api/bills/import/configs", json={
            "name": "pytest推荐模板A",
            "fileFormat": "csv_pytest_recommend",
            "fieldMappings": {
                "date": "交易时间",
                "type": "交易类型",
                "amount": "金额"
            },
            "sampleHeaders": ["交易时间", "交易类型", "金额"],
            "delimiter": ",",
            "hasHeader": True
        }, headers=auth_headers)
        assert first_response.status_code == 201
        first_id = first_response.get_json()["result"]["id"]

        second_response = client.post("/api/bills/import/configs", json={
            "name": "pytest推荐模板B",
            "fileFormat": "csv_pytest_recommend",
            "fieldMappings": {
                "date": "日期",
                "type": "收支类型",
                "amount": "金额"
            },
            "sampleHeaders": ["日期", "收支类型", "金额"],
            "delimiter": ",",
            "hasHeader": True
        }, headers=auth_headers)
        assert second_response.status_code == 201
        second_id = second_response.get_json()["result"]["id"]

        match_response = client.post("/api/bills/import/configs/match", json={
            "fileFormat": "csv_pytest_recommend",
            "headers": ["日期", "收支类型", "金额"]
        }, headers=auth_headers)
        assert match_response.status_code == 200
        match_data = match_response.get_json()
        assert match_data["success"] is True
        assert match_data["result"]["id"] == second_id
        assert match_data["result"]["defaultRecommendation"] is True

        list_response = client.get("/api/bills/import/configs?file_format=csv_pytest_recommend", headers=auth_headers)
        assert list_response.status_code == 200
        list_data = list_response.get_json()
        assert list_data["success"] is True

        recommended_items = [item for item in list_data["result"] if item.get("defaultRecommendation")]
        assert len(recommended_items) == 1
        assert recommended_items[0]["id"] == second_id
        assert recommended_items[0]["isDefault"] is False

        first_delete = client.delete(f"/api/bills/import/configs/{first_id}", headers=auth_headers)
        assert first_delete.status_code == 200
        second_delete = client.delete(f"/api/bills/import/configs/{second_id}", headers=auth_headers)
        assert second_delete.status_code == 200

    def test_parse_import_with_column_mapping(self, client, auth_headers):
        """测试通用表格列映射导入。"""
        csv_content = (
            "交易时间,交易类型,金额,账户,分类,备注\n"
            "2025-01-01 08:30:00,支出,12.34,支付宝,餐饮,早餐\n"
            "2025-01-02 10:00:00,投资,88.00,农业银行,投资理财,基金买入\n"
        ).encode()

        response = client.post(
            "/api/bills/parse_import",
            data={
                "fileType": "csv",
                "columnMapping": json.dumps({
                    "1": 0,
                    "3": 1,
                    "4": 4,
                    "6": 3,
                    "8": 2,
                    "14": 5
                }),
                "transactionTypeMapping": json.dumps({
                    "支出": 3,
                    "投资": 5
                }),
                "hasHeaderLine": "true",
                "timeFormat": "%Y-%m-%d %H:%M:%S",
                "amountDecimalSeparator": ".",
                "tagSeparator": ";",
                "delimiter": ",",
                "file": (BytesIO(csv_content), "pytest_import.csv")
            },
            headers=auth_headers,
            content_type="multipart/form-data"
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["result"]["totalCount"] == 2
        first_item = data["result"]["items"][0]
        second_item = data["result"]["items"][1]
        assert first_item["type"] == 3
        assert first_item["sourceAmount"] == 1234
        assert first_item["originalSourceAccountName"] == "支付宝"
        assert first_item["originalCategoryName"] == "餐饮"
        assert second_item["type"] == 5
        assert second_item["sourceAmount"] == 8800
        assert second_item["comment"] == "基金买入"

    def test_parse_import_with_column_mapping_detects_header_row_after_preamble(self, client, auth_headers):
        """测试通用表格列映射导入可自动跳过前置说明行并识别真正表头。"""
        csv_content = (
            "账单导出说明,,,,,\n"
            "统计周期,2025-01-01 至 2025-01-31,,,,\n"
            "交易时间,交易类型,金额,账户,分类,备注\n"
            "2025-01-01 08:30:00,支出,12.34,支付宝,餐饮,早餐\n"
            "2025-01-02 10:00:00,收入,88.00,农业银行,工资,工资发放\n"
        ).encode()

        response = client.post(
            "/api/bills/parse_import",
            data={
                "fileType": "csv",
                "columnMapping": json.dumps({
                    "1": 0,
                    "3": 1,
                    "4": 4,
                    "6": 3,
                    "8": 2,
                    "14": 5
                }),
                "transactionTypeMapping": json.dumps({
                    "支出": 3,
                    "收入": 2
                }),
                "hasHeaderLine": "true",
                "timeFormat": "%Y-%m-%d %H:%M:%S",
                "amountDecimalSeparator": ".",
                "tagSeparator": ";",
                "delimiter": ",",
                "file": (BytesIO(csv_content), "pytest_import_with_preamble.csv")
            },
            headers=auth_headers,
            content_type="multipart/form-data"
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["result"]["totalCount"] == 2
        first_item = data["result"]["items"][0]
        second_item = data["result"]["items"][1]
        assert first_item["type"] == 3
        assert first_item["sourceAmount"] == 1234
        assert first_item["comment"] == "早餐"
        assert second_item["type"] == 2
        assert second_item["sourceAmount"] == 8800
        assert second_item["comment"] == "工资发放"

    def test_suggest_import_config_with_headers(self, client, auth_headers):
        """测试基于表头和样本行自动建议列映射。"""
        response = client.post("/api/bills/import/configs/suggest", json={
            "fileFormat": "csv",
            "headers": ["交易时间", "交易类型", "金额", "账户", "备注"],
            "sampleRows": [
                ["2025-01-01 08:30:00", "支出", "12.34", "支付宝", "早餐"],
                ["2025-01-02 10:00:00", "投资", "88.00", "农业银行", "基金买入"]
            ]
        }, headers=auth_headers)

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        result = data["result"]
        assert result["includeHeader"] is True
        assert result["columnMapping"]["1"] == 0
        assert result["columnMapping"]["3"] == 1
        assert result["columnMapping"]["8"] == 2
        assert result["columnMapping"]["6"] == 3
        assert result["columnMapping"]["14"] == 4
        assert result["transactionTypeMapping"]["支出"] == 3
        assert result["transactionTypeMapping"]["投资"] == 5

    def test_parse_import_with_column_mapping_applies_learning_rules(self, client, auth_headers):
        """测试通用列映射导入会回放长期学习规则到分类和账户。"""
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)
        user_id = _get_current_user_id(client, auth_headers)
        unique_description = f"pytest-learning-{int(time.time())}"

        _create_import_learning_rule(
            user_id,
            "description",
            unique_description,
            learned_type="支出",
            learned_category_id=int(category["id"]),
            learned_source_account_id=int(source_account["id"])
        )

        csv_content = (
            "交易时间,金额,备注\n"
            f"2025-01-03 08:30:00,23.45,{unique_description}\n"
        ).encode()

        response = client.post(
            "/api/bills/parse_import",
            data={
                "fileType": "csv",
                "columnMapping": json.dumps({
                    "1": 0,
                    "8": 1,
                    "14": 2
                }),
                "hasHeaderLine": "true",
                "timeFormat": "%Y-%m-%d %H:%M:%S",
                "amountDecimalSeparator": ".",
                "tagSeparator": ";",
                "delimiter": ",",
                "file": (BytesIO(csv_content), "pytest_learning_import.csv")
            },
            headers=auth_headers,
            content_type="multipart/form-data"
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["result"]["totalCount"] == 1

        item = data["result"]["items"][0]
        assert item["type"] == 3
        assert item["categoryId"] == str(category["id"])
        assert item["sourceAccountId"] == str(source_account["id"])
        assert unique_description in item["comment"]

    def test_preview_import_file_with_excel(self, client, auth_headers):
        """测试通用导入文件预览接口支持 Excel。"""
        workbook = Workbook()
        sheet = workbook.active
        assert sheet is not None
        sheet.append(["交易时间", "交易类型", "金额", "账户"])
        sheet.append(["2025-01-01 08:30:00", "支出", "12.34", "支付宝"])
        sheet.append(["2025-01-02 10:00:00", "收入", "88.00", "农业银行"])

        excel_buffer = BytesIO()
        workbook.save(excel_buffer)
        excel_buffer.seek(0)

        response = client.post(
            "/api/bills/import/preview",
            data={
                "file": (excel_buffer, "pytest_preview.xlsx")
            },
            headers=auth_headers,
            content_type="multipart/form-data"
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        result = data["result"]
        assert result["headers"] == ["交易时间", "交易类型", "金额", "账户"]
        assert result["totalRows"] == 2
        assert len(result["sampleData"]) >= 3
        assert result["sampleData"][1][0] == "2025-01-01 08:30:00"

    def test_preview_import_file_auto_detects_header_row_after_preamble(self, client, auth_headers):
        """测试预览接口会自动跳过说明区并返回真正表头。"""
        csv_content = (
            "账单导出说明,,,,,\n"
            "统计周期,2025-01-01 至 2025-01-31,,,,\n"
            "交易时间,交易类型,金额,账户,分类,备注\n"
            "2025-01-01 08:30:00,支出,12.34,支付宝,餐饮,早餐\n"
            "2025-01-02 10:00:00,收入,88.00,农业银行,工资,工资发放\n"
        ).encode()

        response = client.post(
            "/api/bills/import/preview",
            data={
                "delimiter": ",",
                "file": (BytesIO(csv_content), "pytest_preview_with_preamble.csv")
            },
            headers=auth_headers,
            content_type="multipart/form-data"
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        result = data["result"]
        assert result["headers"] == ["交易时间", "交易类型", "金额", "账户", "分类", "备注"]
        assert result["totalRows"] == 2
        assert result["sampleData"][0] == ["交易时间", "交易类型", "金额", "账户", "分类", "备注"]
        assert result["sampleData"][1][0] == "2025-01-01 08:30:00"

    def test_get_bills_default(self, client, auth_headers):
        """测试获取账单列表（默认参数）"""
        response = client.get("/api/bills/", headers=auth_headers)
        assert response.status_code == 200

        data = json.loads(response.data)
        assert data["success"] is True
        assert "result" in data
        assert "items" in data["result"]
        assert "total" in data["result"]
        assert "page" in data["result"]
        assert "page_size" in data["result"]

    def test_get_bills_with_filters(self, client, auth_headers):
        """测试带过滤条件的账单列表"""
        response = client.get("/api/bills/?type=支出&page=1&page_size=10", headers=auth_headers)
        assert response.status_code == 200

        data = json.loads(response.data)
        assert data["success"] is True

        # 验证返回的账单都是支出类型
        items = data["result"]["items"]
        if items:
            for item in items:
                assert item.get("type") == 3

    def test_get_bills_pagination(self, client, auth_headers):
        """测试分页功能"""
        # 第一页
        response1 = client.get("/api/bills/?page=1&page_size=5", headers=auth_headers)
        data1 = json.loads(response1.data)

        # 第二页
        response2 = client.get("/api/bills/?page=2&page_size=5", headers=auth_headers)
        data2 = json.loads(response2.data)

        assert response1.status_code == 200
        assert response2.status_code == 200

        # 验证分页信息
        assert data1["result"]["page"] == 1
        assert data2["result"]["page"] == 2
        assert data1["result"]["page_size"] == 5

    def test_get_bill_by_id(self, client, auth_headers):
        """测试获取单个账单"""
        # 先获取列表获取一个有效ID
        list_response = client.get("/api/bills/?page_size=1", headers=auth_headers)
        list_data = json.loads(list_response.data)

        if list_data["result"]["items"]:
            bill_id = list_data["result"]["items"][0]["id"]

            response = client.get(f"/api/bills/{bill_id}", headers=auth_headers)
            assert response.status_code == 200

            data = json.loads(response.data)
            assert data["success"] is True
            assert data["result"]["id"] == bill_id
        else:
            pytest.skip("数据库中没有账单数据")

    def test_get_bill_query_endpoint_and_legacy_route_removed(self, client, auth_headers):
        """账单详情应走 REST 查询端点，旧 v1 get 路径应返回 404。"""
        list_response = client.get("/api/bills/?page_size=1", headers=auth_headers)
        list_data = json.loads(list_response.data)

        if not list_data["result"]["items"]:
            pytest.skip("数据库中没有账单数据")

        bill_id = list_data["result"]["items"][0]["id"]

        rest_response = client.get(f"/api/bills/get?id={bill_id}", headers=auth_headers)
        assert rest_response.status_code == 200
        rest_data = json.loads(rest_response.data)
        assert rest_data["success"] is True
        assert rest_data["result"]["id"] == str(bill_id) or rest_data["result"]["id"] == bill_id

        legacy_response = client.get(
            f"/api/v1/transactions/get.json?id={bill_id}",
            headers=auth_headers
        )
        assert legacy_response.status_code == 404

    def test_get_bill_not_found(self, client, auth_headers):
        """测试获取不存在的账单"""
        response = client.get("/api/bills/999999", headers=auth_headers)
        assert response.status_code == 404

        data = json.loads(response.data)
        assert data["success"] is False

    def test_update_bill(self, client, auth_headers):
        """测试更新账单"""
        # 先获取一个账单
        list_response = client.get("/api/bills/?page_size=1", headers=auth_headers)
        list_data = json.loads(list_response.data)

        if not list_data["result"]["items"]:
            pytest.skip("数据库中没有账单数据")

        bill_id = list_data["result"]["items"][0]["id"]

        # 更新账单
        update_data = {
            "description": "测试更新描述"
        }
        response = client.put(
            f"/api/bills/{bill_id}",
            data=json.dumps(update_data),
            content_type="application/json",
            headers=auth_headers
        )

        assert response.status_code == 200
        data = json.loads(response.data)
        assert data["success"] is True

    def test_batch_create_bills(self, client, auth_headers):
        """测试批量创建账单。"""
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)

        before_response = client.get("/api/bills/?page_size=1", headers=auth_headers)
        before_total = json.loads(before_response.data)["result"]["total"]

        now_ms = int(datetime.now().timestamp() * 1000)
        payload = {
            "clientSessionId": "pytest-batch-create",
            "transactions": [
                {
                    "type": 3,
                    "categoryId": category["id"],
                    "time": now_ms,
                    "utcOffset": 480,
                    "sourceAccountId": source_account["id"],
                    "destinationAccountId": "0",
                    "sourceAmount": 1234,
                    "destinationAmount": 1234,
                    "hideAmount": False,
                    "tagIds": [],
                    "pictureIds": [],
                    "comment": "pytest 批量创建 1",
                    "clientSessionId": "pytest-batch-create-1"
                },
                {
                    "type": 3,
                    "categoryId": category["id"],
                    "time": now_ms + 60000,
                    "utcOffset": 480,
                    "sourceAccountId": source_account["id"],
                    "destinationAccountId": "0",
                    "sourceAmount": 2345,
                    "destinationAmount": 2345,
                    "hideAmount": False,
                    "tagIds": [],
                    "pictureIds": [],
                    "comment": "pytest 批量创建 2",
                    "clientSessionId": "pytest-batch-create-2"
                }
            ]
        }

        response = client.post(
            "/api/bills/batch",
            data=json.dumps(payload),
            content_type="application/json",
            headers=auth_headers
        )

        assert response.status_code == 201
        data = json.loads(response.data)
        assert data["success"] is True
        assert data["result"]["createdCount"] == 2
        assert len(data["result"]["items"]) == 2

        comments = {item["comment"] for item in data["result"]["items"]}
        assert "pytest 批量创建 1" in comments
        assert "pytest 批量创建 2" in comments

        after_response = client.get("/api/bills/?page_size=1", headers=auth_headers)
        after_total = json.loads(after_response.data)["result"]["total"]
        assert after_total >= before_total + 2

    def test_get_recurring_candidates_for_bill(self, client, auth_headers):
        """测试获取账单的定时交易候选。"""
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)

        recurring = _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest定时房租",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=5678,
            start_date="2026-03-01",
            frequency_type=2,
            frequency="8"
        )

        bill = _create_test_bill_for_recurring(
            client,
            auth_headers,
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=5678,
            bill_time_ms=int(datetime(2026, 3, 8, 12, 0, 0).timestamp() * 1000),
            comment="pytest recurring candidate bill"
        )

        response = client.get(
            f"/api/bills/{bill['id']}/recurring-candidates?toleranceDays=2",
            headers=auth_headers
        )
        assert response.status_code == 200

        data = response.get_json()
        assert data["success"] is True
        assert data["result"]["billId"] == int(bill["id"])
        assert isinstance(data["result"]["candidates"], list)
        assert any(str(item["id"]) == str(recurring["id"]) for item in data["result"]["candidates"])

        matched = next(
            item for item in data["result"]["candidates"]
            if str(item["id"]) == str(recurring["id"])
        )
        assert matched["matchedOccurrenceDate"] == "2026-03-08"
        assert matched["matchScore"] >= 90

    def test_bind_and_unbind_recurring_match(self, client, auth_headers):
        """测试账单绑定和解绑定时交易。"""
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)

        recurring = _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest定时水电",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=4321,
            start_date="2026-03-01",
            frequency_type=2,
            frequency="8"
        )
        bill = _create_test_bill_for_recurring(
            client,
            auth_headers,
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=4321,
            bill_time_ms=int(datetime(2026, 3, 8, 8, 30, 0).timestamp() * 1000),
            comment="pytest recurring bind bill"
        )

        bind_response = client.put(
            f"/api/bills/{bill['id']}/recurring-match",
            data=json.dumps({"recurringId": recurring["id"]}),
            content_type="application/json",
            headers=auth_headers
        )
        assert bind_response.status_code == 200
        bind_data = bind_response.get_json()
        assert bind_data["success"] is True
        assert str(bind_data["result"]["recurringId"]) == str(recurring["id"])

        recurring_after_bind = _find_recurring_by_id(recurring["id"], user_id=current_user_id)
        assert recurring_after_bind is not None
        assert str(recurring_after_bind["next_date"]).startswith("2026-04-08")

        candidate_response = client.get(
            f"/api/bills/{bill['id']}/recurring-candidates",
            headers=auth_headers
        )
        assert candidate_response.status_code == 200
        candidate_data = candidate_response.get_json()
        assert str(candidate_data["result"]["linkedRecurringId"]) == str(recurring["id"])
        assert candidate_data["result"]["linkedRecurringName"] == "pytest定时水电"

        unbind_response = client.delete(
            f"/api/bills/{bill['id']}/recurring-match",
            headers=auth_headers
        )
        assert unbind_response.status_code == 200
        unbind_data = unbind_response.get_json()
        assert unbind_data["success"] is True

        recurring_after_unbind = _find_recurring_by_id(recurring["id"], user_id=current_user_id)
        assert recurring_after_unbind is not None
        assert str(recurring_after_unbind["next_date"]).startswith("2026-03-08")

        candidate_response_after = client.get(
            f"/api/bills/{bill['id']}/recurring-candidates",
            headers=auth_headers
        )
        assert candidate_response_after.status_code == 200
        candidate_data_after = candidate_response_after.get_json()
        assert candidate_data_after["result"]["linkedRecurringId"] in [None, "", 0]

    def test_rebind_recurring_match_recalculates_previous_template(self, client, auth_headers):
        """测试账单改绑到新的定时模板时，旧模板和新模板的 next_date 都会正确重算。"""
        current_user_id = _get_current_user_id(client, auth_headers)
        source_account = _ensure_test_account(client, auth_headers)
        category = _ensure_test_expense_category(client, auth_headers)

        recurring_one = _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest定时旧模板",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=5432,
            start_date="2026-03-01",
            frequency_type=2,
            frequency="8"
        )
        recurring_two = _create_test_recurring_template(
            client,
            auth_headers,
            name="pytest定时新模板",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=5432,
            start_date="2026-03-01",
            frequency_type=2,
            frequency="8"
        )
        bill = _create_test_bill_for_recurring(
            client,
            auth_headers,
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=5432,
            bill_time_ms=int(datetime(2026, 3, 8, 9, 0, 0).timestamp() * 1000),
            comment="pytest recurring rebind bill"
        )

        bind_first_response = client.put(
            f"/api/bills/{bill['id']}/recurring-match",
            data=json.dumps({"recurringId": recurring_one["id"]}),
            content_type="application/json",
            headers=auth_headers
        )
        assert bind_first_response.status_code == 200

        recurring_one_after_first_bind = _find_recurring_by_id(recurring_one["id"], user_id=current_user_id)
        assert recurring_one_after_first_bind is not None
        assert str(recurring_one_after_first_bind["next_date"]).startswith("2026-04-08")

        bind_second_response = client.put(
            f"/api/bills/{bill['id']}/recurring-match",
            data=json.dumps({"recurringId": recurring_two["id"]}),
            content_type="application/json",
            headers=auth_headers
        )
        assert bind_second_response.status_code == 200
        bind_second_data = bind_second_response.get_json()
        assert bind_second_data["success"] is True
        assert str(bind_second_data["result"]["recurringId"]) == str(recurring_two["id"])

        recurring_one_after_rebind = _find_recurring_by_id(recurring_one["id"], user_id=current_user_id)
        recurring_two_after_rebind = _find_recurring_by_id(recurring_two["id"], user_id=current_user_id)
        assert recurring_one_after_rebind is not None
        assert recurring_two_after_rebind is not None
        assert str(recurring_one_after_rebind["next_date"]).startswith("2026-03-08")
        assert str(recurring_two_after_rebind["next_date"]).startswith("2026-04-08")

        candidate_response = client.get(
            f"/api/bills/{bill['id']}/recurring-candidates",
            headers=auth_headers
        )
        assert candidate_response.status_code == 200
        candidate_data = candidate_response.get_json()
        assert str(candidate_data["result"]["linkedRecurringId"]) == str(recurring_two["id"])
        assert candidate_data["result"]["linkedRecurringName"] == "pytest定时新模板"

    def test_import_preview_auto_links_recurring_match(self, client, auth_headers):
        """测试三阶段导入预览会自动附带定时账单匹配结果，并在确认导入时写入正式账单。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_recurring_auto")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        source_account = _ensure_test_account(client, isolated_auth_headers)
        category = _ensure_test_expense_category(client, isolated_auth_headers)
        unique_suffix = int(time.time() * 1000)
        recurring_amount_cents = 5097
        bill_comment = f"pytest recurring import preview bill {unique_suffix}"
        recurring_counterparty = f"pytest landlord {unique_suffix}"

        recurring = _create_test_recurring_template(
            client,
            isolated_auth_headers,
            name=f"pytest导入定时宽表匹配-{unique_suffix}",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=recurring_amount_cents,
            start_date="2026-03-01",
            frequency_type=2,
            frequency="8"
        )

        session_id = f"pytest-import-recurring-{unique_suffix}"
        parser_bills = [{
            "date": "2026-03-08 09:00:00",
            "amount": -(recurring_amount_cents / 100.0),
            "type": "支出",
            "description": bill_comment,
            "counterparty": recurring_counterparty,
            "payment_method": "pytest recurring account",
            "source_account_id": source_account["id"]
        }]
        _create_test_import_session(session_id, parser_bills, parser_id="alipay", user_id=current_user_id)

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            data=json.dumps({"session_id": session_id}),
            content_type="application/json",
            headers=isolated_auth_headers
        )
        assert dedup_response.status_code == 200

        dedup_data = dedup_response.get_json()
        assert dedup_data["success"] is True
        preview_items = dedup_data["data"]["preview"]
        assert len(preview_items) == 1
        preview_item = preview_items[0]
        assert str(preview_item["preview_recurring_id"]) == str(recurring["id"])
        assert preview_item["preview_recurring_candidate_count"] >= 1
        assert preview_item["preview_recurring_name"] == f"pytest导入定时宽表匹配-{unique_suffix}"
        assert "schedule" in str(preview_item["preview_recurring_match_reasons"])

        confirm_response = client.post(
            "/api/bills/import/v2/confirm",
            data=json.dumps({
                "session_id": session_id,
                "preview_updates": [{
                    "id": preview_item["id"],
                    "preview_recurring_id": recurring["id"],
                    "selected": True
                }]
            }),
            content_type="application/json",
            headers=isolated_auth_headers
        )
        assert confirm_response.status_code == 200
        confirm_data = confirm_response.get_json()
        assert confirm_data["success"] is True

        inserted_bill = _find_bill_by_comment(bill_comment, user_id=current_user_id)
        assert inserted_bill is not None
        assert str(inserted_bill["created_from_recurring"]) == str(recurring["id"])

        refreshed_recurring = _find_recurring_by_id(recurring["id"], user_id=current_user_id)
        assert refreshed_recurring is not None
        assert str(refreshed_recurring["next_date"]).startswith("2026-04-08")

    def test_import_preview_recurring_candidates_and_clear_match(self, client, auth_headers):
        """测试导入预览可查询多个定时候选，并可在确认导入前清除定时匹配。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_recurring_preview")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        source_account = _ensure_test_account(client, isolated_auth_headers)
        category = _ensure_test_expense_category(client, isolated_auth_headers)
        bill_comment = f"pytest recurring import preview clear bill {int(time.time() * 1000)}"

        recurring_one = _create_test_recurring_template(
            client,
            isolated_auth_headers,
            name="pytest预览定时候选一",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=7600,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1"
        )
        recurring_two = _create_test_recurring_template(
            client,
            isolated_auth_headers,
            name="pytest预览定时候选二",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=7600,
            start_date="2026-03-09",
            frequency_type=1,
            frequency="1"
        )

        session_id = f"pytest-import-recurring-preview-{int(time.time())}"
        parser_bills = [{
            "date": "2026-03-09 10:30:00",
            "amount": -76.0,
            "type": "支出",
            "description": bill_comment,
            "counterparty": "pytest recurring preview vendor",
            "payment_method": "pytest recurring account",
            "source_account_id": source_account["id"]
        }]
        _create_test_import_session(session_id, parser_bills, parser_id="wechat", user_id=current_user_id)

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            data=json.dumps({"session_id": session_id}),
            content_type="application/json",
            headers=isolated_auth_headers
        )
        assert dedup_response.status_code == 200

        dedup_data = dedup_response.get_json()
        assert dedup_data["success"] is True
        preview_item = dedup_data["data"]["preview"][0]

        candidates_response = client.get(
            f"/api/bills/import/v2/preview-item/{preview_item['id']}/recurring-candidates?toleranceDays=3",
            headers=isolated_auth_headers
        )
        assert candidates_response.status_code == 200
        candidates_data = candidates_response.get_json()
        assert candidates_data["success"] is True
        assert candidates_data["result"]["previewId"] == int(preview_item["id"])
        candidates = candidates_data["result"]["candidates"]
        assert len(candidates) >= 2
        candidate_ids = {str(candidate["id"]) for candidate in candidates}
        assert str(recurring_one["id"]) in candidate_ids
        assert str(recurring_two["id"]) in candidate_ids

        confirm_response = client.post(
            "/api/bills/import/v2/confirm",
            data=json.dumps({
                "session_id": session_id,
                "preview_updates": [{
                    "id": preview_item["id"],
                    "preview_recurring_id": None,
                    "preview_recurring_name": "",
                    "preview_recurring_candidate_count": len(candidates),
                    "preview_recurring_match_score": 0,
                    "preview_recurring_match_reasons": "",
                    "preview_recurring_matched_date": "",
                    "selected": True
                }]
            }),
            content_type="application/json",
            headers=isolated_auth_headers
        )
        assert confirm_response.status_code == 200
        confirm_data = confirm_response.get_json()
        assert confirm_data["success"] is True

        inserted_bill = _find_bill_by_comment(bill_comment, user_id=current_user_id)
        assert inserted_bill is not None
        assert inserted_bill["created_from_recurring"] in [None, "", 0]

    def test_import_preview_recurring_match_put_delete_and_stale_snapshot(self, client):
        """导入预览 recurring 候选应支持后端绑定/清除，并且 preview 阶段不推进模板 next_date。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_preview_recurring_match")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        source_account = _ensure_test_account(client, isolated_auth_headers)
        category = _ensure_test_expense_category(client, isolated_auth_headers)
        bill_comment = f"pytest preview recurring match bill {int(time.time() * 1000)}"

        recurring_one = _create_test_recurring_template(
            client,
            isolated_auth_headers,
            name="pytest预览定时绑定候选一",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=7600,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1"
        )
        recurring_two = _create_test_recurring_template(
            client,
            isolated_auth_headers,
            name="pytest预览定时绑定候选二",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=7600,
            start_date="2026-03-09",
            frequency_type=1,
            frequency="1"
        )

        session_id = f"pytest-import-recurring-match-{int(time.time() * 1000)}"
        parser_bills = [{
            "date": "2026-03-09 10:30:00",
            "amount": -76.0,
            "type": "支出",
            "description": bill_comment,
            "counterparty": "pytest recurring match vendor",
            "payment_method": "pytest recurring account",
            "source_account_id": source_account["id"]
        }]
        _create_test_import_session(session_id, parser_bills, parser_id="wechat", user_id=current_user_id)

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            data=json.dumps({"session_id": session_id}),
            content_type="application/json",
            headers=isolated_auth_headers
        )
        assert dedup_response.status_code == 200
        preview_item = dedup_response.get_json()["data"]["preview"][0]

        candidates_response = client.get(
            f"/api/bills/import/v2/preview-item/{preview_item['id']}/recurring-candidates?toleranceDays=3",
            headers=isolated_auth_headers
        )
        assert candidates_response.status_code == 200
        candidates_data = candidates_response.get_json()
        assert candidates_data["success"] is True
        candidates = candidates_data["result"]["candidates"]
        assert len(candidates) >= 2

        initial_recurring_id = preview_item.get("preview_recurring_id")
        target_candidate = next(
            candidate for candidate in candidates if str(candidate["id"]) != str(initial_recurring_id)
        )
        recurring_two_before = _find_recurring_by_id(recurring_two["id"], user_id=current_user_id)
        assert recurring_two_before is not None
        recurring_two_next_date_before = str(recurring_two_before["next_date"])

        def _build_expected_state(item: dict[str, object]) -> dict[str, object]:
            category_id = _find_category_id_by_name(
                str(item.get("preview_main_category") or ""),
                str(item.get("preview_sub_category") or ""),
                user_id=current_user_id,
            )
            transfer_details = ((item.get("matching") or {}).get("transfer") or {})
            transfer_review_status = str(transfer_details.get("review_status") or "").strip().lower()
            if transfer_review_status not in {"accepted", "rejected"}:
                transfer_review_status = "pending" if str(transfer_details.get("candidate_type") or "").strip() else ""
            return {
                "sessionId": session_id,
                "reviewStatus": transfer_review_status,
                "previewType": item.get("preview_type"),
                "categoryId": category_id,
                "recurringId": item.get("preview_recurring_id"),
            }

        invalid_recurring_id_response = client.put(
            f"/api/bills/import/v2/preview-item/{preview_item['id']}/recurring-match",
            data=json.dumps(
                {
                    "recurringId": True,
                    "expectedState": _build_expected_state(preview_item),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert invalid_recurring_id_response.status_code == 400
        assert invalid_recurring_id_response.get_json()["error"] == "Invalid request"

        bind_response = client.put(
            f"/api/bills/import/v2/preview-item/{preview_item['id']}/recurring-match",
            data=json.dumps(
                {
                    "recurringId": target_candidate["id"],
                    "expectedState": _build_expected_state(preview_item),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert bind_response.status_code == 200
        bind_data = bind_response.get_json()
        assert bind_data["success"] is True
        bound_preview = next(
            item for item in bind_data["data"]["preview"] if int(item["id"]) == int(preview_item["id"])
        )
        assert str(bound_preview["preview_recurring_id"]) == str(target_candidate["id"])
        assert bound_preview["preview_recurring_name"] == target_candidate["name"]
        assert bound_preview["preview_recurring_candidate_count"] >= 2
        assert bound_preview["matching"]["recurring"]["id"] == int(target_candidate["id"])

        recurring_two_after_bind = _find_recurring_by_id(recurring_two["id"], user_id=current_user_id)
        assert recurring_two_after_bind is not None
        assert str(recurring_two_after_bind["next_date"]) == recurring_two_next_date_before

        stale_delete_response = client.delete(
            f"/api/bills/import/v2/preview-item/{preview_item['id']}/recurring-match",
            data=json.dumps({"expectedState": _build_expected_state(preview_item)}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert stale_delete_response.status_code == 409
        assert stale_delete_response.get_json()["error"] == "Preview state changed, please refresh"

        clear_response = client.delete(
            f"/api/bills/import/v2/preview-item/{preview_item['id']}/recurring-match",
            data=json.dumps({"expectedState": _build_expected_state(bound_preview)}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert clear_response.status_code == 200
        clear_data = clear_response.get_json()
        assert clear_data["success"] is True
        cleared_preview = next(
            item for item in clear_data["data"]["preview"] if int(item["id"]) == int(preview_item["id"])
        )
        assert cleared_preview["preview_recurring_id"] in (None, "", 0)
        assert cleared_preview["preview_recurring_name"] == ""
        assert cleared_preview["preview_recurring_candidate_count"] >= 2
        assert cleared_preview["preview_recurring_match_score"] == 0
        assert cleared_preview["matching"]["recurring"]["id"] is None

        recurring_two_after_clear = _find_recurring_by_id(recurring_two["id"], user_id=current_user_id)
        assert recurring_two_after_clear is not None
        assert str(recurring_two_after_clear["next_date"]) == recurring_two_next_date_before

    def test_import_stage2_dedup_supports_lightweight_response_and_preview_page(self, client):
        """阶段2应支持不回整批 preview，并通过分页接口拉取富预览数据。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_import_stage2_lightweight_preview")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        session_id = f"pytest-import-lightweight-preview-{int(time.time() * 1000)}"

        parser_bills = [
            {
                "date": "2026-08-01 10:00:00",
                "amount": -18.5,
                "type": "支出",
                "description": "pytest import page one",
                "counterparty": "早餐铺一号",
                "payment_method": "支付宝",
                "source_account_id": 0,
            },
            {
                "date": "2026-08-02 10:00:00",
                "amount": -28.5,
                "type": "支出",
                "description": "pytest import page two",
                "counterparty": "早餐铺二号",
                "payment_method": "微信支付",
                "source_account_id": 0,
            },
        ]
        _create_test_import_session(session_id, parser_bills, parser_id="alipay", user_id=current_user_id)

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            data=json.dumps({"session_id": session_id, "include_preview": False}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert dedup_response.status_code == 200
        dedup_data = dedup_response.get_json()
        assert dedup_data["success"] is True
        assert dedup_data["data"]["preview"] == []
        assert dedup_data["data"]["preview_included"] is False
        assert dedup_data["data"]["after_dedup"] >= 2

        preview_page_response = client.get(
            f"/api/bills/import/v2/preview/{session_id}?page=1&page_size=1",
            headers=isolated_auth_headers,
        )
        assert preview_page_response.status_code == 200
        preview_page_data = preview_page_response.get_json()
        assert preview_page_data["success"] is True
        assert preview_page_data["data"]["page"] == 1
        assert preview_page_data["data"]["page_size"] == 1
        assert preview_page_data["data"]["total"] >= 2
        assert len(preview_page_data["data"]["preview"]) == 1
        first_preview = preview_page_data["data"]["preview"][0]
        assert "matching" in first_preview
        assert "preview_parser_id" in first_preview
        assert "preview_parser_tags" in first_preview
        assert "dedup_source_ids" in first_preview

        second_preview_response = client.get(
            f"/api/bills/import/v2/preview/{session_id}?page=2&page_size=1",
            headers=isolated_auth_headers,
        )
        assert second_preview_response.status_code == 200
        second_preview_data = second_preview_response.get_json()
        assert second_preview_data["success"] is True
        assert second_preview_data["data"]["page"] == 2
        assert len(second_preview_data["data"]["preview"]) == 1
        assert second_preview_data["data"]["preview"][0]["id"] != first_preview["id"]

    def test_import_preview_page_supports_server_paged_sort_contract(self, client):
        """server-paged preview route 应支持受支持列的真实后端排序契约。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_import_preview_page_sort_contract")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        session_id = f"pytest-import-preview-sort-{int(time.time() * 1000)}"

        parser_bills = [
            {
                "date": "2026-08-03 10:00:00",
                "amount": -18.5,
                "type": "支出",
                "description": "gamma row",
                "counterparty": "Charlie Shop",
                "payment_method": "WeChat",
                "source_account_id": 0,
            },
            {
                "date": "2026-08-01 09:00:00",
                "amount": -8.5,
                "type": "收入",
                "description": "alpha row",
                "counterparty": "Alpha Cafe",
                "payment_method": "Bank Card",
                "source_account_id": 0,
            },
            {
                "date": "2026-08-02 08:00:00",
                "amount": -12.5,
                "type": "转账",
                "description": "beta row",
                "counterparty": "Bravo Market",
                "payment_method": "Alipay",
                "source_account_id": 0,
            },
        ]
        _create_test_import_session(session_id, parser_bills, parser_id="alipay", user_id=current_user_id)

        dedup_response = client.post(
            "/api/bills/import/v2/dedup",
            data=json.dumps({"session_id": session_id, "include_preview": False}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert dedup_response.status_code == 200
        assert dedup_response.get_json()["success"] is True

        sort_cases = [
            ("time", "desc", "preview_date"),
            ("sourceAmount", "asc", "preview_amount"),
            ("counterparty", "asc", "preview_counterparty"),
            ("paymentMethod", "asc", "preview_payment_method"),
            ("comment", "asc", "preview_description"),
            ("type", "asc", "preview_type"),
        ]

        for sort_by, sort_direction, response_field in sort_cases:
            preview_page_response = client.get(
                (
                    f"/api/bills/import/v2/preview/{session_id}"
                    f"?page=1&page_size=10&sort_by={sort_by}&sort_direction={sort_direction}"
                ),
                headers=isolated_auth_headers,
            )
            assert preview_page_response.status_code == 200
            payload = preview_page_response.get_json()
            assert payload["success"] is True
            preview_rows = payload["data"]["preview"]
            actual_values = [row[response_field] for row in preview_rows]

            if response_field == "preview_amount":
                expected_values = sorted(actual_values, reverse=sort_direction == "desc")
            else:
                expected_values = sorted(actual_values, key=lambda value: str(value or "").casefold(), reverse=sort_direction == "desc")

            assert actual_values == expected_values

    def test_import_preview_recurring_match_rejects_stale_transfer_review_state(self, client):
        """transfer review 变化后，preview recurring bind 应拒绝旧 reviewStatus 快照。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_preview_recurring_transfer_stale")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        source_account = _ensure_test_account(client, isolated_auth_headers)
        category = _ensure_test_expense_category(client, isolated_auth_headers)

        from bill_analyser.api.app import db

        recurring_template = _create_test_recurring_template(
            client,
            isolated_auth_headers,
            name="pytest preview recurring direct stale transfer",
            account_id=source_account["id"],
            category_id=category["id"],
            amount_cents=9800,
            start_date="2026-03-08",
            frequency_type=1,
            frequency="1",
        )
        session_id = f"pytest-import-recurring-transfer-stale-{int(time.time() * 1000)}"

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-07-27 10:20:00",
                    "preview_type": "支出",
                    "preview_amount": 98.0,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "早餐",
                    "preview_counterparty": "pytest preview recurring direct stale vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest preview recurring direct stale preview",
                    "preview_recurring_id": recurring_template["id"],
                    "preview_recurring_name": recurring_template["name"],
                    "preview_recurring_candidate_count": 1,
                    "preview_recurring_match_score": 0.91,
                    "preview_recurring_match_reasons": "schedule|amount",
                    "preview_recurring_matched_date": "2026-07-27",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[181, 182],
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())
        category_id = _find_category_id_by_name("餐饮", "早餐", user_id=current_user_id)
        expected_state = {
            "sessionId": session_id,
            "reviewStatus": "pending",
            "previewType": "支出",
            "categoryId": category_id,
            "recurringId": recurring_template["id"],
        }

        transfer_reject_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps({"decision": "reject", "expectedState": expected_state}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert transfer_reject_response.status_code == 200

        stale_bind_response = client.put(
            f"/api/bills/import/v2/preview-item/{preview_id}/recurring-match",
            data=json.dumps({"recurringId": recurring_template["id"], "expectedState": expected_state}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert stale_bind_response.status_code == 409
        assert stale_bind_response.get_json()["error"] == "Preview state changed, please refresh"

    def test_import_preview_transfer_decision_accept_reject_clear_and_invalid_payload(self, client):
        """测试导入预览转账建议支持 accept/reject/clear，并返回刷新后的 matching 决策状态。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_transfer_decision")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        session_id = f"pytest-import-transfer-decision-{int(time.time() * 1000)}"

        from bill_analyser.api.app import db

        async def _ensure_breakfast_category_id() -> int:
            category = await db.get_category_by_name("餐饮", "早餐", user_id=current_user_id)
            if category and category.get("id"):
                return int(category["id"])

            category_id = await db.create_category(
                {
                    "type": 3,
                    "main_category": "餐饮",
                    "sub_category": "早餐",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": "",
                    "color": "",
                },
                user_id=current_user_id,
            )
            assert category_id is not None
            return int(category_id)

        async def _create_preview_item():
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            return await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-06-03 09:20:00",
                    "preview_type": "支出",
                    "preview_amount": 88.8,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "早餐",
                    "preview_counterparty": "pytest transfer vendor",
                    "preview_payment_method": "银行卡",
                    "preview_description": "pytest transfer decision",
                    "preview_recurring_id": 77,
                    "preview_recurring_name": "pytest recurring snapshot",
                    "preview_recurring_candidate_count": 1,
                    "preview_recurring_match_score": 0.88,
                    "preview_recurring_match_reasons": "amount|date",
                    "preview_recurring_matched_date": "2026-06-03",
                },
                user_id=current_user_id,
                dedup_type="transfer",
                dedup_source_ids=[91, 92],
            )

        breakfast_category_id = asyncio.run(_ensure_breakfast_category_id())
        preview_id = asyncio.run(_create_preview_item())

        def _build_expected_state(
            *,
            review_status: str,
            preview_type: str,
            category_id: int | None,
            recurring_id: int | None,
        ) -> dict[str, object]:
            return {
                "sessionId": session_id,
                "reviewStatus": review_status,
                "previewType": preview_type,
                "categoryId": category_id,
                "recurringId": recurring_id,
            }

        accept_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps(
                {
                    "decision": "accept",
                    "responseMode": "preview-item",
                    "expectedState": _build_expected_state(
                        review_status="pending",
                        preview_type="支出",
                        category_id=breakfast_category_id,
                        recurring_id=77,
                    ),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert accept_response.status_code == 200
        accept_data = accept_response.get_json()
        assert accept_data["success"] is True
        assert accept_data["data"]["preview"] == []
        accept_preview = accept_data["data"]["previewItem"]
        assert int(accept_preview["id"]) == int(preview_id)
        assert accept_preview["preview_type"] == "转账"
        assert accept_preview["preview_main_category"] == ""
        assert accept_preview["matching"]["transfer"]["review_status"] == "accepted"
        assert accept_preview["matching"]["transfer"]["reviewed_type"] == "转账"
        assert accept_preview["matching"]["transfer"]["suppressed"] is False

        stale_state_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps(
                {
                    "decision": "accept",
                    "expectedState": _build_expected_state(
                        review_status="pending",
                        preview_type="支出",
                        category_id=breakfast_category_id,
                        recurring_id=77,
                    ),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert stale_state_response.status_code == 409
        stale_state_data = stale_state_response.get_json()
        assert stale_state_data["success"] is False
        assert stale_state_data["error"] == "Preview state changed, please refresh"

        reject_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps(
                {
                    "decision": "reject",
                    "expectedState": _build_expected_state(
                        review_status="accepted",
                        preview_type="转账",
                        category_id=None,
                        recurring_id=None,
                    ),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert reject_response.status_code == 200
        reject_data = reject_response.get_json()
        reject_preview = next(
            item for item in reject_data["data"]["preview"] if int(item["id"]) == int(preview_id)
        )
        assert reject_preview["preview_type"] == "支出"
        assert reject_preview["preview_main_category"] == "餐饮"
        assert reject_preview["matching"]["transfer"]["review_status"] == "rejected"
        assert reject_preview["matching"]["transfer"]["suppressed"] is True

        clear_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps(
                {
                    "decision": "clear",
                    "expectedState": _build_expected_state(
                        review_status="rejected",
                        preview_type="支出",
                        category_id=breakfast_category_id,
                        recurring_id=77,
                    ),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert clear_response.status_code == 200
        clear_data = clear_response.get_json()
        clear_preview = next(
            item for item in clear_data["data"]["preview"] if int(item["id"]) == int(preview_id)
        )
        assert clear_preview["preview_type"] == "支出"
        assert clear_preview["preview_main_category"] == "餐饮"
        assert clear_preview["matching"]["transfer"]["review_status"] == "pending"
        assert clear_preview["matching"]["transfer"]["suppressed"] is False

        invalid_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps(
                {
                    "decision": "noop",
                    "expectedState": _build_expected_state(
                        review_status="pending",
                        preview_type="支出",
                        category_id=breakfast_category_id,
                        recurring_id=77,
                    ),
                }
            ),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert invalid_response.status_code == 400
        invalid_data = invalid_response.get_json()
        assert invalid_data["success"] is False
        assert invalid_data["error"] == "Invalid decision"

        invalid_shape_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps(["accept"]),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert invalid_shape_response.status_code == 400
        invalid_shape_data = invalid_shape_response.get_json()
        assert invalid_shape_data["success"] is False
        assert invalid_shape_data["error"] == "Invalid request"

        missing_expected_state_response = client.post(
            f"/api/bills/import/v2/preview-item/{preview_id}/transfer-decision",
            data=json.dumps({"decision": "accept"}),
            content_type="application/json",
            headers=isolated_auth_headers,
        )
        assert missing_expected_state_response.status_code == 400
        missing_expected_state_data = missing_expected_state_response.get_json()
        assert missing_expected_state_data["success"] is False
        assert missing_expected_state_data["error"] == "Invalid request"

    def test_import_preview_update_row_only_returns_preview_item_with_refreshed_learning_signal(self, client):
        """预览单条更新在 row-only 模式下应直接返回刷新后的 preview item。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_preview_update_row_only")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        session_id = f"pytest-import-preview-update-row-only-{int(time.time() * 1000)}"

        rule_id = _create_composite_import_learning_rule(
            current_user_id,
            parser_id="alipay",
            counterparty="星巴克咖啡",
            description="门店消费",
            payment_method="支付宝",
            learned_type="支出",
        )

        from bill_analyser.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                session_id,
                {
                    "preview_date": "2026-06-07 10:30:00",
                    "preview_type": "支出",
                    "preview_amount": 28.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_counterparty": "无关商户",
                    "preview_payment_method": "现金",
                    "preview_description": "无关备注",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())

        update_response = client.put(
            f"/api/bills/import/v2/preview/{session_id}/update",
            json={
                "id": preview_id,
                "counterparty": "星巴克",
                "paymentMethod": "支付宝",
                "description": "咖啡消费",
                "responseMode": "preview-item",
            },
            headers=isolated_auth_headers,
        )
        assert update_response.status_code == 200
        update_data = update_response.get_json()
        assert update_data["success"] is True
        assert update_data["data"]["updated"] is True
        preview_item = update_data["data"]["previewItem"]
        assert int(preview_item["id"]) == preview_id
        assert preview_item["preview_counterparty"] == "星巴克"
        assert preview_item["preview_payment_method"] == "支付宝"
        assert preview_item["preview_description"] == "咖啡消费"
        assert preview_item["matching"]["learning"]["rule_id"] == rule_id
        assert preview_item["matching"]["learning"]["review_status"] == "pending"

    def test_import_preview_update_rejects_preview_id_from_another_session(self, client):
        """session-scoped 预览更新不应接受同用户下其他 session 的 preview id。"""
        isolated_auth_headers = _build_isolated_auth_headers(client, "test_bills_api_preview_update_session_guard")
        current_user_id = _get_current_user_id(client, isolated_auth_headers)
        source_session_id = f"pytest-import-preview-update-source-{int(time.time() * 1000)}"
        other_session_id = f"pytest-import-preview-update-other-{int(time.time() * 1000)}"

        from bill_analyser.api.app import db

        async def _create_preview_item() -> int:
            await db.create_import_session(source_session_id, user_id=current_user_id, file_count=1)
            await db.create_import_session(other_session_id, user_id=current_user_id, file_count=1)
            preview_id = await db.insert_preview_bill(
                source_session_id,
                {
                    "preview_date": "2026-06-08 10:30:00",
                    "preview_type": "支出",
                    "preview_amount": 18.0,
                    "preview_main_category": "",
                    "preview_sub_category": "",
                    "preview_counterparty": "原始商户",
                    "preview_payment_method": "现金",
                    "preview_description": "原始备注",
                    "preview_parser_id": "alipay",
                },
                user_id=current_user_id,
            )
            return int(preview_id)

        preview_id = asyncio.run(_create_preview_item())

        update_response = client.put(
            f"/api/bills/import/v2/preview/{other_session_id}/update",
            json={
                "id": preview_id,
                "counterparty": "错误会话商户",
                "responseMode": "preview-item",
            },
            headers=isolated_auth_headers,
        )
        assert update_response.status_code == 404
        update_data = update_response.get_json()
        assert update_data["success"] is False
        assert update_data["error"] == "Preview bill not found"

        persisted_preview = asyncio.run(db.get_preview_bill_by_id(preview_id, user_id=current_user_id))
        assert persisted_preview is not None
        assert persisted_preview["session_id"] == source_session_id
        assert persisted_preview["preview_counterparty"] == "原始商户"

    def test_delete_bill(self, client, auth_headers):
        """测试删除账单"""
        # 先获取账单总数
        list_response = client.get("/api/bills/", headers=auth_headers)
        list_data = json.loads(list_response.data)
        original_total = list_data["result"]["total"]

        if original_total > 0:
            # 获取最后一个账单的ID
            last_response = client.get("/api/bills/?page_size=1", headers=auth_headers)
            last_data = json.loads(last_response.data)
            bill_id = last_data["result"]["items"][0]["id"]

            # 删除账单
            response = client.delete(f"/api/bills/{bill_id}", headers=auth_headers)
            assert response.status_code == 200

            data = json.loads(response.data)
            assert data["success"] is True
        else:
            pytest.skip("数据库中没有账单数据")

    def test_batch_delete(self, client, auth_headers):
        """测试批量删除账单"""
        # 获取一些账单ID
        list_response = client.get("/api/bills/?page_size=2", headers=auth_headers)
        list_data = json.loads(list_response.data)

        if len(list_data["result"]["items"]) >= 2:
            ids = [item["id"] for item in list_data["result"]["items"][:2]]

            response = client.delete(
                "/api/bills/batch/delete",
                data=json.dumps({"ids": ids}),
                content_type="application/json",
                headers=auth_headers
            )

            assert response.status_code == 200
            data = json.loads(response.data)
            assert data["success"] is True
            assert "deleted_count" in data["result"]
        else:
            pytest.skip("账单数量不足以进行批量删除测试")

    def test_batch_update_respects_user_scope_and_returns_failure_stats(self, client):
        """批量更新应尊重 user_id 隔离，并返回失败统计。"""
        primary_headers = _build_isolated_auth_headers(client, "test_bills_batch_update_primary")
        secondary_headers = _build_isolated_auth_headers(client, "test_bills_batch_update_secondary")
        primary_user_id = _get_current_user_id(client, primary_headers)
        secondary_user_id = _get_current_user_id(client, secondary_headers)

        primary_account = _ensure_test_account(client, primary_headers)
        primary_category = _ensure_test_expense_category(client, primary_headers)
        secondary_account = _ensure_test_account(client, secondary_headers)
        secondary_category = _ensure_test_expense_category(client, secondary_headers)

        unique_suffix = int(time.time() * 1000)
        updated_comment = f"pytest batch update success {unique_suffix}"
        primary_original_comment = f"pytest batch update self {unique_suffix}"
        secondary_original_comment = f"pytest batch update other {unique_suffix}"

        primary_bill = _create_test_bill_for_recurring(
            client,
            primary_headers,
            account_id=primary_account["id"],
            category_id=primary_category["id"],
            amount_cents=1234,
            bill_time_ms=int(datetime.now().timestamp() * 1000),
            comment=primary_original_comment
        )
        secondary_bill = _create_test_bill_for_recurring(
            client,
            secondary_headers,
            account_id=secondary_account["id"],
            category_id=secondary_category["id"],
            amount_cents=2345,
            bill_time_ms=int(datetime.now().timestamp() * 1000) + 60000,
            comment=secondary_original_comment
        )

        response = client.put(
            "/api/bills/batch/update",
            data=json.dumps({
                "ids": [primary_bill["id"], secondary_bill["id"]],
                "updates": {"description": updated_comment}
            }),
            content_type="application/json",
            headers=primary_headers
        )

        assert response.status_code == 200
        data = response.get_json()
        assert data["success"] is True
        assert data["result"]["updated_count"] == 1
        assert data["result"]["failed_count"] == 1
        assert [int(item) for item in data["result"]["failed_ids"]] == [int(secondary_bill["id"])]

        updated_primary_bill = _find_bill_by_comment(updated_comment, user_id=primary_user_id)
        untouched_secondary_bill = _find_bill_by_comment(secondary_original_comment, user_id=secondary_user_id)
        assert updated_primary_bill is not None
        assert str(updated_primary_bill["id"]) == str(primary_bill["id"])
        assert untouched_secondary_bill is not None
        assert str(untouched_secondary_bill["id"]) == str(secondary_bill["id"])

    def test_export_bills_csv(self, client, auth_headers):
        """测试导出CSV格式"""
        response = client.get("/api/bills/export?format=csv", headers=auth_headers)

        # 如果有数据，应该返回CSV文件
        if response.status_code == 200:
            assert response.content_type.startswith("text/csv")
        elif response.status_code == 404:
            # 没有数据也是正常的
            pass
        else:
            assert False, f"Unexpected status code: {response.status_code}"

    def test_export_bills_excel(self, client, auth_headers):
        """测试导出Excel格式"""
        response = client.get("/api/bills/export?format=excel", headers=auth_headers)

        # 可能因为缺少pandas而返回501
        if response.status_code == 200:
            assert "application" in response.content_type
        elif response.status_code in (404, 501):
            # 没有数据或不支持Excel也是正常的
            pass
        else:
            assert False, f"Unexpected status code: {response.status_code}"


class TestBillsAPIValidation:
    """账单API参数验证测试"""

    def test_get_bills_invalid_page(self, client, auth_headers):
        """测试无效的页码"""
        response = client.get("/api/bills/?page=-1", headers=auth_headers)
        # 应该返回错误或使用默认值
        assert response.status_code in (200, 400)

    def test_update_bill_empty_data(self, client, auth_headers):
        """测试空更新数据"""
        response = client.put(
            "/api/bills/1",
            data=json.dumps({}),
            content_type="application/json",
            headers=auth_headers
        )
        assert response.status_code in (400, 404)

    def test_batch_delete_empty_ids(self, client, auth_headers):
        """测试空ID列表"""
        response = client.delete(
            "/api/bills/batch",
            data=json.dumps({"ids": []}),
            content_type="application/json",
            headers=auth_headers
        )
        assert response.status_code in (400, 404, 405)
        if response.content_type and "application/json" in response.content_type:
            data = json.loads(response.data)
            assert data.get("success") is False

    def test_batch_update_rejects_protected_fields(self, client, auth_headers):
        """测试批量更新拒绝受保护字段。"""
        response = client.put(
            "/api/bills/batch/update",
            data=json.dumps({"ids": [1], "updates": {"user_id": 999}}),
            content_type="application/json",
            headers=auth_headers
        )
        assert response.status_code == 400
        data = json.loads(response.data)
        assert data["success"] is False
        assert data["error"] == "unsupported update fields: user_id"

    def test_batch_create_empty_transactions(self, client, auth_headers):
        """测试批量创建传空列表。"""
        response = client.post(
            "/api/bills/batch",
            data=json.dumps({"transactions": []}),
            content_type="application/json",
            headers=auth_headers
        )
        assert response.status_code == 400
        data = json.loads(response.data)
        assert data["success"] is False
