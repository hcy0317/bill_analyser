"""Settings bundle JSON import/export routes."""

from __future__ import annotations

import json
from typing import Any, cast

from flask import Blueprint, Response, current_app, jsonify, request

from bill_analyser.api.middleware.auth import require_auth
from bill_analyser.api.routes.request_context_helpers import (
    get_required_request_int,
)
from bill_analyser.api.routes.request_context_helpers import (
    run_async_in_new_loop as _run_async,
)
from bill_analyser.core.db_settings_bundle import (
    SETTINGS_BUNDLE_SCHEMA_VERSION,
    SETTINGS_BUNDLE_SECTION_KEYS,
)
from bill_analyser.utils.logger import get_logger, log_method

logger = get_logger("SettingsBundleAPI")

bp = Blueprint("settings_bundle", __name__)


def _get_request_user_id() -> int:
    return get_required_request_int("user_id")


def _get_db() -> Any:
    return cast("Any", current_app.config.get("DB_INSTANCE"))


def _reload_category_engines(db: Any, user_id: int) -> None:
    engines = [cast("Any", current_app.config.get("CATEGORY_ENGINE_INSTANCE"))]
    bill_service = cast("Any", current_app.config.get("BILL_SERVICE_INSTANCE"))
    bill_service_engine = cast("Any", getattr(bill_service, "category_engine", None))
    if bill_service_engine is not None and bill_service_engine is not engines[0]:
        engines.append(bill_service_engine)

    for engine in engines:
        if engine is None:
            continue
        engine.invalidate_cache()
        _run_async(engine.load_rules_from_db(db, user_id=user_id))


def _is_valid_section_key(section_key: str) -> bool:
    return section_key in SETTINGS_BUNDLE_SECTION_KEYS


def _section_not_found(section_key: str) -> tuple[Response, int]:
    return jsonify(
        {
            "success": False,
            "error": f"Unsupported settings bundle section: {section_key}",
        }
    ), 404


def _filter_bundle_section(bundle: dict[str, Any], section_key: str) -> dict[str, Any]:
    sections = bundle.get("sections") if isinstance(bundle.get("sections"), dict) else {}
    items = sections.get(section_key, []) if isinstance(sections, dict) else []
    if not isinstance(items, list):
        items = []
    return {
        "schemaVersion": bundle.get("schemaVersion", SETTINGS_BUNDLE_SCHEMA_VERSION),
        "exportedAt": bundle.get("exportedAt"),
        "secretsPolicy": bundle.get("secretsPolicy", {"llmApiKeys": "redacted"}),
        "sections": {section_key: items},
        "counts": {section_key: len(items)},
    }


def _section_bundle_from_request(
    data: dict[str, Any],
    section_key: str,
) -> dict[str, Any]:
    sections = data.get("sections") if isinstance(data.get("sections"), dict) else data
    if not isinstance(sections, dict):
        raise ValueError("Settings bundle sections must be a JSON object")
    section_items = sections.get(section_key, [])
    return {
        "schemaVersion": data.get("schemaVersion"),
        "sections": {section_key: section_items},
    }


def _should_reload_category_engines(section_key: str) -> bool:
    return section_key in {"transactionCategories", "categoryRecognitionRules"}


@bp.route("/export", methods=["GET"])
@log_method
@require_auth
def export_settings_bundle():
    """Export user settings as a unified JSON bundle."""
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        bundle = _run_async(db.export_user_settings_bundle(user_id=user_id))
        json_text = json.dumps(bundle, ensure_ascii=False, separators=(",", ":"))
        response = Response(json_text, mimetype="application/json")
        response.headers["Content-Disposition"] = (
            'attachment; filename="bill-analyser-settings.json"'
        )
        return response
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导出设置包失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to export settings bundle"}), 500


@bp.route("/sections/<section_key>/export", methods=["GET"])
@log_method
@require_auth
def export_settings_bundle_section(section_key: str):
    """Export one settings section as a JSON bundle."""
    if not _is_valid_section_key(section_key):
        return _section_not_found(section_key)
    try:
        db = _get_db()
        user_id = _get_request_user_id()
        bundle = _run_async(db.export_user_settings_bundle(user_id=user_id))
        section_bundle = _filter_bundle_section(bundle, section_key)
        json_text = json.dumps(section_bundle, ensure_ascii=False, separators=(",", ":"))
        response = Response(json_text, mimetype="application/json")
        response.headers["Content-Disposition"] = (
            f'attachment; filename="bill-analyser-settings-{section_key}.json"'
        )
        return response
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导出设置 section 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to export settings section"}), 500


@bp.route("/import/preview", methods=["POST"])
@log_method
@require_auth
def preview_import_settings_bundle():
    """Preview a JSON settings bundle import without committing changes."""
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON bundle"}), 400

        db = _get_db()
        user_id = _get_request_user_id()
        result = _run_async(db.preview_import_user_settings_bundle(data, user_id=user_id))
        return jsonify({"success": True, "result": result})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("预览导入设置包失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to preview settings bundle"}), 500


@bp.route("/sections/<section_key>/import/preview", methods=["POST"])
@log_method
@require_auth
def preview_import_settings_bundle_section(section_key: str):
    """Preview a single settings section import without committing changes."""
    if not _is_valid_section_key(section_key):
        return _section_not_found(section_key)
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON bundle"}), 400

        db = _get_db()
        user_id = _get_request_user_id()
        section_bundle = _section_bundle_from_request(data, section_key)
        result = _run_async(db.preview_import_user_settings_bundle(
            section_bundle,
            user_id=user_id,
        ))
        return jsonify({"success": True, "result": result})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("预览导入设置 section 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to preview settings section"}), 500


@bp.route("/import", methods=["POST"])
@log_method
@require_auth
def import_settings_bundle():
    """Import a JSON settings bundle with non-destructive upsert semantics."""
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON bundle"}), 400

        db = _get_db()
        user_id = _get_request_user_id()
        result = _run_async(db.import_user_settings_bundle(data, user_id=user_id))
        _reload_category_engines(db, user_id)
        return jsonify({"success": True, "result": result})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导入设置包失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to import settings bundle"}), 500


@bp.route("/sections/<section_key>/import", methods=["POST"])
@log_method
@require_auth
def import_settings_bundle_section(section_key: str):
    """Import a single settings section with non-destructive upsert semantics."""
    if not _is_valid_section_key(section_key):
        return _section_not_found(section_key)
    try:
        data = request.get_json(silent=True)
        if not isinstance(data, dict):
            return jsonify({"success": False, "error": "Invalid JSON bundle"}), 400

        db = _get_db()
        user_id = _get_request_user_id()
        section_bundle = _section_bundle_from_request(data, section_key)
        result = _run_async(db.import_user_settings_bundle(
            section_bundle,
            user_id=user_id,
        ))
        if _should_reload_category_engines(section_key):
            _reload_category_engines(db, user_id)
        return jsonify({"success": True, "result": result})
    except ValueError as exc:
        return jsonify({"success": False, "error": str(exc)}), 400
    except Exception as exc:  # pylint: disable=broad-exception-caught
        logger.error("导入设置 section 失败: %s", exc, exc_info=True)
        return jsonify({"success": False, "error": "Failed to import settings section"}), 500
