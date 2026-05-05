# pylint: disable=wildcard-import,unused-wildcard-import
from .support import *  # noqa: F403

@bp.route("/modify", methods=["POST"])
@log_method
@require_auth
def modify_bill():
    """修改账单 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or "id" not in data:
            return jsonify({"success": False, "error": "Missing id parameter"}), 400

        bill_id = int(data["id"])
        db, _, _, adapter = get_app_context_with_adapter()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **关键修复：先获取原账单类型**
        old_bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not old_bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        # 转换前端格式
        backend_data, metadata = adapter.frontend_to_backend(data)

        # **关键修复：只更新传入的字段**
        # 如果frontend_data只包含部分字段（如只有remark），只更新那些字段
        if "remark" in data and "type" not in data:
            # 简单更新模式：只有备注等非关键字段
            backend_data = {}
            if "remark" in data:
                backend_data["description"] = data["remark"]
            if "comment" in data:
                backend_data["description"] = data["comment"]
            # 保持原类型
            backend_data["type"] = old_bill["type"]
            logger.info("简单更新模式：只更新 description=%s", backend_data.get("description"))

        # **保持原类型：如果前端没有传type，使用原账单的类型**
        if "type" not in backend_data or not backend_data["type"]:
            backend_data["type"] = old_bill["type"]
            logger.info("保持原类型: %s", old_bill["type"])

        # source_account_id已经在adapter中设置，无需额外查询
        # 保持原有的source_account_id
        if "source_account_id" not in backend_data and old_bill:
            backend_data["source_account_id"] = old_bill.get("source_account_id", 0)

        # 查询分类
        if metadata.get("category_id"):
            try:
                category = loop.run_until_complete(
                    db.get_category_by_id(int(metadata["category_id"]), user_id=request.user_id)
                )
                if category:
                    backend_data["main_category"] = category.get("main_category")
                    backend_data["sub_category"] = category.get("sub_category")
            except ValueError:
                pass

        # **保持其他关键字段**
        for field in ["destination_account_id", "destination_amount"]:
            if field not in backend_data and field in old_bill:
                backend_data[field] = old_bill[field]
                logger.info("保持原字段 %s: %s", field, old_bill[field])

        # 更新账单
        result = loop.run_until_complete(db.update_bill(bill_id, backend_data, user_id=request.user_id))

        if result:
            # 更新标签
            if "tagIds" in data:
                loop.run_until_complete(db.update_bill_tags(bill_id, metadata["tag_ids"], user_id=request.user_id))

            # **使用全量同步更新余额**
            # 1. 同步旧账单相关的账户余额
            old_sync_data = {
                "source_account_id": old_bill.get("source_account_id"),
                "destination_account_id": old_bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, old_sync_data))

            # 2. 同步新账单相关的账户余额 (如果账户发生了变化)
            new_sync_data = {
                "source_account_id": backend_data.get("source_account_id", old_bill.get("source_account_id")),
                "destination_account_id": backend_data.get(
                    "destination_account_id", old_bill.get("destination_account_id")
                ),
            }
            loop.run_until_complete(sync_balances_for_bill(db, new_sync_data))

        loop.close()

        if result:
            return jsonify({"success": True, "result": {"id": str(bill_id)}})
        else:
            return jsonify({"success": False, "error": "Failed to update bill"}), 500

    except Exception as e:
        logger.error("修改账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


@bp.route("/delete", methods=["POST"])
@log_method
@require_auth
def delete_bill_by_query():
    """删除账单 (v1兼容)"""
    try:
        data = request.get_json()
        if not data or "id" not in data:
            return jsonify({"success": False, "error": "Missing id parameter"}), 400

        bill_id = int(data["id"])
        db, _, _ = get_app_context()

        loop = asyncio.new_event_loop()
        asyncio.set_event_loop(loop)

        # **关键修复：添加余额回滚逻辑**
        # 先获取账单信息
        bill = loop.run_until_complete(db.get_bill_by_id(bill_id, user_id=request.user_id))
        if not bill:
            loop.close()
            return jsonify({"success": False, "error": "Bill not found"}), 404

        result = loop.run_until_complete(db.delete_bill(bill_id, user_id=request.user_id))

        if result:
            # **使用全量同步更新余额**
            # 同步被删除账单相关的账户余额
            sync_data = {
                "source_account_id": bill.get("source_account_id"),
                "destination_account_id": bill.get("destination_account_id"),
            }
            loop.run_until_complete(sync_balances_for_bill(db, sync_data))

        loop.close()

        if result:
            return jsonify({"success": True})
        else:
            return jsonify({"success": False, "error": "Failed to delete bill"}), 500

    except Exception as e:
        logger.error("删除账单失败: %s", e, exc_info=True)
        return jsonify({"success": False, "error": str(e)}), 500


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

__all__ = [name for name in globals() if not name.startswith("__")]
