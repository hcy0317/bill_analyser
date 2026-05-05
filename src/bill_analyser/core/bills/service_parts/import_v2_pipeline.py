"""Three-stage v2 import pipeline and session-level selection/cancel helpers."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    asyncio,
    inspect,
    log_method,
    log_step,
    resolve_parser_tags,
)

class ImportV2PipelineMixin:
    """Three-stage v2 import pipeline and session-level selection/cancel helpers."""

    @log_method
    @log_step("阶段1: 多文件并行解析")
    async def import_stage1_parse(self, file_paths: list[str], session_id: str, user_id: int = 1) -> dict[str, Any]:
        """
        阶段1：多文件并行解析

        使用多线程并行解析多个账单文件，将解析结果写入 bills_parser_template 表。
        每个文件使用独立线程通过 run_in_executor 执行解析。

        Args:
            file_paths: 账单文件路径列表
            session_id: 导入会话ID（UUID格式）
            user_id: 用户ID

        Returns:
            Dict: 解析结果统计
        """
        if not self._initialized:
            await self.initialize()

        result = {
            "success": False,
            "session_id": session_id,
            "file_count": len(file_paths),
            "total_parsed": 0,
            "failed_files": [],
            "file_results": [],
            "errors": [],
        }

        self.logger.info("[阶段1] 开始解析 %d 个文件, session_id=%s", len(file_paths), session_id)

        # 创建导入会话
        await self.db.create_import_session(session_id, user_id, len(file_paths))

        # 定义单文件解析函数（在线程池中执行）
        def parse_single_file(file_path: str) -> dict[str, Any]:
            """解析单个文件（同步，用于线程池）"""
            file_result = {
                "file": file_path,
                "success": False,
                "parser_type": None,
                "count": 0,
                "bills": [],
                "error": None,
            }

            try:
                # 检测解析器类型
                parser_info = self.parser_factory.detect_parser(file_path)
                if not parser_info:
                    file_result["error"] = f"无法识别文件格式: {file_path}"
                    return file_result

                parser_type = parser_info["id"]
                file_result["parser_type"] = parser_type

                # 解析文件
                bills = self.parser_factory.parse(file_path, parser_type)
                file_result["bills"] = bills
                file_result["count"] = len(bills)
                file_result["success"] = True

                self.logger.info("[阶段1] 文件解析完成: %s, parser=%s, count=%d", file_path, parser_type, len(bills))

            except Exception as e:  # pylint: disable=broad-except
                file_result["error"] = str(e)
                self.logger.error("[阶段1] 文件解析失败: %s, error=%s", file_path, e, exc_info=True)

            return file_result

        # 使用 asyncio.gather + run_in_executor 并行解析
        loop = asyncio.get_event_loop()
        parse_tasks = [loop.run_in_executor(None, parse_single_file, file_path) for file_path in file_paths]

        # 等待所有解析任务完成
        file_results = await asyncio.gather(*parse_tasks, return_exceptions=True)

        # 处理解析结果
        for i, file_result in enumerate(file_results):
            if isinstance(file_result, Exception):
                result["failed_files"].append(file_paths[i])
                result["errors"].append(f"{file_paths[i]}: {file_result!s}")
                continue

            result["file_results"].append(
                {
                    "file": file_result["file"],
                    "success": file_result["success"],
                    "parser_type": file_result["parser_type"],
                    "count": file_result["count"],
                    "error": file_result["error"],
                }
            )

            if file_result["success"] and file_result["bills"]:
                # 验证账单
                valid_bills, invalid_bills = self.validator.validate_bills(file_result["bills"])

                if valid_bills:
                    # 写入 bills_parser_template 表
                    inserted = await self.db.insert_parser_templates(
                        session_id, valid_bills, file_result["parser_type"], user_id
                    )
                    result["total_parsed"] += inserted
                    self.logger.info("[阶段1] 写入解析模板: %d 条", inserted)

                if invalid_bills:
                    self.logger.warning("[阶段1] 无效账单: %d 条", len(invalid_bills))
            else:
                result["failed_files"].append(file_result["file"])
                if file_result["error"]:
                    result["errors"].append(f"{file_result['file']}: {file_result['error']}")

        # 更新会话状态
        await self.db.update_import_session_status(
            session_id, "deduping" if result["total_parsed"] > 0 else "failed", total_parsed=result["total_parsed"]
        )

        result["success"] = result["total_parsed"] > 0
        self.logger.info(
            "[阶段1完成] session=%s, 解析成功=%d条, 失败文件=%d个",
            session_id,
            result["total_parsed"],
            len(result["failed_files"]),
        )

        return result

    @log_method
    @log_step("阶段2: 去重匹配预览")
    async def import_stage2_dedup(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        """
        阶段2：去重、账户匹配、分类匹配，生成预览数据

        从 bills_parser_template 读取解析数据，执行以下处理：
        1. 四种去重机制（transfer, platform_bank, similar, split_merge）
        2. 数据库重复检测
        3. 账户别名匹配
        4. 分类关键词匹配
        5. 写入 bills_preview 表

        Args:
            session_id: 导入会话ID
            user_id: 用户ID

        Returns:
            Dict: 去重和匹配结果统计
        """
        if not self._initialized:
            await self.initialize()

        result = {
            "success": False,
            "session_id": session_id,
            "template_count": 0,
            "preview_count": 0,
            "dedup_stats": {},
            "match_stats": {},
            "errors": [],
        }

        self.logger.info("[阶段2] 开始去重和匹配, session_id=%s", session_id)

        try:
            # 1. 获取未处理的解析模板
            templates = await self.db.get_unprocessed_templates_for_dedup(session_id)
            result["template_count"] = len(templates)

            if not templates:
                result["errors"].append("没有待处理的解析数据")
                await self.db.update_import_session_status(session_id, "failed")
                return result

            self.logger.info("[阶段2] 获取解析模板: %d 条", len(templates))

            # 2. 转换为标准账单格式（用于去重引擎）
            bills = []
            template_id_map = {}  # bill_index -> template_id

            for idx, template in enumerate(templates):
                # v6.74: 获取 payment_method，如果为空则使用 parser_id 作为回退
                raw_payment_method = template.get("parser_payment_method", "")
                parser_id = template.get("parser_id", "")
                if raw_payment_method and str(raw_payment_method).strip():
                    payment_method = str(raw_payment_method).strip()
                    payment_method_source = "parser_payment_method"
                elif parser_id and str(parser_id).strip():
                    # 使用 parser_id（如 'alipay', 'wechat', 'abc'）作为回退
                    payment_method = str(parser_id).strip()
                    payment_method_source = "parser_id_fallback"
                else:
                    payment_method = ""
                    payment_method_source = "empty"

                self.logger.debug(
                    "[模板转账单] idx=%d, payment_method='%s' (来源=%s), parser_id='%s'",
                    idx,
                    payment_method,
                    payment_method_source,
                    parser_id,
                )
                parser_tags = resolve_parser_tags(
                    template.get("parser_tags") or template.get("parser_tags_json"),
                    parser_id=parser_id,
                    payment_method=payment_method,
                )

                bill = {
                    "session_id": session_id,
                    "date": template.get("parser_date", ""),
                    "amount": float(template.get("parser_amount", 0)),
                    "type": template.get("parser_type", ""),
                    "description": template.get("parser_description", ""),
                    "counterparty": template.get("parser_counterparty", ""),
                    "payment_method": payment_method,
                    "original_type": template.get("parser_original_type", ""),
                    "original_category": template.get("parser_original_category", ""),
                    "source_account_id": template.get("parser_account_id"),
                    "_template_id": template.get("id"),  # 保存模板ID用于回溯
                    "_session_id": session_id,
                    "_parser_id": parser_id,
                    "_parser_tags": parser_tags,
                    "_payment_method_source": payment_method_source,  # 记录来源便于调试
                }
                bills.append(bill)
                template_id_map[idx] = template.get("id")

            # 3. 执行智能去重（包含数据库对比）
            dedup_result = await self.smart_dedup_engine.process_with_db(bills, self.db, user_id)

            result["dedup_stats"] = {
                "original_count": dedup_result.original_count,
                "removed_count": dedup_result.removed_count,
                "transfer_pairs": len(dedup_result.transfer_pairs),
                "split_groups": len(dedup_result.split_groups),
                "duplicate_groups": len(dedup_result.duplicate_groups),
            }

            kept_bills = dedup_result.kept_bills

            # 4. 加载分类规则（始终刷新 canonical category_rules，避免规则编辑后预览仍用旧缓存）
            await self._reload_category_rules_for_import(user_id=user_id)

            learned_seed_count = await self._apply_import_learning_rules(
                kept_bills, user_id=user_id, type_only=True, record_usage=False
            )
            if learned_seed_count > 0:
                self.logger.info("[阶段2] 长期学习类型预填充 %d 条", learned_seed_count)

            await self._normalize_non_pair_investment_balance_changes(
                kept_bills,
                user_id=user_id,
            )

            # 5. 分类匹配
            # v6.72: 简化分类逻辑 - 让 match_category() 自动根据账单金额正负选择分类类型
            # - 转账配对账单：使用转账类关键词（通过 _dedup_type='transfer' 识别）
            # - 收入账单（amount > 0）：只使用收入类和投资类关键词
            # - 支出账单（amount < 0）：只使用支出类和投资类关键词
            self.logger.info("[阶段2] 分类匹配: 共 %d 条账单", len(kept_bills))

            # 批量分类 - 不传递 types 参数，让 match_category() 自动根据账单特征选择
            categorized_bills = await self.category_engine.batch_match_categories(kept_bills, types=None)

            # 6. 账户匹配
            self.logger.info("[阶段2] 执行账户匹配...")
            matched_bills = await self._match_accounts(categorized_bills, user_id)

            # 7. v6.77: 存取转账检测（用户指定的分类自动转换为转账类型）
            self.logger.info("[阶段2] 存取转账检测...")
            matched_bills = await self._detect_cash_transfers(matched_bills, user_id)

            learning_replay_count = await self._apply_import_learning_rules(
                matched_bills, user_id=user_id, type_only=False, record_usage=True
            )
            if learning_replay_count > 0:
                self.logger.info("[阶段2] 长期学习结果回放 %d 条", learning_replay_count)

            # 7.5 类型/分类一致性验证 — 防止支出/收入类型账单被错误分配到转账分类
            matched_bills = await self._validate_type_category_consistency(matched_bills, user_id)

            # 8. 生成预览数据并写入 bills_preview 表
            self.logger.info("[阶段2] 生成预览数据...")
            preview_list = []
            matched_category_count = 0
            matched_account_count = 0
            recurring_templates = await self.db.get_enabled_recurring_templates(user_id=user_id)

            for bill in matched_bills:
                # 统计匹配情况
                has_category = bool(bill.get("main_category"))
                has_account = bill.get("source_account_id") is not None and (
                    isinstance(bill.get("source_account_id"), int)
                    or (isinstance(bill.get("source_account_id"), str) and bill.get("source_account_id", "").isdigit())
                )

                if has_category:
                    matched_category_count += 1
                if has_account:
                    matched_account_count += 1

                # v6.50: 确定去重类型 - 仅使用 _dedup_type 字段
                # 只有通过转账配对功能配对的账单才会有 _dedup_type='transfer'
                # 不再根据 type 字段回退判断，避免 platform_bank 被错误标记为 transfer
                dedup_type = bill.get("_dedup_type", "remaining")

                # 收集来源模板ID（确保类型安全）
                source_ids: list = []
                template_id = bill.get("_template_id")
                if template_id is not None:
                    source_ids.append(template_id)
                merged_ids = bill.get("_merged_template_ids")
                if merged_ids and isinstance(merged_ids, list):
                    source_ids.extend(merged_ids)

                # v6.50: 投资类型账单特殊处理
                # 投资类型的 destination_amount 应等于 amount（投资金额转入投资账户）
                bill_type = str(bill.get("type", "")).lower()
                is_investment = bill_type in ["投资", "investment", "5"]
                amount = float(bill.get("amount", 0))

                if is_investment:
                    # 投资类型：destination_amount = |amount| (正数)
                    destination_amount = abs(amount)
                else:
                    # 其他类型：使用原有值
                    destination_amount = float(bill.get("destination_amount", 0))

                # v6.74: 获取 preview_payment_method，如果为空则使用 _parser_id 作为回退
                raw_payment_method = bill.get("payment_method", "")
                parser_id = bill.get("_parser_id", "")
                if raw_payment_method and str(raw_payment_method).strip():
                    preview_payment_method = str(raw_payment_method).strip()
                elif parser_id and str(parser_id).strip():
                    # 使用 _parser_id（如 'alipay', 'wechat', 'abc'）作为回退
                    preview_payment_method = str(parser_id).strip()
                    self.logger.debug(
                        "[账单转预览] 使用 parser_id 回退: payment_method 为空, "
                        "使用 parser_id='%s' 作为 preview_payment_method",
                        parser_id,
                    )
                else:
                    preview_payment_method = ""

                recurring_candidates = self.db.build_recurring_candidates_for_bill_data(
                    bill, recurring_templates, linked_recurring_id=bill.get("created_from_recurring"), tolerance_days=3
                )
                top_recurring_candidate = recurring_candidates[0] if recurring_candidates else None

                destination_parser_id = str(bill.get("_destination_parser_id", "") or "").strip()
                preview_parser_tags = resolve_parser_tags(
                    [
                        *list(bill.get("_parser_tags") or []),
                        *([f"parser:{destination_parser_id}"] if destination_parser_id else []),
                    ],
                    parser_id=parser_id,
                    payment_method=preview_payment_method,
                )

                # 构建预览数据
                # v6.54: preview_amount 和 preview_destination_amount 使用绝对值
                preview_data = {
                    "preview_date": bill.get("date", ""),
                    "preview_type": bill.get("type", ""),
                    "preview_amount": abs(amount),
                    "preview_destination_amount": abs(destination_amount),
                    "preview_main_category": bill.get("main_category", ""),
                    "preview_sub_category": bill.get("sub_category", ""),
                    "preview_source_account_id": bill.get("source_account_id"),
                    "preview_destination_account_id": bill.get("destination_account_id"),
                    "preview_counterparty": bill.get("counterparty", ""),
                    "preview_payment_method": preview_payment_method,
                    "preview_description": bill.get("description", ""),
                    "preview_parser_id": bill.get("_parser_id", ""),
                    "preview_parser_tags": preview_parser_tags,
                    "preview_recurring_id": top_recurring_candidate.get("id") if top_recurring_candidate else None,
                    "preview_recurring_name": top_recurring_candidate.get("name", "")
                    if top_recurring_candidate
                    else "",
                    "preview_recurring_candidate_count": len(recurring_candidates),
                    "preview_recurring_match_score": top_recurring_candidate.get("matchScore", 0)
                    if top_recurring_candidate
                    else 0,
                    "preview_recurring_match_reasons": "|".join(top_recurring_candidate.get("matchReasons", []))
                    if top_recurring_candidate
                    else "",
                    "preview_recurring_matched_date": top_recurring_candidate.get("matchedOccurrenceDate", "")
                    if top_recurring_candidate
                    else "",
                    "dedup_type": dedup_type,
                    "dedup_source_ids": source_ids,
                }
                preview_list.append(preview_data)

            # 批量写入预览表
            preview_count = await self.db.insert_preview_bills_batch(session_id, preview_list, user_id)

            result["preview_count"] = preview_count
            result["match_stats"] = {
                "category_matched": matched_category_count,
                "account_matched": matched_account_count,
                "total": len(matched_bills),
            }

            # 标记模板为已处理
            template_ids: list[int] = []
            for t in templates:
                t_id = t.get("id")
                if t_id is not None:
                    template_ids.append(int(t_id))
            await self.db.update_parser_template_status(template_ids, processed=True)

            # 更新会话状态
            await self.db.update_import_session_status(session_id, "previewing", total_preview=preview_count)

            result["success"] = True
            self.logger.info(
                "[阶段2完成] session=%s, 预览=%d条, 分类匹配=%d, 账户匹配=%d",
                session_id,
                preview_count,
                matched_category_count,
                matched_account_count,
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("[阶段2] 处理失败: %s", e, exc_info=True)
            result["errors"].append(str(e))
            await self.db.update_import_session_status(session_id, "failed")

        return result

    @log_method
    @log_step("阶段3: 确认导入")
    async def import_stage3_confirm(
        self,
        session_id: str,
        user_id: int = 1,
        selected_ids: list[int] | None = None,  # v6.56: 改为接收选中的ID列表（可选）
    ) -> dict[str, Any]:
        """
        阶段3：确认导入

        将用户选中的预览账单写入正式账单表 bills。
        注意：预览表的更新（用户编辑、选中状态）由 API 层调用 db.update_preview_bills_batch 完成。
        本方法只负责将选中的预览账单写入正式表。

        Args:
            session_id: 导入会话ID
            user_id: 用户ID
            selected_ids: 可选，用户选中的预览账单ID列表（用于日志记录，实际过滤在 db 层）

        Returns:
            Dict: 确认结果
        """
        if not self._initialized:
            await self.initialize()

        result = {
            "success": False,
            "session_id": session_id,
            "confirmed_count": 0,
            "skipped_count": 0,
            "duplicate_count": 0,
            "errors": [],
        }

        self.logger.info(
            "[阶段3] 开始确认导入, session_id=%s, selected_ids=%s",
            session_id,
            len(selected_ids) if selected_ids else "all",
        )

        try:
            # v6.56: 预览表更新已在 API 层完成，这里直接确认写入正式表
            # confirm_preview_to_bills 会自动只处理 preview_is_selected=1 的账单
            confirm_result = await self.db.confirm_preview_to_bills(session_id, user_id)

            result["imported_count"] = confirm_result.get("confirmed_count", 0)
            result["duplicate_count"] = confirm_result.get("duplicate_count", 0)
            result["errors"].extend(confirm_result.get("errors", []))

            # 清理临时数据
            clear_session_params = inspect.signature(self.db.clear_session_data).parameters
            if "user_id" in clear_session_params:
                clear_result = await self.db.clear_session_data(session_id, user_id=user_id)
            else:
                clear_result = await self.db.clear_session_data(session_id)
            self.logger.info(
                "[阶段3] 清理临时数据: parser=%d, preview=%d",
                clear_result.get("parser_count", 0),
                clear_result.get("preview_count", 0),
            )

            result["success"] = True
            self.logger.info(
                "[阶段3完成] session=%s, 导入=%d条, 重复跳过=%d条",
                session_id,
                result["imported_count"],
                result["duplicate_count"],
            )

        except Exception as e:  # pylint: disable=broad-except
            self.logger.error("[阶段3] 确认失败: %s", e, exc_info=True)
            result["errors"].append(str(e))
            await self.db.update_import_session_status(session_id, "failed")

        return result

    @log_method
    async def update_preview_selections(self, session_id: str, selections: dict[int, bool]) -> int:
        """
        更新预览选中状态

        Args:
            session_id: 导入会话ID
            selections: {preview_id: selected, ...}

        Returns:
            int: 更新的记录数
        """
        self.logger.debug("[更新预览选中] session=%s, 选择数=%d", session_id, len(selections))

        select_ids = [pid for pid, selected in selections.items() if selected]
        deselect_ids = [pid for pid, selected in selections.items() if not selected]

        updated = 0
        if select_ids:
            updated += await self.db.update_preview_selection(select_ids, True)
        if deselect_ids:
            updated += await self.db.update_preview_selection(deselect_ids, False)

        return updated

    @log_method
    async def cancel_import_session(self, session_id: str, user_id: int = 1) -> dict[str, Any]:
        """
        取消导入会话，清理临时数据

        Args:
            session_id: 导入会话ID
            user_id: 用户ID

        Returns:
            Dict: 清理结果
        """
        self.logger.info("[取消导入] session_id=%s", session_id)

        # 清理临时数据
        clear_session_params = inspect.signature(self.db.clear_session_data).parameters
        if "user_id" in clear_session_params:
            clear_result = await self.db.clear_session_data(session_id, user_id=user_id)
        else:
            clear_result = await self.db.clear_session_data(session_id)

        # 更新会话状态
        await self.db.update_import_session_status(session_id, "cancelled")

        return {"success": True, "session_id": session_id, "cleared": clear_result}
