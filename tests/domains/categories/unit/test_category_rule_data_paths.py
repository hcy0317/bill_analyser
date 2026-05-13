from __future__ import annotations

import asyncio
import time
import uuid
from typing import Any

from bill_analyser.core.default_category_seed import ensure_default_category_seed
from tests.user_cleanup_support import register_test_user_for_cleanup


def _run(coroutine: Any) -> Any:
    """Run an async DB helper from sync tests."""
    return asyncio.run(coroutine)


def test_category_rule_migration_is_idempotent_and_user_scoped(
    client: Any,
    auth_context: dict[str, Any],
    db_instance: Any,
) -> None:
    """Legacy keywords still migrate through the canonical DB helper after Flask route deletion."""
    suffix = f"{int(time.time() * 1000)}_{uuid.uuid4().hex[:8]}"
    user_id = int(auth_context["user"]["id"])

    other_username = f"test_slice04a_other_{suffix}"
    other_password = "Test123456!"
    other_register = client.post(
        "/api/auth/register",
        json={
            "username": other_username,
            "email": f"{other_username}@example.com",
            "password": other_password,
            "nickname": other_username,
        },
    )
    assert other_register.status_code in (200, 201), other_register.get_data(as_text=True)
    register_test_user_for_cleanup(db_instance, other_username)
    other_user = _run(db_instance.get_user_by_username(other_username))
    other_user_id = int(other_user["id"])

    migrated_category_id = _run(
        db_instance.create_category(
            {
                "main_category": f"迁移测试{suffix}",
                "sub_category": "咖啡",
                "type": 3,
                "priority": 11,
                "keywords": "OR:星巴克|咖啡&AND:早餐&NOT:退款",
            },
            user_id=user_id,
        )
    )
    assert migrated_category_id is not None

    special_keyword_category_id = _run(
        db_instance.create_category(
            {
                "main_category": f"特殊字符迁移测试{suffix}",
                "sub_category": "完整字面量",
                "type": 3,
                "priority": 10,
                "keywords": "商户A,咖啡+拿铁{热}|杯",
            },
            user_id=user_id,
        )
    )
    assert special_keyword_category_id is not None

    existing_rule_category_id = _run(
        db_instance.create_category(
            {
                "main_category": f"迁移测试{suffix}",
                "sub_category": "已有规则",
                "type": 3,
                "priority": 12,
                "keywords": "OR:不应重复迁移",
            },
            user_id=user_id,
        )
    )
    assert existing_rule_category_id is not None
    existing_rule_id = _run(
        db_instance.create_category_rule(
            {
                "category_id": existing_rule_category_id,
                "name": "manual existing rule",
                "priority": 3,
                "rule_expression": "OR={手工规则}",
                "regex_enabled": False,
                "enabled": True,
            },
            user_id=user_id,
        )
    )
    assert existing_rule_id is not None

    other_category_id = _run(
        db_instance.create_category(
            {
                "main_category": f"跨用户迁移测试{suffix}",
                "sub_category": "隔离",
                "type": 3,
                "priority": 13,
                "keywords": "OR:跨用户关键词",
            },
            user_id=other_user_id,
        )
    )
    assert other_category_id is not None

    assert _run(db_instance.migrate_keywords_to_rules(user_id=user_id)) == {
        "migrated": 3,
        "skipped": 0,
    }

    migrated_rules = _run(
        db_instance.get_category_rules(
            user_id=user_id,
            category_id=migrated_category_id,
            enabled_only=False,
        )
    )
    assert [rule["rule_expression"] for rule in migrated_rules] == [
        "OR={星巴克,咖啡}+AND={早餐}+NOT={退款}",
    ]

    special_keyword_rules = _run(
        db_instance.get_category_rules(
            user_id=user_id,
            category_id=special_keyword_category_id,
            enabled_only=False,
        )
    )
    assert [rule["rule_expression"] for rule in special_keyword_rules] == [
        r"OR={商户A\,咖啡\+拿铁\{热\}\|杯}",
    ]

    existing_rules = _run(
        db_instance.get_category_rules(
            user_id=user_id,
            category_id=existing_rule_category_id,
            enabled_only=False,
        )
    )
    assert [rule["rule_expression"] for rule in existing_rules] == [
        "OR={手工规则}",
        "OR={不应重复迁移}",
    ]
    migrated_existing_rule = next(
        rule for rule in existing_rules if rule["id"] != existing_rule_id
    )
    assert migrated_existing_rule["name"] == f"migrated:迁移测试{suffix}/已有规则"

    other_user_rules = _run(
        db_instance.get_category_rules(
            user_id=other_user_id,
            category_id=other_category_id,
            enabled_only=False,
        )
    )
    assert other_user_rules == []

    migrated_rule_id = migrated_rules[0]["id"]
    assert _run(db_instance.delete_category_rule(migrated_rule_id, user_id=user_id)) is True
    assert _run(db_instance.migrate_keywords_to_rules(user_id=user_id)) == {
        "migrated": 1,
        "skipped": 2,
    }
    assert _run(db_instance.migrate_keywords_to_rules(user_id=user_id)) == {
        "migrated": 0,
        "skipped": 3,
    }

    engine = client.application.config["CATEGORY_ENGINE_INSTANCE"]
    _run(engine.load_rules_from_db(db_instance, user_id=user_id))
    assert engine.match_category(
        {
            "counterparty": "商户A,咖啡+拿铁{热}|杯",
            "description": "",
            "type": "支出",
            "amount": -29.0,
        }
    ) == (f"特殊字符迁移测试{suffix}", "完整字面量")


def test_category_rule_migration_imports_investment_settings_as_rule_expression(
    client: Any,
    auth_context: dict[str, Any],
    db_instance: Any,
) -> None:
    """Investment recognition settings should migrate into canonical category rules."""
    suffix = f"{int(time.time() * 1000)}_{uuid.uuid4().hex[:8]}"
    user_id = int(auth_context["user"]["id"])

    investment_category_id = _run(
        db_instance.create_category(
            {
                "main_category": f"投资理财{suffix}",
                "sub_category": "基金",
                "type": 5,
                "priority": 8,
                "keywords": "",
            },
            user_id=user_id,
        )
    )
    assert investment_category_id is not None

    updated_settings = _run(
        db_instance.update_pairing_investment_settings(
            user_id=user_id,
            investment_platform_keywords=["蚂蚁财富", "天天基金", "蚂蚁财富"],
            investment_product_keywords=["基金", "ETF"],
            investment_exclude_keywords=["还款", "账单"],
        )
    )
    assert updated_settings is not None

    assert _run(db_instance.migrate_keywords_to_rules(user_id=user_id)) == {
        "migrated": 1,
        "skipped": 0,
    }

    investment_rules = _run(
        db_instance.get_category_rules(
            user_id=user_id,
            category_id=investment_category_id,
            enabled_only=False,
        )
    )
    assert [rule["name"] for rule in investment_rules] == [
        "migrated:investment-recognition",
    ]
    assert [rule["rule_expression"] for rule in investment_rules] == [
        "OR={蚂蚁财富,天天基金}+OR={基金,ETF}+NOT={还款,账单}",
    ]

    engine = client.application.config["CATEGORY_ENGINE_INSTANCE"]
    _run(engine.load_rules_from_db(db_instance, user_id=user_id))
    assert engine.match_category(
        {
            "counterparty": "蚂蚁财富",
            "description": "沪深300ETF 买入",
            "type": "支出",
            "amount": -100.0,
        }
    ) == (f"投资理财{suffix}", "基金")
    assert engine.match_category(
        {
            "counterparty": "蚂蚁财富",
            "description": "信用卡账单 ETF",
            "type": "支出",
            "amount": -100.0,
        }
    ) == (None, None)

    assert _run(db_instance.migrate_keywords_to_rules(user_id=user_id)) == {
        "migrated": 0,
        "skipped": 1,
    }


def test_category_rule_default_seed_is_idempotent_after_registration(
    client: Any,
    db_instance: Any,
) -> None:
    """Daily defaults remain present and idempotent through the canonical seed helper."""
    suffix = f"{int(time.time() * 1000)}_{uuid.uuid4().hex[:8]}"
    username = f"test_defaults_{suffix}"
    password = "Test123456!"
    register_response = client.post(
        "/api/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": password,
            "nickname": username,
            "categories": [
                {
                    "name": f"已有分类{suffix}",
                    "type": 3,
                    "subCategories": [],
                }
            ],
        },
    )
    assert register_response.status_code in (200, 201), register_response.get_data(as_text=True)
    register_test_user_for_cleanup(db_instance, username)

    user = _run(db_instance.get_user_by_username(username))
    user_id = int(user["id"])

    result = _run(ensure_default_category_seed(db_instance, user_id=user_id))
    assert result["categories"]["created"] == 0
    assert result["categories"]["skipped"] >= 80
    assert result["rules"]["created"] == 0
    assert result["rules"]["skipped"] >= 30
    assert result["rules"]["missingCategories"] == 0

    food_delivery = _run(db_instance.get_category_by_name("餐饮", "外卖", user_id=user_id))
    salary = _run(db_instance.get_category_by_name("工作收入", "工资", user_id=user_id))
    transfer = _run(
        db_instance.get_category_by_name("账户互转", "信用卡还款", user_id=user_id)
    )
    assert food_delivery is not None
    assert salary is not None
    assert transfer is not None

    delivery_rules = _run(
        db_instance.get_category_rules(
            user_id=user_id,
            category_id=food_delivery["id"],
            enabled_only=False,
        )
    )
    assert [rule["name"] for rule in delivery_rules] == ["default:餐饮/外卖"]

    engine = client.application.config["CATEGORY_ENGINE_INSTANCE"]
    _run(engine.load_rules_from_db(db_instance, user_id=user_id))
    assert engine.match_category(
        {
            "counterparty": "美团外卖",
            "description": "订单-黄焖鸡米饭",
            "type": "支出",
            "amount": -28.5,
        }
    ) == ("餐饮", "外卖")
    assert engine.match_category(
        {
            "counterparty": "招商银行",
            "description": "工资代发",
            "type": "收入",
            "amount": 18000,
        }
    ) == ("工作收入", "工资")

    repeat_result = _run(ensure_default_category_seed(db_instance, user_id=user_id))
    assert repeat_result["categories"]["created"] == 0
    assert repeat_result["rules"]["created"] == 0
    assert repeat_result["rules"]["missingCategories"] == 0
