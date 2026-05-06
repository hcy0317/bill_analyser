# pylint: disable=wildcard-import,unused-wildcard-import,undefined-variable
from .support import *  # noqa: F403
from .import_detection import *  # noqa: F403
from .import_rows import *  # noqa: F403
from .import_mapping import *  # noqa: F403
from .import_review import *  # noqa: F403

@bp.route("/import/configs", methods=["GET"])
@log_method
@require_auth
def list_import_configs():
    """获取当前用户的导入列映射模板。"""
    try:
        db, _, _ = get_app_context()
        file_format = str(request.args.get("file_format", "") or "").strip().lower() or None
        limit = int(request.args.get("limit", 100) or 100)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            configs = loop.run_until_complete(
                db.get_import_configs(user_id=request.user_id, file_format=file_format, limit=limit)
            )

            result = []
            for config in configs:
                result.append(
                    {
                        "id": config.get("id"),
                        "name": config.get("name", ""),
                        "fileFormat": config.get("file_format", ""),
                        "description": config.get("description", ""),
                        "descriptionSummary": config.get("description_summary", ""),
                        "fieldMappings": config.get("field_mappings", {}),
                        "dateFormat": config.get("date_format", ""),
                        "encoding": config.get("encoding", "utf-8"),
                        "delimiter": config.get("delimiter"),
                        "skipRows": int(config.get("skip_rows", 0) or 0),
                        "hasHeader": bool(config.get("has_header", True)),
                        "customRules": config.get("custom_rules", {}),
                        "sampleHeaders": config.get("sample_headers", []),
                        "headerSignature": config.get("header_signature", ""),
                        "isDefault": bool(config.get("is_default", False)),
                        "defaultRecommendation": bool(config.get("default_recommendation", False)),
                        "useCount": int(config.get("use_count", 0) or 0),
                        "lastUsedAt": config.get("last_used_at", ""),
                        "createdAt": config.get("created_at", ""),
                        "updatedAt": config.get("updated_at", ""),
                    }
                )

            return jsonify({"success": True, "result": result})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[导入模板列表] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs", methods=["POST"])
@log_method
@require_auth
def save_import_config():
    """保存导入列映射模板。"""
    try:
        data = request.get_json(silent=True) or {}
        if not data.get("name"):
            return jsonify({"success": False, "error": "name is required"}), 400
        if not data.get("fileFormat"):
            return jsonify({"success": False, "error": "fileFormat is required"}), 400
        if not data.get("fieldMappings"):
            return jsonify({"success": False, "error": "fieldMappings is required"}), 400

        db, _, _ = get_app_context()
        payload = {
            "id": data.get("id"),
            "name": data.get("name"),
            "file_format": data.get("fileFormat"),
            "description": data.get("description", ""),
            "field_mappings": data.get("fieldMappings", {}),
            "date_format": data.get("dateFormat", ""),
            "encoding": data.get("encoding", "utf-8"),
            "delimiter": data.get("delimiter"),
            "skip_rows": data.get("skipRows", 0),
            "has_header": data.get("hasHeader", True),
            "custom_rules": data.get("customRules", {}),
            "sample_headers": data.get("sampleHeaders") or data.get("headers") or [],
            "is_default": data.get("isDefault", False),
        }

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            config_id = loop.run_until_complete(db.save_import_config(payload, user_id=request.user_id))
            return jsonify({"success": True, "result": {"id": config_id}}), 201
        finally:
            loop.close()

    except ValueError as e:
        return jsonify({"success": False, "error": str(e)}), 400
    except Exception as e:
        logger.error("[保存导入模板] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/preview", methods=["POST"])
@log_method
@require_auth
def preview_import_file():
    """预览通用表格导入文件内容。支持上传文件或从服务端临时路径预览。"""
    temp_file_path = None
    should_delete_temp = True
    try:
        # 支持两种模式：1) 上传文件  2) 从服务端temp_path读取（不删除临时文件）
        server_temp_path = request.form.get("temp_path", "").strip()
        if server_temp_path:
            # 从服务端临时路径预览（由 v2/parse 返回的 unmatched_files）
            candidate = Path(server_temp_path)
            # 安全检查：只允许 UPLOAD_FOLDER 下的文件
            if not candidate.resolve().is_relative_to(UPLOAD_FOLDER.resolve()):
                return jsonify({"success": False, "error": "Invalid temp_path"}), 400
            if not candidate.exists():
                return jsonify({"success": False, "error": "Temp file not found"}), 404
            temp_file_path = candidate
            should_delete_temp = False  # 不删除，后续 parse_generic 还要用
        elif "file" in request.files:
            file = request.files["file"]
            if not file or not file.filename:
                return jsonify({"success": False, "error": "No file selected"}), 400
            if not allowed_file(file.filename):
                return jsonify(
                    {"success": False, "error": f"File type not allowed. Supported: {', '.join(ALLOWED_EXTENSIONS)}"}
                ), 400
            filename = secure_filename(file.filename)
            unique_filename = f"preview_{datetime.now().strftime('%Y%m%d_%H%M%S')}_{filename}"
            temp_file_path = UPLOAD_FOLDER / unique_filename
            file.save(str(temp_file_path))
        else:
            return jsonify({"success": False, "error": "No file or temp_path provided"}), 400

        requested_encoding = str(request.form.get("fileEncoding", "") or "").strip()
        requested_delimiter = str(request.form.get("delimiter", "") or "").strip() or None
        rows, actual_encoding, actual_delimiter = _load_generic_import_rows(
            temp_file_path, requested_encoding=requested_encoding, delimiter=requested_delimiter
        )
        rows, detected_header_row = _trim_generic_import_rows_to_header(rows)

        headers = rows[0] if rows else []
        sample_rows = rows[1:11] if len(rows) > 1 else []

        return jsonify(
            {
                "success": True,
                "result": {
                    "headers": headers,
                    "sampleData": rows[:50],
                    "previewRows": sample_rows,
                    "totalRows": max(len(rows) - 1, 0),
                    "encoding": actual_encoding,
                    "delimiter": actual_delimiter,
                    "detectedHeaderRow": detected_header_row,
                },
            }
        )
    except Exception as e:  # pylint: disable=broad-except
        logger.error("[导入文件预览] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500
    finally:
        if should_delete_temp and temp_file_path and temp_file_path.exists():
            try:
                os.remove(temp_file_path)
            except OSError:
                logger.warning("[导入文件预览] 删除临时文件失败: %s", temp_file_path)


@bp.route("/import/configs/match", methods=["POST"])
@log_method
@require_auth
def match_import_config():
    """根据文件格式与表头自动匹配导入模板。"""
    try:
        data = request.get_json(silent=True) or {}
        file_format = str(data.get("fileFormat", "") or "").strip().lower()
        headers = data.get("headers") or []
        if not file_format:
            return jsonify({"success": False, "error": "fileFormat is required"}), 400
        if not isinstance(headers, list) or not headers:
            return jsonify({"success": False, "error": "headers is required"}), 400

        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            matched = loop.run_until_complete(
                db.find_matching_import_config(file_format=file_format, headers=headers, user_id=request.user_id)
            )

            if not matched:
                return jsonify({"success": True, "result": None})

            return jsonify(
                {
                    "success": True,
                    "result": {
                        "id": matched.get("id"),
                        "name": matched.get("name", ""),
                        "fileFormat": matched.get("file_format", ""),
                        "description": matched.get("description", ""),
                        "descriptionSummary": matched.get("description_summary", ""),
                        "fieldMappings": matched.get("field_mappings", {}),
                        "dateFormat": matched.get("date_format", ""),
                        "encoding": matched.get("encoding", "utf-8"),
                        "delimiter": matched.get("delimiter"),
                        "skipRows": int(matched.get("skip_rows", 0) or 0),
                        "hasHeader": bool(matched.get("has_header", True)),
                        "customRules": matched.get("custom_rules", {}),
                        "sampleHeaders": matched.get("sample_headers", []),
                        "defaultRecommendation": bool(matched.get("default_recommendation", False)),
                        "matchScore": matched.get("match_score", 0),
                        "matchReason": matched.get("match_reason", ""),
                        "matchedHeaderCount": matched.get("matched_header_count", 0),
                    },
                }
            )
        finally:
            loop.close()

    except Exception as e:
        logger.error("[匹配导入模板] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs/suggest", methods=["POST"])
@log_method
@require_auth
def suggest_import_config():
    """基于表头和样本行自动建议列映射。"""
    try:
        data = request.get_json(silent=True) or {}
        file_format = str(data.get("fileFormat", "") or "").strip().lower()
        headers = data.get("headers") or []
        sample_rows = data.get("sampleRows") or []
        if not file_format:
            return jsonify({"success": False, "error": "fileFormat is required"}), 400
        if not isinstance(headers, list) or not headers:
            return jsonify({"success": False, "error": "headers is required"}), 400

        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            configs = loop.run_until_complete(
                db.get_import_configs(user_id=request.user_id, file_format=file_format, limit=200)
            )
        finally:
            loop.close()

        suggestion = _build_import_mapping_suggestion(headers, configs, sample_rows)
        return jsonify({"success": True, "result": suggestion})

    except Exception as e:
        logger.error("[导入模板建议] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/configs/<int:config_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_import_config(config_id: int):
    """删除导入列映射模板。"""
    try:
        db, _, _ = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            success = loop.run_until_complete(db.delete_import_config(config_id, user_id=request.user_id))
            if not success:
                return jsonify({"success": False, "error": "Config not found"}), 404

            return jsonify({"success": True, "result": True})
        finally:
            loop.close()

    except Exception as e:
        logger.error("[删除导入模板] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/confirm", methods=["POST"])
@log_method
@require_auth
def confirm_import():
    """
    确认导入预览的账单

    用户在预览后可能修改了分类，然后确认导入

    Request:
        {
            'bills': [...]  # 经用户确认（可能修改）的账单列表
        }

    Response:
        {
            'success': true,
            'result': {
                'total': 100,
                'inserted': 95,
                'duplicates': 5
            }
        }
    """
    try:
        data = request.get_json()
        if not data or "bills" not in data:
            return jsonify({"success": False, "error": "bills is required"}), 400

        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        result = loop.run_until_complete(bill_service.import_preview_confirmed(data["bills"], user_id=user_id))
        loop.close()

        return jsonify({"success": result.get("success", False), "result": result})

    except Exception as e:
        logger.error("确认导入失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
