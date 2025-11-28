"""
Templates API Routes - 模板相关API端点
"""

import asyncio
from flask import Blueprint, request, jsonify

from src.utils.logger import get_logger, log_method
from src.api.middleware.auth import require_auth

logger = get_logger('TemplatesAPI')

bp = Blueprint('templates', __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app
    return current_app.config.get('DB_INSTANCE')


@bp.route('/', methods=['GET'])
@log_method
@require_auth
def get_templates():
    """获取模板列表"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        templates = loop.run_until_complete(db.get_all_templates(user_id=request.user_id))
        loop.close()

        return jsonify({
            'success': True,
            'result': templates
        })

    except Exception as e:
        logger.error("获取模板列表失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:template_id>', methods=['GET'])
@log_method
@require_auth
def get_template(template_id: int):
    """获取模板详情"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        template = loop.run_until_complete(db.get_template_by_id(template_id, user_id=request.user_id))
        loop.close()

        if not template:
            return jsonify({
                'success': False,
                'error': 'Template not found'
            }), 404

        return jsonify({
            'success': True,
            'result': template
        })

    except Exception as e:
        logger.error("获取模板详情失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/', methods=['POST'])
@log_method
@require_auth
def create_template():
    """创建模板"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        template_id = loop.run_until_complete(db.create_template(data, user_id=request.user_id))
        loop.close()

        return jsonify({
            'success': True,
            'result': {'id': template_id}
        }), 201

    except Exception as e:
        logger.error("创建模板失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:template_id>', methods=['PUT'])
@log_method
@require_auth
def update_template(template_id: int):
    """更新模板"""
    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.update_template(template_id, data, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({
                'success': True,
                'message': 'Template updated successfully'
            })
        return jsonify({
            'success': False,
            'error': 'Template not found'
        }), 404

    except Exception as e:
        logger.error("更新模板失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:template_id>', methods=['DELETE'])
@log_method
@require_auth
def delete_template(template_id: int):
    """删除模板"""
    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.delete_template(template_id, user_id=request.user_id))
        loop.close()

        if result:
            return jsonify({
                'success': True,
                'message': 'Template deleted successfully'
            })
        return jsonify({
            'success': False,
            'error': 'Template not found'
        }), 404

    except Exception as e:
        logger.error("删除模板失败: %s", e)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500
