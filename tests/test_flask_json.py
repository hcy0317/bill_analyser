"""Flask jsonify 字段顺序回归。"""

import json

from flask import jsonify

from bill_analyser.api.app import create_app


def test_jsonify_preserves_export_field_order_when_sort_is_disabled() -> None:
    """真实 create_app() 关闭排序后，jsonify 输出应保持字段构造顺序。"""
    app = create_app()

    export_fields = ["type", "main_category", "sub_category", "priority", "keywords", "description", "icon", "color", "hidden"]
    default_values = {
        "type": 3,
        "main_category": "",
        "sub_category": "",
        "priority": 0,
        "keywords": "",
        "description": "",
        "icon": "",
        "color": "",
        "hidden": False,
    }
    cat = {
        "type": 2,
        "main_category": "测试",
        "sub_category": "",
        "priority": 0,
        "keywords": "test",
        "description": "desc",
        "icon": "icon1",
        "color": "ff0000",
        "hidden": False,
    }

    cleaned_cat = {field: cat.get(field, default_values[field]) for field in export_fields}

    with app.app_context():
        response = jsonify({"result": [cleaned_cat]})
        payload = response.get_json()
        raw_json = response.get_data(as_text=True)

    assert app.config["JSON_SORT_KEYS"] is False
    assert app.json.sort_keys is False
    assert list(cleaned_cat.keys()) == export_fields
    assert payload == {"result": [cleaned_cat]}
    serialized_category = json.dumps(cleaned_cat, ensure_ascii=True, separators=(",", ":"))
    expected_positions = [raw_json.index(f'"{field}"') for field in export_fields]
    assert expected_positions == sorted(expected_positions)
    assert serialized_category in raw_json.replace(" ", "").replace("\n", "")
