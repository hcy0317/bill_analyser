"""Settings bundle Rust bridge unit coverage."""

from __future__ import annotations

# pylint: disable=protected-access,too-few-public-methods

import subprocess
from pathlib import Path
from typing import Any

import pytest

from bill_analyser.core import settings_bundle_rust_bridge
from bill_analyser.core.db import Database
from bill_analyser.core.database.settings_bundle.accounts_categories_tags import (
    SettingsBundleAccountsCategoriesTagsMixin,
)
from bill_analyser.core.database.settings_bundle.base import SettingsBundleBaseMixin
from bill_analyser.core.database.settings_bundle.exporters import (
    SettingsBundleExportersMixin,
)
from bill_analyser.core.database.settings_bundle.resolution import (
    SettingsBundleResolutionMixin,
)
from bill_analyser.core.database.settings_bundle.templates import SettingsBundleTemplatesMixin


class _Completed:
    """Small subprocess.CompletedProcess stand-in."""

    def __init__(self, returncode: int, stdout: str) -> None:
        self.returncode = returncode
        self.stdout = stdout


class _TemplateFallbackHarness(
    SettingsBundleTemplatesMixin,
    SettingsBundleResolutionMixin,
):
    """Minimal settings-bundle composition for template fallback helpers."""


async def _create_user(db: Database, username: str) -> int:
    return await db.create_user(
        {
            "username": username,
            "email": f"{username}@example.com",
            "password_hash": "pytest-hash",
            "nickname": username,
            "language": "zh_Hans",
            "default_currency": "CNY",
            "first_day_of_week": 1,
            "is_active": 1,
            "email_verified": 1,
        }
    )


def test_settings_bundle_rust_bridge_wrappers_validate_result_shapes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Wrapper functions should validate Rust settings bundle response shapes."""
    responses: dict[str, Any] = {
        "settings-normalize-sections": {"accounts": [{"name": "Cash"}]},
        "settings-export-taxonomy-sections": {"transactionTags": [{"name": "Work"}]},
        "settings-normalize-account-import": {"name": "Cash", "parent_id": 0},
        "settings-normalize-category-import": {"main_category": "Food"},
        "settings-normalize-tag-import": {"name": "Work"},
        "settings-resolve-template-payload": {
            "payload": {"account": "1", "category": "2", "tag": "3"},
            "warnings": ["Template tag ref not found: tag:x"],
            "unresolved": False,
        },
    }
    monkeypatch.setattr(
        settings_bundle_rust_bridge,
        "_invoke_settings_bundle_bridge",
        lambda command, _payload: responses[command],
    )

    assert settings_bundle_rust_bridge.normalize_sections({"schemaVersion": 1}) == {
        "accounts": [{"name": "Cash"}]
    }
    assert settings_bundle_rust_bridge.export_taxonomy_sections(
        accounts=[],
        categories=[],
        tags=[],
        templates=[],
        scheduled=[],
    ) == {"transactionTags": [{"name": "Work"}]}
    assert settings_bundle_rust_bridge.normalize_account_import({}, {}) == {
        "name": "Cash",
        "parent_id": 0,
    }
    assert settings_bundle_rust_bridge.normalize_category_import({}) == {
        "main_category": "Food"
    }
    assert settings_bundle_rust_bridge.normalize_tag_import({}) == {"name": "Work"}
    assert settings_bundle_rust_bridge.resolve_template_payload(
        {},
        account_ref_map={},
        category_ref_map={},
        tag_ref_map={},
    ) == {
        "payload": {"account": "1", "category": "2", "tag": "3"},
        "warnings": ["Template tag ref not found: tag:x"],
        "unresolved": False,
    }

    responses["settings-normalize-sections"] = {"accounts": ["bad"]}
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.normalize_sections({"schemaVersion": 1})

    responses["settings-normalize-sections"] = []
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.normalize_sections({"schemaVersion": 1})

    responses["settings-export-taxonomy-sections"] = []
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.export_taxonomy_sections(
            accounts=[],
            categories=[],
            tags=[],
            templates=[],
            scheduled=[],
        )

    responses["settings-normalize-account-import"] = []
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.normalize_account_import({}, {})

    responses["settings-normalize-sections"] = {"accounts": "bad"}
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.normalize_sections({"schemaVersion": 1})

    responses["settings-resolve-template-payload"] = {"payload": [], "warnings": []}
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.resolve_template_payload(
            {},
            account_ref_map={},
            category_ref_map={},
            tag_ref_map={},
        )

    responses["settings-resolve-template-payload"] = {"payload": {}, "warnings": "bad"}
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.resolve_template_payload(
            {},
            account_ref_map={},
            category_ref_map={},
            tag_ref_map={},
        )

    responses["settings-resolve-template-payload"] = {"payload": {}, "warnings": []}
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge.resolve_template_payload(
            {},
            account_ref_map={},
            category_ref_map={},
            tag_ref_map={},
        )


def test_settings_bundle_rust_bridge_invocation_error_paths(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Subprocess and response failures should map to clear bridge exceptions."""
    monkeypatch.setattr(
        settings_bundle_rust_bridge,
        "_resolve_bridge_command",
        lambda command: ["bridge", command],
    )

    def _raise_os_error(*_args: Any, **_kwargs: Any) -> None:
        raise OSError("missing")

    monkeypatch.setattr(settings_bundle_rust_bridge.subprocess, "run", _raise_os_error)
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge._invoke_settings_bundle_bridge(
            "settings-normalize-sections", {}
        )

    def _raise_timeout(*_args: Any, **_kwargs: Any) -> None:
        raise subprocess.TimeoutExpired(cmd="bridge", timeout=1)

    monkeypatch.setattr(settings_bundle_rust_bridge.subprocess, "run", _raise_timeout)
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge._invoke_settings_bundle_bridge(
            "settings-normalize-sections", {}
        )

    for completed in [
        _Completed(1, ""),
        _Completed(0, "not-json"),
        _Completed(0, "[]"),
        _Completed(0, '{"success":false,"error":{"bad":true}}'),
        _Completed(0, '{"success":null}'),
    ]:
        monkeypatch.setattr(
            settings_bundle_rust_bridge.subprocess,
            "run",
            lambda *_args, completed=completed, **_kwargs: completed,
        )
        with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
            settings_bundle_rust_bridge._invoke_settings_bundle_bridge(
                "settings-normalize-sections", {}
            )

    monkeypatch.setattr(
        settings_bundle_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: _Completed(
            0, '{"success":false,"error":"domain error"}'
        ),
    )
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeOperationError):
        settings_bundle_rust_bridge._invoke_settings_bundle_bridge(
            "settings-normalize-sections", {}
        )


def test_settings_bundle_mixins_fall_back_when_rust_bridge_is_unavailable(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Legacy Python settings-bundle logic should remain available if Rust is missing."""

    def _unavailable(*_args: Any, **_kwargs: Any) -> None:
        raise settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable("missing")

    monkeypatch.setattr(settings_bundle_rust_bridge, "normalize_sections", _unavailable)
    assert SettingsBundleBaseMixin._normalize_settings_bundle_sections(
        {
            "schemaVersion": 1,
            "accounts": [{"name": "Cash"}],
            "transactionCategories": [{"mainCategory": "Food"}],
        }
    )["accounts"] == [{"name": "Cash"}]

    monkeypatch.setattr(
        settings_bundle_rust_bridge, "normalize_account_import", _unavailable
    )
    assert SettingsBundleAccountsCategoriesTagsMixin._settings_account_import_values(
        {
            "name": "Cash",
            "type": "2",
            "currency": "USD",
            "initialBalance": "12.5",
            "hidden": True,
            "displayOrder": "7",
            "aliases": ["Wallet"],
            "parentRef": "account:1",
        },
        {"account:1": 9},
    ) == {
        "name": "Cash",
        "type": 2,
        "category": None,
        "currency": "USD",
        "icon": "",
        "color": "",
        "balance": 0.0,
        "initial_balance": 12.5,
        "hidden": 1,
        "display_order": 7,
        "comment": "",
        "aliases": '["Wallet"]',
        "parent_id": 9,
    }

    monkeypatch.setattr(
        settings_bundle_rust_bridge, "normalize_category_import", _unavailable
    )
    assert SettingsBundleAccountsCategoriesTagsMixin._settings_category_import_values(
        {
            "type": "4",
            "mainCategory": "Food",
            "subCategory": "Lunch",
            "hidden": True,
        }
    ) == {
        "type": 4,
        "main_category": "Food",
        "sub_category": "Lunch",
        "description": "",
        "priority": 0,
        "keywords": "",
        "hidden": 1,
        "icon": "",
        "color": "",
    }

    monkeypatch.setattr(settings_bundle_rust_bridge, "normalize_tag_import", _unavailable)
    assert SettingsBundleAccountsCategoriesTagsMixin._settings_tag_import_values(
        {"name": "Work", "displayOrder": "3", "hidden": True}
    ) == {
        "name": "Work",
        "color": "",
        "icon": "",
        "display_order": 3,
        "hidden": 1,
    }

    monkeypatch.setattr(
        settings_bundle_rust_bridge, "export_taxonomy_sections", _unavailable
    )
    exported = SettingsBundleExportersMixin()._export_settings_taxonomy_sections(
        accounts=[
            {
                "id": 1,
                "name": "Cash",
                "type": 1,
                "currency": "CNY",
                "balance": 0,
                "initial_balance": 0,
                "parent_id": None,
                "display_order": 0,
                "hidden": 0,
            }
        ],
        categories=[],
        tags=[],
        templates=[],
        scheduled=[],
        account_refs={1: "account:1"},
        account_names={1: "Cash"},
        category_refs={},
        category_names={},
        tag_refs={},
        tag_names={},
    )
    assert exported["accounts"][0]["externalRef"] == "account:1"

    monkeypatch.setattr(
        settings_bundle_rust_bridge, "resolve_template_payload", _unavailable
    )
    warnings: list[str] = []
    payload, unresolved = _TemplateFallbackHarness()._settings_template_resolved_payload(
        {
            "name": "Rent",
            "amount": "100",
            "categoryRef": "missing-category",
            "sourceAccountRef": "account:1",
            "tagRefs": ["tag:1"],
        },
        account_ref_map={"account:1": 1},
        category_ref_map={},
        tag_ref_map={"tag:1": 2},
        warnings=warnings,
    )
    assert payload["account"] == "1"
    assert payload["category"] == ""
    assert payload["tag"] == "2"
    assert unresolved is True
    assert warnings == ["Skipped template Rent with unresolved category reference"]


@pytest.mark.asyncio
async def test_settings_bundle_python_fallback_import_export_round_trip(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Fallback Python settings-bundle path remains covered after Rust route cutover."""

    def _unavailable(*_args: Any, **_kwargs: Any) -> None:
        raise settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable("missing")

    for name in [
        "normalize_sections",
        "export_taxonomy_sections",
        "normalize_account_import",
        "normalize_category_import",
        "normalize_tag_import",
        "resolve_template_payload",
    ]:
        monkeypatch.setattr(settings_bundle_rust_bridge, name, _unavailable)

    db = Database(str(tmp_path / "settings_bundle_fallback.db"))
    await db.init_db()
    try:
        user_id = await _create_user(db, "settings_bundle_fallback_user")
        bundle = {
            "schemaVersion": 1,
            "sections": {
                "accounts": [
                    {
                        "externalRef": "account:cash",
                        "name": "Cash",
                        "type": 1,
                        "currency": "CNY",
                        "initialBalance": 12.5,
                        "aliases": ["Wallet"],
                    },
                    {
                        "externalRef": "account:child",
                        "name": "Cash Child",
                        "type": 2,
                        "parentRef": "account:cash",
                        "hidden": True,
                    },
                    {"name": "", "externalRef": "account:skip"},
                ],
                "transactionCategories": [
                    {
                        "externalRef": "category:food",
                        "type": 3,
                        "mainCategory": "餐饮",
                        "subCategory": "咖啡",
                        "keywords": ["coffee", "latte"],
                        "hidden": False,
                    },
                    {"externalRef": "category:skip", "mainCategory": ""},
                ],
                "transactionTags": [
                    {
                        "externalRef": "tag:work",
                        "name": "Work",
                        "color": "#336699",
                        "displayOrder": 2,
                    },
                    {"name": ""},
                ],
                "transactionTemplates": [
                    {
                        "name": "Coffee template",
                        "sourceAmount": 32.5,
                        "type": 3,
                        "categoryRef": "category:food",
                        "sourceAccountRef": "account:cash",
                        "tagRefs": ["tag:work"],
                        "comment": "morning coffee",
                    },
                    {"name": "", "sourceAmount": 1},
                ],
                "scheduledTransactions": [
                    {
                        "name": "Monthly coffee",
                        "sourceAmount": 88,
                        "type": 3,
                        "categoryRef": "category:food",
                        "sourceAccountRef": "account:cash",
                        "scheduledFrequency": "1",
                        "scheduledFrequencyType": 2,
                        "scheduledStartDate": "2026-01-01",
                        "autoCreate": True,
                    }
                ],
                "categoryRecognitionRules": [
                    {
                        "name": "Coffee rule",
                        "categoryRef": "category:food",
                        "ruleExpression": "counterparty contains Coffee",
                        "regexEnabled": False,
                    },
                    {"name": "Skipped rule", "ruleExpression": ""},
                ],
                "llmConfigs": [
                    {
                        "name": "OpenAI",
                        "provider": "openai",
                        "model": "gpt-test",
                        "apiKey": "sk-test",
                        "baseUrl": "https://example.test",
                        "advancedSettings": {"temperature": 0.1},
                    },
                    {"provider": "openai"},
                ],
                "ocrConfig": [{"provider": "disabled", "lang": "eng"}],
            },
        }

        preview = await db.preview_import_user_settings_bundle(bundle, user_id=user_id)
        assert preview["dryRun"] is True
        assert preview["sections"]["accounts"]["created"] == 2
        assert preview["sections"]["transactionTemplates"]["created"] == 1
        assert preview["sections"]["categoryRecognitionRules"]["skipped"] == 1

        imported = await db.import_user_settings_bundle(bundle, user_id=user_id)
        assert imported["dryRun"] is False
        assert imported["sections"]["accounts"]["created"] == 2
        assert imported["sections"]["llmConfigs"]["created"] == 1
        assert imported["sections"]["ocrConfig"]["created"] == 1

        updated = await db.import_user_settings_bundle(bundle, user_id=user_id)
        assert updated["sections"]["accounts"]["updated"] == 2
        assert updated["sections"]["llmConfigs"]["updated"] == 1
        assert updated["sections"]["ocrConfig"]["updated"] == 1

        exported = await db.export_user_settings_bundle(user_id=user_id)
        sections = exported["sections"]
        assert any(item["name"] == "Cash" for item in sections["accounts"])
        assert any(item["name"] == "Coffee template" for item in sections["transactionTemplates"])
        assert any(item["name"] == "Coffee rule" for item in sections["categoryRecognitionRules"])
        assert sections["llmConfigs"][0]["apiKey"] == ""
        assert sections["llmConfigs"][0]["hasApiKey"] is True
        assert sections["ocrConfig"][0]["provider"] == "disabled"
    finally:
        await db.close()


def test_settings_bundle_rust_bridge_command_resolution(
    monkeypatch: pytest.MonkeyPatch,
    tmp_path: Path,
) -> None:
    """Command resolution should prefer env override then a built bridge binary."""
    monkeypatch.setenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE", "custom-bridge")
    assert settings_bundle_rust_bridge._resolve_bridge_command(
        "settings-normalize-sections"
    ) == ["custom-bridge", "settings-normalize-sections"]

    monkeypatch.delenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE")
    bridge_path = tmp_path
    monkeypatch.setattr(settings_bundle_rust_bridge, "_repo_root", lambda: tmp_path)
    monkeypatch.setattr(
        settings_bundle_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (bridge_path,),
    )
    assert settings_bundle_rust_bridge._resolve_bridge_command(
        "settings-export-taxonomy-sections"
    ) == [str(bridge_path), "settings-export-taxonomy-sections"]

    monkeypatch.setattr(
        settings_bundle_rust_bridge,
        "_candidate_bridge_paths",
        lambda _repo_root: (tmp_path / "missing.exe",),
    )
    with pytest.raises(settings_bundle_rust_bridge.SettingsBundleRustBridgeUnavailable):
        settings_bundle_rust_bridge._resolve_bridge_command(
            "settings-export-taxonomy-sections"
        )
