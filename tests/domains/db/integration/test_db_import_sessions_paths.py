"""DB integration coverage for import-session lifecycle and parser-template staging."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

from bill_analyser.core.db import Database

# pylint: disable=duplicate-code

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_import_sessions_paths.db"))
    await db.init_db()
    return db


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


@pytest.mark.asyncio
async def test_import_session_roundtrip_covers_type_inference_and_status_filters(
    tmp_path: Path,
) -> None:
    """导入会话与解析模板应覆盖 totals、类型推断和状态过滤更新。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "import_session_roundtrip")
        other_user_id = await _create_user(db, "import_session_roundtrip_other")
        session_id = "import-session-roundtrip"

        created_id = await db.create_import_session(session_id, user_id=user_id, file_count=3)
        assert created_id > 0
        assert (
            await db.update_import_session_status(
                session_id,
                "deduped",
                total_parsed=3,
                total_preview=2,
                total_confirmed=1,
            )
            is True
        )

        session = await db.get_import_session(session_id, user_id=user_id)
        assert session is not None
        assert session["status"] == "deduped"
        assert int(session["file_count"]) == 3
        assert int(session["total_parsed"]) == 3
        assert int(session["total_preview"]) == 2
        assert int(session["total_confirmed"]) == 1
        assert await db.get_import_session(session_id, user_id=other_user_id) is None

        inserted = await db.insert_parser_templates(
            session_id,
            [
                {
                    "date": "2026-04-01 08:00:00",
                    "amount": 30.5,
                    "description": "正数推断收入",
                    "counterparty": "商户A",
                },
                {
                    "date": "2026-04-01 09:00:00",
                    "amount": -18.0,
                    "description": "负数推断支出",
                    "counterparty": "商户B",
                },
                {
                    "date": "2026-04-01 10:00:00",
                    "amount": 0,
                    "description": "零值推断其他",
                    "counterparty": "商户C",
                },
            ],
            parser_id="pytest-parser",
            user_id=user_id,
        )
        assert inserted == 3

        all_templates = await db.get_parser_templates_by_session(session_id)
        assert [template["parser_type"] for template in all_templates] == ["收入", "支出", "其他"]
        assert (
            len(await db.get_parser_templates_by_session(session_id, processed_only=False))
            == 3
        )
        assert await db.get_parser_templates_by_session(session_id, processed_only=True) == []

        updated_count = await db.update_parser_template_status(
            [int(all_templates[0]["id"]), int(all_templates[1]["id"])],
            processed=True,
            account_id="acct-42",
        )
        assert updated_count == 2

        processed_templates = await db.get_parser_templates_by_session(
            session_id,
            processed_only=True,
        )
        unprocessed_templates = await db.get_parser_templates_by_session(
            session_id,
            processed_only=False,
        )
        assert {int(template["id"]) for template in processed_templates} == {
            int(all_templates[0]["id"]),
            int(all_templates[1]["id"]),
        }
        assert {template["parser_account_id"] for template in processed_templates} == {"acct-42"}
        assert [template["parser_type"] for template in unprocessed_templates] == ["其他"]
    finally:
        await db.close()


@pytest.mark.asyncio
async def test_import_session_noop_guards_return_zero_for_empty_payloads(tmp_path: Path) -> None:
    """空解析模板输入与空状态更新列表应直接返回 0。"""
    db = await _create_database(tmp_path)
    try:
        assert (
            await db.insert_parser_templates(
                "empty-session",
                [],
                parser_id="pytest-parser",
                user_id=1,
            )
            == 0
        )
        assert await db.update_parser_template_status([], processed=False) == 0
    finally:
        await db.close()
