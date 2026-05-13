"""Regression tests for unified JSON settings bundle import/export."""

from __future__ import annotations

import asyncio
import json
import time
from typing import Any

from tests.new_ui.test_bills_api import (
    _build_isolated_auth_headers,
    _create_account_via_db,
    _ensure_test_expense_category,
    _get_current_user_id,
)

ISOLATED_USER_PASSWORD = "Test123456!"


def _create_account(client, auth_headers, *, name: str) -> dict[str, Any]:
    return _create_account_via_db(
        client,
        auth_headers,
        {
            "name": name,
            "category": 1,
            "type": 1,
            "currency": "CNY",
            "icon": "1",
            "color": "00ccff",
            "balance": 88.5,
            "initialBalance": 88.5,
            "comment": "pytest settings bundle account",
            "hidden": False,
            "aliases": ["settings-bundle-alias"],
        },
    )


def _create_tag(client, auth_headers, *, name: str) -> dict[str, Any]:
    from bill_analyser.api.app import db

    user_id = _get_current_user_id(client, auth_headers)

    async def _create() -> dict[str, Any]:
        tag_id = await db.create_tag(
            {"name": name, "color": "#11aa33", "icon": "tag", "hidden": False},
            user_id=user_id,
        )
        tag = await db.get_tag_by_id(tag_id, user_id=user_id)
        assert tag is not None
        return tag

    return asyncio.run(_create())


def _create_template(
    client,
    auth_headers,
    *,
    name: str,
    template_type: int,
    account_id: str,
    category_id: str,
    tag_id: str,
) -> dict[str, Any]:
    payload = {
        "templateType": template_type,
        "name": name,
        "description": "pytest settings bundle template",
        "type": 3,
        "categoryId": category_id,
        "sourceAccountId": account_id,
        "destinationAccountId": "0",
        "sourceAmount": 1234,
        "destinationAmount": 0,
        "hideAmount": False,
        "tagIds": [tag_id],
        "comment": "pytest settings bundle",
        "displayOrder": 4,
        "hidden": False,
        "utcOffset": 480,
    }
    if template_type == 2:
        payload.update(
            {
                "scheduledFrequencyType": 2,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "scheduledEndDate": "",
            }
        )

    from bill_analyser.api.app import db

    user_id = _get_current_user_id(client, auth_headers)

    async def _create() -> dict[str, Any]:
        template_id = await db.create_template(payload, user_id=user_id)
        template = await db.get_template_by_id(
            template_id,
            user_id=user_id,
            template_type=template_type,
        )
        assert template is not None
        return template

    return asyncio.run(_create())


def _create_category_rule_via_db(
    client,
    auth_headers,
    *,
    category_id: int,
    name: str,
) -> int:
    """Create a category rule directly after the Flask category-rules route shell is removed."""
    from bill_analyser.api.app import db

    user_id = _get_current_user_id(client, auth_headers)

    async def _create() -> int:
        rule_id = await db.create_category_rule(
            {
                "category_id": category_id,
                "name": name,
                "priority": 5,
                "rule_expression": "OR={pytest-settings-bundle}",
                "regex_enabled": False,
                "enabled": True,
            },
            user_id=user_id,
        )
        assert rule_id is not None
        return int(rule_id)

    return asyncio.run(_create())


def test_settings_bundle_export_redacts_llm_and_contains_requested_sections(client):
    suffix = int(time.time() * 1000)
    auth_headers = _build_isolated_auth_headers(client, "test_settings_bundle_export")
    category = _ensure_test_expense_category(client, auth_headers)
    account = _create_account(client, auth_headers, name=f"bundle-account-{suffix}")
    tag = _create_tag(client, auth_headers, name=f"bundle-tag-{suffix}")
    _create_template(
        client,
        auth_headers,
        name=f"bundle-template-{suffix}",
        template_type=1,
        account_id=str(account["id"]),
        category_id=str(category["id"]),
        tag_id=str(tag["id"]),
    )
    _create_template(
        client,
        auth_headers,
        name=f"bundle-recurring-{suffix}",
        template_type=2,
        account_id=str(account["id"]),
        category_id=str(category["id"]),
        tag_id=str(tag["id"]),
    )
    _create_category_rule_via_db(
        client,
        auth_headers,
        category_id=int(category["id"]),
        name=f"bundle-rule-{suffix}",
    )
    secret = "sk-settings-bundle-secret-should-not-export"
    llm_response = client.post(
        "/api/llm/configs",
        headers=auth_headers,
        json={
            "name": f"bundle-llm-{suffix}",
            "provider": "openai",
            "model": "gpt-settings-bundle",
            "api_key": secret,
            "base_url": "https://example.test/v1",
            "is_active": True,
            "advanced_settings": {"reasoning_depth": "high"},
        },
    )
    assert llm_response.status_code == 200, llm_response.get_data(as_text=True)
    ocr_response = client.put(
        "/api/ml/receipt-recognition/config",
        headers=auth_headers,
        json={"provider": "cloud_stub", "lang": "eng"},
    )
    assert ocr_response.status_code == 200, ocr_response.get_data(as_text=True)

    export_response = client.get("/api/settings/bundle/export", headers=auth_headers)

    assert export_response.status_code == 200, export_response.get_data(as_text=True)
    assert secret not in export_response.get_data(as_text=True)
    bundle = export_response.get_json()
    assert bundle["schemaVersion"] == 1
    for section_name in (
        "accounts",
        "transactionCategories",
        "transactionTags",
        "transactionTemplates",
        "scheduledTransactions",
        "categoryRecognitionRules",
        "llmConfigs",
        "ocrConfig",
    ):
        assert section_name in bundle["sections"]
        assert bundle["counts"][section_name] >= 1

        if section_name in {"llmConfigs", "ocrConfig"}:
            get_without_password = client.get(
                f"/api/settings/bundle/sections/{section_name}/export",
                headers=auth_headers,
            )
            assert get_without_password.status_code == 400

            bad_password = client.post(
                f"/api/settings/bundle/sections/{section_name}/export",
                headers=auth_headers,
                json={"password": "wrong-password"},
            )
            assert bad_password.status_code == 401

            section_export_response = client.post(
                f"/api/settings/bundle/sections/{section_name}/export",
                headers=auth_headers,
                json={"password": ISOLATED_USER_PASSWORD},
            )
        else:
            section_export_response = client.get(
                f"/api/settings/bundle/sections/{section_name}/export",
                headers=auth_headers,
            )
        assert section_export_response.status_code == 200, (
            section_export_response.get_data(as_text=True)
        )
        section_bundle = section_export_response.get_json()
        assert section_bundle["schemaVersion"] == 1
        assert list(section_bundle["sections"]) == [section_name]
        assert list(section_bundle["counts"]) == [section_name]

    llm_config = next(
        item for item in bundle["sections"]["llmConfigs"]
        if item["name"] == f"bundle-llm-{suffix}"
    )
    assert llm_config["apiKey"] == ""
    assert llm_config["hasApiKey"] is True
    assert llm_config["activeInSource"] is True
    assert bundle["sections"]["ocrConfig"][0]["provider"] == "cloud_stub"
    assert bundle["sections"]["ocrConfig"][0]["lang"] == "eng"

    llm_section_response = client.post(
        "/api/settings/bundle/sections/llmConfigs/export",
        headers=auth_headers,
        json={"password": ISOLATED_USER_PASSWORD},
    )
    assert llm_section_response.status_code == 200, llm_section_response.get_data(as_text=True)
    assert secret not in llm_section_response.get_data(as_text=True)


def test_settings_bundle_section_import_only_writes_requested_section(client):
    suffix = int(time.time() * 1000)
    auth_headers = _build_isolated_auth_headers(
        client, "test_settings_bundle_section_import_scope"
    )
    user_id = _get_current_user_id(client, auth_headers)
    bundle = {
        "schemaVersion": 1,
        "sections": {
            "accounts": [
                {
                    "externalRef": "account:ignored",
                    "name": f"section-ignored-account-{suffix}",
                    "type": 1,
                    "currency": "CNY",
                    "balance": 1,
                    "initialBalance": 1,
                }
            ],
            "transactionTags": [
                {
                    "externalRef": "tag:section",
                    "name": f"section-import-tag-{suffix}",
                    "color": "#224466",
                    "icon": "tag",
                }
            ],
        },
    }

    preview_response = client.post(
        "/api/settings/bundle/sections/transactionTags/import/preview",
        headers=auth_headers,
        json=bundle,
    )
    assert preview_response.status_code == 200, preview_response.get_data(as_text=True)
    preview = preview_response.get_json()["result"]
    assert preview["dryRun"] is True
    assert preview["sections"]["transactionTags"]["created"] == 1
    assert preview["sections"]["accounts"]["created"] == 0

    import_response = client.post(
        "/api/settings/bundle/sections/transactionTags/import",
        headers=auth_headers,
        json=bundle,
    )
    assert import_response.status_code == 200, import_response.get_data(as_text=True)
    result = import_response.get_json()["result"]
    assert result["sections"]["transactionTags"]["created"] == 1
    assert result["sections"]["accounts"]["created"] == 0

    from bill_analyser.api import app as api_app

    async def _count_rows() -> dict[str, int]:
        conn = await api_app.db._get_connection()
        checks = {
            "accounts": ("accounts", "name", f"section-ignored-account-{suffix}"),
            "tags": ("tags", "name", f"section-import-tag-{suffix}"),
        }
        counts = {}
        for key, (table, column, value) in checks.items():
            async with conn.execute(
                f"SELECT COUNT(*) FROM {table} WHERE user_id = ? AND {column} = ?",
                (user_id, value),
            ) as cursor:
                row = await cursor.fetchone()
            counts[key] = int(row[0])
        return counts

    assert asyncio.run(_count_rows()) == {"accounts": 0, "tags": 1}

    for method, url in (
        ("get", "/api/settings/bundle/sections/notASection/export"),
        ("post", "/api/settings/bundle/sections/notASection/import/preview"),
        ("post", "/api/settings/bundle/sections/notASection/import"),
    ):
        kwargs = {"headers": auth_headers}
        if method == "post":
            kwargs["json"] = bundle
        response = getattr(client, method)(url, **kwargs)
        assert response.status_code == 404


def test_settings_bundle_import_preview_is_dry_run_and_import_upserts(client):
    suffix = int(time.time() * 1000)
    auth_headers = _build_isolated_auth_headers(client, "test_settings_bundle_import")
    user_id = _get_current_user_id(client, auth_headers)
    bundle = {
        "schemaVersion": 1,
        "sections": {
            "accounts": [
                {
                    "externalRef": "account:source",
                    "name": f"import-account-{suffix}",
                    "type": 1,
                    "currency": "CNY",
                    "balance": 10.25,
                    "initialBalance": 10.25,
                    "aliases": ["import-alias"],
                }
            ],
            "transactionCategories": [
                {
                    "externalRef": "category:coffee",
                    "type": 3,
                    "mainCategory": f"import-main-{suffix}",
                    "subCategory": "coffee",
                    "priority": 3,
                    "keywords": "",
                }
            ],
            "transactionTags": [
                {
                    "externalRef": "tag:work",
                    "name": f"import-tag-{suffix}",
                    "color": "#224466",
                    "icon": "tag",
                }
            ],
            "transactionTemplates": [
                {
                    "name": f"import-template-{suffix}",
                    "templateType": 1,
                    "type": 3,
                    "categoryRef": "category:coffee",
                    "sourceAccountRef": "account:source",
                    "sourceAmount": 66,
                    "tagRefs": ["tag:work"],
                }
            ],
            "scheduledTransactions": [
                {
                    "name": f"import-scheduled-{suffix}",
                    "templateType": 2,
                    "type": 3,
                    "categoryRef": "category:coffee",
                    "sourceAccountRef": "account:source",
                    "sourceAmount": 88,
                    "scheduledFrequencyType": 2,
                    "scheduledFrequency": "1",
                    "scheduledStartDate": "2026-07-01",
                    "tagRefs": ["tag:work"],
                }
            ],
            "categoryRecognitionRules": [
                {
                    "categoryRef": "category:coffee",
                    "name": f"import-rule-{suffix}",
                    "priority": 2,
                    "ruleExpression": "OR={pytest-import-bundle}",
                    "regexEnabled": False,
                    "enabled": True,
                }
            ],
            "llmConfigs": [
                {
                    "name": f"import-llm-{suffix}",
                    "provider": "openai",
                    "model": "gpt-imported",
                    "apiKey": "sk-imported-secret",
                    "baseUrl": "https://example.test/v1",
                    "advancedSettings": {"reasoning_depth": "medium"},
                    "isActive": True,
                }
            ],
            "ocrConfig": [
                {
                    "externalRef": "ocrConfig:receipt-recognition",
                    "provider": "cloud_stub",
                    "lang": "eng",
                }
            ],
        },
    }

    preview_response = client.post(
        "/api/settings/bundle/import/preview",
        headers=auth_headers,
        json=bundle,
    )
    assert preview_response.status_code == 200, preview_response.get_data(as_text=True)
    preview = preview_response.get_json()["result"]
    assert preview["dryRun"] is True
    assert preview["sections"]["accounts"]["created"] == 1

    from bill_analyser.api import app as api_app

    async def _count_imported_rows() -> dict[str, int]:
        conn = await api_app.db._get_connection()
        checks = {
            "accounts": ("accounts", "name", f"import-account-{suffix}"),
            "categories": ("categories", "main_category", f"import-main-{suffix}"),
            "tags": ("tags", "name", f"import-tag-{suffix}"),
            "llm": ("llm_configs", "name", f"import-llm-{suffix}"),
        }
        counts = {}
        for key, (table, column, value) in checks.items():
            async with conn.execute(
                f"SELECT COUNT(*) FROM {table} WHERE user_id = ? AND {column} = ?",
                (user_id, value),
            ) as cursor:
                row = await cursor.fetchone()
            counts[key] = int(row[0])
        return counts

    assert asyncio.run(_count_imported_rows()) == {
        "accounts": 0,
        "categories": 0,
        "tags": 0,
        "llm": 0,
    }

    api_app.app.config["OCR_SERVICE"] = object()
    import_response = client.post(
        "/api/settings/bundle/import",
        headers=auth_headers,
        json=bundle,
    )
    assert import_response.status_code == 200, import_response.get_data(as_text=True)
    imported = import_response.get_json()["result"]
    assert imported["dryRun"] is False
    assert imported["sections"]["categoryRecognitionRules"]["created"] == 1
    assert imported["sections"]["ocrConfig"]["created"] + imported["sections"]["ocrConfig"]["updated"] == 1
    assert "OCR_SERVICE" not in api_app.app.config

    async def _fetch_imported_state() -> dict[str, Any]:
        conn = await api_app.db._get_connection()
        configs = await api_app.db.get_llm_configs(user_id=user_id)
        templates = await api_app.db.get_all_templates(user_id=user_id, template_type=1)
        scheduled = await api_app.db.get_all_templates(user_id=user_id, template_type=2)
        async with conn.execute(
            "SELECT balance, initial_balance FROM accounts WHERE user_id = ? AND name = ?",
            (user_id, f"import-account-{suffix}"),
        ) as cursor:
            account = dict(await cursor.fetchone())
        async with conn.execute(
            "SELECT amount FROM bill_templates WHERE user_id = ? AND name = ?",
            (user_id, f"import-template-{suffix}"),
        ) as cursor:
            template_row = dict(await cursor.fetchone())
        async with conn.execute(
            """
            SELECT cr.*, c.main_category, c.sub_category
            FROM category_rules cr
            JOIN categories c ON c.id = cr.category_id
            WHERE cr.user_id = ? AND cr.name = ?
            """,
            (user_id, f"import-rule-{suffix}"),
        ) as cursor:
            rule = dict(await cursor.fetchone())
        return {
            "llm": next(item for item in configs if item["name"] == f"import-llm-{suffix}"),
            "templates": [item for item in templates if item["name"] == f"import-template-{suffix}"],
            "scheduled": [item for item in scheduled if item["name"] == f"import-scheduled-{suffix}"],
            "account": account,
            "templateRow": template_row,
            "rule": rule,
            "ocr": await api_app.db.get_app_setting("receipt_ocr_config"),
        }

    imported_state = asyncio.run(_fetch_imported_state())
    assert imported_state["llm"]["api_key"] == "sk-imported-secret"
    assert imported_state["llm"]["is_active"] == 0
    assert imported_state["templates"][0]["sourceAccountId"] != "0"
    assert imported_state["scheduled"][0]["scheduledStartDate"] == "2026-07-01"
    assert imported_state["account"]["balance"] == 10.25
    assert imported_state["account"]["initial_balance"] == 10.25
    assert imported_state["templateRow"]["amount"] == 66
    assert imported_state["rule"]["main_category"] == f"import-main-{suffix}"
    assert json.loads(imported_state["ocr"]) == {"provider": "cloud_stub", "lang": "eng"}

    second_import = client.post(
        "/api/settings/bundle/import",
        headers=auth_headers,
        json=bundle,
    )
    assert second_import.status_code == 200, second_import.get_data(as_text=True)
    second_result = second_import.get_json()["result"]
    assert second_result["sections"]["accounts"]["updated"] == 1
    assert second_result["sections"]["accounts"]["created"] == 0


def test_settings_bundle_import_rejects_unsupported_schema_version(client):
    auth_headers = _build_isolated_auth_headers(
        client, "test_settings_bundle_import_bad_schema"
    )
    for bad_schema_version in (999, "1", True, 1.5):
        bundle = {"schemaVersion": bad_schema_version, "sections": {"accounts": []}}

        preview_response = client.post(
            "/api/settings/bundle/import/preview",
            headers=auth_headers,
            json=bundle,
        )
        import_response = client.post(
            "/api/settings/bundle/import",
            headers=auth_headers,
            json=bundle,
        )

        assert preview_response.status_code == 400
        assert import_response.status_code == 400
        assert "Unsupported settings bundle schemaVersion" in preview_response.get_json()["error"]
        assert "Unsupported settings bundle schemaVersion" in import_response.get_json()["error"]


def test_settings_bundle_import_preserves_existing_active_llm_config(client):
    suffix = int(time.time() * 1000)
    auth_headers = _build_isolated_auth_headers(
        client, "test_settings_bundle_import_active_llm"
    )
    user_id = _get_current_user_id(client, auth_headers)
    config_name = f"active-import-llm-{suffix}"
    create_response = client.post(
        "/api/llm/configs",
        headers=auth_headers,
        json={
            "name": config_name,
            "provider": "openai",
            "model": "gpt-original",
            "api_key": "sk-original-secret",
            "base_url": "https://original.test/v1",
            "advanced_settings": {"reasoning_depth": "low"},
            "is_active": True,
        },
    )
    assert create_response.status_code == 200, create_response.get_data(as_text=True)

    bundle = {
        "schemaVersion": 1,
        "sections": {
            "llmConfigs": [
                {
                    "name": config_name,
                    "provider": "openai",
                    "model": "gpt-updated",
                    "apiKey": "",
                    "baseUrl": "https://updated.test/v1",
                    "advancedSettings": {"reasoning_depth": "high"},
                }
            ]
        },
    }
    import_response = client.post(
        "/api/settings/bundle/import",
        headers=auth_headers,
        json=bundle,
    )
    assert import_response.status_code == 200, import_response.get_data(as_text=True)
    result = import_response.get_json()["result"]
    assert result["sections"]["llmConfigs"]["updated"] == 1

    from bill_analyser.api import app as api_app

    async def _fetch_config() -> dict[str, Any]:
        configs = await api_app.db.get_llm_configs(user_id=user_id)
        return next(item for item in configs if item["name"] == config_name)

    config = asyncio.run(_fetch_config())
    assert config["api_key"] == "sk-original-secret"
    assert config["model"] == "gpt-updated"
    assert config["is_active"] == 1


def test_settings_bundle_import_deduplicates_llm_configs_within_same_bundle(client):
    suffix = int(time.time() * 1000)
    auth_headers = _build_isolated_auth_headers(
        client, "test_settings_bundle_import_duplicate_llm"
    )
    user_id = _get_current_user_id(client, auth_headers)
    config_name = f"duplicate-import-llm-{suffix}"
    bundle = {
        "schemaVersion": 1,
        "sections": {
            "llmConfigs": [
                {
                    "name": config_name,
                    "provider": "openai",
                    "model": "gpt-first",
                    "apiKey": "sk-first",
                    "baseUrl": "https://first.test/v1",
                },
                {
                    "name": config_name,
                    "provider": "openai",
                    "model": "gpt-second",
                    "apiKey": "",
                    "baseUrl": "https://second.test/v1",
                },
            ]
        },
    }

    import_response = client.post(
        "/api/settings/bundle/import",
        headers=auth_headers,
        json=bundle,
    )
    assert import_response.status_code == 200, import_response.get_data(as_text=True)
    result = import_response.get_json()["result"]
    assert result["sections"]["llmConfigs"]["created"] == 1
    assert result["sections"]["llmConfigs"]["updated"] == 1

    from bill_analyser.api import app as api_app

    async def _fetch_configs() -> list[dict[str, Any]]:
        configs = await api_app.db.get_llm_configs(user_id=user_id)
        return [item for item in configs if item["name"] == config_name]

    configs = asyncio.run(_fetch_configs())
    assert len(configs) == 1
    assert configs[0]["model"] == "gpt-second"
    assert configs[0]["api_key"] == "sk-first"


def test_settings_bundle_external_refs_do_not_bind_to_local_ids(client):
    suffix = int(time.time() * 1000)
    auth_headers = _build_isolated_auth_headers(
        client, "test_settings_bundle_import_ref_collision"
    )
    user_id = _get_current_user_id(client, auth_headers)
    category = _ensure_test_expense_category(client, auth_headers)
    account = _create_account(client, auth_headers, name=f"collision-account-{suffix}")
    tag = _create_tag(client, auth_headers, name=f"collision-tag-{suffix}")
    account_id = int(account["id"])
    category_id = int(category["id"])
    tag_id = int(tag["id"])
    bundle = {
        "schemaVersion": 1,
        "sections": {
            "transactionTemplates": [
                {
                    "name": f"collision-template-{suffix}",
                    "templateType": 1,
                    "type": 3,
                    "categoryRef": f"category:{category_id}",
                    "sourceAccountRef": f"account:{account_id}",
                    "sourceAmount": 7.5,
                    "tagRefs": [f"tag:{tag_id}"],
                },
                {
                    "name": f"legacy-id-template-{suffix}",
                    "templateType": 1,
                    "type": 3,
                    "categoryId": category_id,
                    "sourceAccountId": account_id,
                    "sourceAmount": 9.5,
                    "tagIds": [tag_id],
                },
            ],
            "categoryRecognitionRules": [
                {
                    "categoryRef": f"category:{category_id}",
                    "name": f"collision-rule-{suffix}",
                    "priority": 1,
                    "ruleExpression": f"OR={{collision-{suffix}}}",
                    "enabled": True,
                },
                {
                    "categoryId": category_id,
                    "name": f"legacy-id-rule-{suffix}",
                    "priority": 1,
                    "ruleExpression": f"OR={{legacy-id-{suffix}}}",
                    "enabled": True,
                },
            ],
        },
    }

    import_response = client.post(
        "/api/settings/bundle/import",
        headers=auth_headers,
        json=bundle,
    )
    assert import_response.status_code == 200, import_response.get_data(as_text=True)
    result = import_response.get_json()["result"]
    assert result["sections"]["categoryRecognitionRules"]["created"] == 1
    assert result["sections"]["categoryRecognitionRules"]["skipped"] == 1
    assert result["sections"]["transactionTemplates"]["created"] == 1
    assert result["sections"]["transactionTemplates"]["skipped"] == 1

    from bill_analyser.api import app as api_app

    async def _fetch_collision_state() -> dict[str, Any]:
        conn = await api_app.db._get_connection()
        async with conn.execute(
            """
            SELECT name, account, category, amount, tag
            FROM bill_templates
            WHERE user_id = ? AND name IN (?, ?)
            """,
            (
                user_id,
                f"collision-template-{suffix}",
                f"legacy-id-template-{suffix}",
            ),
        ) as cursor:
            rows = {
                row["name"]: dict(row)
                for row in await cursor.fetchall()
            }
        async with conn.execute(
            """
            SELECT name, category_id
            FROM category_rules
            WHERE user_id = ? AND name IN (?, ?)
            """,
            (
                user_id,
                f"collision-rule-{suffix}",
                f"legacy-id-rule-{suffix}",
            ),
        ) as cursor:
            rules = {
                row["name"]: dict(row)
                for row in await cursor.fetchall()
            }
        return {"templates": rows, "rules": rules}

    state = asyncio.run(_fetch_collision_state())
    legacy_template = state["templates"][f"legacy-id-template-{suffix}"]
    assert f"collision-template-{suffix}" not in state["templates"]
    assert str(legacy_template["account"]) == str(account_id)
    assert str(legacy_template["category"]) == str(category_id)
    assert str(tag_id) in str(legacy_template["tag"])
    assert legacy_template["amount"] == 9.5
    assert f"collision-rule-{suffix}" not in state["rules"]
    assert state["rules"][f"legacy-id-rule-{suffix}"]["category_id"] == category_id
