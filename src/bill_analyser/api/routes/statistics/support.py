# pylint: disable=unused-import
"""Shared support for Python-proxied Analyzer statistics routes."""

from typing import Any, cast

from flask import Blueprint, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.core.analyzer import Analyzer
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("StatisticsAPI")

bp = Blueprint("statistics", __name__)


def get_app_context():
    """获取应用上下文中的服务实例"""
    return cast(Any, current_app.config.get("DB_INSTANCE"))


__all__ = [name for name in globals() if not name.startswith("__")]
