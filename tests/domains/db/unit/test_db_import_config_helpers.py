"""Unit coverage for import-config helper guard and fallback branches."""

from __future__ import annotations

import pytest

from bill_analyser.core.db import Database

# pylint: disable=protected-access


def test_import_config_helpers_cover_normalization_defaults_summary_and_validation() -> None:
    """导入配置 helper 应覆盖空值、坏 JSON、摘要过滤和 payload 校验。"""
    db = Database(":memory:")
    fallback = {"fallback": True}

    assert db._normalize_import_config_header(None) == ""
    assert db._normalize_import_config_headers(None) == []
    assert db._build_import_config_header_signature([None, "", "   "]) == ""

    parsed_none = db._parse_json_object(None, fallback)
    parsed_invalid = db._parse_json_object("{bad-json", fallback)
    parsed_non_dict = db._parse_json_object('["not", "dict"]', fallback)
    assert parsed_none == fallback
    assert parsed_none is not fallback
    assert parsed_invalid == fallback
    assert parsed_invalid is not fallback
    assert parsed_non_dict == fallback
    assert parsed_non_dict is not fallback

    summary = db._build_import_config_description_summary(
        {
            "field_mappings": {
                "date": "",
                "description": "备注",
                "customField": "自定义列",
            },
            "sample_headers": ["", "备注", None, "自定义列"],
        }
    )
    assert summary == "映射: 描述->备注 / customField->自定义列 | 表头: 备注 / 自定义列"

    recommended = db._mark_import_config_default_recommendation(
        [
            {
                "id": 1,
                "use_count": 0,
                "last_used_at": "",
                "updated_at": "2026-04-01T00:00:00",
                "created_at": "2026-04-01T00:00:00",
                "is_default": False,
            },
            {
                "id": 2,
                "use_count": 2,
                "last_used_at": "2026-04-02T00:00:00",
                "updated_at": "2026-04-02T00:00:00",
                "created_at": "2026-04-01T00:00:00",
                "is_default": False,
            },
        ]
    )
    recommended_item = next(config for config in recommended if config["id"] == 2)
    assert recommended_item["default_recommendation"] is True

    with pytest.raises(ValueError, match="name is required"):
        db._validate_import_config_payload("", "csv", {"date": "交易时间"})
    with pytest.raises(ValueError, match="file_format is required"):
        db._validate_import_config_payload("模板", "", {"date": "交易时间"})
    with pytest.raises(ValueError, match="field_mappings is required"):
        db._validate_import_config_payload("模板", "csv", {})
