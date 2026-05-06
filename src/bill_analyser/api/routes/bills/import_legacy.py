# pylint: disable=wildcard-import,unused-wildcard-import,undefined-variable
from .support import *  # noqa: F403
from .import_detection import *  # noqa: F403
from .import_rows import *  # noqa: F403
from .import_mapping import *  # noqa: F403
from .import_review import *  # noqa: F403

@bp.route("/import/upload", methods=["POST"])
@log_method
@require_auth
def upload_and_import():
    """
    上传文件并导入账单

    Request:
        - file: 上传的文件（支持csv, xlsx, xls, txt）
        - parser_type: 解析器类型（wechat/alipay/icbc/cmbc/abc/ccb）
        - preview_only: 是否仅预览（true/false）

    Response:
        {
            'success': true,
            'data': {
                'preview': [...],  # 预览数据
                'total': 100,      # 总记录数
                'imported': 95,    # 导入成功数
                'failed': 5,       # 导入失败数
                'duplicates': 10   # 重复记录数
            }
        }
    """
    try:
        # 检查文件是否在请求中
        if "file" not in request.files:
            return jsonify({"success": False, "error": "No file provided"}), 400

        file = request.files["file"]

        # 检查文件名是否为空
        if file.filename == "":
            return jsonify({"success": False, "error": "No file selected"}), 400

        # 检查文件扩展名
        if not allowed_file(file.filename):
            return jsonify(
                {"success": False, "error": f"File type not allowed. Supported: {', '.join(ALLOWED_EXTENSIONS)}"}
            ), 400

        # 获取解析器类型
        parser_type = request.form.get("parser_type", "auto")
        preview_only = request.form.get("preview_only", "false").lower() == "true"

        # 保存文件
        if file.filename is None:
            return jsonify({"success": False, "error": "Invalid filename"}), 400

        filename = secure_filename(file.filename)
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        unique_filename = f"{timestamp}_{filename}"
        file_path = UPLOAD_FOLDER / unique_filename

        file.save(str(file_path))
        logger.info("文件已保存: %s", file_path)

        # 导入账单
        _, bill_service, _ = get_app_context()

        # 获取用户ID
        user_id = getattr(request, "user_id", 1)

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取解析结果
        result = loop.run_until_complete(
            bill_service.import_bills(
                str(file_path), parser_type=parser_type, preview_only=preview_only, user_id=user_id
            )
        )

        loop.close()

        # 如果不是预览模式且导入成功，删除临时文件
        if not preview_only and result.get("success"):
            try:
                os.remove(file_path)
                logger.info("临时文件已删除: %s", file_path)
            except Exception as error:  # pylint: disable=broad-except
                logger.warning("删除临时文件失败: %s", error)

        # 日志记录返回数据
        preview_count = len(result.get("preview", []))
        logger.info(
            "[导入API返回] success=%s, preview_count=%s, total=%s, valid=%s",
            result.get("success"),
            preview_count,
            result.get("total"),
            result.get("valid"),
        )

        # 前端期望格式: { success, data: { preview: [...] } }
        return jsonify(
            {
                "success": result.get("success", False),
                "data": {
                    "preview": result.get("preview", []),
                    "total": result.get("total", 0),
                    "valid": result.get("valid", 0),
                    "invalid": result.get("invalid", 0),
                    "inserted": result.get("inserted", 0),
                    "duplicates": result.get("duplicates", 0),
                    "dedup_stats": result.get("dedup_stats"),
                    "parser_type": result.get("parser_type", "unknown"),
                    "errors": result.get("errors", []),
                },
            }
        )

    except Exception as error:
        logger.error("上传并导入账单失败: %s", error, exc_info=True)
        return jsonify({"success": False, "error": str(error)}), 500


@bp.route("/import/parsers", methods=["GET"])
@log_method
@require_auth
def get_available_parsers():
    """
    获取可用的解析器列表

    Response:
        {
            'success': true,
            'data': [
                {'id': 'wechat', 'name': '微信支付', 'description': '...'},
                {'id': 'alipay', 'name': '支付宝', 'description': '...'},
                ...
            ]
        }
    """
    try:
        parsers = [
            {
                "id": "auto",
                "name": "自动识别",
                "description": "自动检测文件类型并选择合适的解析器",
                "supported_formats": ["csv"],
            },
            {
                "id": "wechat",
                "name": "微信支付",
                "description": "解析微信支付账单CSV文件",
                "supported_formats": ["csv"],
            },
            {
                "id": "alipay",
                "name": "支付宝",
                "description": "解析支付宝交易明细CSV文件",
                "supported_formats": ["csv"],
            },
            {
                "id": "icbc",
                "name": "工商银行",
                "description": "解析工商银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
            {
                "id": "cmbc",
                "name": "民生银行",
                "description": "解析民生银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
            {
                "id": "abc",
                "name": "农业银行",
                "description": "解析农业银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
            {
                "id": "ccb",
                "name": "建设银行",
                "description": "解析建设银行流水文件",
                "supported_formats": ["csv", "xlsx", "xls"],
            },
        ]

        return jsonify({"success": True, "result": parsers})

    except Exception as e:
        logger.error("获取解析器列表失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/import/reclassify", methods=["POST"])
@log_method
@require_auth
def reclassify_transactions():
    """重新计算导入预览交易的分类与账户匹配。"""
    try:
        data = request.get_json()
        if not data or "transactions" not in data:
            return jsonify({"success": False, "error": "缺少transactions字段"}), 400

        transactions = data["transactions"]
        if not isinstance(transactions, list):
            return jsonify({"success": False, "error": "transactions必须是数组"}), 400

        logger.info("[重新分类] 收到 %s 条交易", len(transactions))

        # 获取分类引擎和数据库
        db, _, category_engine = get_app_context()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            # 加载分类规则
            loop.run_until_complete(category_engine.load_rules_from_db(db, user_id=request.user_id))

            # 获取所有账户用于匹配
            all_accounts = loop.run_until_complete(db.get_all_accounts(user_id=request.user_id))

            # 获取所有分类用于ID查询
            all_categories = loop.run_until_complete(db.get_all_categories(user_id=request.user_id))
            category_map = {}
            for cat in all_categories:
                key = (cat.get("main_category", ""), cat.get("sub_category", ""))
                category_map[key] = cat

            results = []
            for idx, trans in enumerate(transactions):
                result = {"index": idx}

                # 构建用于分类匹配的bill结构
                bill = {
                    "description": trans.get("description", ""),
                    "counterparty": trans.get("counterparty", ""),
                    "amount": float(trans.get("amount", 0)),
                    "type": trans.get("type", "支出"),
                }

                # 1. 分类匹配
                main_cat, sub_cat = category_engine.match_category(bill)
                if main_cat:
                    result["mainCategory"] = main_cat
                    result["subCategory"] = sub_cat or ""
                    # 查找分类ID
                    cat_info = category_map.get((main_cat, sub_cat or ""))
                    if cat_info:
                        result["categoryId"] = str(cat_info.get("id", ""))
                        result["categoryName"] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
                    else:
                        result["categoryId"] = ""
                        result["categoryName"] = f"{main_cat}-{sub_cat}" if sub_cat else main_cat
                else:
                    result["categoryId"] = ""
                    result["categoryName"] = ""

                # 2. 账户匹配（通过账户名称或别名）
                original_source = trans.get("originalSourceAccountName", "")
                original_dest = trans.get("originalDestinationAccountName", "")

                if original_source:
                    source_account = _match_account_by_name(all_accounts, original_source)
                    result["sourceAccountId"] = str(source_account["id"]) if source_account else ""
                    result["sourceAccountName"] = source_account["name"] if source_account else ""

                if original_dest:
                    dest_account = _match_account_by_name(all_accounts, original_dest)
                    result["destinationAccountId"] = str(dest_account["id"]) if dest_account else ""
                    result["destinationAccountName"] = dest_account["name"] if dest_account else ""

                results.append(result)

            logger.info("[重新分类] 完成 %s 条交易的重新分类", len(results))

            return jsonify({"success": True, "result": results})

        finally:
            loop.close()

    except Exception as e:
        logger.error("重新分类失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
