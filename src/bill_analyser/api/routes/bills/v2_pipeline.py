# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .import_detection import *  # noqa: F403
from .import_rows import *  # noqa: F403
from .import_mapping import *  # noqa: F403
from .import_review import *  # noqa: F403

@bp.route("/import/v2/parse", methods=["POST"])
@log_method
@require_auth
def import_stage1_parse():
    """三阶段导入阶段1：解析上传文件并写入 parser template。"""
    try:
        logger.info("[阶段1-解析] 开始处理上传文件")

        # 检查是否有文件
        if "files" not in request.files and "file" not in request.files:
            logger.warning("[阶段1-解析] 未找到上传文件")
            return jsonify({"success": False, "error": "No files provided"}), 400

        # 兼容单文件和多文件上传
        files = request.files.getlist("files") or [request.files["file"]]

        if not files or (len(files) == 1 and files[0].filename == ""):
            logger.warning("[阶段1-解析] 文件列表为空")
            return jsonify({"success": False, "error": "No files selected"}), 400

        # 获取服务实例
        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        # 保存文件到临时目录
        saved_files = []
        for file in files:
            if file.filename and allowed_file(file.filename):
                filename = secure_filename(file.filename)
                timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")
                unique_filename = f"{timestamp}_{filename}"
                file_path = UPLOAD_FOLDER / unique_filename
                file.save(str(file_path))
                saved_files.append({"path": str(file_path), "original_name": file.filename})
                logger.info("[阶段1-解析] 文件已保存: %s", file_path)
            else:
                logger.warning("[阶段1-解析] 跳过不支持的文件: %s", file.filename)

        if not saved_files:
            logger.error("[阶段1-解析] 没有有效的文件")
            return jsonify({"success": False, "error": "No valid files to process"}), 400

        # 生成session_id
        session_id = str(uuid.uuid4())
        logger.info("[阶段1-解析] 生成会话ID: %s", session_id)

        # 调用阶段1解析（仅处理特定解析器能识别的文件）
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            file_paths = [f["path"] for f in saved_files]
            result = loop.run_until_complete(bill_service.import_stage1_parse(file_paths, session_id, user_id))

            logger.info(
                "[阶段1-解析] 完成: session=%s, 总数=%s",
                result.get("session_id"),
                result.get("total_parsed", 0),
            )

            # 区分已匹配和未匹配的文件
            unmatched_files = []
            matched_files_to_clean = []
            file_results = result.get("file_results", [])

            for saved_file in saved_files:
                file_path = saved_file["path"]
                original_name = saved_file["original_name"]
                # 查找此文件是否被成功解析
                matched = False
                for fr in file_results:
                    if fr.get("file") == file_path and fr.get("success"):
                        matched = True
                        break
                if matched:
                    matched_files_to_clean.append(saved_file)
                else:
                    # 未匹配文件需要保留供后续列映射使用
                    unmatched_files.append({
                        "original_name": original_name,
                        "temp_path": file_path,
                    })

            # 仅清理已匹配文件的临时文件；未匹配文件保留
            for f in matched_files_to_clean:
                try:
                    os.remove(f["path"])
                    logger.debug("[阶段1-解析] 临时文件已删除: %s", f["path"])
                except Exception as e:
                    logger.warning("[阶段1-解析] 删除临时文件失败: %s", e)

            # 即使有未匹配文件，只要 session 已创建就算成功
            success = result.get("success", False) or len(unmatched_files) > 0

            return jsonify(
                {
                    "success": success,
                    "data": {
                        "session_id": result.get("session_id"),
                        "parsed_count": result.get("total_parsed", 0),
                        "files": file_results,
                        "unmatched_files": unmatched_files,
                        "errors": result.get("errors", []),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error("[阶段1-解析] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/parse_generic", methods=["POST"])
@log_method
@require_auth
def import_parse_generic_into_session():
    """
    三阶段导入 - 为未匹配的文件通过列映射解析并追加到已有会话。

    Request:
        JSON:
            - session_id: 已有的导入会话ID
            - temp_path: 后端保留的临时文件路径
            - column_mapping: 列映射 {columnType: columnIndex}
            - transaction_type_mapping: 类型映射
            - has_header_line: 是否含表头
            - time_format: 时间格式
            - amount_decimal_separator: 小数分隔符
            - amount_digit_grouping_symbol: 千分位符
            - tag_separator: 标签分隔符
            - file_encoding: 文件编码
            - delimiter: CSV分隔符
    """
    try:
        data = request.get_json(force=True)
        session_id = data.get("session_id", "").strip()
        temp_path = data.get("temp_path", "").strip()
        column_mapping = data.get("column_mapping") or {}
        transaction_type_mapping = data.get("transaction_type_mapping") or {}
        has_header_line = data.get("has_header_line", True)
        time_format = data.get("time_format", "")
        amount_decimal_separator = data.get("amount_decimal_separator", ".")
        amount_digit_grouping_symbol = data.get("amount_digit_grouping_symbol", "")
        tag_separator = data.get("tag_separator", ";")
        file_encoding = data.get("file_encoding", "")
        delimiter = data.get("delimiter", "")

        if not session_id:
            return jsonify({"success": False, "error": "Missing session_id"}), 400
        if not temp_path:
            return jsonify({"success": False, "error": "Missing temp_path"}), 400

        # 安全校验: 临时文件必须位于 UPLOAD_FOLDER 内
        temp_file_path = Path(temp_path).resolve()
        upload_folder_resolved = UPLOAD_FOLDER.resolve()
        if not str(temp_file_path).startswith(str(upload_folder_resolved)):
            logger.warning("[阶段1-通用解析] 路径安全校验失败: %s", temp_path)
            return jsonify({"success": False, "error": "Invalid file path"}), 400
        if not temp_file_path.exists():
            return jsonify({"success": False, "error": "Temp file not found"}), 404

        # 使用列映射解析
        bills, _actual_encoding, _actual_delimiter = _parse_import_file_with_column_mapping(
            temp_file_path,
            column_mapping=column_mapping,
            transaction_type_mapping=transaction_type_mapping,
            has_header_line=has_header_line,
            time_format=time_format,
            amount_decimal_separator=amount_decimal_separator,
            amount_digit_grouping_symbol=amount_digit_grouping_symbol,
            tag_separator=tag_separator,
            file_encoding=file_encoding,
            delimiter=delimiter,
        )

        if not bills:
            # 清理临时文件
            try:
                os.remove(str(temp_file_path))
            except OSError:
                pass
            return jsonify({"success": True, "data": {"parsed_count": 0}})

        # 获取服务实例
        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            # 验证
            for bill in bills:
                if not bill.get("date") and bill.get("trade_time"):
                    bill["date"] = bill["trade_time"]
            valid_bills, _invalid_bills = bill_service.validator.validate_bills(bills)
            # 写入 bills_parser_template
            inserted = 0
            if valid_bills:
                inserted = loop.run_until_complete(
                    bill_service.db.insert_parser_templates(
                        session_id, valid_bills, "generic", user_id
                    )
                )
            logger.info("[阶段1-通用解析] session=%s, 写入 %d 条通用解析模板", session_id, inserted)

            return jsonify({"success": True, "data": {"parsed_count": inserted}})
        finally:
            loop.close()
            # 清理临时文件
            try:
                os.remove(str(temp_file_path))
            except OSError:
                pass

    except Exception as e:
        logger.error("[阶段1-通用解析] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/dedup", methods=["POST"])
@log_method
@require_auth
def import_stage2_dedup():
    """
    三阶段导入 - 阶段2: 去重并预览

    对 bills_parser_template 表中的账单进行去重处理，
    结果写入 bills_preview 表供用户确认。

    Request:
        JSON:
            - session_id: 导入会话ID
            - include_preview: 可选，是否在响应中携带整批预览（默认 true，前端 OOM 修复路径会传 false）

    Response:
        {
            'success': true,
            'data': {
                'session_id': 'uuid',
                'preview': [...],         # 预览账单列表
                'total': 100,             # 原始总数
                'after_dedup': 80,        # 去重后数量
                'dedup_stats': {          # 去重统计
                    'transfer_pairs': 5,
                    'platform_bank': 10,
                    'similar': 3,
                    'split_merge': 2
                }
            }
        }
    """
    try:
        logger.info("[阶段2-去重] 开始处理")

        data = request.get_json()
        if not data or "session_id" not in data:
            logger.warning("[阶段2-去重] 缺少session_id")
            return jsonify({"success": False, "error": "Missing session_id"}), 400

        session_id = data["session_id"]
        include_preview = bool(data.get("include_preview", True))

        # 获取服务实例
        _, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 执行阶段2去重处理
            result = loop.run_until_complete(bill_service.import_stage2_dedup(session_id, user_id))

            # 获取预览数据返回给前端
            preview_data = []
            if result.get("success") and include_preview:
                preview_method_params = inspect.signature(bill_service.get_import_preview).parameters
                if "user_id" in preview_method_params:
                    preview_data = loop.run_until_complete(
                        bill_service.get_import_preview(session_id, selected_only=False, user_id=user_id)
                    )
                else:
                    preview_data = loop.run_until_complete(
                        bill_service.get_import_preview(session_id, selected_only=False)
                    )

            logger.info(
                "[阶段2-去重] 完成: session=%s, 原始=%s, 去重后=%s, 预览数据=%s条",
                session_id,
                result.get("template_count", 0),
                result.get("preview_count", 0),
                len(preview_data),
            )

            return jsonify(
                {
                    "success": result.get("success", False),
                    "data": {
                        "session_id": session_id,
                        "preview": preview_data,
                        "preview_included": include_preview,
                        "total": result.get("template_count", 0),
                        "after_dedup": result.get("preview_count", 0),
                        "dedup_stats": result.get("dedup_stats", {}),
                        "match_stats": result.get("match_stats", {}),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error("[阶段2-去重] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/v2/confirm", methods=["POST"])
@log_method
@require_auth
def import_stage3_confirm():
    """
    三阶段导入 - 阶段3: 确认导入

    将 bills_preview 表中的账单正式写入 bills 表，
    并清理临时表数据。

    Request:
        JSON:
            - session_id: 导入会话ID
            - selected_ids: 可选，用户选择的预览账单ID列表（不传则全部导入）
            - preview_updates: 可选，用户编辑后的预览数据列表

    Response:
        {
            'success': true,
            'data': {
                'imported_count': 80,     # 导入成功数量
                'skipped_count': 0,       # 跳过数量
                'errors': []
            }
        }
    """
    try:
        logger.info("[阶段3-确认] 开始处理")

        data = request.get_json()
        if not data or "session_id" not in data:
            logger.warning("[阶段3-确认] 缺少session_id")
            return jsonify({"success": False, "error": "Missing session_id"}), 400

        session_id = data["session_id"]
        selected_ids = data.get("selected_ids")  # 可选：用户选择的账单ID
        preview_updates = data.get("preview_updates")  # 可选：用户编辑后的数据

        # 获取服务实例
        db, bill_service, _ = get_app_context()
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # v6.61: 如果有preview_updates，先重置所有选中状态，再更新选中的账单
            # 这确保只有前端传入的选中账单才会被导入，未选中的不会被导入
            if preview_updates:
                # 第一步：重置该会话所有账单的选中状态为未选中
                reset_selection_params = inspect.signature(db.reset_session_preview_selection).parameters
                if "user_id" in reset_selection_params:
                    reset_count = loop.run_until_complete(
                        db.reset_session_preview_selection(session_id, user_id)
                    )
                else:
                    reset_count = loop.run_until_complete(db.reset_session_preview_selection(session_id))
                logger.info("[阶段3-确认] 已重置 %s 条账单的选中状态", reset_count)

                # 第二步：更新前端传入的选中账单
                logger.info("[阶段3-确认] 更新 %s 条预览数据", len(preview_updates))
                loop.run_until_complete(db.update_preview_bills_batch(session_id, preview_updates, user_id))
                # 从preview_updates中提取选中的ID
                selected_ids = [u["id"] for u in preview_updates if u.get("selected", True) and u.get("id")]
                logger.info("[阶段3-确认] 选中的账单ID数: %s", len(selected_ids))

            result = loop.run_until_complete(bill_service.import_stage3_confirm(session_id, user_id, selected_ids))

            logger.info("[阶段3-确认] 完成: session=%s, 导入=%s", session_id, result.get("imported_count", 0))

            return jsonify(
                {
                    "success": result.get("success", False),
                    "data": {
                        "imported_count": result.get("imported_count", 0),
                        "skipped_count": result.get("skipped_count", 0),
                        "errors": result.get("errors", []),
                    },
                }
            )

        finally:
            loop.close()

    except Exception as e:
        logger.error("[阶段3-确认] 失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
