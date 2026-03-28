"""
Authentication Routes - 认证相关API端点

提供用户登录、注册、登出等认证功能
"""

# pylint: disable=too-many-lines,broad-exception-caught,line-too-long,import-outside-toplevel,duplicate-code
# pylint: disable=too-many-return-statements,too-many-branches,too-many-locals,too-many-statements
# pylint: disable=too-many-arguments,too-many-positional-arguments

import asyncio
import base64
import csv
import hashlib
import io
import json
import mimetypes
import secrets
from datetime import datetime, timedelta
from typing import Any, cast

import bcrypt
import jwt
import pyotp
import qrcode
from flask import Blueprint, Response, current_app, jsonify, request

from bill_analyser import __version__
from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
    get_required_request_str,
)
from bill_analyser.core.investment_settings import (
    build_user_investment_keyword_settings,
    serialize_keyword_list,
)
from bill_analyser.utils.config import load_auth_settings as load_server_auth_settings
from bill_analyser.utils.constants import FRONTEND_TO_BACKEND_TYPE
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("AuthAPI")

bp = Blueprint("auth", __name__)

TOKEN_TYPE_DEFAULT = 0
TOKEN_TYPE_MCP = 5
TOKEN_TYPE_API = 8

CLOUD_SETTING_TYPE_STRING = "string"
CLOUD_SETTING_TYPE_NUMBER = "number"
CLOUD_SETTING_TYPE_BOOLEAN = "boolean"
CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP = "string_boolean_map"

SUPPORTED_APPLICATION_CLOUD_SETTING_KEY_TYPES = {
    "showAccountBalance": CLOUD_SETTING_TYPE_BOOLEAN,
    "showAmountInHomePage": CLOUD_SETTING_TYPE_BOOLEAN,
    "timezoneUsedForStatisticsInHomePage": CLOUD_SETTING_TYPE_NUMBER,
    "overviewAccountFilterInHomePage": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "overviewTransactionCategoryFilterInHomePage": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "itemsCountInTransactionListPage": CLOUD_SETTING_TYPE_NUMBER,
    "showTotalAmountInTransactionListPage": CLOUD_SETTING_TYPE_BOOLEAN,
    "showTagInTransactionListPage": CLOUD_SETTING_TYPE_BOOLEAN,
    "autoSaveTransactionDraft": CLOUD_SETTING_TYPE_STRING,
    "autoGetCurrentGeoLocation": CLOUD_SETTING_TYPE_BOOLEAN,
    "alwaysShowTransactionPicturesInMobileTransactionEditPage": CLOUD_SETTING_TYPE_BOOLEAN,
    "totalAmountExcludeAccountIds": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "currencySortByInExchangeRatesPage": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultChartDataType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultTimezoneType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultAccountFilter": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "statistics.defaultTransactionCategoryFilter": CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP,
    "statistics.defaultSortingType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultCategoricalChartType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultCategoricalChartDataRangeType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultTrendChartType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultTrendChartDataRangeType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultAssetTrendsChartType": CLOUD_SETTING_TYPE_NUMBER,
    "statistics.defaultAssetTrendsChartDataRangeType": CLOUD_SETTING_TYPE_NUMBER,
}

TWO_FACTOR_RECOVERY_CODES: dict[int, list[str]] = {}


def load_auth_config():
    """加载认证配置"""
    return load_server_auth_settings()


def get_app_context():
    """获取应用上下文"""
    db = cast(Any, current_app.config.get("DB_INSTANCE"))
    if db is None:
        # 回退方案：尝试从模块导入
        import bill_analyser.api.app as app_module

        db = app_module.db
    if db is None:
        raise RuntimeError("Database not initialized. Please ensure server is properly started.")
    return db


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return get_required_request_int("user_id")


def _get_request_username() -> str:
    """获取认证中间件注入的当前用户名。"""
    return get_required_request_str("username")


def _get_request_session_id() -> int:
    """获取认证中间件注入的当前会话 ID。"""
    return get_required_request_int("session_id")


def get_client_ip():
    """获取客户端IP地址"""
    forwarded_for = request.headers.get("X-Forwarded-For")
    if forwarded_for:
        return forwarded_for.split(",")[0].strip()

    real_ip = request.headers.get("X-Real-IP")
    if real_ip:
        return real_ip

    return request.remote_addr or ""


def calculate_token_hash(token: str) -> str:
    """计算令牌哈希值"""
    return hashlib.sha256(token.encode("utf-8")).hexdigest()


def generate_access_token(
    user_id: int,
    username: str,
    config: dict,
    expires_in_seconds: int = 0,
    token_kind: str = "session",
) -> dict:
    """生成单个访问令牌，用于 API/MCP token 与普通会话。"""
    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")

    now = datetime.now()
    if expires_in_seconds and expires_in_seconds > 0:
        expires_at = now + timedelta(seconds=expires_in_seconds)
    else:
        # 前端的“不过期”语义使用超长有效期实现。
        expires_at = now + timedelta(days=365 * 100)

    payload = {
        "user_id": user_id,
        "username": username,
        "type": "access",
        "token_kind": token_kind,
        "iat": int(now.timestamp()),
        "exp": int(expires_at.timestamp()),
        "nonce": secrets.token_hex(16),
    }
    access_token = jwt.encode(payload, jwt_secret, algorithm=jwt_algorithm)

    return {
        "access_token": access_token,
        "expires_at": expires_at.isoformat(),
    }


def _verify_user_password(user: dict, password: str) -> bool:
    """校验当前用户密码。"""
    password_hash = (user or {}).get("password_hash", "")
    if not password_hash or not password:
        return False
    return bcrypt.checkpw(password.encode("utf-8"), password_hash.encode("utf-8"))


def _build_external_auth_info(external_auth: dict) -> dict:
    """构建统一的第三方登录响应。"""
    return {
        "externalAuthCategory": external_auth.get("external_auth_category", ""),
        "externalAuthType": external_auth.get("external_auth_type", ""),
        "linked": bool(external_auth.get("linked", True)),
        "externalUsername": external_auth.get("external_username") or "",
        "createdAt": _datetime_to_unix_millis(external_auth.get("created_at", "")),
    }


def _build_application_cloud_setting_info(setting: dict) -> dict:
    """构建统一的应用云同步设置响应。"""
    return {
        "settingKey": setting.get("setting_key", ""),
        "settingValue": setting.get("setting_value", ""),
    }


def _load_application_cloud_settings(db, user_id: int, loop) -> list[dict]:
    """读取用户应用云同步设置。"""
    settings = loop.run_until_complete(db.get_user_application_cloud_settings(user_id))
    return [_build_application_cloud_setting_info(setting) for setting in settings]


def _validate_application_cloud_setting(setting: dict) -> str:
    """校验单条应用云同步设置，返回错误信息，合法时返回空字符串。"""
    setting_key = str((setting or {}).get("settingKey", "") or "").strip()
    setting_value = (setting or {}).get("settingValue", "")

    if not setting_key:
        return "settingKey is required"

    setting_type = SUPPORTED_APPLICATION_CLOUD_SETTING_KEY_TYPES.get(setting_key)
    if not setting_type:
        return f"Unsupported setting key: {setting_key}"

    if not isinstance(setting_value, str):
        return f"Invalid setting value for {setting_key}"

    if setting_type == CLOUD_SETTING_TYPE_STRING:
        return ""

    if setting_type == CLOUD_SETTING_TYPE_NUMBER:
        try:
            float(setting_value)
            return ""
        except (TypeError, ValueError):
            return f"Invalid number value for {setting_key}"

    if setting_type == CLOUD_SETTING_TYPE_BOOLEAN:
        if setting_value in ("true", "false"):
            return ""
        return f"Invalid boolean value for {setting_key}"

    if setting_type == CLOUD_SETTING_TYPE_STRING_BOOLEAN_MAP:
        try:
            parsed = json.loads(setting_value)
        except (TypeError, ValueError, json.JSONDecodeError):
            return f"Invalid JSON value for {setting_key}"

        if not isinstance(parsed, dict):
            return f"Invalid map value for {setting_key}"

        for map_key, map_value in parsed.items():
            if not isinstance(map_key, str) or not isinstance(map_value, bool):
                return f"Invalid map value for {setting_key}"
        return ""

    return f"Unsupported setting type for {setting_key}"


def _normalize_application_cloud_settings(settings: list[dict]) -> list[dict]:
    """标准化应用云同步设置请求。"""
    normalized_settings = []

    for setting in settings:
        normalized_settings.append(
            {
                "setting_key": str(setting.get("settingKey", "") or "").strip(),
                "setting_value": str(setting.get("settingValue", "") or ""),
            }
        )

    return normalized_settings


def _build_user_profile_info(user: dict) -> dict:
    """构建统一的用户资料响应。"""
    username = user.get("username", "")
    email = user.get("email", "")
    nickname = user.get("nickname") or username
    investment_keyword_settings = build_user_investment_keyword_settings(user)
    default_account_id = user.get("default_account_id")
    cash_account_id = user.get("cash_account_id")
    cash_transfer_category_id = user.get("cash_transfer_category_id")

    return {
        "username": username,
        "email": email,
        "nickname": nickname,
        "avatar": user.get("avatar") or "",
        "avatarProvider": "internal",
        "defaultAccountId": str(default_account_id) if default_account_id not in (None, "") else "",
        "transactionEditScope": user.get("transaction_edit_scope", 0),
        "language": user.get("language") or "zh_Hans",
        "defaultCurrency": user.get("default_currency") or "CNY",
        "firstDayOfWeek": user.get("first_day_of_week", 1),
        "fiscalYearStart": user.get("fiscal_year_start", 1),
        "calendarDisplayType": user.get("calendar_display_type", 0),
        "dateDisplayType": user.get("date_display_type", 0),
        "longDateFormat": user.get("long_date_format", 0),
        "shortDateFormat": user.get("short_date_format", 0),
        "longTimeFormat": user.get("long_time_format", 0),
        "shortTimeFormat": user.get("short_time_format", 0),
        "fiscalYearFormat": user.get("fiscal_year_format", 0),
        "currencyDisplayType": user.get("currency_display_type", 0),
        "numeralSystem": user.get("numeral_system", 0),
        "decimalSeparator": user.get("decimal_separator", 0),
        "digitGroupingSymbol": user.get("digit_grouping_symbol", 0),
        "digitGrouping": user.get("digit_grouping", 0),
        "coordinateDisplayType": user.get("coordinate_display_type", 0),
        "expenseAmountColor": user.get("expense_amount_color", 0),
        "incomeAmountColor": user.get("income_amount_color", 0),
        "cashAccountId": str(cash_account_id) if cash_account_id not in (None, "") else "",
        "cashTransferCategoryId": str(cash_transfer_category_id) if cash_transfer_category_id not in (None, "") else "",
        "importLearningEnabled": bool(user.get("import_learning_enabled", True)),
        "investmentPlatformKeywords": investment_keyword_settings["platform_keywords"],
        "investmentProductKeywords": investment_keyword_settings["product_keywords"],
        "investmentExcludeKeywords": investment_keyword_settings["exclude_keywords"],
        "emailVerified": bool(user.get("email_verified", False)),
    }


def _extract_bearer_token() -> str:
    """从请求头提取 Bearer token。"""
    auth_header = request.headers.get("Authorization", "")
    parts = auth_header.split()

    if len(parts) != 2 or parts[0].lower() != "bearer":
        return ""

    return parts[1]


def _build_auth_success_result(user: dict, tokens: dict, application_cloud_settings: list[dict] | None = None) -> dict:
    """构建统一的认证成功响应结构。"""
    return {
        "token": tokens["access_token"],
        "refreshToken": tokens.get("refresh_token"),
        "need2FA": False,
        "user": user,
        "applicationCloudSettings": application_cloud_settings or [],
    }


def _set_recovery_codes(user_id: int, recovery_codes: list[str]) -> None:
    """缓存用户当前有效的 2FA 恢复码。"""
    TWO_FACTOR_RECOVERY_CODES[user_id] = [str(code).upper() for code in recovery_codes]


def _consume_recovery_code(user_id: int, recovery_code: str) -> bool:
    """消费单个恢复码。"""
    normalized = str(recovery_code or "").strip().upper()
    current_codes = TWO_FACTOR_RECOVERY_CODES.get(user_id, [])

    if normalized not in current_codes:
        return False

    current_codes.remove(normalized)
    TWO_FACTOR_RECOVERY_CODES[user_id] = current_codes
    return True


def _build_avatar_data_url(uploaded_file) -> str:
    """将上传头像文件转为 data URL。"""
    content = uploaded_file.read()
    if not content:
        raise ValueError("Avatar file is empty")

    mime_type = uploaded_file.mimetype or mimetypes.guess_type(uploaded_file.filename or "")[0]
    if not mime_type:
        mime_type = "application/octet-stream"

    encoded = base64.b64encode(content).decode("utf-8")
    return f"data:{mime_type};base64,{encoded}"


def _generate_2fa_qrcode_data_url(username: str, secret: str) -> str:
    """生成 2FA 二维码 data URL。"""
    provisioning_uri = pyotp.TOTP(secret).provisioning_uri(name=username, issuer_name="Bill Analyser")

    qr_image = qrcode.make(provisioning_uri)
    buffer = io.BytesIO()
    qr_image.save(buffer, "PNG")
    encoded = base64.b64encode(buffer.getvalue()).decode("utf-8")
    return f"data:image/png;base64,{encoded}"


def _generate_recovery_codes() -> list[str]:
    """生成恢复码列表。"""
    return [f"{secrets.token_hex(4)[:4]}-{secrets.token_hex(4)[:4]}".upper() for _ in range(8)]


def _create_new_session_payload(user_id: int, username: str, config: dict, db, loop) -> dict:
    """生成新的会话 token 并写入 session。"""
    tokens = generate_jwt_token(user_id, username, config)

    new_token_hash = calculate_token_hash(tokens["access_token"])
    new_refresh_hash = calculate_token_hash(tokens["refresh_token"])

    loop.run_until_complete(
        db.create_session(
            {
                "user_id": user_id,
                "token_hash": new_token_hash,
                "refresh_token_hash": new_refresh_hash,
                "expires_at": tokens["expires_at"],
                "refresh_expires_at": tokens["refresh_expires_at"],
                "user_agent": request.headers.get("User-Agent", ""),
                "ip_address": get_client_ip(),
            }
        )
    )

    return tokens


def _build_api_base_url() -> str:
    """构建 API token 示例使用的 API 基础地址。"""
    return f"{request.url_root.rstrip('/')}/api"


def _build_mcp_url() -> str:
    """构建 MCP token 示例地址。"""
    return f"{request.url_root.rstrip('/')}/mcp"


def _get_token_user_agent(token_kind: str) -> str:
    """为生成的 token 构造稳定的 user_agent 标识。"""
    if token_kind == "api":
        return "Bill Analyser API Token"
    if token_kind == "mcp":
        return "Bill Analyser MCP Token"
    return request.headers.get("User-Agent", "")


def _infer_token_type(user_agent: str) -> int:
    """根据 user_agent 推断 token 类型，兼容前端 Session 解析。"""
    user_agent_lower = (user_agent or "").lower()
    if "mcp token" in user_agent_lower:
        return TOKEN_TYPE_MCP
    if "api token" in user_agent_lower:
        return TOKEN_TYPE_API
    return TOKEN_TYPE_DEFAULT


def _datetime_to_unix_millis(value: str) -> int:
    """将 ISO 时间字符串转换为 Unix 毫秒。"""
    if not value:
        return 0

    try:
        return int(datetime.fromisoformat(value).timestamp() * 1000)
    except (TypeError, ValueError):
        return 0


def _parse_comma_separated_ints(raw_value: str) -> list[int]:
    """解析逗号分隔的整数列表。"""
    if not raw_value:
        return []

    result: list[int] = []
    for item in raw_value.split(","):
        item = item.strip()
        if not item:
            continue
        try:
            result.append(int(item))
        except ValueError:
            logger.warning("忽略非法整数参数: %s", item)
    return result


def _parse_export_datetime(raw_value: str) -> str | None:
    """将前端 Unix ms 时间戳转换为数据库日期字符串。"""
    if not raw_value or raw_value == "0":
        return None
    try:
        return datetime.fromtimestamp(int(raw_value) / 1000).strftime("%Y-%m-%d %H:%M:%S")
    except (TypeError, ValueError, OSError):
        logger.warning("非法时间戳参数: %s", raw_value)
        return None


def _build_export_filters(categories: list[dict[str, Any]]) -> dict[str, Any]:
    """构建导出接口使用的账单筛选条件。"""
    filters: dict[str, Any] = {}

    start_date = _parse_export_datetime(request.args.get("min_time", "0"))
    end_date = _parse_export_datetime(request.args.get("max_time", "0"))
    if start_date:
        filters["start_date"] = start_date
    if end_date:
        filters["end_date"] = end_date

    type_value = request.args.get("type", "0")
    if type_value and type_value != "0":
        try:
            filters["type"] = FRONTEND_TO_BACKEND_TYPE.get(int(type_value), "")
        except ValueError:
            filters["type"] = type_value
        if not filters["type"]:
            filters.pop("type", None)

    keyword = (request.args.get("keyword") or "").strip()
    if keyword:
        filters["keyword"] = keyword

    amount_filter = (request.args.get("amount_filter") or "").strip()
    if amount_filter:
        filters["amount_filter"] = amount_filter

    account_ids = _parse_comma_separated_ints(request.args.get("account_ids", ""))
    if account_ids:
        filters["account_ids"] = account_ids

    tag_ids = _parse_comma_separated_ints(request.args.get("tag_ids", ""))
    if tag_ids:
        filters["tag_ids"] = tag_ids

    category_ids = _parse_comma_separated_ints(request.args.get("category_ids", ""))
    if category_ids:
        category_map = {int(cat["id"]): cat for cat in categories if cat.get("id") is not None}
        selected_categories = []
        for category_id in category_ids:
            category = category_map.get(category_id)
            if category:
                selected_categories.append(
                    {"main": category.get("main_category", ""), "sub": category.get("sub_category", "")}
                )
        if selected_categories:
            filters["categories"] = selected_categories

    return filters


def _render_bills_export(
    bills: list[dict[str, Any]],
    accounts: list[dict[str, Any]],
    tags_map: dict[int, list[dict[str, Any]]],
    delimiter: str,
) -> str:
    """渲染账单导出文本。"""
    output = io.StringIO()
    writer = csv.writer(output, delimiter=delimiter, lineterminator="\n")
    writer.writerow(
        [
            "id",
            "date",
            "type",
            "amount",
            "main_category",
            "sub_category",
            "source_account",
            "destination_account",
            "counterparty",
            "payment_method",
            "description",
            "tags",
            "comment",
            "created_at",
            "updated_at",
        ]
    )

    account_map = {int(account["id"]): account.get("name", "") for account in accounts}
    for bill in bills:
        bill_id = bill.get("id")
        normalized_bill_id = int(bill_id) if bill_id is not None else -1
        tag_names = "|".join(tag.get("name", "") for tag in tags_map.get(normalized_bill_id, []))
        writer.writerow(
            [
                bill_id or "",
                bill.get("date", ""),
                bill.get("type", ""),
                bill.get("amount", 0),
                bill.get("main_category", ""),
                bill.get("sub_category", ""),
                account_map.get(int(bill.get("source_account_id") or 0), ""),
                account_map.get(int(bill.get("destination_account_id") or 0), ""),
                bill.get("counterparty", ""),
                bill.get("payment_method", ""),
                bill.get("description", ""),
                tag_names,
                bill.get("comment", ""),
                bill.get("created_at", ""),
                bill.get("updated_at", ""),
            ]
        )

    return output.getvalue()


def generate_jwt_token(user_id: int, username: str, config: dict) -> dict:
    """
    生成JWT令牌和刷新令牌

    Returns:
        dict: {
            'access_token': str,
            'refresh_token': str,
            'expires_at': str,
            'refresh_expires_at': str
        }
    """
    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")
    access_exp_days = config.get("jwt_expiration_days", 7)
    refresh_exp_days = config.get("refresh_token_expiration_days", 30)

    now = datetime.now()
    access_expires_at = now + timedelta(days=access_exp_days)
    refresh_expires_at = now + timedelta(days=refresh_exp_days)

    # 生成访问令牌（添加nonce确保唯一性）
    access_payload = {
        "user_id": user_id,
        "username": username,
        "type": "access",
        "iat": int(now.timestamp()),
        "exp": int(access_expires_at.timestamp()),
        "nonce": secrets.token_hex(16),  # 添加32字符随机值
    }
    access_token = jwt.encode(access_payload, jwt_secret, algorithm=jwt_algorithm)

    # 生成刷新令牌（添加nonce确保唯一性）
    refresh_payload = {
        "user_id": user_id,
        "username": username,
        "type": "refresh",
        "iat": int(now.timestamp()),
        "exp": int(refresh_expires_at.timestamp()),
        "nonce": secrets.token_hex(16),  # 添加32字符随机值
    }
    refresh_token_str = jwt.encode(refresh_payload, jwt_secret, algorithm=jwt_algorithm)

    return {
        "access_token": access_token,
        "refresh_token": refresh_token_str,
        "expires_at": access_expires_at.isoformat(),
        "refresh_expires_at": refresh_expires_at.isoformat(),
    }


def generate_action_token(
    user_id: int,
    username: str,
    email: str,
    config: dict,
    token_type: str,
    expires_in_hours: int = 24,
) -> str:
    """生成用于邮箱验证/重置密码的一次性动作令牌。"""
    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")
    now = datetime.now()
    expires_at = now + timedelta(hours=expires_in_hours)

    payload = {
        "user_id": user_id,
        "username": username,
        "email": email,
        "type": token_type,
        "iat": int(now.timestamp()),
        "exp": int(expires_at.timestamp()),
        "nonce": secrets.token_hex(16),
    }
    return jwt.encode(payload, jwt_secret, algorithm=jwt_algorithm)


def decode_action_token(token: str, config: dict, expected_type: str) -> dict | None:
    """解码并校验动作令牌。"""
    if not token:
        return None

    jwt_secret = config.get("jwt_secret")
    jwt_algorithm = config.get("jwt_algorithm", "HS256")

    try:
        payload = jwt.decode(token, jwt_secret, algorithms=[jwt_algorithm])
    except jwt.ExpiredSignatureError:
        logger.warning("动作令牌已过期: expected_type=%s", expected_type)
        return None
    except jwt.InvalidTokenError:
        logger.warning("动作令牌无效: expected_type=%s", expected_type)
        return None

    if payload.get("type") != expected_type:
        logger.warning("动作令牌类型不匹配: expected=%s actual=%s", expected_type, payload.get("type"))
        return None

    return payload


def validate_password(password: str, config: dict) -> tuple:
    """
    验证密码强度

    Returns:
        tuple: (is_valid: bool, error_message: str)
    """
    min_length = config.get("password_min_length", 8)

    if len(password) < min_length:
        return False, f"Password must be at least {min_length} characters long"

    if config.get("password_require_uppercase", False):
        if not any(c.isupper() for c in password):
            return False, "Password must contain at least one uppercase letter"

    if config.get("password_require_lowercase", False):
        if not any(c.islower() for c in password):
            return False, "Password must contain at least one lowercase letter"

    if config.get("password_require_digit", False):
        if not any(c.isdigit() for c in password):
            return False, "Password must contain at least one digit"

    if config.get("password_require_special", False):
        special_chars = "!@#$%^&*()_+-=[]{}|;:,.<>?"
        if not any(c in special_chars for c in password):
            return False, "Password must contain at least one special character"

    return True, ""


def _build_default_accounts(language: str = "zh_Hans") -> list[dict[str, Any]]:
    """构建默认账户模板（国内常见账户）"""
    is_chinese = str(language).lower().startswith("zh")

    if is_chinese:
        return [
            {
                "name": "现金",
                "type": 1,
                "category": 1,
                "currency": "CNY",
                "icon": "1",
                "color": "4caf50",
                "aliases": json.dumps(["现金", "现金钱包", "cash"], ensure_ascii=False),
                "display_order": 0,
            },
            {
                "name": "借记卡",
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "icon": "100",
                "color": "2196f3",
                "aliases": json.dumps(["借记卡", "储蓄卡", "银行卡", "debit card"], ensure_ascii=False),
                "display_order": 1,
            },
            {
                "name": "信用卡",
                "type": 1,
                "category": 3,
                "currency": "CNY",
                "icon": "100",
                "color": "ff9800",
                "aliases": json.dumps(["信用卡", "贷记卡", "credit card"], ensure_ascii=False),
                "display_order": 2,
            },
            {
                "name": "支付宝",
                "type": 1,
                "category": 4,
                "currency": "CNY",
                "icon": "500",
                "color": "1677ff",
                "aliases": json.dumps(["支付宝", "alipay", "花呗", "余额宝"], ensure_ascii=False),
                "display_order": 3,
            },
            {
                "name": "微信",
                "type": 1,
                "category": 4,
                "currency": "CNY",
                "icon": "500",
                "color": "07c160",
                "aliases": json.dumps(["微信", "微信支付", "wechat"], ensure_ascii=False),
                "display_order": 4,
            },
        ]

    return [
        {
            "name": "Cash",
            "type": 1,
            "category": 1,
            "currency": "CNY",
            "icon": "1",
            "color": "4caf50",
            "aliases": json.dumps(["cash", "wallet"]),
            "display_order": 0,
        },
        {
            "name": "Debit Card",
            "type": 1,
            "category": 2,
            "currency": "CNY",
            "icon": "100",
            "color": "2196f3",
            "aliases": json.dumps(["debit card", "bank card", "checking"]),
            "display_order": 1,
        },
        {
            "name": "Credit Card",
            "type": 1,
            "category": 3,
            "currency": "CNY",
            "icon": "100",
            "color": "ff9800",
            "aliases": json.dumps(["credit card"]),
            "display_order": 2,
        },
        {
            "name": "Alipay",
            "type": 1,
            "category": 4,
            "currency": "CNY",
            "icon": "500",
            "color": "1677ff",
            "aliases": json.dumps(["alipay"]),
            "display_order": 3,
        },
        {
            "name": "WeChat",
            "type": 1,
            "category": 4,
            "currency": "CNY",
            "icon": "500",
            "color": "07c160",
            "aliases": json.dumps(["wechat", "wechat pay"]),
            "display_order": 4,
        },
    ]


async def _save_register_categories(db, user_id: int, categories: list[dict[str, Any]]) -> bool:
    """保存注册请求携带的预设分类"""
    if not categories:
        return True

    success = True

    for item in categories:
        try:
            main_category = str(item.get("name", "")).strip()
            if not main_category:
                continue

            category_type = int(item.get("type", 3))
            icon = str(item.get("icon", "") or "")
            color = str(item.get("color", "") or "")

            await db.create_category(
                {
                    "type": category_type,
                    "main_category": main_category,
                    "sub_category": "",
                    "description": "",
                    "priority": 0,
                    "keywords": "",
                    "hidden": False,
                    "icon": icon,
                    "color": color,
                },
                user_id=user_id,
            )

            sub_categories = item.get("subCategories", []) or []
            for sub_item in sub_categories:
                sub_category = str(sub_item.get("name", "")).strip()
                if not sub_category:
                    continue

                await db.create_category(
                    {
                        "type": category_type,
                        "main_category": main_category,
                        "sub_category": sub_category,
                        "description": "",
                        "priority": 0,
                        "keywords": "",
                        "hidden": False,
                        "icon": str(sub_item.get("icon", "") or icon),
                        "color": str(sub_item.get("color", "") or color),
                    },
                    user_id=user_id,
                )
        except Exception:  # pylint: disable=broad-except
            success = False
            logger.exception("保存注册预设分类失败: user_id=%s, category=%s", user_id, item)

    return success


async def _create_register_default_accounts(db, user_id: int, language: str) -> dict[str, Any]:
    """创建注册默认账户，并回填 default_account_id/cash_account_id"""
    result = {
        "success": True,
        "cash_account_id": None,
        "default_account_id": None,
    }

    try:
        default_accounts = _build_default_accounts(language)
        created_account_ids: list[int] = []

        for account_data in default_accounts:
            account_id = await db.create_account(account_data, user_id=user_id)
            created_account_ids.append(account_id)

            if account_data.get("category") == 1 and result["cash_account_id"] is None:
                result["cash_account_id"] = account_id

        if created_account_ids:
            if result["default_account_id"] is None:
                result["default_account_id"] = created_account_ids[0]

            await db.update_user(
                user_id,
                {
                    "default_account_id": result["default_account_id"],
                    "cash_account_id": result["cash_account_id"],
                },
            )
    except Exception:  # pylint: disable=broad-except
        result["success"] = False
        logger.exception("创建注册默认账户失败: user_id=%s", user_id)

    return result


@bp.route("/auth/login", methods=["POST"])
@log_method
def login():
    """
    用户登录端点

    Request Body:
        {
            "loginName": "username or email",
            "password": "password"
        }

    Response:
        {
            "success": true,
            "result": {
                "token": "jwt_access_token",
                "user": {
                    "username": "...",
                    "email": "...",
                    ...
                }
            }
        }
    """
    try:
        data = request.json
        login_name = data.get("loginName", "").strip()
        password = data.get("password", "")

        if not login_name or not password:
            logger.warning("登录失败: 缺少用户名或密码")
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Username and password are required"}
            ), 400

        # 获取配置和数据库
        config = load_auth_config()
        db = get_app_context()

        # 获取客户端信息
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        # 查找用户（支持用户名或邮箱登录）
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        logger.info("查找用户: login_name=%s", login_name)
        user = loop.run_until_complete(db.get_user_by_username(login_name))
        if not user:
            user = loop.run_until_complete(db.get_user_by_email(login_name))

        if not user:
            logger.warning("登录失败: 用户不存在 - %s", login_name)

            # 记录失败日志
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "username": login_name,
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "User not found",
                    }
                )
            )
            loop.close()

            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Invalid username or password"}
            ), 401

        # 检查账户是否被锁定
        logger.info("检查账户锁定状态: user_id=%s", user["id"])
        is_locked = loop.run_until_complete(db.is_user_locked(user["id"]))
        if is_locked:
            logger.warning("登录失败: 账户被锁定 - user_id=%s", user["id"])

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Account locked",
                    }
                )
            )
            loop.close()

            return jsonify(
                {
                    "success": False,
                    "error": "Account locked",
                    "message": "Account is temporarily locked due to multiple failed login attempts",
                }
            ), 403

        # 验证密码
        logger.info("验证密码: user_id=%s", user["id"])
        password_hash = user["password_hash"]
        if not bcrypt.checkpw(password.encode("utf-8"), password_hash.encode("utf-8")):
            logger.warning("登录失败: 密码错误 - user_id=%s", user["id"])

            # 增加失败次数
            lockout_minutes = config.get("lockout_duration_minutes", 15)
            loop.run_until_complete(db.increment_failed_login(user["id"], lockout_minutes))

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Invalid password",
                    }
                )
            )
            loop.close()

            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Invalid username or password"}
            ), 401

        # 检查账户是否激活
        if not user.get("is_active"):
            logger.warning("登录失败: 账户未激活 - user_id=%s", user["id"])

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Account not active",
                    }
                )
            )
            loop.close()

            return jsonify(
                {"success": False, "error": "Account not active", "message": "Your account has been deactivated"}
            ), 403

        if user.get("two_factor_enabled"):
            pending_token = generate_action_token(
                user["id"], user["username"], user.get("email", ""), config, "pending_2fa", expires_in_hours=1
            )

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "login_2fa_pending",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": True,
                    }
                )
            )
            loop.close()

            return jsonify({"success": True, "result": {"token": pending_token, "need2FA": True}})

        # 生成JWT令牌
        logger.info("生成JWT令牌: user_id=%s", user["id"])
        tokens = generate_jwt_token(user["id"], user["username"], config)

        # 计算令牌哈希
        token_hash = calculate_token_hash(tokens["access_token"])
        refresh_token_hash = calculate_token_hash(tokens["refresh_token"])

        # 创建会话
        logger.info("创建会话: user_id=%s", user["id"])
        session_id = loop.run_until_complete(
            db.create_session(
                {
                    "user_id": user["id"],
                    "token_hash": token_hash,
                    "refresh_token_hash": refresh_token_hash,
                    "expires_at": tokens["expires_at"],
                    "refresh_expires_at": tokens["refresh_expires_at"],
                    "user_agent": user_agent,
                    "ip_address": ip_address,
                }
            )
        )

        # 更新最后登录时间
        loop.run_until_complete(db.update_user_last_login(user["id"], ip_address or ""))

        # 记录成功日志
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "login_success",
                    "ip_address": ip_address,
                    "user_agent": user_agent,
                    "success": True,
                    "metadata": json.dumps({"session_id": session_id}),
                }
            )
        )

        application_cloud_settings = _load_application_cloud_settings(db, user["id"], loop)

        loop.close()

        # 构建用户信息响应
        logger.info(
            "[login] 数据库用户记录: id=%s, username=%s, fiscal_year_start=%s",
            user["id"],
            user["username"],
            user.get("fiscal_year_start", "NOT_SET"),
        )

        user_info = _build_user_profile_info(user)
        user_info["id"] = user["id"]

        logger.info("登录成功: user_id=%s, username=%s, ip=%s", user["id"], user["username"], ip_address)
        logger.info(
            "[login] 返回给前端的user_info: id=%s, username=%s, fiscalYearStart=%s (0x%x)",
            user_info["id"],
            user_info["username"],
            user_info["fiscalYearStart"],
            user_info["fiscalYearStart"],
        )

        return jsonify(
            {"success": True, "result": _build_auth_success_result(user_info, tokens, application_cloud_settings)}
        )

    except Exception as error:
        logger.error("登录过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


@bp.route("/auth/register", methods=["POST"])
@log_method
def register():
    """
    用户注册端点

    Request Body:
        {
            "username": "username",
            "email": "email@example.com",
            "password": "password",
            "nickname": "nickname",
            "language": "zh_Hans",
            "defaultCurrency": "CNY",
            "firstDayOfWeek": 1
        }
    """
    try:
        data = request.json or {}
        username = data.get("username", "").strip()
        email = data.get("email", "").strip()
        password = data.get("password", "")
        nickname = data.get("nickname", "").strip() or username
        register_categories = data.get("categories") if isinstance(data.get("categories"), list) else []

        # 验证必填字段
        if not username or not email or not password:
            logger.warning("注册失败: 缺少必填字段")
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Username, email and password are required"}
            ), 400

        # 加载配置
        config = load_auth_config()

        # 检查是否允许注册
        if not config.get("enable_user_registration", True):
            logger.warning("注册失败: 注册功能已禁用")
            return jsonify(
                {
                    "success": False,
                    "error": "Registration disabled",
                    "message": "User registration is currently disabled",
                }
            ), 403

        # 验证密码强度
        logger.info("验证密码强度: username=%s", username)
        is_valid, error_msg = validate_password(password, config)
        if not is_valid:
            logger.warning("注册失败: 密码不符合要求 - %s", error_msg)
            return jsonify({"success": False, "error": "Invalid password", "message": error_msg}), 400

        db = get_app_context()
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 检查用户名是否已存在
        logger.info("检查用户名: username=%s", username)
        existing_user = loop.run_until_complete(db.get_user_by_username(username))
        if existing_user:
            logger.warning("注册失败: 用户名已存在 - %s", username)

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "username": username,
                        "event_type": "register_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Username already exists",
                    }
                )
            )
            loop.close()

            return jsonify({"success": False, "error": "Username exists", "message": "Username already exists"}), 409

        # 检查邮箱是否已存在
        logger.info("检查邮箱: email=%s", email)
        existing_email = loop.run_until_complete(db.get_user_by_email(email))
        if existing_email:
            logger.warning("注册失败: 邮箱已存在 - %s", email)

            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "username": username,
                        "event_type": "register_failed",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": False,
                        "error_message": "Email already exists",
                    }
                )
            )
            loop.close()

            return jsonify({"success": False, "error": "Email exists", "message": "Email already exists"}), 409

        # 哈希密码
        logger.info("正在哈希密码...")
        password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")

        # 创建用户
        logger.info("创建用户: username=%s, email=%s", username, email)
        user_id = loop.run_until_complete(
            db.create_user(
                {
                    "username": username,
                    "email": email,
                    "password_hash": password_hash,
                    "nickname": nickname,
                    "language": data.get("language", "zh_Hans"),
                    "default_currency": data.get("defaultCurrency", "CNY"),
                    "first_day_of_week": data.get("firstDayOfWeek", 1),
                    "is_active": 1,
                    "email_verified": 0 if config.get("require_email_verification") else 1,
                }
            )
        )

        normalized_register_categories = register_categories if isinstance(register_categories, list) else []
        preset_categories_saved = loop.run_until_complete(
            _save_register_categories(db, user_id, normalized_register_categories)
        )

        default_accounts_result = loop.run_until_complete(
            _create_register_default_accounts(db, user_id, data.get("language", "zh_Hans"))
        )

        # 记录成功日志
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "register_success",
                    "ip_address": ip_address,
                    "user_agent": user_agent,
                    "success": True,
                }
            )
        )

        loop.close()

        logger.info("注册成功: user_id=%s, username=%s, email=%s", user_id, username, email)

        return jsonify(
            {
                "success": True,
                "result": {
                    "user_id": user_id,
                    "username": username,
                    "email": email,
                    "needVerifyEmail": bool(config.get("require_email_verification")),
                    "presetCategoriesSaved": preset_categories_saved,
                    "presetAccountsSaved": bool(default_accounts_result.get("success")),
                    "message": "Registration successful",
                },
            }
        )

    except Exception as error:
        logger.error("注册过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


@bp.route("/auth/email/verify", methods=["POST"])
@log_method
def verify_email_by_token():
    """使用动作令牌验证邮箱（REST）。"""
    loop = None
    try:
        data = request.json or {}
        token = str(data.get("token", "") or "").strip()
        request_new_token = bool(data.get("requestNewToken", False))

        if not token:
            return jsonify({"success": False, "error": "Bad Request", "message": "Verification token is required"}), 400

        config = load_auth_config()
        payload = decode_action_token(token, config, "verify_email")
        if not payload:
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Verification token is invalid or expired"}
            ), 400

        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Verification token is invalid"},
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        loop.run_until_complete(db.update_user(user_id, {"email_verified": 1}))
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        new_token = None
        if request_new_token and user:
            tokens = _create_new_session_payload(user_id, user["username"], config, db, loop)
            new_token = tokens["access_token"]

        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": user["username"],
                    "event_type": "email_verified",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        return jsonify(
            {
                "success": True,
                "result": {
                    "newToken": new_token,
                    "user": _build_user_profile_info(user),
                    "notificationContent": "",
                },
            }
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("邮箱验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/email/resend-verification", methods=["POST"])
@log_method
def resend_verification_email_unauthed():
    """未登录用户重发验证邮件（REST）。"""
    loop = None
    try:
        data = request.json or {}
        email = str(data.get("email", "") or "").strip()
        password = data.get("password", "")

        if not email or not password:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Email and password are required"}
            ), 400

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_email(email))
        if not user or not _verify_user_password(user, password):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Invalid email or password"}
            ), 401

        verification_token = generate_action_token(
            user["id"], user["username"], user.get("email", ""), config, "verify_email"
        )

        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "verification_email_resend_requested",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                    "metadata": json.dumps(
                        {
                            "email": email,
                            "delivery": "not_configured_mock_success",
                            "verification_token": verification_token,
                        },
                        ensure_ascii=False,
                    ),
                }
            )
        )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("未登录重发验证邮件失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/password/forgot", methods=["POST"])
@log_method
def request_password_reset():
    """请求密码重置（REST）。"""
    loop = None
    try:
        data = request.json or {}
        email = str(data.get("email", "") or "").strip()
        if not email:
            return jsonify({"success": False, "error": "Bad Request", "message": "Email is required"}), 400

        config = load_auth_config()
        if not config.get("enable_user_forget_password", False):
            return jsonify(
                {
                    "success": False,
                    "error": "Forget password disabled",
                    "message": "Forget password is currently disabled",
                }
            ), 403

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user = loop.run_until_complete(db.get_user_by_email(email))

        if user:
            reset_token = generate_action_token(
                user["id"], user["username"], user.get("email", ""), config, "reset_password"
            )
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user["id"],
                        "username": user["username"],
                        "event_type": "password_reset_requested",
                        "ip_address": get_client_ip(),
                        "user_agent": request.headers.get("User-Agent", ""),
                        "success": True,
                        "metadata": json.dumps(
                            {"email": email, "delivery": "not_configured_mock_success", "reset_token": reset_token},
                            ensure_ascii=False,
                        ),
                    }
                )
            )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("请求重置密码失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/password/reset", methods=["POST"])
@log_method
def reset_password_by_token():
    """使用动作令牌重置密码（REST）。"""
    loop = None
    try:
        data = request.json or {}
        email = str(data.get("email", "") or "").strip()
        password = data.get("password", "")
        token = str(data.get("token", "") or "").strip()

        if not email or not password or not token:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Email, password and token are required"}
            ), 400

        config = load_auth_config()
        if not config.get("enable_user_forget_password", False):
            return jsonify(
                {
                    "success": False,
                    "error": "Forget password disabled",
                    "message": "Forget password is currently disabled",
                }
            ), 403

        is_valid, error_msg = validate_password(password, config)
        if not is_valid:
            return jsonify({"success": False, "error": "Invalid password", "message": error_msg}), 400

        payload = decode_action_token(token, config, "reset_password")
        if not payload:
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Reset password token is invalid or expired"}
            ), 400

        if payload.get("email") != email:
            return jsonify(
                {"success": False, "error": "Invalid token", "message": "Reset password token does not match email"}
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            loop.close()
            return jsonify({"success": False, "error": "Invalid token"}), 400

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user or (user.get("email") or "").strip() != email:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        password_hash = bcrypt.hashpw(password.encode("utf-8"), bcrypt.gensalt()).decode("utf-8")
        loop.run_until_complete(db.update_user(user["id"], {"password_hash": password_hash}))
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "password_reset_completed",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("重置密码失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/oauth2/authorize", methods=["POST"])
@log_method
def authorize_oauth2_callback():
    """OAuth2 回调授权入口（REST）。"""
    try:
        config = load_auth_config()
        if not config.get("enable_oauth2", False):
            return jsonify(
                {"success": False, "error": "OAuth2 disabled", "message": "OAuth2 login is currently disabled"}
            ), 403

        return jsonify(
            {
                "success": False,
                "error": "Not Implemented",
                "message": "OAuth2 callback authorization is not implemented in this workspace build",
            }
        ), 501
    except Exception as exc:
        logger.error("OAuth2 回调授权失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/auth/logout", methods=["POST"])
@log_method
def logout():
    """
    用户登出端点

    需要Authorization头
    """
    try:
        auth_header = request.headers.get("Authorization", "")

        if not auth_header:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Missing authorization header"}), 401

        parts = auth_header.split()
        if len(parts) != 2 or parts[0].lower() != "bearer":
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid authorization header"}), 401

        token = parts[1]
        token_hash = calculate_token_hash(token)

        db = get_app_context()
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取会话信息
        session = loop.run_until_complete(db.get_session_by_token_hash(token_hash))

        if session:
            # 使会话失效
            logger.info("使会话失效: session_id=%s", session["id"])
            loop.run_until_complete(db.invalidate_session(token_hash))

            # 记录登出日志
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": session["user_id"],
                        "username": session["username"],
                        "event_type": "logout",
                        "ip_address": ip_address,
                        "user_agent": user_agent,
                        "success": True,
                    }
                )
            )

            logger.info("登出成功: user_id=%s, username=%s", session["id"], session["username"])
        else:
            logger.warning("登出时未找到会话: token_hash=%s...", token_hash[:16])

        loop.close()

        # 🆕 返回result字段以匹配前端期望 (stores/index.ts Line 428)
        return jsonify(
            {
                "success": True,
                "result": True,  # ← 前端检查这个字段！
                "message": "Logged out successfully",
            }
        )

    except Exception as error:
        logger.error("登出过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


@bp.route("/tokens/refresh", methods=["POST"])
@log_method
def refresh_token():
    """
    刷新令牌端点

    Request Body:
        {
            "refreshToken": "refresh_token_string"
        }
    """
    try:
        data = request.json
        refresh_token_str = data.get("refreshToken", "")

        if not refresh_token_str:
            return jsonify({"success": False, "error": "Invalid request", "message": "Refresh token is required"}), 400

        config = load_auth_config()
        jwt_secret = config.get("jwt_secret")
        jwt_algorithm = config.get("jwt_algorithm", "HS256")

        # 验证刷新令牌
        try:
            payload = jwt.decode(refresh_token_str, jwt_secret, algorithms=[jwt_algorithm])

            if payload.get("type") != "refresh":
                return jsonify({"success": False, "error": "Invalid token", "message": "Not a refresh token"}), 400

            user_id = payload.get("user_id")
            username = payload.get("username")

            if not isinstance(user_id, int) or not isinstance(username, str) or not username:
                return jsonify(
                    {"success": False, "error": "Invalid token", "message": "Invalid refresh token"},
                ), 401

        except jwt.ExpiredSignatureError:
            return jsonify({"success": False, "error": "Token expired", "message": "Refresh token has expired"}), 401
        except jwt.InvalidTokenError:
            return jsonify({"success": False, "error": "Invalid token", "message": "Invalid refresh token"}), 401

        # 生成新的访问令牌
        logger.info("生成新令牌: user_id=%s", user_id)
        tokens = generate_jwt_token(user_id, username, config)

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found", "message": "User does not exist"}), 404

        # 查找旧会话并更新
        # old_refresh_hash = calculate_token_hash(refresh_token_str)  # 保留以供将来使用
        new_token_hash = calculate_token_hash(tokens["access_token"])
        new_refresh_hash = calculate_token_hash(tokens["refresh_token"])

        # 这里简化处理：创建新会话
        ip_address = get_client_ip()
        user_agent = request.headers.get("User-Agent", "")

        loop.run_until_complete(
            db.create_session(
                {
                    "user_id": user_id,
                    "token_hash": new_token_hash,
                    "refresh_token_hash": new_refresh_hash,
                    "expires_at": tokens["expires_at"],
                    "refresh_expires_at": tokens["refresh_expires_at"],
                    "user_agent": user_agent,
                    "ip_address": ip_address,
                }
            )
        )

        application_cloud_settings = _load_application_cloud_settings(db, user_id, loop)

        loop.close()

        logger.info("令牌刷新成功: user_id=%s, username=%s", user_id, username)

        return jsonify(
            {
                "success": True,
                "result": {
                    "token": tokens["access_token"],
                    "refreshToken": tokens["refresh_token"],
                    "newToken": tokens["access_token"],  # 兼容前端期望的字段名
                    "user": _build_user_profile_info(user),
                    "applicationCloudSettings": application_cloud_settings,
                },
            }
        )

    except Exception as error:
        logger.error("刷新令牌过程发生错误: %s", error, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(error)}), 500


@bp.route("/profile", methods=["GET", "PUT"])
@log_method
@require_auth
def profile():
    """
    获取或更新用户资料端点（需要认证）

    GET: 获取用户资料
    PUT: 更新用户资料
    """
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        if request.method == "GET":
            # 获取用户资料
            user = loop.run_until_complete(db.get_user_by_id(user_id))
            loop.close()

            if not user:
                return jsonify({"success": False, "error": "User not found"}), 404

            user_info = _build_user_profile_info(user)

            logger.info(
                "返回用户资料: user_id=%s, username=%s, calendarDisplayType=%s, fiscalYearStart=%s",
                user_id,
                user_info["username"],
                user_info["calendarDisplayType"],
                user_info["fiscalYearStart"],
            )
            logger.debug("完整用户资料数据: %s", user_info)

            return jsonify({"success": True, "result": user_info})

        # PUT - 更新用户资料
        data = request.json
        if not data:
            return jsonify({"success": False, "error": "Bad Request", "message": "Request body is required"}), 400

        update_data = {}

        # 允许更新的字段（基本信息）
        if "nickname" in data:
            update_data["nickname"] = data["nickname"]
        if "email" in data:
            update_data["email"] = data["email"]
        if "avatar" in data:
            update_data["avatar"] = data["avatar"]
        if "language" in data:
            update_data["language"] = data["language"]
        if "defaultCurrency" in data:
            update_data["default_currency"] = data["defaultCurrency"]
        if "firstDayOfWeek" in data:
            update_data["first_day_of_week"] = data["firstDayOfWeek"]
        if "defaultAccountId" in data:
            update_data["default_account_id"] = data["defaultAccountId"]
        if "transactionEditScope" in data:
            update_data["transaction_edit_scope"] = data["transactionEditScope"]

        # 允许更新的字段（显示配置）
        if "fiscalYearStart" in data:
            update_data["fiscal_year_start"] = data["fiscalYearStart"]
        if "calendarDisplayType" in data:
            update_data["calendar_display_type"] = data["calendarDisplayType"]
        if "dateDisplayType" in data:
            update_data["date_display_type"] = data["dateDisplayType"]
        if "longDateFormat" in data:
            update_data["long_date_format"] = data["longDateFormat"]
        if "shortDateFormat" in data:
            update_data["short_date_format"] = data["shortDateFormat"]
        if "longTimeFormat" in data:
            update_data["long_time_format"] = data["longTimeFormat"]
        if "shortTimeFormat" in data:
            update_data["short_time_format"] = data["shortTimeFormat"]
        if "fiscalYearFormat" in data:
            update_data["fiscal_year_format"] = data["fiscalYearFormat"]
        if "currencyDisplayType" in data:
            update_data["currency_display_type"] = data["currencyDisplayType"]
        if "numeralSystem" in data:
            update_data["numeral_system"] = data["numeralSystem"]
        if "decimalSeparator" in data:
            update_data["decimal_separator"] = data["decimalSeparator"]
        if "digitGroupingSymbol" in data:
            update_data["digit_grouping_symbol"] = data["digitGroupingSymbol"]
        if "digitGrouping" in data:
            update_data["digit_grouping"] = data["digitGrouping"]
        if "coordinateDisplayType" in data:
            update_data["coordinate_display_type"] = data["coordinateDisplayType"]
        if "expenseAmountColor" in data:
            update_data["expense_amount_color"] = data["expenseAmountColor"]
        if "incomeAmountColor" in data:
            update_data["income_amount_color"] = data["incomeAmountColor"]
        if "cashAccountId" in data:
            update_data["cash_account_id"] = data["cashAccountId"]
        if "cashTransferCategoryId" in data:
            update_data["cash_transfer_category_id"] = data["cashTransferCategoryId"]
        if "importLearningEnabled" in data:
            update_data["import_learning_enabled"] = 1 if data["importLearningEnabled"] else 0
        if "investmentPlatformKeywords" in data:
            update_data["investment_platform_keywords"] = serialize_keyword_list(data["investmentPlatformKeywords"])
        if "investmentProductKeywords" in data:
            update_data["investment_product_keywords"] = serialize_keyword_list(data["investmentProductKeywords"])
        if "investmentExcludeKeywords" in data:
            update_data["investment_exclude_keywords"] = serialize_keyword_list(data["investmentExcludeKeywords"])

        logger.info("将更新用户资料: user_id=%s, fields=%s", user_id, list(update_data.keys()))
        logger.debug("更新数据详情: %s", update_data)

        if update_data:
            success = loop.run_until_complete(db.update_user(user_id, update_data))
            if not success:
                loop.close()
                return jsonify(
                    {"success": False, "error": "Update failed", "message": "Failed to update user profile"}
                ), 500
            logger.info("用户资料更新成功: user_id=%s", user_id)

        # 返回更新后的用户信息
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()

        if not user:
            return jsonify({"success": False, "error": "User not found after update"}), 404

        user_info = _build_user_profile_info(user)

        logger.info("用户资料更新并返回: user_id=%s, updated_fields=%s", user_id, len(update_data))
        logger.debug("返回的用户资料: %s", user_info)

        # v6.79: 前端 updateUserProfile() 期望 result.user 格式
        # 见 index.ts 第 606 行: if (data.result.user && isObject(data.result.user))
        return jsonify({"success": True, "result": {"user": user_info}})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("处理用户资料失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/profile/avatar", methods=["POST"])
@log_method
@require_auth
def update_profile_avatar():
    """更新当前用户头像（REST）。"""
    loop = None
    try:
        avatar_file = request.files.get("avatar")
        if not avatar_file:
            return jsonify({"success": False, "error": "Bad Request", "message": "Avatar file is required"}), 400

        avatar_data_url = _build_avatar_data_url(avatar_file)
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        success = loop.run_until_complete(db.update_user(user_id, {"avatar": avatar_data_url}))
        if not success:
            loop.close()
            return jsonify({"success": False, "error": "Update failed", "message": "Failed to update avatar"}), 500

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()
        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        return jsonify({"success": True, "result": _build_user_profile_info(user)})
    except ValueError as exc:
        if loop and not loop.is_closed():
            loop.close()
        return jsonify({"success": False, "error": "Bad Request", "message": str(exc)}), 400
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("更新用户头像失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/profile/avatar", methods=["DELETE"])
@log_method
@require_auth
def remove_profile_avatar():
    """删除当前用户头像（REST）。"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        success = loop.run_until_complete(db.update_user(user_id, {"avatar": ""}))
        if not success:
            loop.close()
            return jsonify({"success": False, "error": "Update failed", "message": "Failed to remove avatar"}), 500

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()
        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        return jsonify({"success": True, "result": _build_user_profile_info(user)})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("删除用户头像失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/profile/email/resend-verification", methods=["POST"])
@log_method
@require_auth
def resend_profile_verification_email():
    """重发当前用户的验证邮件（REST）。

    当前项目尚未接入真实邮件发送基础设施，因此此端点提供
    与前端契约一致的幂等成功响应，并记录审计日志，便于后续
    接入 SMTP/第三方邮件服务时平滑扩展。
    """
    loop = None
    try:
        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        email = (user.get("email") or "").strip()
        if not email:
            loop.close()
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Email is required to resend verification email"}
            ), 400

        email_verified = bool(user.get("email_verified", False))
        require_email_verification = bool(config.get("require_email_verification", False))

        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "verification_email_resend_requested",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                    "metadata": json.dumps(
                        {
                            "email": email,
                            "email_verified": email_verified,
                            "require_email_verification": require_email_verification,
                            "delivery": "not_configured_mock_success",
                        },
                        ensure_ascii=False,
                    ),
                }
            )
        )
        loop.close()

        logger.info(
            "重发验证邮件请求已记录: user_id=%s, email=%s, verified=%s, require_email_verification=%s",
            user_id,
            email,
            email_verified,
            require_email_verification,
        )

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("重发验证邮件失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/profile/external-auths", methods=["GET"])
@log_method
@require_auth
def list_profile_external_auths():
    """获取当前用户第三方登录绑定列表（REST）。"""
    loop = None
    try:
        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        external_auths = loop.run_until_complete(db.get_user_external_auths(user_id))
        result = [_build_external_auth_info(item) for item in external_auths]

        oauth2_enabled = bool(config.get("enable_oauth2", False))
        oauth2_provider = str(config.get("oauth2_provider", "") or "").strip()
        linked_types = {item.get("externalAuthType") for item in result}

        if oauth2_enabled and oauth2_provider and oauth2_provider not in linked_types:
            result.append(
                {
                    "externalAuthCategory": "oauth2",
                    "externalAuthType": oauth2_provider,
                    "linked": False,
                    "externalUsername": "",
                    "createdAt": 0,
                }
            )

        loop.close()

        result.sort(
            key=lambda item: (
                0 if item.get("linked") else 1,
                item.get("externalAuthType", ""),
                -(item.get("createdAt") or 0),
            )
        )

        return jsonify({"success": True, "result": result})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取第三方登录列表失败: %s", exc, exc_info=True)
        return jsonify(
            {"success": False, "error": "Internal Server Error", "message": str(exc), "errorMessage": str(exc)}
        ), 500


@bp.route("/profile/external-auths/unlink", methods=["POST"])
@log_method
@require_auth
def unlink_profile_external_auth():
    """解绑当前用户第三方登录（REST）。"""
    loop = None
    try:
        data = request.get_json() or {}
        external_auth_type = str(data.get("externalAuthType", "") or "").strip()
        password = str(data.get("password", "") or "")

        if not external_auth_type:
            return jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "externalAuthType is required",
                    "errorMessage": "externalAuthType is required",
                }
            ), 400

        if not password:
            return jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "password is required",
                    "errorMessage": "password is required",
                }
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found", "errorMessage": "User not found"}), 404

        if not _verify_user_password(user, password):
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "Invalid password",
                    "errorMessage": "Invalid password",
                }
            ), 400

        existing = loop.run_until_complete(db.get_user_external_auth(user_id, external_auth_type))
        if not existing:
            loop.close()
            return jsonify(
                {
                    "success": False,
                    "error": "Not Found",
                    "message": "Third-party login is not linked",
                    "errorMessage": "Third-party login is not linked",
                }
            ), 404

        success = loop.run_until_complete(db.delete_user_external_auth(user_id, external_auth_type))
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": "external_auth_unlinked",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": success,
                    "metadata": json.dumps(
                        {
                            "external_auth_type": external_auth_type,
                            "external_auth_category": existing.get("external_auth_category", ""),
                        },
                        ensure_ascii=False,
                    ),
                }
            )
        )
        loop.close()

        return jsonify({"success": True, "result": success})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("解绑第三方登录失败: %s", exc, exc_info=True)
        return jsonify(
            {"success": False, "error": "Internal Server Error", "message": str(exc), "errorMessage": str(exc)}
        ), 500


@bp.route("/tokens/api", methods=["POST"])
@log_method
@require_auth
def generate_api_token():
    """生成 API token（需要认证与当前密码确认）。"""
    return _generate_personal_token("api")


@bp.route("/tokens/mcp", methods=["POST"])
@log_method
@require_auth
def generate_mcp_token():
    """生成 MCP token（需要认证与当前密码确认）。"""
    return _generate_personal_token("mcp")


def _generate_personal_token(token_kind: str):
    """生成个人访问 token 的共享实现。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = data.get("password", "")
        expires_in_seconds = int(data.get("expiresInSeconds", 0) or 0)

        if not password:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Current password is required"}
            ), 400

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found", "message": "User not found"}), 404

        if not _verify_user_password(user, password):
            loop.run_until_complete(
                db.create_auth_log(
                    {
                        "user_id": user_id,
                        "username": username,
                        "event_type": f"{token_kind}_token_generate_failed",
                        "ip_address": get_client_ip(),
                        "user_agent": request.headers.get("User-Agent", ""),
                        "success": False,
                        "error_message": "Invalid password",
                    }
                )
            )
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        token_data = generate_access_token(
            user_id,
            username,
            config,
            expires_in_seconds=expires_in_seconds,
            token_kind=token_kind,
        )
        token_hash = calculate_token_hash(token_data["access_token"])
        session_id = loop.run_until_complete(
            db.create_session(
                {
                    "user_id": user_id,
                    "token_hash": token_hash,
                    "refresh_token_hash": None,
                    "expires_at": token_data["expires_at"],
                    "refresh_expires_at": None,
                    "user_agent": _get_token_user_agent(token_kind),
                    "ip_address": get_client_ip(),
                }
            )
        )
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user_id,
                    "username": username,
                    "event_type": f"{token_kind}_token_generate_success",
                    "ip_address": get_client_ip(),
                    "user_agent": _get_token_user_agent(token_kind),
                    "success": True,
                    "metadata": json.dumps({"session_id": session_id}),
                }
            )
        )
        loop.close()

        result = {
            "token": token_data["access_token"],
        }
        if token_kind == "api":
            result["apiBaseUrl"] = _build_api_base_url()
        else:
            result["mcpUrl"] = _build_mcp_url()

        return jsonify({"success": True, "result": result})
    except (TypeError, ValueError):
        if loop and not loop.is_closed():
            loop.close()
        return jsonify(
            {"success": False, "error": "Invalid request", "message": "expiresInSeconds must be a valid integer"}
        ), 400
    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("生成 %s token 失败: %s", token_kind, e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/tokens/<token_id>", methods=["DELETE"])
@log_method
@require_auth
def revoke_token(token_id: str):
    """撤销指定 token 会话。"""
    loop = None
    try:
        session_id = int(token_id)
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.invalidate_session_by_id(session_id, _get_request_user_id()))
        loop.close()

        if not result:
            return jsonify({"success": False, "error": "Not Found", "message": "Token not found"}), 404

        return jsonify({"success": True, "result": True})
    except ValueError:
        if loop and not loop.is_closed():
            loop.close()
        return jsonify(
            {"success": False, "error": "Invalid request", "message": "tokenId must be a valid integer"}
        ), 400
    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("撤销 token 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/tokens", methods=["GET", "DELETE"])
@log_method
@require_auth
def list_tokens():
    """获取用户所有令牌或撤销其他令牌（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        session_id = _get_request_session_id()

        if request.method == "DELETE":
            revoked_count = loop.run_until_complete(db.invalidate_other_user_sessions(user_id, session_id))
            loop.close()
            return jsonify({"success": True, "result": True, "revokedCount": revoked_count})

        # 先清理过期会话
        loop.run_until_complete(db.cleanup_expired_sessions())

        # 获取所有活跃会话
        sessions = loop.run_until_complete(db.get_user_sessions(user_id))
        loop.close()

        # 转换为前端期望的格式
        tokens = []
        # seen_devices = set()  # Unused

        for session in sessions:
            user_agent = session.get("user_agent", "")
            ip_address = session.get("ip_address", "")
            token_type = _infer_token_type(user_agent)
            is_current = session["id"] == session_id
            last_seen = _datetime_to_unix_millis(session.get("last_activity_at") or session.get("created_at", ""))

            # 简单的设备标识: IP + UA的前50个字符
            # device_key = f"{ip_address}_{user_agent[:50]}" # Unused

            # 解析User-Agent提取设备信息
            device_name = parse_user_agent(user_agent)

            tokens.append(
                {
                    "tokenId": str(session["id"]),
                    "tokenType": token_type,
                    "userAgent": user_agent,
                    "deviceName": device_name,
                    "ipAddress": ip_address,
                    "createdAt": session.get("created_at", ""),
                    "expiresAt": session.get("expires_at", ""),
                    "lastActivityAt": session.get("last_activity_at", ""),
                    "lastSeen": last_seen,
                    "isCurrent": is_current,
                    "isCurrentToken": is_current,
                }
            )

        logger.info("返回会话列表: user_id=%s, count=%s", user_id, len(tokens))

        return jsonify({"success": True, "result": tokens})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取令牌列表失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


def parse_user_agent(user_agent: str) -> str:
    """解析User-Agent字符串,返回友好的设备名称"""
    if not user_agent:
        return "未知设备"

    ua_lower = user_agent.lower()

    # 检测操作系统
    if "windows" in ua_lower:
        os_name = "Windows"
        if "windows nt 10" in ua_lower:
            os_name = "Windows 10"
        elif "windows nt 11" in ua_lower:
            os_name = "Windows 11"
    elif "mac os" in ua_lower or "macos" in ua_lower:
        os_name = "macOS"
    elif "linux" in ua_lower:
        os_name = "Linux"
    elif "android" in ua_lower:
        os_name = "Android"
    elif "iphone" in ua_lower or "ipad" in ua_lower:
        os_name = "iOS"
    else:
        os_name = "其他系统"

    # 检测浏览器
    if "edg/" in ua_lower or "edge/" in ua_lower:
        browser = "Edge"
    elif "chrome/" in ua_lower and "edg/" not in ua_lower:
        browser = "Chrome"
    elif "firefox/" in ua_lower:
        browser = "Firefox"
    elif "safari/" in ua_lower and "chrome" not in ua_lower:
        browser = "Safari"
    else:
        browser = "其他浏览器"

    return f"{os_name} ({browser})"


@bp.route("/2fa/status", methods=["GET"])
@log_method
@require_auth
def get_2fa_status():
    """获取用户2FA状态（需要认证）"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()

        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        # 返回2FA状态
        is_enabled = user.get("two_factor_enabled", False)

        return jsonify({"success": True, "result": {"enable": bool(is_enabled), "isEnabled": bool(is_enabled)}})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取2FA状态失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/2fa/enable/request", methods=["POST"])
@log_method
@require_auth
def enable_2fa_request():
    """请求启用 2FA，返回 secret 与二维码。"""
    try:
        secret = pyotp.random_base32()
        qrcode_data = _generate_2fa_qrcode_data_url(_get_request_username(), secret)

        return jsonify({"success": True, "result": {"secret": secret, "qrcode": qrcode_data}})
    except Exception as exc:
        logger.error("请求启用2FA失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/profile/cloud-settings", methods=["GET", "PUT", "DELETE"])
@log_method
@require_auth
def profile_cloud_settings():
    """获取、更新或禁用用户应用云同步设置。"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        if request.method == "GET":
            settings = _load_application_cloud_settings(db, user_id, loop)
            loop.close()
            return jsonify({"success": True, "result": settings or False})

        if request.method == "DELETE":
            loop.run_until_complete(db.delete_user_application_cloud_settings(user_id))
            loop.close()
            return jsonify({"success": True, "result": True})

        data = request.get_json(silent=True) or {}
        settings = data.get("settings", [])
        full_update = bool(data.get("fullUpdate", False))

        if not isinstance(settings, list):
            loop.close()
            return jsonify({"success": False, "error": "Bad Request", "message": "settings must be an array"}), 400

        for setting in settings:
            error_message = _validate_application_cloud_setting(setting)
            if error_message:
                loop.close()
                return jsonify({"success": False, "error": "Bad Request", "message": error_message}), 400

        normalized_settings = _normalize_application_cloud_settings(settings)
        loop.run_until_complete(
            db.update_user_application_cloud_settings(user_id, normalized_settings, full_update=full_update)
        )
        loop.close()

        return jsonify({"success": True, "result": True})

    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("处理用户应用云同步设置失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/enable/confirm", methods=["POST"])
@log_method
@require_auth
def enable_2fa_confirm():
    """确认启用 2FA。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        secret = (data.get("secret") or "").strip().replace(" ", "")
        passcode = (data.get("passcode") or "").strip()

        if not secret or not passcode:
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Secret and passcode are required"}
            ), 400

        if not pyotp.TOTP(secret).verify(passcode, valid_window=1):
            return jsonify(
                {"success": False, "error": "Invalid passcode", "message": "The current passcode is incorrect"}
            ), 400

        db = get_app_context()
        config = load_auth_config()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        username = _get_request_username()

        success = loop.run_until_complete(
            db.update_user(user_id, {"two_factor_enabled": 1, "two_factor_secret": secret})
        )
        if not success:
            loop.close()
            return jsonify(
                {"success": False, "error": "Update failed", "message": "Failed to enable two-factor authentication"}
            ), 500

        tokens = _create_new_session_payload(user_id, username, config, db, loop)
        recovery_codes = _generate_recovery_codes()
        _set_recovery_codes(user_id, recovery_codes)
        loop.close()

        return jsonify(
            {
                "success": True,
                "result": {
                    "token": tokens["access_token"],
                    "refreshToken": tokens["refresh_token"],
                    "recoveryCodes": recovery_codes,
                },
            }
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("确认启用2FA失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/disable", methods=["POST"])
@log_method
@require_auth
def disable_2fa():
    """禁用 2FA。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = data.get("password", "")
        if not password:
            return jsonify({"success": False, "error": "Bad Request", "message": "Current password is required"}), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        if not _verify_user_password(user, password):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        success = loop.run_until_complete(
            db.update_user(user_id, {"two_factor_enabled": 0, "two_factor_secret": ""})
        )
        TWO_FACTOR_RECOVERY_CODES.pop(user_id, None)
        loop.close()
        if not success:
            return jsonify(
                {"success": False, "error": "Update failed", "message": "Failed to disable two-factor authentication"}
            ), 500

        return jsonify({"success": True, "result": True})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("禁用2FA失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/recovery/regenerate", methods=["POST"])
@log_method
@require_auth
def regenerate_2fa_recovery_codes():
    """重新生成 2FA 恢复码。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = data.get("password", "")
        if not password:
            return jsonify({"success": False, "error": "Bad Request", "message": "Current password is required"}), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        loop.close()
        if not user:
            return jsonify({"success": False, "error": "User not found"}), 404

        if not _verify_user_password(user, password):
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        if not user.get("two_factor_enabled"):
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Two-factor authentication is not enabled"}
            ), 400

        recovery_codes = _generate_recovery_codes()
        _set_recovery_codes(user_id, recovery_codes)

        return jsonify({"success": True, "result": {"recoveryCodes": recovery_codes}})
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("重新生成2FA恢复码失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/verify", methods=["POST"])
@log_method
def verify_2fa_login():
    """使用 TOTP 验证待登录 2FA 令牌。"""
    loop = None
    try:
        token = _extract_bearer_token()
        data = request.get_json(silent=True) or {}
        passcode = (data.get("passcode") or "").strip()

        if not token:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Missing authorization header"}), 401

        if not passcode:
            return jsonify(
                {"success": False, "error": "Bad Request", "errorCode": 203005, "message": "Passcode is required"}
            ), 400

        config = load_auth_config()
        payload = decode_action_token(token, config, "pending_2fa")
        if not payload:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            loop.close()
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        secret = (user.get("two_factor_secret") or "").strip().replace(" ", "")
        if not user.get("two_factor_enabled") or not secret:
            loop.close()
            return jsonify(
                {"success": False, "error": "Bad Request", "message": "Two-factor authentication is not enabled"}
            ), 400

        if not pyotp.TOTP(secret).verify(passcode, valid_window=1):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid passcode", "message": "The current passcode is incorrect"}
            ), 401

        tokens = _create_new_session_payload(user["id"], user["username"], config, db, loop)
        application_cloud_settings = _load_application_cloud_settings(db, user["id"], loop)
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "login_2fa_success",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        user_info = _build_user_profile_info(user)
        user_info["id"] = user["id"]
        return jsonify(
            {"success": True, "result": _build_auth_success_result(user_info, tokens, application_cloud_settings)}
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("2FA 登录验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/2fa/recovery/verify", methods=["POST"])
@log_method
def verify_2fa_login_by_recovery_code():
    """使用恢复码验证待登录 2FA 令牌。"""
    loop = None
    try:
        token = _extract_bearer_token()
        data = request.get_json(silent=True) or {}
        recovery_code = (data.get("recoveryCode") or "").strip()

        if not token:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Missing authorization header"}), 401

        if not recovery_code:
            return jsonify({"success": False, "error": "Bad Request", "message": "Recovery code is required"}), 400

        config = load_auth_config()
        payload = decode_action_token(token, config, "pending_2fa")
        if not payload:
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        user_id = payload.get("user_id")
        if not isinstance(user_id, int):
            return jsonify({"success": False, "error": "Unauthorized", "message": "Invalid or expired 2FA token"}), 401

        if not _consume_recovery_code(user_id, recovery_code):
            return jsonify(
                {
                    "success": False,
                    "error": "Invalid recovery code",
                    "message": "Recovery code is invalid or already used",
                }
            ), 401

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user = loop.run_until_complete(db.get_user_by_id(user_id))
        if not user:
            loop.close()
            return jsonify({"success": False, "error": "User not found"}), 404

        tokens = _create_new_session_payload(user["id"], user["username"], config, db, loop)
        application_cloud_settings = _load_application_cloud_settings(db, user["id"], loop)
        loop.run_until_complete(
            db.create_auth_log(
                {
                    "user_id": user["id"],
                    "username": user["username"],
                    "event_type": "login_2fa_recovery_success",
                    "ip_address": get_client_ip(),
                    "user_agent": request.headers.get("User-Agent", ""),
                    "success": True,
                }
            )
        )
        loop.close()

        user_info = _build_user_profile_info(user)
        user_info["id"] = user["id"]
        return jsonify(
            {"success": True, "result": _build_auth_success_result(user_info, tokens, application_cloud_settings)}
        )
    except Exception as exc:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("2FA 恢复码登录验证失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(exc)}), 500


@bp.route("/data/statistics", methods=["GET"])
@log_method
@require_auth
def get_user_data_statistics():
    """获取用户数据统计（需要认证）。"""
    loop = None
    try:
        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        statistics = loop.run_until_complete(db.get_user_data_statistics(user_id=user_id))

        loop.close()

        logger.info(
            "返回用户数据统计: user_id=%s, bills=%s, accounts=%s, categories=%s, tags=%s, templates=%s",
            user_id,
            statistics["billCount"],
            statistics["accountCount"],
            statistics["categoryCount"],
            statistics["tagCount"],
            statistics["templateCount"],
        )

        return jsonify({"success": True, "result": statistics})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("获取用户数据统计失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/data/export.<file_type>", methods=["GET"])
@log_method
@require_auth
def export_user_data(file_type: str):
    """导出当前用户账单数据为 CSV/TSV。"""
    loop = None
    try:
        normalized_file_type = file_type.lower()
        if normalized_file_type not in ["csv", "tsv"]:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Unsupported export file type"}
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        categories = loop.run_until_complete(db.get_all_categories(user_id=user_id))
        filters = _build_export_filters(categories)
        bills = loop.run_until_complete(db.get_bills(filters=filters, user_id=user_id))
        accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
        bill_ids = [int(bill["id"]) for bill in bills if bill.get("id") is not None]
        tags_map = loop.run_until_complete(db.get_tags_for_bills(bill_ids, user_id=user_id)) if bill_ids else {}
        loop.close()

        delimiter = "," if normalized_file_type == "csv" else "\t"
        mimetype = "text/csv" if normalized_file_type == "csv" else "text/tab-separated-values"
        content = "\ufeff" + _render_bills_export(bills, accounts, tags_map, delimiter)
        filename = f"bill_analyser_export_{datetime.now().strftime('%Y%m%d_%H%M%S')}.{normalized_file_type}"

        response = Response(content, mimetype=mimetype)
        response.headers["Content-Disposition"] = f"attachment; filename={filename}"
        return response

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("导出用户数据失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/data/clear/transactions", methods=["POST"])
@log_method
@require_auth
def clear_user_transactions():
    """清空当前用户全部交易数据。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = data.get("password", "")
        if not password:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Current password is required"}
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        if not loop.run_until_complete(db.verify_operation_password(password)):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        result = loop.run_until_complete(db.clear_user_transactions(user_id))
        loop.run_until_complete(
            db.create_audit_log(
                operation_type="clear_transactions",
                operation_target="user_data",
            target_id=user_id,
                details={"deleted_count": result.get("deleted_count", 0)},
                affected_count=result.get("deleted_count", 0),
                status="success" if result.get("success") else "failed",
                error_message=None if result.get("success") else result.get("message"),
                ip_address=get_client_ip(),
                user_agent=request.headers.get("User-Agent", ""),
            )
        )
        loop.close()

        if not result.get("success"):
            return jsonify(
                {
                    "success": False,
                    "error": "Internal Server Error",
                    "message": result.get("message", "Failed to clear transactions"),
                }
            ), 500

        return jsonify({"success": True, "result": True, "deletedCount": result.get("deleted_count", 0)})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("清空用户交易失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/data/clear/all", methods=["POST"])
@log_method
@require_auth
def clear_all_user_data():
    """清空当前用户全部业务数据。"""
    loop = None
    try:
        data = request.get_json(silent=True) or {}
        password = data.get("password", "")
        if not password:
            return jsonify(
                {"success": False, "error": "Invalid request", "message": "Current password is required"}
            ), 400

        db = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        user_id = _get_request_user_id()

        if not loop.run_until_complete(db.verify_operation_password(password)):
            loop.close()
            return jsonify(
                {"success": False, "error": "Invalid credentials", "message": "Current password is incorrect"}
            ), 401

        result = loop.run_until_complete(db.clear_user_data(user_id))
        loop.run_until_complete(
            db.create_audit_log(
                operation_type="clear_all_user_data",
                operation_target="user_data",
            target_id=user_id,
                details=result.get("counts", {}),
                status="success" if result.get("success") else "failed",
                error_message=None if result.get("success") else result.get("message"),
                ip_address=get_client_ip(),
                user_agent=request.headers.get("User-Agent", ""),
            )
        )
        loop.close()

        if not result.get("success"):
            return jsonify(
                {
                    "success": False,
                    "error": "Internal Server Error",
                    "message": result.get("message", "Failed to clear user data"),
                }
            ), 500

        return jsonify({"success": True, "result": True, "counts": result.get("counts", {})})

    except Exception as e:
        if loop and not loop.is_closed():
            loop.close()
        logger.error("清空用户全部业务数据失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": "Internal Server Error", "message": str(e)}), 500


@bp.route("/system/version", methods=["GET"])
@log_method
def get_system_version():
    """获取服务端版本信息。"""
    return jsonify({"success": True, "result": {"version": __version__, "commitHash": "", "buildTime": ""}})
