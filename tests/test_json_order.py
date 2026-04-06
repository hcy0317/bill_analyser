"""导出字段构建顺序回归。"""

import json


def test_manual_category_export_builder_keeps_declared_field_order() -> None:
    """旧分支式构建逻辑也应严格保持导出字段顺序。"""
    export_fields = ["type", "main_category", "sub_category", "priority", "keywords", "description", "icon", "color", "hidden"]
    cat = {
        "id": 1,
        "type": 2,
        "main_category": "测试",
        "sub_category": "",
        "priority": 0,
        "keywords": "test",
        "description": "desc",
        "icon": "icon1",
        "color": "ff0000",
        "hidden": False,
        "created_at": "2024-01-01",
    }

    cleaned_cat = {}
    for field in export_fields:
        if field == "type":
            cleaned_cat[field] = cat.get(field, 3)
        elif field == "sub_category":
            cleaned_cat[field] = cat.get(field, "")
        elif field == "priority":
            cleaned_cat[field] = cat.get(field, 0)
        elif field == "keywords":
            cleaned_cat[field] = cat.get(field, "")
        elif field == "description":
            cleaned_cat[field] = cat.get(field, "")
        elif field == "icon":
            cleaned_cat[field] = cat.get(field, "")
        elif field == "color":
            cleaned_cat[field] = cat.get(field, "")
        elif field == "hidden":
            cleaned_cat[field] = cat.get(field, False)
        elif field == "main_category":
            cleaned_cat[field] = cat.get(field, "")

    serialized = json.dumps(cleaned_cat, ensure_ascii=False, separators=(",", ":"))
    expected_positions = [serialized.index(f'"{field}"') for field in export_fields]

    assert list(cleaned_cat.keys()) == export_fields
    assert cleaned_cat["type"] == 2
    assert cleaned_cat["main_category"] == "测试"
    assert cleaned_cat["hidden"] is False
    assert expected_positions == sorted(expected_positions)
    assert '"created_at"' not in serialized
