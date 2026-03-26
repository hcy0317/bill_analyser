"""测试Flask jsonify字段顺序"""
from flask import Flask, jsonify
import json

app = Flask(__name__)
app.config['JSON_SORT_KEYS'] = False

export_fields = ['type', 'main_category', 'sub_category', 'priority', 
                'keywords', 'description', 'icon', 'color', 'hidden']

cat = {
    'id': 1,
    'type': 2,
    'main_category': '测试',
    'sub_category': '',
    'priority': 0,
    'keywords': 'test',
    'description': 'desc',
    'icon': 'icon1',
    'color': 'ff0000',
    'hidden': False,
    'created_at': '2024-01-01'
}

# 当前代码的构建方式
cleaned_cat = {}
for field in export_fields:
    if field == 'type':
        cleaned_cat[field] = cat.get(field, 3)
    elif field == 'sub_category':
        cleaned_cat[field] = cat.get(field, '')
    elif field == 'priority':
        cleaned_cat[field] = cat.get(field, 0)
    elif field == 'keywords':
        cleaned_cat[field] = cat.get(field, '')
    elif field == 'description':
        cleaned_cat[field] = cat.get(field, '')
    elif field == 'icon':
        cleaned_cat[field] = cat.get(field, '')
    elif field == 'color':
        cleaned_cat[field] = cat.get(field, '')
    elif field == 'hidden':
        cleaned_cat[field] = cat.get(field, False)
    elif field == 'main_category':
        cleaned_cat[field] = cat.get(field, '')

print("构建后的字典键顺序:")
print(list(cleaned_cat.keys()))

with app.app_context():
    resp = jsonify(cleaned_cat)
    print("\nFlask jsonify后的JSON:")
    print(json.dumps(resp.get_json(), indent=2, ensure_ascii=False))
