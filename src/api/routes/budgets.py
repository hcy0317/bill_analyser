"""
Budgets API Routes - 预算管理API端点

提供预算CRUD操作、执行状态查询、周期预测等功能
"""

import asyncio
import calendar
import json
from datetime import datetime, date
from flask import Blueprint, request, jsonify, Response

from src.utils.logger import get_logger, log_method
from src.api.middleware.auth import require_auth

logger = get_logger('BudgetsAPI')

# RESTful API 蓝图
bp = Blueprint('budgets', __name__)

# v1兼容API蓝图
bp_v1 = Blueprint('budgets_v1', __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    from flask import current_app
    return current_app.config.get('DB_INSTANCE')


@bp.route('/', methods=['GET'])
@log_method
@require_auth
def get_budgets():
    """
    获取预算列表

    Query Parameters:
        - period_type: 周期类型（daily/weekly/monthly/yearly）
        - enabled: 是否启用（true/false）
        - category: 分类名称
    """
    logger.info("[get_budgets] 开始获取预算列表")

    try:
        filters = {}
        if request.args.get('period_type'):
            filters['period_type'] = request.args.get('period_type')
        enabled_arg = request.args.get('enabled')
        if enabled_arg:
            filters['enabled'] = enabled_arg.lower() == 'true'
        if request.args.get('category'):
            filters['category'] = request.args.get('category')

        logger.info(f"[get_budgets] 筛选条件: {filters}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            budgets = loop.run_until_complete(db.get_budgets(filters, user_id=request.user_id))
        finally:
            loop.close()

        logger.info(f"[get_budgets] 返回{len(budgets)}条预算")

        return jsonify({
            'success': True,
            'result': budgets
        })

    except Exception as e:
        logger.error("获取预算列表失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:budget_id>', methods=['GET'])
@log_method
@require_auth
def get_budget(budget_id: int):
    """获取预算详情"""
    logger.info(f"[get_budget] 获取预算ID: {budget_id}")

    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            budget = loop.run_until_complete(db.get_budget_by_id(budget_id, user_id=request.user_id))
        finally:
            loop.close()

        if not budget:
            logger.warning(f"[get_budget] 预算不存在: {budget_id}")
            return jsonify({
                'success': False,
                'error': 'Budget not found'
            }), 404

        logger.info(f"[get_budget] 成功获取预算: {budget['name']}")

        return jsonify({
            'success': True,
            'result': budget
        })

    except Exception as e:
        logger.error("获取预算详情失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/', methods=['POST'])
@log_method
@require_auth
def create_budget():
    """
    创建预算

    Request Body:
        {
            "name": "餐饮预算",
            "category": "餐饮",
            "sub_category": "午餐",
            "period_type": "monthly",
            "amount": 1000.00,
            "start_date": "2024-01-01",
            "end_date": "2024-12-31",
            "alert_threshold": 80,
            "enabled": true
        }
    """
    logger.info("[create_budget] 开始创建预算")

    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.info(f"[create_budget] 请求数据: {data}")

        # 验证必填字段
        required_fields = ['name', 'period_type', 'amount', 'start_date']
        for field in required_fields:
            if field not in data:
                logger.warning(f"[create_budget] 缺少必填字段: {field}")
                return jsonify({
                    'success': False,
                    'error': f'Missing required field: {field}'
                }), 400

        # 设置默认值
        data.setdefault('enabled', True)
        data.setdefault('alert_threshold', 80)
        data['created_at'] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        data['updated_at'] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            budget_id = loop.run_until_complete(db.create_budget(data, user_id=request.user_id))

            if budget_id:
                budget = loop.run_until_complete(db.get_budget_by_id(budget_id, user_id=request.user_id))
                logger.info(f"[create_budget] 创建成功: ID={budget_id}, name={data['name']}")

                return jsonify({
                    'success': True,
                    'result': budget
                }), 201
        finally:
            loop.close()

        logger.error("[create_budget] 创建失败")
        return jsonify({
            'success': False,
            'error': 'Failed to create budget'
        }), 500

    except Exception as e:
        logger.error("创建预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:budget_id>', methods=['PUT'])
@log_method
@require_auth
def update_budget(budget_id: int):
    """更新预算"""
    logger.info(f"[update_budget] 更新预算ID: {budget_id}")

    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.info(f"[update_budget] 更新数据: {data}")

        data['updated_at'] = datetime.now().strftime('%Y-%m-%d %H:%M:%S')

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_budget(budget_id, data, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            logger.info(f"[update_budget] 更新成功: ID={budget_id}")
            return jsonify({
                'success': True,
                'message': 'Budget updated successfully'
            })

        logger.warning(f"[update_budget] 预算不存在: {budget_id}")
        return jsonify({
            'success': False,
            'error': 'Budget not found'
        }), 404

    except Exception as e:
        logger.error("更新预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/<int:budget_id>', methods=['DELETE'])
@log_method
@require_auth
def delete_budget(budget_id: int):
    """删除预算"""
    logger.info(f"[delete_budget] 删除预算ID: {budget_id}")

    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.delete_budget(budget_id, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            logger.info(f"[delete_budget] 删除成功: ID={budget_id}")
            return jsonify({
                'success': True,
                'message': 'Budget deleted successfully'
            })

        logger.warning(f"[delete_budget] 预算不存在: {budget_id}")
        return jsonify({
            'success': False,
            'error': 'Budget not found'
        }), 404

    except Exception as e:
        logger.error("删除预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/execution', methods=['GET'])
@log_method
@require_auth
def get_budget_execution():
    """
    获取预算执行详情

    Query Parameters:
        - budget_type: 预算类型（3=支出, 5=投资）
        - start_date: 开始日期
        - end_date: 结束日期
        - category_id: 分类ID
        - account_ids: 账户ID（逗号分隔）
        - tag_ids: 标签ID（逗号分隔）
    """
    logger.info("[get_budget_execution] 开始获取预算执行详情")

    try:
        budget_type = int(request.args.get('budget_type', 3))
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')
        category_id = request.args.get('category_id')
        account_ids_str = request.args.get('account_ids', '')
        tag_ids_str = request.args.get('tag_ids', '')

        # 解析ID列表
        account_ids = None
        if account_ids_str:
            account_ids = [int(x) for x in account_ids_str.split(',') if x.strip()]

        tag_ids = None
        if tag_ids_str:
            tag_ids = [int(x) for x in tag_ids_str.split(',') if x.strip()]

        category_id_int = int(category_id) if category_id else None

        logger.info(
            f"[get_budget_execution] 参数: type={budget_type}, "
            f"dates={start_date}~{end_date}, category={category_id_int}, "
            f"accounts={account_ids}, tags={tag_ids}"
        )

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            results = loop.run_until_complete(db.get_budget_execution_details(
                budget_type=budget_type,
                start_date=start_date,
                end_date=end_date,
                category_id=category_id_int,
                account_ids=account_ids,
                tag_ids=tag_ids,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        # 计算汇总
        total_budget = sum(r['budget_amount'] for r in results)
        total_spent = sum(r['spent_amount'] for r in results)
        overall_execution_rate = (total_spent / total_budget * 100) if total_budget > 0 else 0

        logger.info(f"[get_budget_execution] 返回{len(results)}条执行详情")

        return jsonify({
            'success': True,
            'result': {
                'items': results,
                'summary': {
                    'total_budget': total_budget,
                    'total_spent': total_spent,
                    'total_remaining': total_budget - total_spent,
                    'overall_execution_rate': round(overall_execution_rate, 2),
                    'count': len(results)
                }
            }
        })

    except Exception as e:
        logger.error("获取预算执行详情失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/forecast', methods=['GET'])
@log_method
@require_auth
def get_period_forecast():
    """
    获取周期预计

    Query Parameters:
        - budget_type: 预算类型（3=支出, 5=投资）
        - period_type: 周期类型（daily/weekly/monthly/yearly）
        - start_date: 开始日期
        - end_date: 结束日期
    """
    logger.info("[get_period_forecast] 开始获取周期预计")

    try:
        budget_type = int(request.args.get('budget_type', 3))
        period_type = request.args.get('period_type', 'monthly')
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')

        logger.info(
            f"[get_period_forecast] 参数: type={budget_type}, "
            f"period={period_type}, dates={start_date}~{end_date}"
        )

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            results = loop.run_until_complete(db.get_period_forecast(
                budget_type=budget_type,
                period_type=period_type,
                start_date=start_date,
                end_date=end_date,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        # 计算汇总
        total_forecast = sum(r['forecast_amount'] for r in results)

        logger.info(f"[get_period_forecast] 返回{len(results)}条预测数据")

        return jsonify({
            'success': True,
            'result': {
                'items': results,
                'summary': {
                    'total_forecast': round(total_forecast, 2),
                    'count': len(results)
                }
            }
        })

    except Exception as e:
        logger.error("获取周期预计失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/export', methods=['GET'])
@log_method
@require_auth
def export_budgets():
    """导出预算"""
    logger.info("[export_budgets] 开始导出预算")

    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            budgets = loop.run_until_complete(db.export_budgets(user_id=request.user_id))
        finally:
            loop.close()

        logger.info(f"[export_budgets] 导出{len(budgets)}条预算")

        # 保持字段顺序
        json_str = json.dumps({
            'success': True,
            'result': budgets
        }, ensure_ascii=False, separators=(',', ':'))

        return Response(json_str, mimetype='application/json')

    except Exception as e:
        logger.error("导出预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/import', methods=['POST'])
@log_method
@require_auth
def import_budgets():
    """
    导入预算

    Request Body:
        [
            {
                "name": "餐饮预算",
                "category": "餐饮",
                "period_type": "monthly",
                "amount": 1000.00,
                "start_date": "2024-01-01"
            }
        ]
    """
    logger.info("[import_budgets] 开始导入预算")

    try:
        data = request.get_json()
        if not data or not isinstance(data, list):
            return jsonify({
                'success': False,
                'error': 'Invalid data format. Expected array of budgets.'
            }), 400

        logger.info(f"[import_budgets] 准备导入{len(data)}条预算")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.import_budgets(data, user_id=request.user_id))
        finally:
            loop.close()

        logger.info(
            f"[import_budgets] 导入完成: 创建={result['created']}, "
            f"更新={result['updated']}, 错误={result['errors']}"
        )

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error("导入预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


# ==================== v1兼容API ====================


@bp_v1.route('/v1/budgets/list.json', methods=['GET', 'POST'])
@log_method
@require_auth
def get_budgets_v1():
    """v1兼容 - 获取预算列表"""
    logger.info("[get_budgets_v1] 获取预算列表")

    try:
        # 支持GET和POST的参数
        if request.method == 'POST':
            data = request.get_json() or {}
        else:
            data = dict(request.args)

        filters = {}
        if data.get('periodType'):
            filters['period_type'] = data['periodType']
        if data.get('enabled') is not None:
            filters['enabled'] = data['enabled'] in [True, 'true', '1', 1]
        if data.get('category'):
            filters['category'] = data['category']

        budget_type = int(data.get('budgetType', data.get('type', 3)))
        start_date = data.get('startDate', data.get('start_date'))
        end_date = data.get('endDate', data.get('end_date'))

        logger.info(f"[get_budgets_v1] 筛选条件: {filters}, type={budget_type}, start_date={start_date}, end_date={end_date}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # 获取预算执行详情（包含分类信息和执行度）
            results = loop.run_until_complete(db.get_budget_execution_details(
                budget_type=budget_type,
                start_date=start_date,
                end_date=end_date,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        # 转换为前端期望的格式 (BudgetInfoResponse接口)
        items = []
        for r in results:
            # 获取分类信息
            category_info = r.get('category_info') or {}

            items.append({
                'id': str(r['id']),
                'name': r['name'],
                'category': r.get('category', ''),          # 主分类名称
                'subCategory': r.get('sub_category', ''),   # 子分类名称
                'categoryId': str(category_info.get('id', '')),
                'periodType': r['period_type'],
                'amount': int(r['budget_amount'] * 100),     # 转换为分（前端期望amount字段）
                'startDate': r['start_date'],
                'endDate': r.get('end_date'),
                'alertThreshold': r['alert_threshold'],
                'enabled': r['enabled'],
                'type': budget_type,  # 使用查询条件的预算类型（3=支出, 5=投资）
                # 运行时执行数据（用于列表显示）
                'spentAmount': int(r['spent_amount'] * 100),
                'remainingAmount': int(r['remaining_amount'] * 100),
                'executionRate': r['execution_rate'],
                # 额外字段用于分类显示
                'categoryName': r['category'] or '',
                'categoryIcon': category_info.get('icon', ''),
                'categoryColor': category_info.get('color', '')
            })

        # 计算汇总
        total_budget = sum(r['budget_amount'] for r in results)
        total_spent = sum(r['spent_amount'] for r in results)

        logger.info(f"[get_budgets_v1] 返回{len(items)}条预算")

        return jsonify({
            'success': True,
            'result': {
                'items': items,
                'totalBudget': int(total_budget * 100),
                'totalSpent': int(total_spent * 100),
                'totalRemaining': int((total_budget - total_spent) * 100),
                'count': len(items)
            }
        })

    except Exception as e:
        logger.error("v1获取预算列表失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/execution.json', methods=['GET', 'POST'])
@log_method
@require_auth
def get_execution_v1():
    """
    v1兼容 - 获取预算执行详情
    
    Query Parameters:
        - type: 预算类型（3=支出, 5=投资）
        - period_type: 周期类型（monthly/quarterly/yearly）
        - year: 年份
        - month: 月份
        - quarter: 季度
    """
    logger.info("[get_execution_v1] 获取预算执行详情")

    try:
        # 支持GET和POST的参数
        if request.method == 'POST':
            data = request.get_json() or {}
        else:
            data = dict(request.args)

        budget_type = int(data.get('type', 3))
        period_type = data.get('period_type', 'monthly')
        year = data.get('year')
        month = data.get('month')
        quarter = data.get('quarter')

        # 转换年月季度为整数
        if year:
            year = int(year)
        if month:
            month = int(month)
        if quarter:
            quarter = int(quarter)

        # 计算日期范围
        if not year:
            year = date.today().year
        if not month and period_type == 'monthly':
            month = date.today().month

        # 根据周期类型计算开始和结束日期
        if period_type == 'monthly':
            if not month:
                month = date.today().month
            start_date = f"{year}-{month:02d}-01"
            last_day = calendar.monthrange(year, month)[1]
            end_date = f"{year}-{month:02d}-{last_day:02d}"
        elif period_type == 'quarterly':
            if not quarter:
                quarter = (date.today().month - 1) // 3 + 1
            start_month = (quarter - 1) * 3 + 1
            end_month = quarter * 3
            start_date = f"{year}-{start_month:02d}-01"
            last_day = calendar.monthrange(year, end_month)[1]
            end_date = f"{year}-{end_month:02d}-{last_day:02d}"
        else:  # yearly
            start_date = f"{year}-01-01"
            end_date = f"{year}-12-31"

        logger.info(
            f"[get_execution_v1] 参数: type={budget_type}, "
            f"period={period_type}, year={year}, month={month}, "
            f"quarter={quarter}, dates={start_date}~{end_date}"
        )

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # 获取预算执行详情
            results = loop.run_until_complete(db.get_budget_execution_details(
                budget_type=budget_type,
                start_date=start_date,
                end_date=end_date,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        # 转换为前端期望的格式
        categories = []
        for r in results:
            category_info = r.get('category_info') or {}

            categories.append({
                'categoryId': str(r.get('category_id', category_info.get('id', ''))),
                'categoryName': r.get('category') or category_info.get('name', ''),
                'categoryIcon': category_info.get('icon', ''),
                'categoryColor': category_info.get('color', ''),
                'budgetAmount': int(r['budget_amount'] * 100),  # 转换为分
                'spentAmount': int(r['spent_amount'] * 100),
                'remainingAmount': int(r['remaining_amount'] * 100),
                'executionRate': round(r['execution_rate'], 2),
                'alertThreshold': r.get('alert_threshold', 80),
                'isOverBudget': r['spent_amount'] > r['budget_amount'],
                'alertTriggered': r['execution_rate'] >= r.get('alert_threshold', 80)
            })

        # 计算汇总
        total_budget = sum(r['budget_amount'] for r in results)
        total_spent = sum(r['spent_amount'] for r in results)
        overall_rate = (total_spent / total_budget * 100) if total_budget > 0 else 0

        logger.info(f"[get_execution_v1] 返回{len(categories)}条执行详情")

        # 响应格式匹配前端 BudgetExecutionResponse 接口
        # 前端期望: totalBudget, totalSpent, totalExecutionRate, categories, periodStart, periodEnd
        return jsonify({
            'success': True,
            'result': {
                'totalBudget': int(total_budget * 100),       # 总预算金额（分）
                'totalSpent': int(total_spent * 100),         # 总已花费金额（分）
                'totalExecutionRate': round(overall_rate, 2), # 总执行率（百分比）
                'categories': categories,                      # 分类执行详情列表
                'periodStart': start_date,                     # 统计周期开始（ISO日期）
                'periodEnd': end_date                          # 统计周期结束（ISO日期）
            }
        })

    except Exception as e:
        logger.error("v1获取预算执行详情失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/forecast.json', methods=['GET', 'POST'])
@log_method
@require_auth
def get_forecast_v1():
    """v1兼容 - 获取周期预计"""
    logger.info("[get_forecast_v1] 获取周期预计")

    try:
        # 支持GET和POST的参数
        if request.method == 'POST':
            data = request.get_json() or {}
        else:
            data = dict(request.args)

        budget_type = int(data.get('budgetType', data.get('type', 3)))
        period_type = data.get('periodType', data.get('period_type', 'monthly'))
        start_date = data.get('startDate', data.get('start_date'))
        end_date = data.get('endDate', data.get('end_date'))

        logger.info(
            f"[get_forecast_v1] 参数: type={budget_type}, "
            f"period={period_type}, dates={start_date}~{end_date}"
        )

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            results = loop.run_until_complete(db.get_period_forecast(
                budget_type=budget_type,
                period_type=period_type,
                start_date=start_date,
                end_date=end_date,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        # 转换为前端期望的格式
        items = []
        for r in results:
            category_info = r.get('category_info') or {}

            items.append({
                'categoryName': r['category'],
                'categoryId': str(category_info.get('id', '')),
                'categoryIcon': category_info.get('icon', ''),
                'categoryColor': category_info.get('color', ''),
                'totalAmount': int(r['total_amount'] * 100),
                'averageAmount': int(r['average_amount'] * 100),
                'forecastAmount': int(r['forecast_amount'] * 100),
                'periodCount': r['period_count'],
                'periods': r.get('periods', [])
            })

        total_forecast = sum(r['forecast_amount'] for r in results)

        logger.info(f"[get_forecast_v1] 返回{len(items)}条预测数据")

        return jsonify({
            'success': True,
            'result': {
                'items': items,
                'totalForecast': int(total_forecast * 100),
                'count': len(items)
            }
        })

    except Exception as e:
        logger.error("v1获取周期预计失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/add.json', methods=['POST'])
@log_method
@require_auth
def add_budget_v1():
    """
    v1兼容 - 创建预算

    如果创建的是二级分类预算，会自动检查并创建/更新一级分类预算，
    一级分类预算的金额为该分类下所有二级分类预算的总和。
    """
    logger.info("[add_budget_v1] 创建预算")

    try:
        data = request.get_json()
        if not data:
            return jsonify({
                'success': False,
                'error': 'No data provided'
            }), 400

        logger.info(f"[add_budget_v1] 请求数据: {data}")

        # 转换前端格式到后端格式
        budget_data = {
            'name': data.get('name', ''),
            'category': data.get('category', ''),
            'sub_category': data.get('subCategory', ''),
            'period_type': data.get('periodType', 'monthly'),
            'amount': data.get('amount', 0) / 100 if data.get('amount') else 0,
            'start_date': data.get('startDate', datetime.now().strftime('%Y-%m-%d')),
            'end_date': data.get('endDate'),
            'alert_threshold': data.get('alertThreshold', 80),
            'enabled': data.get('enabled', True),
            'created_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S'),
            'updated_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S')
        }

        # 验证必填字段 - name可以为空（备注），但category是必需的
        if not budget_data['category']:
            return jsonify({
                'success': False,
                'error': 'Missing required field: category'
            }), 400

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # 检查是否已存在相同分类的预算（唯一性检测）
            existing_budget = loop.run_until_complete(
                db.get_budget_by_category(
                    category=budget_data['category'],
                    sub_category=budget_data['sub_category'],
                    period_type=budget_data['period_type'],
                    start_date=budget_data['start_date'],
                    user_id=request.user_id
                )
            )
            if existing_budget:
                logger.warning(f"[add_budget_v1] 检测到重复预算: category={budget_data['category']}, "
                             f"sub_category={budget_data['sub_category']}")
                return jsonify({
                    'success': False,
                    'error': 'Budget already exists for this category'
                }), 400

            budget_id = loop.run_until_complete(db.create_budget(budget_data, user_id=request.user_id))

            if budget_id:
                budget = loop.run_until_complete(db.get_budget_by_id(budget_id, user_id=request.user_id))
                logger.info(f"[add_budget_v1] 创建成功: ID={budget_id}")

                # 如果是二级分类预算，自动创建/更新一级分类预算
                if budget_data['sub_category']:
                    logger.info("[add_budget_v1] 检测到二级分类预算，检查一级分类预算...")

                    # 检查是否存在一级分类预算
                    primary_budget = loop.run_until_complete(
                        db.get_primary_category_budget(
                            budget_data['category'],
                            budget_data['period_type'],
                            budget_data['start_date'],
                            user_id=request.user_id
                        )
                    )

                    # 计算所有二级分类预算的总金额
                    sub_total = loop.run_until_complete(
                        db.get_sub_category_budgets_total(
                            budget_data['category'],
                            budget_data['period_type'],
                            budget_data['start_date'],
                            user_id=request.user_id
                        )
                    )

                    if primary_budget:
                        # 更新一级分类预算金额
                        logger.info(f"[add_budget_v1] 更新一级分类预算金额: {sub_total}")
                        loop.run_until_complete(
                            db.update_budget(primary_budget['id'], {
                                'amount': sub_total,
                                'updated_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S')
                            }, user_id=request.user_id)
                        )
                    else:
                        # 创建一级分类预算（名称为空，不是"XX总预算"）
                        primary_data = {
                            'name': '',  # 一级分类预算名称为空，前端显示分类名
                            'category': budget_data['category'],
                            'sub_category': '',
                            'period_type': budget_data['period_type'],
                            'amount': sub_total,
                            'start_date': budget_data['start_date'],
                            'end_date': budget_data.get('end_date'),
                            'alert_threshold': budget_data.get('alert_threshold', 80),
                            'enabled': budget_data.get('enabled', True),
                            'created_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S'),
                            'updated_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S')
                        }
                        primary_id = loop.run_until_complete(db.create_budget(primary_data, user_id=request.user_id))
                        logger.info(f"[add_budget_v1] 自动创建一级分类预算: ID={primary_id}")

                # 转换为前端格式
                result = {
                    'id': str(budget['id']),
                    'name': budget['name'],
                    'category': budget.get('category', ''),
                    'subCategory': budget.get('sub_category', ''),
                    'periodType': budget['period_type'],
                    'amount': int(budget['amount'] * 100),
                    'startDate': budget['start_date'],
                    'endDate': budget.get('end_date'),
                    'alertThreshold': budget.get('alert_threshold', 80),
                    'enabled': budget.get('enabled', 1)
                }

                return jsonify({
                    'success': True,
                    'result': result
                }), 201
        finally:
            loop.close()

        return jsonify({
            'success': False,
            'error': 'Failed to create budget'
        }), 500

    except Exception as e:
        logger.error("v1创建预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/modify.json', methods=['POST'])
@log_method
@require_auth
def modify_budget_v1():
    """v1兼容 - 更新预算"""
    logger.info("[modify_budget_v1] 更新预算")

    try:
        data = request.get_json()
        if not data or not data.get('id'):
            return jsonify({
                'success': False,
                'error': 'Missing budget id'
            }), 400

        budget_id = int(data['id'])
        logger.info(f"[modify_budget_v1] 更新预算ID: {budget_id}")

        # 转换前端格式到后端格式
        update_data = {'updated_at': datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

        if 'name' in data:
            update_data['name'] = data['name']
        if 'category' in data:
            update_data['category'] = data['category']
        if 'subCategory' in data:
            update_data['sub_category'] = data['subCategory']
        if 'periodType' in data:
            update_data['period_type'] = data['periodType']
        if 'amount' in data:
            update_data['amount'] = data['amount'] / 100  # 分转元
        if 'startDate' in data:
            update_data['start_date'] = data['startDate']
        if 'endDate' in data:
            update_data['end_date'] = data['endDate']
        if 'alertThreshold' in data:
            update_data['alert_threshold'] = data['alertThreshold']
        if 'enabled' in data:
            update_data['enabled'] = 1 if data['enabled'] else 0

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.update_budget(budget_id, update_data, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            logger.info(f"[modify_budget_v1] 更新成功: ID={budget_id}")
            return jsonify({
                'success': True,
                'result': True
            })

        return jsonify({
            'success': False,
            'error': 'Budget not found'
        }), 404

    except Exception as e:
        logger.error("v1更新预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/delete.json', methods=['POST'])
@log_method
@require_auth
def delete_budget_v1():
    """v1兼容 - 删除预算"""
    logger.info("[delete_budget_v1] 删除预算")

    try:
        data = request.get_json()
        if not data or not data.get('id'):
            return jsonify({
                'success': False,
                'error': 'Missing budget id'
            }), 400

        budget_id = int(data['id'])
        logger.info(f"[delete_budget_v1] 删除预算ID: {budget_id}")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.delete_budget(budget_id, user_id=request.user_id))
        finally:
            loop.close()

        if result:
            logger.info(f"[delete_budget_v1] 删除成功: ID={budget_id}")
            return jsonify({
                'success': True,
                'result': True
            })

        return jsonify({
            'success': False,
            'error': 'Budget not found'
        }), 404

    except Exception as e:
        logger.error("v1删除预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/export.json', methods=['GET'])
@log_method
@require_auth
def export_budgets_v1():
    """v1兼容 - 导出预算"""
    logger.info("[export_budgets_v1] 导出预算")

    try:
        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            budgets = loop.run_until_complete(db.export_budgets(user_id=request.user_id))
        finally:
            loop.close()

        logger.info(f"[export_budgets_v1] 导出{len(budgets)}条预算")

        return jsonify({
            'success': True,
            'result': budgets
        })

    except Exception as e:
        logger.error("v1导出预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp_v1.route('/v1/budgets/import.json', methods=['POST'])
@log_method
@require_auth
def import_budgets_v1():
    """v1兼容 - 导入预算"""
    logger.info("[import_budgets_v1] 导入预算")

    try:
        data = request.get_json()
        if not data or not isinstance(data, list):
            return jsonify({
                'success': False,
                'error': 'Invalid data format. Expected array of budgets.'
            }), 400

        logger.info(f"[import_budgets_v1] 准备导入{len(data)}条预算")

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.import_budgets(data, user_id=request.user_id))
        finally:
            loop.close()

        logger.info(
            f"[import_budgets_v1] 导入完成: 创建={result['created']}, "
            f"更新={result['updated']}, 错误={result['errors']}"
        )

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error("v1导入预算失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500
