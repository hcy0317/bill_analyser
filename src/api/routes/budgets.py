"""
Budgets API Routes - 预算管理API端点

提供预算CRUD操作、执行状态查询、周期预测等功能
"""

import asyncio
import calendar
import json
from datetime import date, datetime
from flask import Blueprint, request, jsonify, Response, current_app

from src.utils.logger import get_logger, log_method
from src.api.middleware.auth import require_auth

logger = get_logger('BudgetsAPI')

# RESTful API 蓝图
bp = Blueprint('budgets', __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    return current_app.config.get('DB_INSTANCE')


def _resolve_budget_period_range(period_type, year=None, month=None, quarter=None, start_date=None, end_date=None):
    """解析预算执行/快照使用的日期范围。"""
    if start_date and end_date:
        return start_date, end_date

    today = date.today()
    resolved_year = year or today.year
    resolved_period_type = period_type or 'monthly'

    if resolved_period_type == 'yearly':
        return f'{resolved_year}-01-01', f'{resolved_year}-12-31'

    if resolved_period_type == 'quarterly':
        resolved_quarter = quarter or ((today.month - 1) // 3 + 1)
        start_month = (resolved_quarter - 1) * 3 + 1
        end_month = start_month + 2
        end_day = calendar.monthrange(resolved_year, end_month)[1]
        return (
            f'{resolved_year}-{str(start_month).zfill(2)}-01',
            f'{resolved_year}-{str(end_month).zfill(2)}-{str(end_day).zfill(2)}'
        )

    resolved_month = month or today.month
    end_day = calendar.monthrange(resolved_year, resolved_month)[1]
    return (
        f'{resolved_year}-{str(resolved_month).zfill(2)}-01',
        f'{resolved_year}-{str(resolved_month).zfill(2)}-{str(end_day).zfill(2)}'
    )


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

        # 预算名称已改为备注，允许为空；分类仍为必填
        required_fields = ['period_type', 'amount', 'start_date']
        for field in required_fields:
            if field not in data:
                logger.warning(f"[create_budget] 缺少必填字段: {field}")
                return jsonify({
                    'success': False,
                    'error': f'Missing required field: {field}'
                }), 400

        if not data.get('category'):
            logger.warning('[create_budget] 缺少必填字段: category')
            return jsonify({
                'success': False,
                'error': 'Missing required field: category'
            }), 400

        # 设置默认值
        data.setdefault('name', '')
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
        period_type = request.args.get('period_type', 'monthly')
        year = int(request.args.get('year')) if request.args.get('year') else None
        month = int(request.args.get('month')) if request.args.get('month') else None
        quarter = int(request.args.get('quarter')) if request.args.get('quarter') else None
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')
        category_id = request.args.get('category_id')
        account_ids_str = request.args.get('account_ids', '')
        tag_ids_str = request.args.get('tag_ids', '')

        start_date, end_date = _resolve_budget_period_range(
            period_type,
            year=year,
            month=month,
            quarter=quarter,
            start_date=start_date,
            end_date=end_date
        )

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
            f"period={period_type}, dates={start_date}~{end_date}, category={category_id_int}, "
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
                },
                'period_start': start_date,
                'period_end': end_date
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
        - forecast_strategy: 预测策略（historical_average/moving_average）
        - months_history: 历史周期数量
    """
    logger.info("[get_period_forecast] 开始获取周期预计")

    try:
        budget_type = int(request.args.get('budget_type', 3))
        period_type = request.args.get('period_type', 'monthly')
        start_date = request.args.get('start_date')
        end_date = request.args.get('end_date')
        forecast_strategy = request.args.get('forecast_strategy', 'historical_average')
        months_history = int(request.args.get('months_history', 6))

        logger.info(
            f"[get_period_forecast] 参数: type={budget_type}, "
            f"period={period_type}, dates={start_date}~{end_date}, "
            f"strategy={forecast_strategy}, months_history={months_history}"
        )

        period_start, period_end = _resolve_budget_period_range(
            period_type,
            start_date=start_date,
            end_date=end_date
        )

        today = date.today()
        start_dt = datetime.strptime(period_start, '%Y-%m-%d').date()
        end_dt = datetime.strptime(period_end, '%Y-%m-%d').date()
        total_days = max((end_dt - start_dt).days + 1, 1)
        if today < start_dt:
            days_elapsed = 0
            days_remaining = total_days
        elif today > end_dt:
            days_elapsed = total_days
            days_remaining = 0
        else:
            days_elapsed = (today - start_dt).days + 1
            days_remaining = max((end_dt - today).days, 0)

        db = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            results = loop.run_until_complete(db.get_period_forecast(
                budget_type=budget_type,
                period_type=period_type,
                start_date=start_date,
                end_date=end_date,
                forecast_strategy=forecast_strategy,
                history_periods=months_history,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        # 计算汇总
        total_forecast = sum(r['forecast_amount'] for r in results)
        mape_values = [r['backtest_mape'] for r in results if r.get('backtest_mape') is not None]
        avg_backtest_mape = round(sum(mape_values) / len(mape_values), 2) if mape_values else None

        logger.info(f"[get_period_forecast] 返回{len(results)}条预测数据")

        return jsonify({
            'success': True,
            'result': {
                'items': results,
                'period_start': period_start,
                'period_end': period_end,
                'periodStart': period_start,
                'periodEnd': period_end,
                'daysElapsed': days_elapsed,
                'daysRemaining': days_remaining,
                'summary': {
                    'total_forecast': round(total_forecast, 2),
                    'count': len(results),
                    'forecast_strategy': forecast_strategy,
                    'history_periods': months_history,
                    'avg_backtest_mape': avg_backtest_mape,
                    'days_elapsed': days_elapsed,
                    'days_remaining': days_remaining
                }
            }
        })

    except Exception as e:
        logger.error("获取周期预计失败: %s", e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/history/snapshot', methods=['POST'])
@log_method
@require_auth
def create_budget_history_snapshot():
    """创建预算执行历史快照。"""
    logger.info('[create_budget_history_snapshot] 开始创建预算执行快照')

    try:
        data = request.get_json(silent=True) or {}
        budget_type = int(data.get('budget_type', 3))
        period_type = data.get('period_type', 'monthly')
        year = int(data.get('year')) if data.get('year') else None
        month = int(data.get('month')) if data.get('month') else None
        quarter = int(data.get('quarter')) if data.get('quarter') else None
        budget_id = int(data.get('budget_id')) if data.get('budget_id') else None
        category_id = int(data.get('category_id')) if data.get('category_id') else None
        account_ids = [int(x) for x in data.get('account_ids', []) if str(x).strip()]
        tag_ids = [int(x) for x in data.get('tag_ids', []) if str(x).strip()]
        start_date, end_date = _resolve_budget_period_range(
            period_type,
            year=year,
            month=month,
            quarter=quarter,
            start_date=data.get('start_date'),
            end_date=data.get('end_date')
        )

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            result = loop.run_until_complete(db.create_budget_execution_snapshots(
                budget_type=budget_type,
                period_type=period_type,
                start_date=start_date,
                end_date=end_date,
                budget_id=budget_id,
                category_id=category_id,
                account_ids=account_ids or None,
                tag_ids=tag_ids or None,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        return jsonify({
            'success': True,
            'result': result
        })

    except Exception as e:
        logger.error('创建预算执行快照失败: %s', e, exc_info=True)
        return jsonify({
            'success': False,
            'error': str(e)
        }), 500


@bp.route('/history', methods=['GET'])
@log_method
@require_auth
def get_budget_history():
    """获取预算执行历史快照。"""
    logger.info('[get_budget_history] 开始获取预算执行历史')

    try:
        budget_type = int(request.args.get('budget_type', 3))
        period_type = request.args.get('period_type', 'monthly')
        year = int(request.args.get('year')) if request.args.get('year') else None
        month = int(request.args.get('month')) if request.args.get('month') else None
        quarter = int(request.args.get('quarter')) if request.args.get('quarter') else None
        budget_id = int(request.args.get('budget_id')) if request.args.get('budget_id') else None
        category_id = int(request.args.get('category_id')) if request.args.get('category_id') else None
        account_ids_str = request.args.get('account_ids', '')
        tag_ids_str = request.args.get('tag_ids', '')
        account_ids = [int(x) for x in account_ids_str.split(',') if x.strip()] if account_ids_str else None
        tag_ids = [int(x) for x in tag_ids_str.split(',') if x.strip()] if tag_ids_str else None
        start_date, end_date = _resolve_budget_period_range(
            period_type,
            year=year,
            month=month,
            quarter=quarter,
            start_date=request.args.get('start_date'),
            end_date=request.args.get('end_date')
        )

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            items = loop.run_until_complete(db.get_budget_execution_history(
                budget_type=budget_type,
                period_type=period_type,
                start_date=start_date,
                end_date=end_date,
                budget_id=budget_id,
                category_id=category_id,
                account_ids=account_ids,
                tag_ids=tag_ids,
                user_id=request.user_id
            ))
        finally:
            loop.close()

        return jsonify({
            'success': True,
            'result': {
                'items': items,
                'summary': {
                    'count': len(items),
                    'period_start': start_date,
                    'period_end': end_date
                }
            }
        })

    except Exception as e:
        logger.error('获取预算执行历史失败: %s', e, exc_info=True)
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









