"""
统计分析 API 路由
"""


import asyncio
import time
from calendar import monthrange
from datetime import datetime, timedelta
from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.core.analyzer import Analyzer
from bill_analyser.core.exchange_rate_providers import (
    BOCChinaProvider,
    CMBChinaProvider,
    ECBProvider,
    RBAProvider,
)
from bill_analyser.utils.currency import yuan_to_cents
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("StatisticsAPI")

bp = Blueprint("statistics", __name__)

EXCHANGE_RATE_PROVIDER_OPTIONS = {
    "auto": {"label": "自动选择", "reference_url": "", "region": "mixed"},
    "boc_cn": {
        "label": "中国银行外汇牌价",
        "reference_url": "https://www.boc.cn/sourcedb/whpj/",
        "region": "domestic",
    },
    "cmb_cn": {
        "label": "招商银行实时汇率",
        "reference_url": "https://fx.cmbchina.com/hq/",
        "region": "domestic",
    },
    "ecb": {
        "label": "ECB (欧洲央行)",
        "reference_url": "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml",
        "region": "foreign",
    },
    "rba": {
        "label": "RBA (澳大利亚储备银行)",
        "reference_url": "https://www.rba.gov.au/rss/rss-cb-exchange-rates.xml",
        "region": "foreign",
    },
}

DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER = ["boc_cn", "cmb_cn", "ecb", "rba"]


def get_app_context():
    """获取应用上下文中的服务实例"""
    return cast("Any", current_app.config.get("DB_INSTANCE"))


def _get_request_user_id() -> int:
    """获取认证中间件注入的当前用户 ID。"""
    return get_required_request_int("user_id")


def _get_request_base_currency(db) -> str:
    """获取请求使用的基准币种。"""
    requested = (request.args.get("base_currency") or "").strip().upper()
    if requested:
        return requested

    user = _run_async(db.get_user_by_id(_get_request_user_id()))
    return (user or {}).get("default_currency", "CNY") or "CNY"


def _build_user_custom_exchange_rates_result(
    base_currency: str,
    custom_rates: list[dict[str, Any]],
) -> dict[str, Any]:
    """构建用户自定义汇率响应。"""
    update_time = int(time.time())
    exchange_rates_list = [{"currency": base_currency, "rate": "1.0"}]

    latest_update_time = update_time
    for rate in custom_rates:
        exchange_rates_list.append(
            {"currency": rate.get("to_currency", ""), "rate": str(rate.get("rate", "1.0"))},
        )
        effective_date = rate.get("effective_date")
        if effective_date:
            try:
                latest_update_time = max(
                    latest_update_time,
                    int(datetime.fromisoformat(str(effective_date)).timestamp()),
                )
            except ValueError:
                logger.debug("忽略非法 effective_date: %s", effective_date)

    return {
        "providerKey": "user_custom",
        "requestedProvider": "auto",
        "fallbackUsed": False,
        "dataSource": "user_custom",
        "referenceUrl": "",
        "updateTime": latest_update_time,
        "baseCurrency": base_currency,
        "exchangeRates": exchange_rates_list,
    }


def _normalize_requested_exchange_rate_provider() -> str:
    """标准化请求中的汇率 provider 参数。"""
    provider = (request.args.get("provider") or "auto").strip().lower()
    return provider or "auto"


def _build_provider_candidate_order(requested_provider: str) -> list[str]:
    """根据用户选择构建 provider 尝试顺序。"""
    if requested_provider == "auto":
        return DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER.copy()

    if requested_provider not in EXCHANGE_RATE_PROVIDER_OPTIONS:
        return []

    candidate_order = [requested_provider]
    for provider_key in DEFAULT_EXCHANGE_RATE_PROVIDER_ORDER:
        if provider_key != requested_provider:
            candidate_order.append(provider_key)

    return candidate_order



__all__ = [name for name in globals() if not name.startswith("__")]
