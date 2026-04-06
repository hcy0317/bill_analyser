from __future__ import annotations

from typing import TYPE_CHECKING, Any

import pytest

from bill_analyser.core.db import Database

if TYPE_CHECKING:
	from pathlib import Path


async def _create_database(tmp_path: Path) -> Database:
	db = Database(str(tmp_path / "test_db_import_config_paths.db"))
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
	is_default: bool = False,
	extra: dict[str, Any] | None = None,
) -> dict[str, Any]:
	payload = {
		"name": name,
		"file_format": file_format,
		"description": f"{name} 描述",
		"field_mappings": field_mappings
		or {
			"date": "交易时间",
			"type": "交易类型",
			"amount": "金额",
			"description": "备注",
		},
		"date_format": "%Y-%m-%d %H:%M:%S",
		"encoding": "utf-8",
		"delimiter": ",",
		"skip_rows": 0,
		"has_header": True,
		"sample_headers": sample_headers or ["交易时间", "交易类型", "金额", "备注"],
		"custom_rules": {"source": "pytest", "name": name},
		"is_default": is_default,
	}
	if extra:
		payload = {**payload, **extra}
	return payload


@pytest.mark.asyncio
async def test_failed_import_config_update_keeps_existing_default(tmp_path: Path) -> None:
	"""更新不存在的配置失败时，不应顺手清空现有默认模板。"""
	db = await _create_database(tmp_path)
	try:
		user_id = await _create_user(db, "db_import_config_guard")
		default_config_id = await db.save_import_config(
			_build_import_config_payload(name="默认 CSV 模板", is_default=True),
			user_id=user_id,
		)

		with pytest.raises(ValueError, match="import config not found"):
			await db.save_import_config(
				_build_import_config_payload(
					name="幽灵配置",
					is_default=True,
					extra={"id": 999999},
				),
				user_id=user_id,
			)

		configs = await db.get_import_configs(user_id=user_id, file_format="csv")
		default_ids = [int(config["id"]) for config in configs if config.get("is_default")]
		assert default_ids == [default_config_id]
	finally:
		await db.close()


@pytest.mark.asyncio
async def test_find_matching_import_config_normalizes_headers_and_updates_usage(tmp_path: Path) -> None:
	"""精确表头匹配应走规范化签名，并累计 usage 计数。"""
	db = await _create_database(tmp_path)
	try:
		user_id = await _create_user(db, "db_import_config_exact")
		config_id = await db.save_import_config(
			_build_import_config_payload(
				name="英文表头模板",
				sample_headers=[" Transaction Date ", "Amount", "Description "],
				field_mappings={
					"date": "Transaction Date",
					"amount": "Amount",
					"description": "Description",
				},
			),
			user_id=user_id,
		)

		matched = await db.find_matching_import_config(
			"csv",
			["transaction   date", " amount ", "DESCRIPTION"],
			user_id=user_id,
		)

		assert matched is not None
		assert int(matched["id"]) == config_id
		assert matched["match_reason"] == "exact_header_signature"
		assert matched["match_score"] == 1.0
		assert matched["matched_header_count"] == 3

		configs = await db.get_import_configs(user_id=user_id, file_format="csv")
		refreshed = next(config for config in configs if int(config["id"]) == config_id)
		assert int(refreshed["use_count"]) == 1
		assert refreshed["last_used_at"]
	finally:
		await db.close()


@pytest.mark.asyncio
async def test_find_matching_import_config_falls_back_to_default_template(tmp_path: Path) -> None:
	"""低分命中时应回退到显式默认模板，并更新默认模板 usage。"""
	db = await _create_database(tmp_path)
	try:
		user_id = await _create_user(db, "db_import_config_fallback")
		default_id = await db.save_import_config(
			_build_import_config_payload(name="默认回退模板", is_default=True),
			user_id=user_id,
		)
		await db.save_import_config(
			_build_import_config_payload(
				name="低重叠模板",
				sample_headers=["商户名称", "交易分类", "附加字段"],
				field_mappings={
					"description": "商户名称",
					"category": "交易分类",
				},
			),
			user_id=user_id,
		)

		matched = await db.find_matching_import_config(
			"csv",
			["完全不同列一", "完全不同列二", "完全不同列三"],
			user_id=user_id,
			min_score=0.95,
		)

		assert matched is not None
		assert int(matched["id"]) == default_id
		assert matched["match_reason"] == "default_template_fallback"
		assert matched["match_score"] == 0.0
		assert matched["matched_header_count"] == 0

		configs = await db.get_import_configs(user_id=user_id, file_format="csv")
		refreshed_default = next(config for config in configs if int(config["id"]) == default_id)
		assert int(refreshed_default["use_count"]) == 1
		assert refreshed_default["last_used_at"]
	finally:
		await db.close()


@pytest.mark.asyncio
async def test_save_import_config_switches_default_only_within_same_user_and_format(tmp_path: Path) -> None:
	"""默认模板切换只应影响同一用户、同一格式下的配置。"""
	db = await _create_database(tmp_path)
	try:
		user_id = await _create_user(db, "db_import_config_scope")
		other_user_id = await _create_user(db, "db_import_config_scope_other")

		original_csv_default_id = await db.save_import_config(
			_build_import_config_payload(name="用户一 CSV 默认", is_default=True),
			user_id=user_id,
		)
		tsv_default_id = await db.save_import_config(
			_build_import_config_payload(name="用户一 TSV 默认", file_format="tsv", is_default=True),
			user_id=user_id,
		)
		other_user_csv_default_id = await db.save_import_config(
			_build_import_config_payload(name="用户二 CSV 默认", is_default=True),
			user_id=other_user_id,
		)

		new_csv_default_id = await db.save_import_config(
			_build_import_config_payload(name="用户一 CSV 新默认", is_default=True),
			user_id=user_id,
		)

		user_csv_configs = await db.get_import_configs(user_id=user_id, file_format="csv")
		user_tsv_configs = await db.get_import_configs(user_id=user_id, file_format="tsv")
		other_user_csv_configs = await db.get_import_configs(user_id=other_user_id, file_format="csv")

		assert [int(config["id"]) for config in user_csv_configs if config.get("is_default")] == [new_csv_default_id]
		assert any(
			int(config["id"]) == original_csv_default_id and not config.get("is_default")
			for config in user_csv_configs
		)
		assert [int(config["id"]) for config in user_tsv_configs if config.get("is_default")] == [tsv_default_id]
		assert [
			int(config["id"]) for config in other_user_csv_configs if config.get("is_default")
		] == [other_user_csv_default_id]
	finally:
		await db.close()
