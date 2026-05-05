# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .import_detection import *  # noqa: F403
from .import_rows import *  # noqa: F403
from .import_mapping import *  # noqa: F403
from .import_review import *  # noqa: F403

def _parse_import_request_options() -> dict[str, Any]:
    requested_file_type = str(request.form.get("fileType", "auto") or "auto").strip().lower()
    from bill_analyser.parsers.factory import PARSER_CLASS_REGISTRY

    column_mapping = _parse_json_form_field(request.form.get("columnMapping"), {})
    generic_file_types = {"csv", "xlsx", "xls", "txt"}
    return {
        "requested_file_type": requested_file_type,
        "force_generic_parser": requested_file_type == "generic" or requested_file_type in generic_file_types,
        "parser_type": requested_file_type if requested_file_type in set(PARSER_CLASS_REGISTRY) else None,
        "column_mapping": column_mapping,
        "transaction_type_mapping": _parse_json_form_field(request.form.get("transactionTypeMapping"), {}),
        "has_header_line": _parse_bool_form_field(request.form.get("hasHeaderLine"), default=True),
        "time_format": str(request.form.get("timeFormat", "") or "").strip(),
        "amount_decimal_separator": str(request.form.get("amountDecimalSeparator", ".") or ".").strip() or ".",
        "amount_digit_grouping_symbol": str(request.form.get("amountDigitGroupingSymbol", "") or "").strip(),
        "tag_separator": str(request.form.get("tagSeparator", ";") or ";").strip() or ";",
        "file_encoding": str(request.form.get("fileEncoding", "") or "").strip(),
        "delimiter": str(request.form.get("delimiter", "") or "").strip() or None,
        "use_column_mapping": isinstance(column_mapping, dict) and bool(column_mapping),
    }


def _save_parse_import_upload(file) -> Path:
    filename = secure_filename(file.filename)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    file_path = UPLOAD_FOLDER / f"{timestamp}_{filename}"
    file.save(str(file_path))
    logger.info("临时文件已保存: %s", file_path)
    return file_path


def _parse_normalized_bills_for_preview(
    file_path: Path,
    options: dict[str, Any],
    db,
    loop,
    user_id: int,
) -> tuple[list[dict[str, Any]], str, str, str]:
    from bill_analyser.parsers.factory import ParserFactory

    parser_factory = ParserFactory()
    detected_parser_info = parser_factory.detect_parser(str(file_path))
    detected_parser_type = detected_parser_info.get("id", "") if detected_parser_info else ""
    detected_parser_name = str(
        (detected_parser_info or {}).get("name") or (detected_parser_info or {}).get("parser_name") or ""
    ).strip()
    resolved_parser_type = "generic"
    normalized_bills: list[dict[str, Any]] = []

    if detected_parser_info and not options["force_generic_parser"]:
        resolved_parser_type = options["parser_type"] or detected_parser_type
        logger.info("[导入解析] 已识别专用解析器 %s，走 parser-first 主链", resolved_parser_type)
        normalized_bills = parser_factory.parse(str(file_path), parser_type=resolved_parser_type)

    if normalized_bills:
        return normalized_bills, resolved_parser_type, detected_parser_type, detected_parser_name

    configs = loop.run_until_complete(
        db.get_import_configs(user_id=user_id, file_format=file_path.suffix.lower().lstrip("."), limit=200)
    )
    if options["use_column_mapping"]:
        logger.info(
            "[通用导入] 使用列映射解析: file_type=%s, has_header=%s",
            options["requested_file_type"],
            options["has_header_line"],
        )
        normalized_bills, actual_encoding, actual_delimiter = _parse_import_file_with_column_mapping(
            file_path,
            column_mapping=options["column_mapping"],
            transaction_type_mapping=options["transaction_type_mapping"],
            has_header_line=options["has_header_line"],
            time_format=options["time_format"],
            amount_decimal_separator=options["amount_decimal_separator"],
            amount_digit_grouping_symbol=options["amount_digit_grouping_symbol"],
            tag_separator=options["tag_separator"],
            file_encoding=options["file_encoding"],
            delimiter=options["delimiter"],
        )
    else:
        logger.info("[通用导入] 未命中专用解析器，使用自动列映射兜底解析")
        normalized_bills, auto_suggestion, actual_encoding, actual_delimiter = _parse_import_file_with_auto_mapping(
            file_path,
            configs=configs,
            requested_encoding=options["file_encoding"],
            delimiter=options["delimiter"],
        )
        if auto_suggestion.get("columnMapping"):
            logger.info("[通用导入] 自动建议列映射: %s", auto_suggestion.get("columnMapping"))

    logger.info("[通用导入] 解析完成: %s 条记录, encoding=%s, delimiter=%s", len(normalized_bills), actual_encoding, actual_delimiter)
    return normalized_bills, "generic", detected_parser_type, detected_parser_name


def _apply_parse_import_parser_metadata(
    normalized_bills: list[dict[str, Any]],
    resolved_parser_type: str,
    detected_parser_type: str,
    detected_parser_name: str,
) -> None:
    for bill in normalized_bills:
        bill.setdefault("_parser_id", detected_parser_type or resolved_parser_type or "generic")
        bill["parser_tags"] = resolve_parser_tags(
            bill.get("parser_tags"),
            parser_id=bill.get("_parser_id", ""),
            payment_method=str(bill.get("payment_method", "")).strip(),
            channel=str(bill.get("channel", "")).strip(),
        )
        if detected_parser_type and not str(bill.get("payment_method") or "").strip():
            bill["payment_method"] = detected_parser_name or detected_parser_type


def _cleanup_parse_import_upload(file_path: Path) -> None:
    try:
        os.remove(file_path)
        logger.debug("临时文件已删除: %s", file_path)
    except Exception as e:
        logger.warning("删除临时文件失败: %s", e)


@bp.route("/parse_import", methods=["POST"])
@log_method
@require_auth
def parse_import_file():
    """解析导入文件（兼容 v1 API）。"""
    logger.info("=" * 50)
    logger.info("解析导入文件")

    try:
        if "file" not in request.files:
            logger.error("未提供文件")
            return jsonify({"success": False, "error": "No file provided"}), 400

        file = request.files["file"]
        if file.filename == "":
            logger.error("文件名为空")
            return jsonify({"success": False, "error": "No file selected"}), 400

        options = _parse_import_request_options()
        logger.info("文件: %s, 请求解析器类型: %s, force_generic=%s", file.filename, options["requested_file_type"], options["force_generic_parser"])

        if not allowed_file(file.filename):
            logger.error("不支持的文件类型: %s", file.filename)
            return jsonify(
                {"success": False, "error": f"File type not allowed. Supported: {', '.join(ALLOWED_EXTENSIONS)}"}
            ), 400
        if file.filename is None:
            return jsonify({"success": False, "error": "Invalid filename"}), 400

        file_path = _save_parse_import_upload(file)
        db, bill_service, _ = get_app_context(user_id=request.user_id)
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        try:
            normalized_bills, resolved_parser_type, detected_parser_type, detected_parser_name = (
                _parse_normalized_bills_for_preview(file_path, options, db, loop, request.user_id)
            )
            _apply_parse_import_parser_metadata(normalized_bills, resolved_parser_type, detected_parser_type, detected_parser_name)
            normalized_bills, account_mappings, category_mappings, review_stats = loop.run_until_complete(
                _prepare_import_review_bills(normalized_bills, db, bill_service, request.user_id)
            )
            items = [
                _convert_bill_to_import_item_with_mappings(
                    bill, account_mappings=account_mappings, category_mappings=category_mappings
                )
                for bill in normalized_bills
            ]
            logger.info("[导入解析] 预览后处理完成: %s", review_stats)
        finally:
            loop.close()

        _cleanup_parse_import_upload(file_path)
        logger.info("=" * 50)
        return jsonify(
            {
                "success": True,
                "result": {
                    "items": items,
                    "totalCount": len(items),
                    "parserType": resolved_parser_type,
                    "detectedParserType": detected_parser_type,
                },
            }
        )

    except Exception as e:
        logger.error("解析导入文件失败: %s", e, exc_info=True)
        logger.info("=" * 50)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
