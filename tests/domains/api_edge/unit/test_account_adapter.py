from __future__ import annotations

import copy

import pytest

from bill_analyser.api.adapters.account_adapter import AccountAdapter, _parse_aliases


@pytest.mark.parametrize(
    ("raw_value", "expected_aliases"),
    [
        (None, []),
        ("", []),
        ([], []),
        ([" 主卡 ", "", "工资卡"], ["主卡", "工资卡"]),
        ('["主卡", " 备用卡 ", ""]', ["主卡", "备用卡"]),
        ("主卡, 备用卡, , 工资卡", ["主卡", "备用卡", "工资卡"]),
        ("[broken-json", ["[broken-json"]),
        (123, []),
    ],
)
def test_parse_aliases_supports_lists_json_and_comma_text(raw_value: object, expected_aliases: list[str]) -> None:
    """账户别名解析应兼容多种输入形态。"""
    assert _parse_aliases(raw_value) == expected_aliases



def test_frontend_to_backend_transforms_balance_aliases_and_visibility() -> None:
    """前端到账户后端模型的转换应保持字段契约清晰。"""
    adapter = AccountAdapter()
    frontend_payload = {
        "name": "招商银行卡",
        "parent_id": "7",
        "category": 2,
        "type": 1,
        "icon": "bank",
        "color": "#0088ff",
        "currency": "CNY",
        "initial_balance": "12345",
        "comment": "工资卡",
        "aliases": [" 主卡 ", "", "备用卡"],
        "display_order": 9,
        "visible": False,
        "creditCardStatementDate": 18,
    }
    original_payload = copy.deepcopy(frontend_payload)

    backend_payload = adapter.frontend_to_backend(frontend_payload)

    assert backend_payload == {
        "name": "招商银行卡",
        "parent_id": 7,
        "category": 2,
        "type": 1,
        "icon": "bank",
        "color": "#0088ff",
        "currency": "CNY",
        "balance": 123.45,
        "initial_balance": 123.45,
        "comment": "工资卡",
        "aliases": '["主卡", "备用卡"]',
        "display_order": 9,
        "hidden": True,
        "subAccounts": [],
        "credit_card_statement_date": 18,
    }
    assert frontend_payload == original_payload



def test_backend_to_frontend_recurses_sub_accounts_and_parses_aliases() -> None:
    """后端账户应转换为前端结构，并递归处理子账户。"""
    adapter = AccountAdapter()
    backend_account = {
        "id": 1,
        "name": "招商银行卡",
        "parent_id": 0,
        "category": 2,
        "type": 1,
        "icon": "bank",
        "color": "#0088ff",
        "currency": "CNY",
        "initial_balance": 88.88,
        "comment": "工资卡",
        "aliases": '[" 招商 ", "", "工资卡 "]',
        "credit_card_statement_date": 25,
        "display_order": 3,
        "hidden": 1,
        "subAccounts": [
            {
                "id": 2,
                "name": "招商附属卡",
                "parent_id": 1,
                "category": 2,
                "type": 0,
                "currency": "CNY",
                "balance": 5.01,
                "aliases": "备用, 零钱",
                "hidden": 0,
            }
        ],
    }

    frontend_account = adapter.backend_to_frontend(backend_account)

    assert frontend_account["id"] == "1"
    assert frontend_account["parentId"] == "0"
    assert frontend_account["balance"] == 8888
    assert frontend_account["aliases"] == ["招商", "工资卡"]
    assert frontend_account["creditCardStatementDate"] == 25
    assert frontend_account["hidden"] is True
    assert frontend_account["visible"] is False
    assert frontend_account["subAccounts"][0]["id"] == "2"
    assert frontend_account["subAccounts"][0]["parentId"] == "1"
    assert frontend_account["subAccounts"][0]["aliases"] == ["备用", "零钱"]



def test_format_list_response_builds_hierarchy_and_can_skip_grouping() -> None:
    """列表响应应既支持层级树，也支持保留平铺结果。"""
    adapter = AccountAdapter()
    backend_accounts = [
        {
            "id": 1,
            "name": "主账户",
            "parent_id": 0,
            "currency": "CNY",
            "balance": 10.0,
            "aliases": [],
            "hidden": 0,
        },
        {
            "id": 2,
            "name": "子账户",
            "parent_id": 1,
            "currency": "CNY",
            "balance": 5.0,
            "aliases": [],
            "hidden": 0,
        },
    ]

    hierarchical = adapter.format_list_response(backend_accounts, build_hierarchy_flag=True)
    flat = adapter.format_list_response(backend_accounts, build_hierarchy_flag=False)

    assert hierarchical["success"] is True
    assert len(hierarchical["result"]) == 1
    assert hierarchical["result"][0]["id"] == "1"
    assert hierarchical["result"][0]["subAccounts"][0]["id"] == "2"

    assert flat["success"] is True
    assert len(flat["result"]) == 2
    assert [account["id"] for account in flat["result"]] == ["1", "2"]
