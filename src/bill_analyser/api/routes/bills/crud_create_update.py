# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403
from .crud_prepare import *  # noqa: F403

@bp.route("/", methods=["POST"])
@log_method
@require_auth
def create_bill():
    """创建单条账单"""
    try:
        frontend_data = request.get_json()
        if not frontend_data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        logger.info("收到前端数据: %s", frontend_data)

        db, _, category_engine, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            backend_data, metadata = _prepare_backend_bill_for_create(
                frontend_data, db, category_engine, adapter, loop, request.user_id
            )
            bill_id, v1_bill = _create_bill_and_build_response(
                backend_data, metadata, db, adapter, loop, request.user_id
            )

            logger.info("✅ 账单创建成功，ID: %s", bill_id)
            logger.info("返回创建的账单(v1格式): %s", v1_bill)

            return jsonify({"success": True, "result": v1_bill}), 201
        finally:
            loop.close()

    except ValueError as e:
        logger.error("创建账单参数错误: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 400

    except Exception as e:
        logger.error("创建账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch", methods=["POST"])
@log_method
@require_auth
def batch_create_bills():
    """批量创建账单。"""
    try:
        payload = request.get_json(silent=True)
        transactions = []

        if isinstance(payload, dict):
            transactions = payload.get("transactions") or payload.get("bills") or []
        elif isinstance(payload, list):
            transactions = payload

        if not isinstance(transactions, list) or not transactions:
            return jsonify({"success": False, "error": "transactions is required"}), 400

        db, _, category_engine, adapter = get_app_context_with_adapter()
        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        try:
            prepared_items: list[tuple[dict[str, Any], dict[str, Any]]] = []
            for index, transaction in enumerate(transactions):
                if not isinstance(transaction, dict):
                    return jsonify({"success": False, "error": f"transactions[{index}] must be an object"}), 400

                try:
                    prepared_items.append(
                        _prepare_backend_bill_for_create(
                            transaction, db, category_engine, adapter, loop, request.user_id
                        )
                    )
                except ValueError as prepare_error:
                    logger.error("批量创建预校验失败: index=%s, error=%s", index, prepare_error)
                    return jsonify(
                        {
                            "success": False,
                            "error": str(prepare_error),
                            "result": {"failedIndex": index, "createdCount": 0, "items": []},
                        }
                    ), 400

            created_items: list[dict[str, Any]] = []
            created_ids: list[str] = []

            for index, (backend_data, metadata) in enumerate(prepared_items):
                try:
                    bill_id, frontend_bill = _create_bill_and_build_response(
                        backend_data, metadata, db, adapter, loop, request.user_id
                    )
                    created_items.append(frontend_bill)
                    created_ids.append(str(bill_id))
                except Exception as create_error:
                    logger.error("批量创建账单失败: index=%s, error=%s", index, create_error, exc_info=True)
                    return jsonify(
                        {
                            "success": False,
                            "error": str(create_error),
                            "result": {
                                "failedIndex": index,
                                "createdCount": len(created_items),
                                "items": created_items,
                                "ids": created_ids,
                            },
                        }
                    ), 500

            return jsonify(
                {
                    "success": True,
                    "result": {"items": created_items, "ids": created_ids, "createdCount": len(created_items)},
                }
            ), 201
        finally:
            loop.close()

    except Exception as e:
        logger.error("批量创建账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>", methods=["PUT"])
@log_method
@require_auth
def update_bill(bill_id: int):
    """更新账单(支持v1格式)"""
    try:
        frontend_data = request.get_json()
        if not frontend_data:
            return jsonify({"success": False, "error": "No data provided"}), 400

        logger.info("更新账单 %s, 收到前端数据: %s", bill_id, frontend_data)

        db, _, category_engine, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 获取原账单，用于余额回滚
        old_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not old_bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 判断是前端格式还是后端格式
        if "sourceAmount" in frontend_data or "sourceAccountId" in frontend_data:
            # 前端v1格式,需要转换
            backend_data, metadata = adapter.frontend_to_backend(frontend_data)

            # source_account_id已在adapter中处理，无需额外查询
            # 验证source_account_id是否有效即可
            if metadata.get("source_account_id"):
                logger.info("源账户ID: %s", metadata["source_account_id"])

            # 查询分类名称
            if metadata.get("category_id"):
                try:
                    category = loop.run_until_complete(
                        db.get_category_by_id(int(metadata["category_id"]), user_id=request.user_id)
                    )
                    if category:
                        backend_data["main_category"] = category.get("main_category", "")
                        backend_data["sub_category"] = category.get("sub_category", "")
                        logger.info(
                            "查询到分类: %s - %s",
                            backend_data["main_category"],
                            backend_data["sub_category"],
                        )
                except ValueError:
                    logger.error("无效的分类ID: %s", metadata["category_id"])
        else:
            # 后端格式,直接使用
            backend_data = frontend_data
            metadata = {}
            # 如果有描述或对方变更，重新分类
            if "description" in backend_data or "counterparty" in backend_data:
                # 合并旧数据
                old_bill.update(backend_data)
                # 重新分类
                main_cat, sub_cat = category_engine.match_category(old_bill)
                if main_cat:
                    backend_data["main_category"] = main_cat
                    backend_data["sub_category"] = sub_cat

        backend_data["updated_at"] = datetime.now().strftime("%Y-%m-%d %H:%M:%S")

        logger.info("准备更新的后端数据: %s", backend_data)

        # 更新账单
        result = loop.run_until_complete(db.update_bill(bill_id, backend_data, user_id=request.user_id))

        if result:
            # **处理标签更新**
            if "tag_ids" in metadata:
                tag_ids = metadata["tag_ids"]
                logger.info("[更新账单] 更新标签: %s", tag_ids)
                loop.run_until_complete(db.update_bill_tags(bill_id, tag_ids, user_id=request.user_id))
            else:
                logger.debug("[更新账单] 未提供标签数据，保持现有标签")

            # 获取更新后的账单
            updated_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))

            # 获取标签
            tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=request.user_id))
            logger.info("[更新账单] 获取到标签: %s", tags)

            # **同步账户余额**
            # 1. 同步旧账单关联的账户 (回滚旧金额)
            sync_data_old = {
                "source_account_id": old_bill.get("source_account_id"),
                "destination_account_id": old_bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data_old))

            # 2. 同步新账单关联的账户 (应用新金额)
            # 如果账户ID没变，其实会被同步两次，但这保证了数据的最终一致性
            sync_data_new = {
                "source_account_id": updated_bill.get("source_account_id"),
                "destination_account_id": updated_bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data_new))

            # 使用adapter转换为v1格式
            v1_bill = loop.run_until_complete(adapter.backend_to_frontend(updated_bill, tags=tags))
            loop.close()

            return jsonify({"success": True, "result": v1_bill})

        loop.close()
        return jsonify({"success": False, "error": "Bill not found or update failed"}), 404

    except Exception as e:
        logger.error("更新账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/<int:bill_id>", methods=["DELETE"])
@log_method
@require_auth
def delete_bill(bill_id: int):
    """删除账单"""
    try:
        db, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # 先获取账单信息，用于回滚余额
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 删除账单
        result = loop.run_until_complete(db.delete_bill(bill_id, user_id=request.user_id))

        if result:
            # **使用全量同步更新余额**
            # 同步被删除账单相关的账户余额
            try:
                sync_data = {
                    "source_account_id": bill.get("source_account_id"),
                    "destination_account_id": bill.get("destination_account_id"),
                }
                loop.run_until_complete(sync_balances_for_bill(db, sync_data))
            except Exception as e:
                logger.error("删除账单后同步余额失败: %s", e, exc_info=True)
                # 不阻断删除成功的响应

            loop.close()
            return jsonify({"success": True, "result": True, "message": "Bill deleted successfully"})

        loop.close()
        return jsonify({"success": False, "error": "Failed to delete bill"}), 500
    except Exception as e:
        logger.error("删除账单失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/batch", methods=["POST"])
@log_method
@require_auth
def import_bills_batch():
    """批量导入账单"""
    try:
        data = request.get_json()
        if not data or "file_path" not in data:
            return jsonify({"success": False, "error": "file_path is required"}), 400

        _, bill_service, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)
        result = loop.run_until_complete(bill_service.import_bills(data["file_path"]))
        loop.close()

        return jsonify({"success": result["success"], "result": result})

    except Exception as e:
        logger.error("批量导入账单失败: %s", e)
        return jsonify({"success": False, "error": str(e)}), 500

__all__ = [name for name in globals() if not name.startswith("__")]
