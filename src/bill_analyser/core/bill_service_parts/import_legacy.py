"""Legacy one-shot import and confirmed-preview import orchestration."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    DeduplicationType,
    asyncio,
    datetime,
    log_method,
    log_step,
)

class ImportLegacyMixin:
    """Legacy one-shot import and confirmed-preview import orchestration."""

    @log_method
    @log_step("导入账单文件")
    async def import_bills(
        self, file_path: str, parser_type: str = "auto", preview_only: bool = False, user_id: int = 1
    ) -> dict[str, Any]:
        """
        导入账单文件

        参数：
            file_path: 账单文件路径
            parser_type: 解析器类型（auto/wechat/alipay/icbc/cmbc/abc/ccb）
            preview_only: 是否仅预览，不实际写入数据库
            user_id: 用户ID（多用户隔离）

        返回：
            Dict: 导入结果统计，包含：
                - success: 是否成功
                - total: 原始解析数量
                - valid: 验证通过数量
                - inserted: 插入数量
                - duplicates: 数据库重复数量
                - dedup_stats: 智能去重统计
                - uncategorized: 未分类账单列表（供人工分类）
                - preview: 预览数据（仅preview_only=True时）
        """
        if not self._initialized:
            await self.initialize()

        result = {
            "success": False,
            "file": file_path,
            "parser_type": parser_type,
            "preview_only": preview_only,
            "total": 0,
            "valid": 0,
            "invalid": 0,
            "inserted": 0,
            "duplicates": 0,
            "dedup_stats": None,
            "categories": {},
            "uncategorized": [],  # 未分类账单，需人工处理
            "errors": [],
            "preview": [],  # 预览数据
        }

        try:
            # 1. 解析文件
            self.logger.info("步骤 1/5: 解析文件 (parser_type=%s)", parser_type)
            loop = asyncio.get_event_loop()

            # 检测实际解析器类型
            if parser_type == "auto":
                parser_info = self.parser_factory.detect_parser(file_path)
                detected_type = parser_info["id"] if parser_info else None
                self.logger.info("自动检测解析器类型: %s", detected_type)
                result["detected_parser"] = detected_type
            else:
                detected_type = parser_type

            # 使用检测到的解析器解析
            bills = await loop.run_in_executor(
                None, self.parser_factory.parse, file_path, detected_type if detected_type else None
            )

            result["total"] = len(bills)
            result["parser_type"] = detected_type or "unknown"

            if not bills:
                result["errors"].append("文件解析失败或无有效数据")
                return result

            self.logger.info("解析完成: %d 条账单 (使用 %s 解析器)", len(bills), result["parser_type"])

            # 2. 验证账单
            self.logger.info("步骤 2/5: 验证数据")
            valid_bills, invalid_bills = await loop.run_in_executor(None, self.validator.validate_bills, bills)

            result["valid"] = len(valid_bills)
            result["invalid"] = len(invalid_bills)

            if invalid_bills:
                for invalid_bill in invalid_bills[:10]:  # 只记录前10个错误
                    result["errors"].append(
                        {"index": invalid_bill.get("_index"), "errors": invalid_bill.get("_validation_errors")}
                    )

            if not valid_bills:
                result["errors"].append("没有有效的账单数据")
                return result

            # 3. 智能去重（包含数据库对比，跨文件去重）
            self.logger.info("步骤 3/5: 智能去重（含数据库对比）")
            # v6.45: 使用 process_with_db() 替代 process()
            # process_with_db() 会额外与数据库已有账单对比，实现跨文件去重
            dedup_result = await self.smart_dedup_engine.process_with_db(valid_bills, self.db, user_id)
            deduplicated_bills = dedup_result.kept_bills

            # 统计数据库重复数量
            db_dup_count = sum(
                1 for g in dedup_result.duplicate_groups if g.type == DeduplicationType.DATABASE_DUPLICATE
            )

            result["dedup_stats"] = {
                "original_count": dedup_result.original_count,
                "removed_count": dedup_result.removed_count,
                "transfer_pairs": len(dedup_result.transfer_pairs),
                "split_groups": len(dedup_result.split_groups),
                "duplicate_groups": len(dedup_result.duplicate_groups),
                "database_duplicates": db_dup_count,
            }
            self.logger.info(
                "智能去重完成: 移除 %d, 转账对 %d, 分账组 %d, 数据库重复 %d",
                dedup_result.removed_count,
                len(dedup_result.transfer_pairs),
                len(dedup_result.split_groups),
                db_dup_count,
            )

            # 4. 分类账单（预览和正式导入都需要）
            self.logger.info("步骤 4/5: 自动分类 (user_id=%d)", user_id)

            # 分类规则以 category_rules 为 canonical source；每次导入分类前刷新，
            # 避免规则编辑后复用同一服务实例内的旧规则缓存。
            await self._reload_category_rules_for_import(user_id=user_id)

            learned_seed_count = await self._apply_import_learning_rules(
                deduplicated_bills, user_id=user_id, type_only=True, record_usage=False
            )
            if learned_seed_count > 0:
                self.logger.info("[长期学习] 分类前类型预填充 %d 条", learned_seed_count)

            await self._normalize_non_pair_investment_balance_changes(
                deduplicated_bills,
                user_id=user_id,
            )

            # v6.72: 简化分类逻辑 - 让 match_category() 自动根据账单金额正负选择分类类型
            # - 转账配对账单：使用转账类关键词（通过 _dedup_type='transfer' 识别）
            # - 收入账单（amount > 0）：只使用收入类和投资类关键词
            # - 支出账单（amount < 0）：只使用支出类和投资类关键词
            # 不再需要在调用层分离账单，由 match_category() 内部根据金额自动判断

            self.logger.info("[分类匹配] 共 %d 条账单待分类", len(deduplicated_bills))

            # 批量分类 - 不传递 types 参数，让 match_category() 自动根据账单特征选择
            categorized_bills = await self.category_engine.batch_match_categories(deduplicated_bills, types=None)

            # 5. 账户匹配（基于账户名称）
            self.logger.info("步骤 4.5/5: 自动匹配账户")
            matched_bills = await self._match_accounts(categorized_bills, user_id)

            # 6. v6.77: 存取转账检测（用户指定的分类自动转换为转账类型）
            self.logger.info("步骤 4.6/5: 存取转账检测")
            matched_bills = await self._detect_cash_transfers(matched_bills, user_id)

            learned_replay_count = await self._apply_import_learning_rules(
                matched_bills, user_id=user_id, type_only=False, record_usage=True
            )
            if learned_replay_count > 0:
                self.logger.info("[长期学习] 匹配后回放 %d 条", learned_replay_count)

            # 如果是预览模式，返回分类和匹配后的数据供前端确认
            if preview_only:
                # 分离已匹配和未匹配的账单
                matched_list = []
                unmatched_list = []

                for bill in matched_bills:
                    preview_bill = self._bill_to_preview(bill)

                    # 判断是否有有效分类（非空字符串）
                    has_category = bool(bill.get("main_category"))

                    # 判断是否有有效账户ID（必须是数字类型，不能是解析器标识符）
                    source_account_id = bill.get("source_account_id")
                    has_account = source_account_id is not None and (
                        isinstance(source_account_id, int)
                        or (isinstance(source_account_id, str) and source_account_id.isdigit())
                    )

                    preview_bill["is_matched"] = has_category and has_account
                    preview_bill["has_category"] = has_category
                    preview_bill["has_account"] = has_account

                    # 添加调试信息
                    self.logger.debug(
                        "预览账单: date=%s, main_category=%s, source_account_id=%s, "
                        "has_category=%s, has_account=%s, is_matched=%s",
                        bill.get("date", "")[:10],
                        bill.get("main_category"),
                        source_account_id,
                        has_category,
                        has_account,
                        preview_bill["is_matched"],
                    )

                    if preview_bill["is_matched"]:
                        matched_list.append(preview_bill)
                    else:
                        unmatched_list.append(preview_bill)

                result["preview"] = matched_list + unmatched_list  # 已匹配在前，未匹配在后
                result["matched_count"] = len(matched_list)
                result["unmatched_count"] = len(unmatched_list)
                result["success"] = True
                self.logger.info(
                    "预览模式统计: 已匹配 %d 条 (分类+账户), 未匹配 %d 条", len(matched_list), len(unmatched_list)
                )
                return result

            # 统计分类结果，收集未分类账单
            for bill in matched_bills:
                main_cat = bill.get("main_category")
                if main_cat:
                    result["categories"][main_cat] = result["categories"].get(main_cat, 0) + 1
                else:
                    # 收集未分类账单供人工分类
                    result["uncategorized"].append(self._bill_to_preview(bill))

            self.logger.info(
                "分类完成: 已分类 %d 条, 未分类 %d 条",
                len(matched_bills) - len(result["uncategorized"]),
                len(result["uncategorized"]),
            )

            # 5. 写入数据库
            self.logger.info("步骤 5/5: 写入数据库")
            batch_id = datetime.now().strftime("%Y%m%d%H%M%S")
            inserted = await self.db.insert_bills(matched_bills, batch_id, user_id=user_id)

            result["inserted"] = inserted
            result["duplicates"] = len(deduplicated_bills) - inserted
            result["success"] = True

            self.logger.info(
                "导入完成: 总计 %d 条, 有效 %d 条, 去重后 %d 条, 插入 %d 条, 数据库重复 %d 条, 未分类 %d 条",
                result["total"],
                result["valid"],
                len(deduplicated_bills),
                result["inserted"],
                result["duplicates"],
                len(result["uncategorized"]),
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("导入账单失败: %s", e, exc_info=True)
            result["errors"].append(str(e))

        return result

    @log_method
    async def import_multiple_files(self, file_paths: list[str], user_id: int = 1) -> dict[str, Any]:
        """
        批量导入多个文件

        参数：
            file_paths: 文件路径列表
            user_id: 用户ID

        返回：
            Dict: 汇总结果
        """
        summary = {
            "total_files": len(file_paths),
            "success_files": 0,
            "failed_files": 0,
            "total_bills": 0,
            "inserted_bills": 0,
            "uncategorized_bills": [],
            "results": [],
        }

        self.logger.info("开始批量导入 %d 个文件", len(file_paths))

        for file_path in file_paths:
            result = await self.import_bills(file_path, user_id=user_id)
            summary["results"].append(result)

            if result["success"]:
                summary["success_files"] += 1
                summary["total_bills"] += result["total"]
                summary["inserted_bills"] += result["inserted"]
                summary["uncategorized_bills"].extend(result.get("uncategorized", []))
            else:
                summary["failed_files"] += 1

        self.logger.info(
            "批量导入完成: 成功 %d 个文件，失败 %d 个文件，共插入 %d 条账单，未分类 %d 条",
            summary["success_files"],
            summary["failed_files"],
            summary["inserted_bills"],
            len(summary["uncategorized_bills"]),
        )

        return summary

    @log_method
    async def import_preview_confirmed(self, preview_bills: list[dict[str, Any]], user_id: int = 1) -> dict[str, Any]:
        """
        确认导入预览的账单

        用户在预览后确认，可能修改了分类，然后调用此方法实际导入

        参数：
            preview_bills: 经用户确认（可能修改）的预览账单列表
            user_id: 用户ID

        返回：
            Dict: 导入结果
        """
        if not self._initialized:
            await self.initialize()

        result = {"success": False, "total": len(preview_bills), "inserted": 0, "duplicates": 0, "errors": []}

        try:
            # 写入数据库
            batch_id = datetime.now().strftime("%Y%m%d%H%M%S")
            inserted = await self.db.insert_bills(preview_bills, batch_id, user_id=user_id)

            result["inserted"] = inserted
            result["duplicates"] = len(preview_bills) - inserted
            result["success"] = True

            self.logger.info(
                "预览确认导入完成: 总计 %d 条, 插入 %d 条, 重复 %d 条",
                result["total"],
                result["inserted"],
                result["duplicates"],
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("预览确认导入失败: %s", e, exc_info=True)
            result["errors"].append(str(e))

        return result
