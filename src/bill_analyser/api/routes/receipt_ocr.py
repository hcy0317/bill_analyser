"""Receipt OCR REST routes."""

import json
import os
from typing import Any

from flask import Blueprint, Flask, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.core.ai.ocr.provider import DISABLED_PROVIDER_NAME, OcrProviderFactory
from bill_analyser.core.ai.ocr.service import (
    OcrService,
    OcrServiceError,
    normalize_ocr_config,
)

bp = Blueprint("receipt_ocr", __name__)

_OCR_CONFIG_SETTING_KEY = "receipt_ocr_config"


def _default_ocr_config() -> dict:
    return normalize_ocr_config(
        {
            "provider": OcrProviderFactory.resolve_default_provider_name(),
            "lang": os.environ.get("BILL_OCR_LANG"),
        }
    )


def _ocr_config_response_payload(config: dict) -> dict:
    normalized = normalize_ocr_config(config)
    available_providers = [DISABLED_PROVIDER_NAME, *OcrProviderFactory.available_providers()]
    return {
        **normalized,
        "available_providers": available_providers,
        "configured": normalized["provider"] != DISABLED_PROVIDER_NAME,
    }


def _decode_stored_ocr_config(raw_value: str | None) -> dict:
    if not raw_value:
        return _default_ocr_config()
    try:
        loaded = json.loads(raw_value)
    except json.JSONDecodeError:
        return _default_ocr_config()
    return normalize_ocr_config(loaded)


def _get_config_database(flask_app: Flask) -> Any:
    database = flask_app.config.get("DB_INSTANCE")
    if database is not None:
        return database

    from bill_analyser.api import app as api_app  # pylint: disable=import-outside-toplevel

    return api_app.db


def _load_ocr_config(flask_app: Flask, run_async) -> dict:
    database = _get_config_database(flask_app)
    if database is None or not hasattr(database, "get_app_setting"):
        return _default_ocr_config()
    raw_value = run_async(database.get_app_setting(_OCR_CONFIG_SETTING_KEY))
    return _decode_stored_ocr_config(raw_value)


def _store_ocr_config(flask_app: Flask, config: dict, run_async) -> bool:
    database = _get_config_database(flask_app)
    if database is None or not hasattr(database, "set_app_setting"):
        return False
    return bool(
        run_async(
            database.set_app_setting(
                _OCR_CONFIG_SETTING_KEY,
                json.dumps(normalize_ocr_config(config), ensure_ascii=False),
                value_type="json",
                description="Receipt OCR runtime configuration",
                is_encrypted=False,
            )
        )
    )


@bp.route("/receipt-recognition", methods=["POST"])
@require_auth
def receipt_recognition_endpoint():
    """AI 小票识图 OCR 端点。"""
    service: OcrService | None = current_app.config.get("OCR_SERVICE")
    if service is None:
        service = OcrService.from_config(_load_ocr_config(current_app, _run_async))
        current_app.config["OCR_SERVICE"] = service

    file_obj = request.files.get("image")
    image_bytes = b""
    mime = ""
    if file_obj is not None:
        image_bytes = file_obj.read() or b""
        mime = file_obj.mimetype or "application/octet-stream"
    elif request.data:
        image_bytes = request.data
        mime = request.mimetype or "application/octet-stream"

    cancelled_flag = (request.form.get("cancelled") or "").strip().lower()
    cancelled = cancelled_flag in {"1", "true", "yes", "on"}

    try:
        result = _run_async(
            service.recognize(
                user_id=getattr(request, "user_id", None),
                image_bytes=image_bytes,
                mime=mime,
                cancelled=cancelled,
            )
        )
    except OcrServiceError as svc_err:
        payload = {
            "success": False,
            "errorCode": svc_err.code,
            "errorMessage": svc_err.message or "Receipt recognition not implemented"
            if svc_err.code == "provider_unconfigured"
            else svc_err.message,
            "message": svc_err.message
            or (
                "Receipt recognition not implemented"
                if svc_err.code == "provider_unconfigured"
                else svc_err.code
            ),
        }
        return payload, svc_err.http_status

    return {"success": True, "result": result.to_payload()}, 200


@bp.route("/receipt-recognition/config", methods=["GET", "PUT"])
@require_auth
def receipt_recognition_config_endpoint():
    """OCR 配置端点；配置只保存 provider/lang，不保存上传图片或识别结果。"""
    if request.method == "GET":
        config = _load_ocr_config(current_app, _run_async)
        return jsonify({"success": True, "result": _ocr_config_response_payload(config)})

    data = request.get_json(silent=True) or {}
    provider = str(data.get("provider") or DISABLED_PROVIDER_NAME).strip().lower()
    if provider in {"", "none", "off"}:
        provider = DISABLED_PROVIDER_NAME

    available_providers = {
        DISABLED_PROVIDER_NAME,
        *OcrProviderFactory.available_providers(),
    }
    if provider not in available_providers:
        return (
            jsonify(
                {
                    "success": False,
                    "error": "Bad Request",
                    "message": "Unknown OCR provider",
                }
            ),
            400,
        )

    config = normalize_ocr_config({**data, "provider": provider})
    if not _store_ocr_config(current_app, config, _run_async):
        return (
            jsonify(
                {
                    "success": False,
                    "error": "Service Unavailable",
                    "message": "Unable to persist OCR config",
                }
            ),
            503,
        )

    current_app.config.pop("OCR_SERVICE", None)
    return jsonify({"success": True, "result": _ocr_config_response_payload(config)})


__all__ = [name for name in globals() if not name.startswith("__")]
