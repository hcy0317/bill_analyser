"""
分类 API 路由
"""

# pylint: disable=too-many-nested-blocks,too-many-lines

import json
import sqlite3
from typing import cast

from flask import Blueprint, Response, current_app, jsonify, request

from bill_analyser.api.adapters.category_adapter import CategoryAdapter
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.utils.config import save_config
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("CategoriesAPI")

bp = Blueprint("categories", __name__)
category_adapter = CategoryAdapter()

# 分类图标映射 (MDI图标)
CATEGORY_ICONS = {
    "餐饮": "mdi-food",
    "交通": "mdi-bus",
    "购物": "mdi-shopping",
    "娱乐": "mdi-gamepad-variant",
    "居住": "mdi-home",
    "通讯": "mdi-phone",
    "人情": "mdi-account-group",
    "医疗": "mdi-medical-bag",
    "教育": "mdi-school",
    "投资": "mdi-chart-line",
    "收入": "mdi-cash-plus",
    "转账": "mdi-bank-transfer",
    "其他": "mdi-dots-horizontal",
    "工资": "mdi-wallet-membership",
    "奖金": "mdi-gift",
    "兼职": "mdi-briefcase-clock",
    "理财": "mdi-finance",
    "服饰": "mdi-tshirt-crew",
    "日用": "mdi-basket",
    "数码": "mdi-laptop",
    "美容": "mdi-lipstick",
    "装修": "mdi-hammer",
    "房贷": "mdi-home-city",
    "房租": "mdi-home-account",
    "水电": "mdi-water",
    "话费": "mdi-cellphone",
    "网费": "mdi-wifi",
    "红包": "mdi-email-open-outline",
    "礼金": "mdi-gift-outline",
    "药品": "mdi-pill",
    "治疗": "mdi-hospital",
    "学费": "mdi-book-open-variant",
    "书籍": "mdi-book",
    "培训": "mdi-presentation",
    "打车": "mdi-taxi",
    "加油": "mdi-gas-station",
    "停车": "mdi-parking",
    "地铁": "mdi-subway",
    "火车": "mdi-train",
    "机票": "mdi-airplane",
}


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return get_required_request_int("user_id")


def get_app_context(user_id: int | None = None):
    """获取应用上下文中的服务实例

    参数：
        user_id: 用户ID (如果为None，自动从request获取)
    """
    db = cast("Any", current_app.config.get("DB_INSTANCE"))
    bill_service = cast("Any", current_app.config.get("BILL_SERVICE_INSTANCE"))
    category_engine = cast("Any", current_app.config.get("CATEGORY_ENGINE_INSTANCE"))

    # 自动获取user_id
    if user_id is None:
        user_id = _get_request_user_id()

    return db, bill_service, category_engine

__all__ = [name for name in globals() if not name.startswith("__")]
