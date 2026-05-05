"""Regression coverage for the Rust taxonomy tag bridge."""

# pylint: disable=protected-access

from __future__ import annotations

import sqlite3
import subprocess
from pathlib import Path
from types import SimpleNamespace
from typing import Any

import pytest

from bill_analyser.core import tag_rust_bridge
from bill_analyser.core.db import Database
from bill_analyser.core.tag_rust_bridge import (
    TagRustBridgeOperationError,
    TagRustBridgeUnavailable,
)


def _create_tags_schema(db_path: Path) -> None:
    connection = sqlite3.connect(db_path)
    try:
        connection.execute(
            """
            CREATE TABLE tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                name TEXT NOT NULL,
                color TEXT,
                icon TEXT,
                display_order INTEGER DEFAULT 0,
                hidden BOOLEAN DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(user_id, name)
            )
            """
        )
        connection.commit()
    finally:
        connection.close()


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


def test_tag_rust_bridge_covers_crud_display_order_and_user_scope(tmp_path: Path) -> None:
    """Rust taxonomy bridge should preserve tag CRUD, sort order, and user isolation."""
    db_path = tmp_path / "tags_bridge.db"
    _create_tags_schema(db_path)

    first_id = tag_rust_bridge.create_tag(
        db_path,
        {"name": "早餐", "color": "#FF0000", "icon": "1"},
        user_id=7,
    )
    second_id = tag_rust_bridge.create_tag(db_path, {"name": "通勤", "hidden": True}, user_id=7)
    other_user_id = tag_rust_bridge.create_tag(db_path, {"name": "其他用户"}, user_id=8)

    assert tag_rust_bridge.update_tag(db_path, first_id, {"displayOrder": 2}, user_id=7) is True
    assert tag_rust_bridge.update_tag(
        db_path,
        second_id,
        {"display_order": 1, "icon": "2"},
        user_id=7,
    ) is True
    assert tag_rust_bridge.update_tag(db_path, other_user_id, {"name": "越权"}, user_id=7) is False

    tags = tag_rust_bridge.list_tags(db_path, user_id=7)
    assert [tag["name"] for tag in tags] == ["通勤", "早餐"]
    assert tags[0]["hidden"] == 1
    assert tags[0]["icon"] == "2"
    assert tag_rust_bridge.get_tag(db_path, other_user_id, user_id=7) is None
    assert tag_rust_bridge.get_tag(db_path, other_user_id, user_id=8)["name"] == "其他用户"

    assert tag_rust_bridge.update_display_orders(
        db_path,
        [(first_id, 10), (second_id, 20), (other_user_id, 1)],
        user_id=7,
    ) is True
    assert tag_rust_bridge.get_tag(db_path, other_user_id, user_id=8)["display_order"] == 0

    assert tag_rust_bridge.delete_tag(db_path, first_id, user_id=8) is False
    assert tag_rust_bridge.delete_tag(db_path, first_id, user_id=7) is True
    assert tag_rust_bridge.get_tag(db_path, first_id, user_id=7) is None


def test_tag_rust_bridge_maps_domain_failures_and_malformed_responses(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Bridge domain failures should be visible without falling back to Python SQL."""
    db_path = tmp_path / "tags_bridge_errors.db"
    _create_tags_schema(db_path)
    tag_rust_bridge.create_tag(db_path, {"name": "重复"}, user_id=7)

    with pytest.raises(TagRustBridgeOperationError) as error_info:
        tag_rust_bridge.create_tag(db_path, {"name": "重复"}, user_id=7)
    assert "UNIQUE constraint failed" in str(error_info.value)

    monkeypatch.setattr(
        tag_rust_bridge,
        "_invoke_tag_bridge",
        lambda _command, _payload: {"not": "a tag"},
    )
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge.list_tags(db_path, user_id=7)

    monkeypatch.setattr(tag_rust_bridge, "_invoke_tag_bridge", lambda _command, _payload: [])
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge.get_tag(db_path, 1, user_id=7)

    monkeypatch.setattr(tag_rust_bridge, "_invoke_tag_bridge", lambda _command, _payload: True)
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge.create_tag(db_path, {"name": "bad"}, user_id=7)

    monkeypatch.setattr(tag_rust_bridge, "_invoke_tag_bridge", lambda _command, _payload: "yes")
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge.update_tag(db_path, 1, {"name": "bad"}, user_id=7)

    monkeypatch.setattr(tag_rust_bridge, "_invoke_tag_bridge", lambda _command, _payload: [1])
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge.list_tags(db_path, user_id=7)


def test_tag_rust_bridge_process_resolution_and_failures(monkeypatch: pytest.MonkeyPatch) -> None:
    """Runtime request handling should use a prebuilt bridge executable only."""
    monkeypatch.setenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE", "custom-taxonomy-bridge")
    assert tag_rust_bridge._resolve_bridge_command("list-tags") == [
        "custom-taxonomy-bridge",
        "list-tags",
    ]

    original_resolve_bridge_command = tag_rust_bridge._resolve_bridge_command
    monkeypatch.setattr(
        tag_rust_bridge,
        "_resolve_bridge_command",
        lambda command: ["bridge", command],
    )

    def raise_os_error(*_args: Any, **_kwargs: Any) -> None:
        raise OSError("bridge missing")

    monkeypatch.setattr(tag_rust_bridge.subprocess, "run", raise_os_error)
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge._invoke_tag_bridge("list-tags", {"db_path": "x", "user_id": 1})

    monkeypatch.setattr(
        tag_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 1, "", "failed"),
    )
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge._invoke_tag_bridge("list-tags", {"db_path": "x", "user_id": 1})

    monkeypatch.setattr(
        tag_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 0, "not-json", ""),
    )
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge._invoke_tag_bridge("list-tags", {"db_path": "x", "user_id": 1})

    monkeypatch.setattr(
        tag_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(["bridge"], 0, "[]", ""),
    )
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge._invoke_tag_bridge("list-tags", {"db_path": "x", "user_id": 1})

    monkeypatch.setattr(
        tag_rust_bridge.subprocess,
        "run",
        lambda *_args, **_kwargs: subprocess.CompletedProcess(
            ["bridge"],
            0,
            '{"success":false,"error":{"message":"bad"}}',
            "",
        ),
    )
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge._invoke_tag_bridge("list-tags", {"db_path": "x", "user_id": 1})

    monkeypatch.setattr(tag_rust_bridge, "_resolve_bridge_command", original_resolve_bridge_command)
    monkeypatch.delenv("BILL_ANALYSER_RUST_TAXONOMY_BRIDGE", raising=False)
    monkeypatch.setattr(tag_rust_bridge, "_repo_root", lambda: Path("missing-root"))
    with pytest.raises(TagRustBridgeUnavailable):
        tag_rust_bridge._resolve_bridge_command("list-tags")


@pytest.mark.asyncio
async def test_database_tags_mixin_keeps_memory_databases_on_python_path(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """:memory: databases must not call the Rust bridge because it would open a new DB."""
    db = Database(":memory:")
    await db.init_db()
    try:
        monkeypatch.setattr(
            tag_rust_bridge,
            "create_tag",
            lambda *_args, **_kwargs: pytest.fail("memory DB should stay on Python tag path"),
        )

        tag_id = await db.create_tag({"name": "内存标签", "color": "#123456"}, user_id=1)
        tag = await db.get_tag_by_id(tag_id, user_id=1)

        assert tag is not None
        assert tag["name"] == "内存标签"
        assert [item["name"] for item in await db.get_all_tags(user_id=1)] == ["内存标签"]
        assert await db.update_tag(tag_id, {}, user_id=1) is False
        assert await db.update_tag(tag_id, {"name": "内存标签更新"}, user_id=1) is True
        assert await db.update_tag_display_orders([], user_id=1) is True
        assert await db.update_tag_display_orders([(tag_id, 5)], user_id=1) is True
        updated_tag = await db.get_tag_by_id(tag_id, user_id=1)
        assert updated_tag is not None
        assert updated_tag["name"] == "内存标签更新"
        assert int(updated_tag["display_order"]) == 5
        assert await db.delete_tag(999999, user_id=1) is False
        assert await db.delete_tag(tag_id, user_id=1) is True
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_database_tags_mixin_keeps_sqlcipher_databases_on_python_path(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """SQLCipher-enabled databases must keep using the configured Python connection."""
    db = Database(str(tmp_path / "sqlcipher_fallback_tags.db"))
    await db.init_db()
    db._encryption_config = SimpleNamespace(enabled=True)
    try:
        monkeypatch.setattr(
            tag_rust_bridge,
            "create_tag",
            lambda *_args, **_kwargs: pytest.fail("SQLCipher DB should stay on Python tag path"),
        )

        tag_id = await db.create_tag({"name": "加密标签", "color": "#654321"}, user_id=1)
        tag = await db.get_tag_by_id(tag_id, user_id=1)

        assert tag is not None
        assert tag["name"] == "加密标签"
        assert await db.update_tag(tag_id, {"name": "加密标签更新"}, user_id=1) is True
        assert await db.delete_tag(tag_id, user_id=1) is True
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_settings_bundle_export_reads_file_db_tags_through_facade(tmp_path: Path) -> None:
    """Settings export should keep tag output while file-DB tag reads are Rust-backed."""
    db = Database(str(tmp_path / "settings_bundle_tags.db"))
    await db.init_db()
    try:
        user_id = await _create_user(db, "settings_bundle_tag_user")
        tag_id = await db.create_tag(
            {"name": "设置包标签", "color": "#123456", "icon": "tag"},
            user_id=user_id,
        )

        bundle = await db.export_user_settings_bundle(user_id=user_id)
        tags = bundle["sections"]["transactionTags"]

        assert any(
            tag["externalRef"] == f"tag:{tag_id}"
            and tag["name"] == "设置包标签"
            and tag["color"] == "#123456"
            for tag in tags
        )
    finally:
        await db.close()
