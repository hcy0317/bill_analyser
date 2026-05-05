# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .import_review import *  # noqa: F403

@bp.route("/", methods=["GET"])
@log_method
@require_auth
def get_bills():
    """
    获取账单列表

    Query Parameters:
        - page: 页码（默认1）
        - page_size: 每页数量（默认20）
        - type: 类型过滤（数字或中文: 0=全部, 2=收入, 3=支出, 4=转账, 5=投资）
        - main_category: 主分类过滤
        - sub_category: 子分类过滤
        - start_date: 开始日期
        - end_date: 结束日期
        - keyword: 关键词搜索
    """
    try:
        # 获取查询参数
        page = int(_get_query_arg(request.args, "page", default=1))
        page_size = int(_get_query_arg(request.args, "page_size", "count", default=20))
        max_time = int(_get_query_arg(request.args, "max_time", default=0) or 0)
        min_time = int(_get_query_arg(request.args, "min_time", default=0) or 0)

        # 构建过滤条件
        filters = {}
        # 交易类型映射：前端v1格式(数字) -> 后端格式(中文)
        # 0=全部(不过滤), 2=收入, 3=支出, 4=转账, 5=投资
        if request.args.get("type"):
            type_param = request.args.get("type")
            try:
                type_int = int(type_param)
                type_mapping = {2: "收入", 3: "支出", 4: "转账", 5: "投资"}
                # type=0表示全部类型，不设置过滤条件
                if type_int in type_mapping:
                    filters["type"] = type_mapping[type_int]
                elif type_int != 0:
                    logger.warning("未知的type参数: %s, 已忽略", type_int)
            except ValueError:
                # 如果不是数字，认为是中文类型名，直接使用
                filters["type"] = type_param
        if request.args.get("main_category"):
            filters["main_category"] = request.args.get("main_category")
        if request.args.get("sub_category"):
            filters["sub_category"] = request.args.get("sub_category")
        if request.args.get("start_date"):
            filters["start_date"] = request.args.get("start_date")
        if request.args.get("end_date"):
            filters["end_date"] = request.args.get("end_date")
        if min_time > 0:
            filters["start_date"] = datetime.fromtimestamp(min_time / 1000).strftime("%Y-%m-%d")
        if max_time > 0:
            filters["end_date"] = datetime.fromtimestamp(max_time / 1000).strftime("%Y-%m-%d")

        db, _, _, adapter = get_app_context_with_adapter()

        # 异步调用
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        loop.run_until_complete(_apply_common_transaction_filters(request.args, filters, db, request.user_id))
        bills, total = loop.run_until_complete(
            db.query_bills(page=page, page_size=page_size, filters=filters, user_id=request.user_id)
        )

        # 使用adapter批量转换为v1格式
        response = loop.run_until_complete(adapter.backend_list_to_frontend(bills, total, page, page_size))
        loop.close()

        # 添加额外的分页信息
        response["result"]["total"] = total
        response["result"]["page"] = page
        response["result"]["page_size"] = page_size
        response["result"]["total_pages"] = (total + page_size - 1) // page_size

        return jsonify(response)

    except Exception as e:
        logger.error("获取账单列表失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/pictures", methods=["POST"])
@log_method
@require_auth
def upload_transaction_picture_rest():
    """上传交易图片（REST 主链）。"""
    try:
        if "picture" not in request.files:
            return jsonify({"success": False, "error": "Missing picture file"}), 400

        picture = request.files["picture"]
        if not picture or not picture.filename:
            return jsonify({"success": False, "error": "Invalid picture file"}), 400

        if not allowed_picture_file(picture.filename):
            return jsonify(
                {
                    "success": False,
                    "error": f"Picture type not allowed. Supported: {', '.join(sorted(ALLOWED_PICTURE_EXTENSIONS))}",
                }
            ), 400

        filename = secure_filename(picture.filename)
        suffix = Path(filename).suffix.lower()
        picture_id = f"{uuid.uuid4().hex}{suffix}"
        file_path = UPLOAD_FOLDER / picture_id
        picture.save(str(file_path))

        logger.info("[交易图片上传] user_id=%s, picture_id=%s", request.user_id, picture_id)

        return jsonify(
            {"success": True, "result": {"pictureId": picture_id, "originalUrl": _build_picture_data_url(file_path)}}
        )

    except Exception as e:
        logger.error("上传交易图片失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/pictures/unused", methods=["POST"])
@log_method
@require_auth
def remove_unused_transaction_picture_rest():
    """删除未使用的交易图片（REST 主链）。"""
    try:
        data = request.get_json() or {}
        picture_id = str(data.get("id", "") or "").strip()

        if not picture_id:
            return jsonify({"success": False, "error": "Missing picture id"}), 400

        file_path = UPLOAD_FOLDER / secure_filename(picture_id)
        if file_path.exists() and file_path.is_file():
            os.remove(file_path)
            logger.info("[交易图片删除] user_id=%s, picture_id=%s", request.user_id, picture_id)

        return jsonify({"success": True, "result": True})

    except Exception as e:
        logger.error("删除未使用交易图片失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/by-month", methods=["GET"])
@log_method
@require_auth
def get_bills_rest_by_month():
    """按月查询账单列表（REST 主链）。"""
    try:
        year = int(_get_query_arg(request.args, "year", default=datetime.now().year))
        month = int(_get_query_arg(request.args, "month", default=datetime.now().month))
        transaction_type = _get_query_arg(request.args, "type", default="0")

        filters = {}
        start_date = f"{year:04d}-{month:02d}-01"
        if month == 12:
            end_date = f"{year + 1:04d}-01-01"
        else:
            end_date = f"{year:04d}-{month + 1:02d}-01"

        filters["start_date"] = start_date
        filters["end_date"] = end_date

        if transaction_type and int(transaction_type) > 0:
            type_int = int(transaction_type)
            if type_int in BACKEND_TO_FRONTEND_TYPE.values():
                for chinese, v1_type in BACKEND_TO_FRONTEND_TYPE.items():
                    if v1_type == type_int:
                        filters["type"] = chinese
                        break

        db, _, _, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        loop.run_until_complete(_apply_common_transaction_filters(request.args, filters, db, request.user_id))
        bills, total = loop.run_until_complete(
            db.query_bills(page=1, page_size=10000, filters=filters, user_id=request.user_id)
        )
        response = loop.run_until_complete(adapter.backend_list_to_frontend(bills, total, 1, 10000))
        loop.close()

        return jsonify(response)

    except Exception as e:
        logger.error("按月获取账单列表失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>", methods=["GET"])
@log_method
@require_auth
def get_bill(bill_id: int):
    """获取单个账单详情"""
    try:
        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 获取标签
        tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

        # 使用adapter转换为v1格式
        v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
        loop.close()

        return jsonify({"success": True, "result": v1_bill})

    except Exception as e:
        logger.error("获取账单详情失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>/recurring-candidates", methods=["GET"])
@log_method
@require_auth
def get_bill_recurring_candidates(bill_id: int):
    """获取账单可匹配的定时交易候选。"""
    try:
        tolerance_days = request.args.get("toleranceDays", default=3, type=int)
        tolerance_days = max(0, min(tolerance_days, 31))

        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(
            db.get_recurring_candidates_for_bill(bill_id, user_id=request.user_id, tolerance_days=tolerance_days)
        )
        loop.close()

        if not result.get("bill"):
            return jsonify({"success": False, "error": "Bill not found"}), 404

        return jsonify(
            {
                "success": True,
                "result": {
                    "billId": bill_id,
                    "linkedRecurringId": result.get("linked_recurring_id"),
                    "linkedRecurringName": result.get("linked_recurring_name", ""),
                    "candidates": result.get("candidates", []),
                },
            }
        )
    except Exception as e:
        logger.error("获取定时交易候选失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>/recurring-match", methods=["PUT"])
@log_method
@require_auth
def bind_bill_recurring_match(bill_id: int):
    """将账单绑定到定时交易。"""
    try:
        data = request.get_json(silent=True) or {}
        recurring_id = data.get("recurringId")
        if recurring_id in (None, ""):
            return jsonify({"success": False, "error": "Missing recurringId"}), 400

        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.bind_bill_to_recurring(bill_id, int(recurring_id), user_id=request.user_id))
        loop.close()

        if not result:
            return jsonify({"success": False, "error": "Bill or recurring template not found"}), 404

        return jsonify({"success": True, "result": result})
    except Exception as e:
        logger.error("绑定定时交易失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>/recurring-match", methods=["DELETE"])
@log_method
@require_auth
def unbind_bill_recurring_match(bill_id: int):
    """取消账单与定时交易的绑定。"""
    try:
        db = get_app_context()[0]
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(db.unbind_bill_from_recurring(bill_id, user_id=request.user_id))
        loop.close()

        if not result:
            return jsonify({"success": False, "error": "Bill not found"}), 404

        return jsonify({"success": True, "result": True})
    except Exception as e:
        logger.error("取消定时交易绑定失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/get", methods=["GET"])
@log_method
@require_auth
def get_bill_by_query():
    """通过查询参数获取单个账单详情 (v1兼容)"""
    try:
        bill_id = request.args.get("id", type=int)
        if not bill_id:
            return jsonify({"success": False, "error": "Missing id parameter"}), 400

        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 获取标签
        tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))

        # 使用adapter转换为v1格式
        v1_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))
        loop.close()

        return jsonify({"success": True, "result": v1_bill})

    except Exception as e:
        logger.error("获取账单详情失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
