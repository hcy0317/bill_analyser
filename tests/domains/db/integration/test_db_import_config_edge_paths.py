"""DB integration coverage for import-config no-match, overlap, and delete-false branches."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core.db import Database

# pylint: disable=duplicate-code

if TYPE_CHECKING:
    from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
    db = Database(str(tmp_path / "test_db_import_config_edge_paths.db"))
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


def _build_import_config_payload(
    *,
    name: str,
    file_format: str = "csv",
    sample_headers: list[str] | None = None,
    field_mappings: dict[str, Any] | None = None,
    extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
    payload = {
        "name": name,
        "file_format": file_format,
        "description": f"{name} 描述",
        "field_mappings": field_mappings
        or {
            "date": "交易时间",
            "amount": "交易金额",
            "description": "备注",
        },
        "date_format": "%Y-%m-%d %H:%M:%S",
        "encoding": "utf-8",
        "delimiter": ",",
        "skip_rows": 0,
        "has_header": True,
        "sample_headers": sample_headers or ["交易时间", "交易金额", "备注"],
        "custom_rules": {"source": "pytest", "name": name},
        "is_default": False,
    }
    if extra:
        payload = {**payload, **extra}
    return payload


@pytest.mark.asyncio
async def test_import_config_edge_paths_cover_empty_headers_overlap_and_delete_false(
    tmp_path: Path,
) -> None:
    """导入配置应覆盖空表头、无配置、无默认回退、重叠匹配和删除失败分支。"""
    db = await _create_database(tmp_path)
    try:
        user_id = await _create_user(db, "db_import_config_edge")

        assert (
            await db.find_matching_import_config(
                "csv",
                [None, "", "   "],
                user_id=user_id,
            )
            is None
        )
        assert await db.find_matching_import_config("csv", ["交易时间"], user_id=user_id) is None

        await db.save_import_config(
            _build_import_config_payload(
                name="无表头模板",
                sample_headers=[],
                extra={"sample_headers": [], "headers": []},
            ),
            user_id=user_id,
        )
        assert await db.find_matching_import_config(
            "csv",
            ["任意列一", "任意列二"],
            user_id=user_id,
            min_score=0.1,
        ) is None

        overlap_config_id = await db.save_import_config(
            _build_import_config_payload(
                name="重叠模板",
                sample_headers=["交易时间", "交易金额", "备注"],
                field_mappings={
                    "date": "交易时间",
                    "amount": "交易金额",
                    "description": "备注",
                },
            ),
            user_id=user_id,
        )

        matched = await db.find_matching_import_config(
            "csv",
            ["交易时间", "交易金额", "扩展字段"],
            user_id=user_id,
        )
        assert matched is not None
        assert int(matched["id"]) == overlap_config_id
        assert matched["match_reason"] == "header_overlap"
        assert matched["matched_header_count"] == 2
        assert matched["match_score"] > 0.6

        assert await db.delete_import_config(999999, user_id=user_id) is False
    finally:
        await db.close()
