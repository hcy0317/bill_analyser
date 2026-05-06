"""
账单 API 路由

重构后使用统一事务适配器进行数据格式转换，
消除冗余代码，提升性能和可维护性。
"""

import asyncio
import base64
import csv
import inspect
import json
import mimetypes
import os
import re
import uuid
from datetime import datetime
from pathlib import Path
from typing import Any

from flask import Blueprint, jsonify, request
from werkzeug.utils import secure_filename

from bill_analyser.api.config.bills import (
    ALLOWED_BILLS_FILE_EXTENSIONS,
    ALLOWED_BILLS_PICTURE_EXTENSIONS,
    AUTO_TRANSACTION_TYPE_MAPPING,
    DEFAULT_BILL_CATEGORY_MAPPING,
    GENERIC_IMPORT_DATE_HEADERS,
    GENERIC_IMPORT_EXPENSE_AMOUNT_HEADERS,
    GENERIC_IMPORT_INCOME_AMOUNT_HEADERS,
    GENERIC_IMPORT_TIME_HEADERS,
    IMPORT_COLUMN_TYPE_KEYWORDS,
    IMPORT_CONFIG_BASE_WEIGHT,
    IMPORT_CONFIG_USE_COUNT_CAP,
    IMPORT_CONFIG_USE_COUNT_FACTOR,
    IMPORT_HEADER_ACCEPT_SCORE,
    IMPORT_HEADER_CANDIDATE_MIN_SCORE,
    IMPORT_HEADER_CONTEXT_SCORE,
    IMPORT_HEADER_DATA_LIKE_PENALTY,
    IMPORT_HEADER_EXACT_SCORE,
    IMPORT_HEADER_LOW_MATCH_TYPES_PENALTY,
    IMPORT_HEADER_MATCH_REPEAT_BONUS,
    IMPORT_HEADER_MATCH_UNIQUE_BONUS,
    IMPORT_HEADER_MATCHED_TYPES_WEIGHT,
    IMPORT_HEADER_MIN_MATCHED_TYPES,
    IMPORT_HEADER_MIN_REVIEW_SCORE,
    IMPORT_HEADER_NEXT_ROW_SCORE_CAP,
    IMPORT_HEADER_NEXT_ROW_WEIGHT,
    IMPORT_HEADER_NON_EMPTY_CELL_CAP,
    IMPORT_HEADER_NON_EMPTY_CELL_WEIGHT,
    IMPORT_HEADER_PARTIAL_SCORE,
    IMPORT_HEADER_SCAN_LIMIT,
    IMPORT_HEADER_SEMANTIC_MEDIUM_SCORE,
    IMPORT_HEADER_SEMANTIC_SCORE,
    IMPORT_HEADER_STRONG_EXACT_SCORE,
    IMPORT_HEADER_TYPE_HINT_SCORE,
    LEGACY_IMPORT_FIELD_TO_COLUMN_TYPE,
    MAX_BILLS_FILE_SIZE,
)

try:
    import openpyxl
except ImportError:  # pragma: no cover - 依赖在运行环境通常存在
    openpyxl = None

try:
    import pandas as pd
except ImportError:  # pragma: no cover - 依赖在运行环境通常存在
    pd = None

from bill_analyser.api.adapters.transaction_adapter import TransactionAdapter
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.constants import UPLOADS_DIR
from bill_analyser.core.bill_date_utils import parse_bill_datetime
from bill_analyser.import_contracts.parser_tags import resolve_parser_tags
from bill_analyser.import_contracts.preview_selection import preview_update_is_selected
from bill_analyser.utils.constants import BACKEND_TO_FRONTEND_TYPE
from bill_analyser.utils.currency import yuan_to_cents  # 金额单位转换工具
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("BillsAPI")

# 主蓝图 - 现代RESTful API
bp = Blueprint("bills", __name__)

# 上传文件配置 - 固定到 data/uploads
UPLOAD_FOLDER = UPLOADS_DIR
ALLOWED_EXTENSIONS = set(ALLOWED_BILLS_FILE_EXTENSIONS)
ALLOWED_PICTURE_EXTENSIONS = set(ALLOWED_BILLS_PICTURE_EXTENSIONS)
MAX_FILE_SIZE = MAX_BILLS_FILE_SIZE

# 确保上传目录存在
UPLOAD_FOLDER.mkdir(parents=True, exist_ok=True)

def _get_query_arg(args, *names, default=None):
    """按顺序获取第一个非空查询参数。"""
    for name in names:
        value = args.get(name)
        if value not in (None, ""):
            return value
    return default


def _parse_int_list(raw_value: str):
    """解析逗号分隔的整数列表。"""
    if not raw_value:
        return []
    return [int(item) for item in str(raw_value).split(",") if str(item).strip()]


async def _apply_common_transaction_filters(args, filters, db, user_id: int):
    """应用交易列表公共筛选条件。"""
    keyword = _get_query_arg(args, "keyword", default="")
    if keyword:
        from urllib.parse import unquote

        filters["keyword"] = unquote(keyword)

    account_ids_str = _get_query_arg(args, "accountIds", "account_ids", default="")
    if account_ids_str:
        try:
            account_ids = _parse_int_list(account_ids_str)
            if account_ids:
                filters["account_ids"] = account_ids
        except ValueError:
            logger.warning("无效的account_ids参数: %s", account_ids_str)

    category_ids_str = _get_query_arg(args, "categoryIds", "category_ids", default="")
    if category_ids_str:
        try:
            category_ids = _parse_int_list(category_ids_str)
            if category_ids:
                all_categories = await db.get_all_categories(user_id=user_id)
                target_categories = []

                for cat in all_categories:
                    if cat["id"] in category_ids:
                        target_categories.append({"main": cat.get("main_category"), "sub": cat.get("sub_category")})

                if target_categories:
                    # 主蓝图 - 现代RESTful API
                    filters["categories"] = target_categories
        except ValueError:
            logger.warning("无效的category_ids参数: %s", category_ids_str)

    tag_ids_str = _get_query_arg(args, "tagIds", "tag_ids", default="")
    if tag_ids_str:
        try:
            tag_ids = _parse_int_list(tag_ids_str)
            if tag_ids:
                filters["tag_ids"] = tag_ids
        except ValueError:
            logger.warning("无效的tag_ids参数: %s", tag_ids_str)

    amount_filter = _get_query_arg(args, "amountFilter", "amount_filter", default="")
    if amount_filter:
        filters["amount_filter"] = amount_filter


def allowed_file(filename):
    """检查文件扩展名是否允许"""
    return "." in filename and filename.rsplit(".", 1)[1].lower() in ALLOWED_BILLS_FILE_EXTENSIONS

def allowed_picture_file(filename):
    """检查图片扩展名是否允许。"""
    return "." in filename and filename.rsplit(".", 1)[1].lower() in ALLOWED_BILLS_PICTURE_EXTENSIONS


def _parse_json_form_field(raw_value, default):
    """解析 multipart/form-data 中的 JSON 字段。"""
    if raw_value in (None, ""):
        return default

    try:
        return json.loads(raw_value)
    except (TypeError, ValueError):
        return default


def _parse_bool_form_field(raw_value, default=False):
    """解析布尔表单字段。"""
    if raw_value in (None, ""):
        return default

    return str(raw_value).strip().lower() in ("1", "true", "yes", "on")

def get_app_context(user_id: int = None):
    """获取应用上下文中的基础服务实例。"""
    from flask import current_app

    db = current_app.config.get("DB_INSTANCE")
    bill_service = current_app.config.get("BILL_SERVICE_INSTANCE")
    category_engine = current_app.config.get("CATEGORY_ENGINE_INSTANCE")

    # 自动获取user_id
    if user_id is None:
        user_id = getattr(request, "user_id", 1)

    _ = user_id

    return db, bill_service, category_engine


def get_app_context_with_adapter(user_id: int = None):
    """获取应用上下文中的基础服务实例及事务适配器。"""
    db, bill_service, category_engine = get_app_context(user_id=user_id)

    if user_id is None:
        user_id = getattr(request, "user_id", 1)

    # 创建adapter实例(带数据库引用和用户ID)
    adapter = TransactionAdapter(db=db, user_id=user_id)

    return db, bill_service, category_engine, adapter


async def sync_balances_for_bill(db, bill_data):
    """同步账单相关账户的余额

    Args:
        db: 数据库实例
        bill_data: 账单数据字典 (包含 source_account_id, destination_account_id)
    """
    try:
        # 同步源账户
        source_id = bill_data.get("source_account_id")
        if source_id:
            await db.sync_account_balance(int(source_id))
            logger.info("已同步源账户余额: %s", source_id)

        # 同步目标账户 (转账/投资)
        dest_id = bill_data.get("destination_account_id")
        if dest_id:
            await db.sync_account_balance(int(dest_id))
            logger.info("已同步目标账户余额: %s", dest_id)

    except Exception as error:
        logger.error("同步账户余额失败: %s", error, exc_info=True)


def get_category_id_from_names(main_category: str, sub_category: str, db, user_id: int = 1) -> str:
    """
    根据主分类和子分类名称查询分类ID

    Args:
        main_category: 主分类名称
        sub_category: 子分类名称
        db: 数据库实例
        user_id: 用户ID

    Returns:
        分类ID字符串，找不到返回'0'
    """
    if not main_category:
        return "0"

    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
    loop.close()

    for cat in categories:
        if cat.get("main_category") == main_category and cat.get("sub_category", "") == sub_category:
            return str(cat["id"])

    return "0"


def parse_bill_date(date_str):
    """
    解析账单日期字段,支持多种格式

    Args:
        date_str: 日期字符串,可能是 '%Y-%m-%d' 或 '%Y-%m-%d %H:%M:%S'

    Returns:
        datetime对象
    """
    # 先尝试完整格式(包含时分秒)
    try:
        return datetime.strptime(date_str, "%Y-%m-%d %H:%M:%S")
    except ValueError:
        pass

    # 回退到只有日期的格式
    try:
        return datetime.strptime(date_str, "%Y-%m-%d")
    except ValueError as error:
        logger.error("无法解析日期字符串: %s, 错误: %s", date_str, error)
        return datetime.now()  # 返回当前时间作为默认值


def _match_account_by_name(accounts: list, name: str) -> dict:
    """通过名称、别名或包含关系匹配账户。"""
    if not name:
        return None

    name_lower = name.lower().strip()
    for account in accounts:
        if account.get("name", "").lower() == name_lower:
            return account

        aliases_str = account.get("aliases", "") or account.get("comment", "")
        if aliases_str:
            aliases = [a.strip().lower() for a in aliases_str.split(",")]
            if name_lower in aliases:
                return account

    fuzzy_matches: list[tuple[int, dict[str, Any]]] = []
    for account in accounts:
        account_name = account.get("name", "").lower().strip()
        if account_name and (name_lower in account_name or account_name in name_lower):
            fuzzy_matches.append((len(account_name), account))

    if fuzzy_matches:
        fuzzy_matches.sort(key=lambda item: item[0], reverse=True)
        return fuzzy_matches[0][1]

    return None


__all__ = [name for name in globals() if not name.startswith("__")]
