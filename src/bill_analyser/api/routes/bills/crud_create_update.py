# pylint: disable=wildcard-import,unused-wildcard-import,undefined-variable,line-too-long,missing-module-docstring,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,broad-exception-caught
from .support import *  # noqa: F403


def _prepare_backend_bill_for_create(
    frontend_data: dict[str, Any], db, category_engine, adapter, loop, user_id: int
) -> tuple[dict[str, Any], dict[str, Any]]:
    """将前端交易转换为可写入数据库的账单数据。"""
    backend_data, metadata = adapter.frontend_to_backend(frontend_data)
    logger.info("转换后的后端数据: %s", backend_data)
    logger.info("元数据: %s", metadata)

    # v6.89: create_bill() 会直接按传入字段构造 INSERT，bills.counterparty 为 NOT NULL。
    # 批量手工录入请求通常不显式提供 counterparty，因此这里统一补齐回退值，
    # 同时也保证 description 在前端未填写时仍有可写入的默认文本。
    description_fallback = str(
        frontend_data.get("comment") or frontend_data.get("remark") or frontend_data.get("description") or ""
    ).strip()
    counterparty_fallback = str(
        frontend_data.get("counterparty")
        or frontend_data.get("payee")
        or frontend_data.get("merchant")
        or frontend_data.get("merchantName")
        or frontend_data.get("shopName")
        or frontend_data.get("targetAccountName")
        or description_fallback
        or backend_data.get("payment_method")
        or "手工录入"
    ).strip()

    backend_data["description"] = str(
        backend_data.get("description") or description_fallback or counterparty_fallback
    ).strip()
    backend_data["counterparty"] = str(backend_data.get("counterparty") or counterparty_fallback).strip()

    if metadata.get("auto_invest_account", False) and backend_data.get("type") == "投资":
        all_accounts = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
        investment_account = None

        for acc in all_accounts:
            if acc["name"] in ["活期资产", "投资账户", "中信建投证券"]:
                investment_account = acc
                break

        if investment_account:
            backend_data["destination_account_id"] = investment_account["id"]
            backend_data["destination_amount"] = backend_data["amount"]
            logger.info("投资自动设置目标账户: %s (ID=%s)", investment_account["name"], investment_account["id"])
        else:
            logger.warning("未找到合适的投资目标账户，destination_account_id保持为0")

    source_account_id = backend_data.get("source_account_id")
    logger.info("[创建账单] 收到的source_account_id: %s (类型: %s)", source_account_id, type(source_account_id))
    logger.info(
        "[创建账单] 收到的destination_account_id: %s (类型: %s)",
        backend_data.get("destination_account_id"),
        type(backend_data.get("destination_account_id")),
    )

    if not source_account_id or source_account_id == 0:
        all_accounts_fallback = loop.run_until_complete(db.get_all_accounts(user_id=user_id))
        if all_accounts_fallback:
            fallback_account = all_accounts_fallback[0]
            backend_data["source_account_id"] = fallback_account["id"]
            logger.warning(
                "⚠ source_account_id为0，使用默认账户: %s (ID=%s)", fallback_account["name"], fallback_account["id"]
            )
        else:
            logger.error("✗✗✗ 没有可用账户，创建将失败！")
            raise ValueError("No account available")

    if metadata.get("category_id"):
        try:
            category = loop.run_until_complete(db.get_category_by_id(int(metadata["category_id"]), user_id=user_id))
            if category:
                backend_data["main_category"] = category.get("main_category", "")
                backend_data["sub_category"] = category.get("sub_category", "")
                logger.info("查询到分类: %s - %s", backend_data["main_category"], backend_data["sub_category"])
        except ValueError:
            logger.error("无效的分类ID: %s", metadata["category_id"])

    if not backend_data.get("main_category"):
        main_cat, sub_cat = category_engine.match_category(backend_data)
        if main_cat:
            backend_data["main_category"] = main_cat
            backend_data["sub_category"] = sub_cat
            logger.info("自动分类（规则匹配）: %s - %s", main_cat, sub_cat)
        else:
            bill_type = backend_data.get("type", "支出")
            if bill_type in DEFAULT_BILL_CATEGORY_MAPPING:
                backend_data["main_category"], backend_data["sub_category"] = DEFAULT_BILL_CATEGORY_MAPPING[bill_type]
                logger.info(
                    "自动分类（默认分类）: %s - %s", backend_data["main_category"], backend_data["sub_category"]
                )
            else:
                backend_data["main_category"] = "其他"
                backend_data["sub_category"] = ""
                logger.warning("未知账单类型: %s，使用默认分类'其他'", bill_type)

    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    backend_data["created_at"] = now
    backend_data["updated_at"] = now

    logger.info("=" * 60)
    logger.info("📝 准备插入数据库的完整数据:")
    logger.info("  - type: %s", backend_data.get("type"))
    logger.info("  - amount: %s", backend_data.get("amount"))
    logger.info("  - counterparty: %s", backend_data.get("counterparty"))
    logger.info("  - description: %s", backend_data.get("description"))
    logger.info("  - source_account_id: %s", backend_data.get("source_account_id"))
    logger.info("  - destination_account_id: %s", backend_data.get("destination_account_id"))
    logger.info("  - destination_amount: %s", backend_data.get("destination_amount"))
    logger.info("  - date: %s", backend_data.get("date"))
    logger.info("  - main_category: %s", backend_data.get("main_category"))
    logger.info("  - sub_category: %s", backend_data.get("sub_category"))
    logger.info("=" * 60)

    return backend_data, metadata


def _create_bill_and_build_response(
    backend_data: dict[str, Any], metadata: dict[str, Any], db, adapter, loop, user_id: int
) -> tuple[int, dict[str, Any]]:
    """写入账单并返回前端响应格式。"""
    bill_id = loop.run_until_complete(db.create_bill(backend_data, user_id=user_id))

    if not bill_id:
        raise RuntimeError("Failed to create bill")

    if metadata.get("tag_ids"):
        logger.info("[创建账单] 保存标签: %s", metadata["tag_ids"])
        loop.run_until_complete(db.add_tags_to_bill(bill_id, metadata["tag_ids"], user_id=user_id))

    sync_data = {
        "source_account_id": backend_data.get("source_account_id"),
        "destination_account_id": backend_data.get("destination_account_id"),
    }
    logger.info(
        "🔄 同步账户余额: source=%s, dest=%s", sync_data["source_account_id"], sync_data["destination_account_id"]
    )
    loop.run_until_complete(sync_balances_for_bill(db, sync_data))

    bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=user_id))
    tags = loop.run_until_complete(db.get_tags_for_bill(bill_id, user_id=user_id))
    frontend_bill = loop.run_until_complete(adapter.backend_to_frontend(bill, tags=tags))

    return bill_id, frontend_bill

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

    except ValueError as e:
        logger.error("更新账单参数错误: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 400

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


@bp.route("/import/batch", methods=["POST"])
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
