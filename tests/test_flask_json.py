import flask
import json

print(f'Flask版本: {flask.__version__}')
from flask import Flask
app = Flask(__name__)
app.config['JSON_SORT_KEYS'] = False
print(f'Flask JSON_SORT_KEYS配置: {app.config.get("JSON_SORT_KEYS")}')

# 测试jsonify
from flask import jsonify
export_fields = ['type', 'main_category', 'sub_category', 'priority',
                'keywords', 'description', 'icon', 'color', 'hidden']
default_values = {
    'type': 3,
    'main_category': '',
    'sub_category': '',
    'priority': 0,
    'keywords': '',
    'description': '',
    'icon': '',
    'color': '',
    'hidden': False
}
cat = {
    'type': 2,
    'main_category': '测试',
    'sub_category': '',
    'priority': 0,
    'keywords': 'test',
    'description': 'desc',
    'icon': 'icon1',
    'color': 'ff0000',
    'hidden': False
}

cleaned_cat = {field: cat.get(field, default_values[field]) for field in export_fields}
print(f'\n字典键顺序: {list(cleaned_cat.keys())}')

with app.app_context():
    response = jsonify({'result': [cleaned_cat]})
    json_str = response.get_data(as_text=True)
    print(f'\nFlask jsonify输出 (前200字符):')
    print(json_str[:200])
