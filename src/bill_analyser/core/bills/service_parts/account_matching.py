"""Preview conversion, account matching, cash transfers, and type/category consistency."""
from __future__ import annotations

# pylint: disable=too-few-public-methods,too-many-lines,too-many-arguments,too-many-positional-arguments,too-many-locals,too-many-branches,too-many-statements,too-many-return-statements,too-many-nested-blocks,broad-exception-caught,duplicate-code,line-too-long,invalid-name,protected-access,consider-using-dict-items,use-implicit-booleaness-not-comparison,import-outside-toplevel,too-many-boolean-expressions

from .common import (
    Any,
    TransactionType,
    datetime,
    log_method,
)

class AccountMatchingMixin:
    """Preview conversion, account matching, cash transfers, and type/category consistency."""

    def _prepare_preview_data(self, bills: list[dict[str, Any]]) -> list[dict[str, Any]]:
        """准备预览数据，转换datetime为字符串，添加前端期望的字段"""
        preview_bills = []
        for bill in bills:
            preview_bills.append(self._bill_to_preview(bill))

        self.logger.info("[预览数据] 准备了 %d 条预览数据", len(preview_bills))
        if preview_bills:
            sample = preview_bills[0]
            self.logger.info("[预览数据] 示例字段: %s", list(sample.keys()))

        return preview_bills

    def _bill_to_preview(self, bill: dict[str, Any]) -> dict[str, Any]:
        """将账单转换为预览格式，添加前端期望的字段"""
        bill_copy = bill.copy()

        # 处理datetime类型
        for k, v in bill_copy.items():
            if isinstance(v, datetime):
                bill_copy[k] = v.strftime("%Y-%m-%d %H:%M:%S")

        # 前端期望字段: time, type, amount, description, counterparty, paymentMethod,
        #              main_category, sub_category, account
        # 后端实际字段: date, type, amount, description, counterparty, payment_method,
        #              main_category, sub_category, source_account_id

        # 添加time字段（前端期望）
        if "time" not in bill_copy and "date" in bill_copy:
            bill_copy["time"] = bill_copy["date"]

        # 添加account字段（前端期望）
        if "account" not in bill_copy:
            bill_copy["account"] = bill_copy.get("source_account_id", "")

        # v6.32: 添加paymentMethod字段（驼峰命名，前端期望）
        if "paymentMethod" not in bill_copy:
            bill_copy["paymentMethod"] = bill_copy.get("payment_method", "")

        self.logger.debug(
            "[预览数据] time=%s, counterparty=%s, paymentMethod=%s",
            bill_copy.get("time", ""),
            bill_copy.get("counterparty", "")[:20],
            bill_copy.get("paymentMethod", "")[:20],
        )
        return bill_copy

    @classmethod
    def _get_parser_source_label(cls, parser_id: Any) -> str:
        """Translate normalized parser IDs into human-readable source labels."""
        normalized_parser_id = str(parser_id or "").strip().lower()
        if not normalized_parser_id:
            return ""
        return cls._PARSER_SOURCE_LABELS.get(normalized_parser_id, normalized_parser_id)

    @classmethod
    def _build_account_match_text_candidates(
        cls,
        *,
        parser_id: Any = "",
        payment_method: Any = "",
        counterparty: Any = "",
        account_name: Any = "",
        tags: Any = None,
    ) -> list[str]:
        """Collect stable text hints for alias-based account matching."""
        text_candidates: list[str] = []
        for value in (payment_method, counterparty, account_name, parser_id):
            normalized_value = str(value or "").strip()
            if normalized_value and normalized_value not in text_candidates:
                text_candidates.append(normalized_value)

        parser_label = cls._get_parser_source_label(parser_id)
        if parser_label and parser_label not in text_candidates:
            text_candidates.append(parser_label)

        for tag in list(tags or []):
            normalized_tag = str(tag or "").strip().lower()
            if not normalized_tag.startswith("parser:"):
                continue
            related_parser_id = normalized_tag.split(":", 1)[1].strip()
            related_label = cls._get_parser_source_label(related_parser_id)
            if related_label and related_label not in text_candidates:
                text_candidates.append(related_label)

        return text_candidates

    @staticmethod
    def _extract_transfer_destination_hints(bill: dict[str, Any]) -> dict[str, Any]:
        """Read the stable transfer contract emitted by smart dedup, with legacy fallback."""
        transfer_sources = bill.get("_transfer_pair_sources")
        if isinstance(transfer_sources, list):
            for source in transfer_sources:
                if not isinstance(source, dict):
                    continue
                if str(source.get("role") or "").strip().lower() != "incoming":
                    continue
                return {
                    "parser_id": str(source.get("parser_id", "") or "").strip(),
                    "payment_method": str(source.get("payment_method", "") or "").strip(),
                    "counterparty": str(source.get("counterparty", "") or "").strip(),
                    "account_name": str(source.get("account_name", "") or "").strip(),
                    "account_id": source.get("source_account_id"),
                    "tags": list(source.get("tags") or []),
                }

        return {
            "parser_id": str(bill.get("_destination_parser_id", "") or "").strip(),
            "payment_method": str(bill.get("_destination_payment_method", "") or "").strip(),
            "counterparty": str(bill.get("_destination_counterparty", "") or "").strip(),
            "account_name": str(bill.get("_destination_account_name", "") or "").strip(),
            "account_id": bill.get("_destination_account_id"),
            "tags": [],
        }

    @log_method
    async def _match_accounts(self, bills: list[dict[str, Any]], user_id: int = 1) -> list[dict[str, Any]]:
        """
        根据账单描述自动匹配账户

        v6.39重构：简化账户匹配逻辑，只使用两级匹配

        匹配规则（按优先级）：
        1. 检查是否已是有效的数据库账户ID
        2. 使用 payment_method 与账户名称/别名匹配（优先级最高）
        3. 从 description 中提取关键词与账户名称/别名匹配（次优先级）

        注意：转账/投资类型的账户已在 SmartDeduplicationEngine 配对时设置，
        此方法主要处理收入/支出类型的源账户匹配。

        参数：
            bills: 账单列表
            user_id: 用户ID

        返回：
            List[Dict]: 添加了账户信息的账单列表
        """
        # 获取用户的账户别名映射
        alias_mapping = await self.db.get_account_alias_mapping(user_id)
        if not alias_mapping:
            self.logger.warning("用户没有任何账户，跳过账户匹配")
            return bills

        # 获取用户的所有账户（用于通过ID查找账户名称）
        accounts = await self.db.get_all_accounts(user_id=user_id)
        account_by_id = {str(acc.get("id")): acc for acc in accounts}

        matched_count = 0

        # v6.63: 简化日志，只输出别名数量
        self.logger.debug("账户别名映射: %d条", len(alias_mapping))

        # 预先按别名长度排序（长的优先匹配，更精确）
        sorted_aliases = sorted(alias_mapping.keys(), key=len, reverse=True)

        for bill in bills:
            # 获取当前的 source_account_id
            current_account_id = bill.get("source_account_id")

            # 检查是否已经是有效的数据库账户ID（数字类型）
            is_valid_account_id = isinstance(current_account_id, int) or (
                isinstance(current_account_id, str) and current_account_id.isdigit()
            )

            if is_valid_account_id and str(current_account_id) in account_by_id:
                matched_count += 1
                bill["account_name"] = account_by_id[str(current_account_id)].get("name")
                self.logger.debug("账单已有有效账户ID: %s", current_account_id)
            else:
                matched_account_id = None
                match_source = None

                # v6.39 优先级1: 使用 payment_method 匹配账户
                payment_method = str(bill.get("payment_method", "")).strip()
                if payment_method:
                    payment_method_lower = payment_method.lower()
                    for alias in sorted_aliases:
                        if alias.lower() in payment_method_lower:
                            matched_account_id = alias_mapping[alias]
                            match_source = f"payment_method:{alias}"
                            break

                # v6.66: 优先级2: 从 description/counterparty 中匹配账户别名
                # 将此优先级提前到 _parser_id 匹配之前
                if not matched_account_id:
                    description = str(bill.get("description", ""))
                    counterparty = str(bill.get("counterparty", ""))
                    combined_text = f"{description} {counterparty}"
                    combined_text_lower = combined_text.lower()

                    for alias in sorted_aliases:
                        if alias.lower() in combined_text_lower:
                            matched_account_id = alias_mapping[alias]
                            match_source = f"description:{alias}"
                            break

                # M2-1: 历史账单记忆匹配（优先于 parser_id 回退）
                if not matched_account_id:
                    history_match = await self.db.get_historical_source_account_suggestion(
                        user_id=user_id,
                        payment_method=str(bill.get("payment_method", "")),
                        counterparty=str(bill.get("counterparty", "")),
                        description=str(bill.get("description", "")),
                        bill_type=str(bill.get("type", "")),
                    )
                    if history_match:
                        matched_account_id = history_match["account_id"]
                        match_source = "history:" + ",".join(history_match.get("reasons", []))

                # v6.66: 优先级3（最低）- 使用 _parser_id 匹配账户（回退机制）
                # _parser_id 是解析器标识，如 'alipay', 'wechat', 'abc', 'icbc' 等
                # 仅当 payment_method 和 description/counterparty 都无法匹配时才使用
                if not matched_account_id:
                    parser_id = str(bill.get("_parser_id", "")).strip()
                    if parser_id:
                        parser_id_lower = parser_id.lower()
                        for alias in sorted_aliases:
                            if (
                                alias.lower() == parser_id_lower
                                or alias.lower() in parser_id_lower
                                or parser_id_lower in alias.lower()
                            ):
                                matched_account_id = alias_mapping[alias]
                                match_source = f"parser_id:{alias}"
                                self.logger.debug(
                                    "[源账户-parser_id匹配] parser_id='%s' 匹配别名='%s'", parser_id, alias
                                )
                                break

                # 如果匹配到账户，更新账单
                if matched_account_id:
                    matched_account = account_by_id.get(str(matched_account_id))
                    if matched_account:
                        bill["source_account_id"] = matched_account_id
                        bill["account_name"] = matched_account.get("name")
                        matched_count += 1
                        self.logger.debug(
                            "源账户匹配成功: 来源=%s -> 账户=%s (ID=%s)",
                            match_source,
                            matched_account.get("name"),
                            matched_account_id,
                        )
                else:
                    # 未匹配到账户，清空（避免混淆）
                    bill["source_account_id"] = None
                    self.logger.debug(
                        "源账户匹配失败: payment_method='%s', parser_id='%s', description='%s'",
                        payment_method[:20] if payment_method else "",
                        str(bill.get("_parser_id", ""))[:20],
                        str(bill.get("description", ""))[:30],
                    )

            # v6.42.1: 投资类型账单的目标账户匹配
            # 如果账单类型是投资，且没有destination_account_id，尝试从counterparty/description匹配
            bill_type = str(bill.get("type", "")).lower()
            is_investment = bill_type in ["投资", "investment", "5"]

            if is_investment:
                if str(bill.get("_investment_signal_type", "") or "") == "pnl_change":
                    source_id = bill.get("source_account_id")
                    is_valid_source = isinstance(source_id, int) or (
                        isinstance(source_id, str) and source_id.isdigit()
                    )
                    if is_valid_source and str(source_id) in account_by_id:
                        bill["destination_account_id"] = source_id
                        self.logger.debug("[投资盈亏变化] 使用同一账户作为目标账户: account_id=%s", source_id)
                    continue

                current_dest_id = bill.get("destination_account_id")
                # 检查目标账户是否已有效
                is_valid_dest = isinstance(current_dest_id, int) or (
                    isinstance(current_dest_id, str) and current_dest_id.isdigit()
                )

                if not is_valid_dest or str(current_dest_id) not in account_by_id:
                    # v6.65: 优先使用 counterparty 和 description 匹配目标账户
                    counterparty = str(bill.get("counterparty", ""))
                    description = str(bill.get("description", ""))
                    parser_id = str(bill.get("_parser_id", ""))
                    investment_hint = str(bill.get("_investment_hint", ""))
                    # 将 parser_id 也加入匹配文本，当 counterparty/description 无法匹配时提供回退
                    combined_text = f"{counterparty} {description} {parser_id} {investment_hint}"
                    combined_text_lower = combined_text.lower()

                    dest_account_id = None
                    dest_match_source = None

                    for alias in sorted_aliases:
                        if alias.lower() in combined_text_lower:
                            potential_id = alias_mapping[alias]
                            # 确保目标账户不同于源账户
                            if str(potential_id) != str(bill.get("source_account_id")):
                                dest_account_id = potential_id
                                dest_match_source = alias
                                break

                    # v6.65: 如果上面匹配失败，单独尝试使用 parser_id 精确匹配
                    if not dest_account_id and parser_id:
                        parser_id_lower = parser_id.lower()
                        for alias in sorted_aliases:
                            if (
                                alias.lower() == parser_id_lower
                                or alias.lower() in parser_id_lower
                                or parser_id_lower in alias.lower()
                            ):
                                potential_id = alias_mapping[alias]
                                if str(potential_id) != str(bill.get("source_account_id")):
                                    dest_account_id = potential_id
                                    dest_match_source = f"parser_id:{alias}"
                                    break

                    if not dest_account_id:
                        history_dest = await self.db.get_historical_destination_account_suggestion(
                            user_id=user_id,
                            payment_method=str(bill.get("payment_method", "")),
                            counterparty=counterparty,
                            description=description,
                            bill_type=str(bill.get("type", "")),
                            source_account_id=(
                                int(bill.get("source_account_id"))
                                if isinstance(bill.get("source_account_id"), int)
                                or (
                                    isinstance(bill.get("source_account_id"), str)
                                    and str(bill.get("source_account_id")).isdigit()
                                )
                                else None
                            ),
                        )
                        if history_dest:
                            dest_account_id = history_dest["account_id"]
                            dest_match_source = "history:" + ",".join(history_dest.get("reasons", []))

                    if dest_account_id:
                        dest_account = account_by_id.get(str(dest_account_id))
                        if dest_account:
                            bill["destination_account_id"] = dest_account_id
                            self.logger.debug(
                                "[投资目标账户] 匹配成功: 别名='%s' -> 账户=%s",
                                dest_match_source,
                                dest_account.get("name"),
                            )
                    else:
                        self.logger.debug(
                            "[投资目标账户] 匹配失败: counterparty='%s', parser_id='%s'",
                            counterparty[:30] if counterparty else "",
                            parser_id[:20] if parser_id else "",
                        )

            # v6.62: 转账类型账单的目标账户匹配
            # 使用转账配对时记录的 _destination_parser_id 和 _destination_payment_method
            is_transfer = bill_type in ["转账", "transfer", "4"]

            if is_transfer:
                current_dest_id = bill.get("destination_account_id")
                # 检查目标账户是否已有效
                is_valid_dest = isinstance(current_dest_id, int) or (
                    isinstance(current_dest_id, str) and current_dest_id.isdigit()
                )

                if not is_valid_dest or str(current_dest_id) not in account_by_id:
                    # 使用转账配对时记录的转入账单信息来匹配目标账户
                    destination_hints = self._extract_transfer_destination_hints(bill)
                    dest_parser_id = str(destination_hints.get("parser_id", "") or "")
                    dest_payment_method = str(destination_hints.get("payment_method", "") or "")
                    dest_counterparty = str(destination_hints.get("counterparty", "") or "")
                    dest_account_name = str(destination_hints.get("account_name", "") or "")
                    combined_text = " ".join(
                        self._build_account_match_text_candidates(
                            parser_id=dest_parser_id,
                            payment_method=dest_payment_method,
                            counterparty=dest_counterparty,
                            account_name=dest_account_name,
                            tags=destination_hints.get("tags"),
                        )
                    )
                    combined_text_lower = combined_text.lower()

                    dest_account_id = None
                    dest_match_source = None
                    dest_account_hint = destination_hints.get("account_id")
                    if (
                        (isinstance(dest_account_hint, int) or str(dest_account_hint).isdigit())
                        and str(dest_account_hint) in account_by_id
                        and str(dest_account_hint) != str(bill.get("source_account_id"))
                    ):
                        dest_account_id = dest_account_hint
                        dest_match_source = "pair_source_account"

                    if not dest_account_id:
                        for alias in sorted_aliases:
                            if alias.lower() in combined_text_lower:
                                potential_id = alias_mapping[alias]
                                # 确保目标账户不同于源账户
                                if str(potential_id) != str(bill.get("source_account_id")):
                                    dest_account_id = potential_id
                                    dest_match_source = alias
                                    break

                    if not dest_account_id:
                        history_dest = await self.db.get_historical_destination_account_suggestion(
                            user_id=user_id,
                            payment_method=dest_payment_method,
                            counterparty=dest_counterparty,
                            description=dest_account_name,
                            bill_type=str(bill.get("type", "")),
                            source_account_id=(
                                int(bill.get("source_account_id"))
                                if isinstance(bill.get("source_account_id"), int)
                                or (
                                    isinstance(bill.get("source_account_id"), str)
                                    and str(bill.get("source_account_id")).isdigit()
                                )
                                else None
                            ),
                        )
                        if history_dest:
                            dest_account_id = history_dest["account_id"]
                            dest_match_source = "history:" + ",".join(history_dest.get("reasons", []))

                    if dest_account_id:
                        dest_account = account_by_id.get(str(dest_account_id))
                        if dest_account:
                            bill["destination_account_id"] = dest_account_id
                            self.logger.debug(
                                "[转账目标账户] 匹配成功: parser_id='%s', "
                                "payment_method='%s', 匹配别名='%s' -> 账户=%s (ID=%s)",
                                dest_parser_id,
                                dest_payment_method[:20] if dest_payment_method else "",
                                dest_match_source,
                                dest_account.get("name"),
                                dest_account_id,
                            )
                    else:
                        self.logger.debug(
                            "[转账目标账户] 匹配失败: parser_id='%s', payment_method='%s'",
                            dest_parser_id,
                            dest_payment_method[:20] if dest_payment_method else "",
                        )

        self.logger.info("账户匹配完成: 源账户匹配 %d/%d 条", matched_count, len(bills))

        return bills

    @log_method
    async def _detect_cash_transfers(self, bills: list[dict[str, Any]], user_id: int = 1) -> list[dict[str, Any]]:
        """
        v6.77: 检测存取转账（用户指定的分类自动转换为转账类型）

        当账单的分类匹配用户设置的"存取分类"时，将其转换为转账类型：
        - 原收入账单：来源账户=现金账户，目标账户=原匹配账户
        - 原支出账单：来源账户=原匹配账户，目标账户=现金账户

        Args:
            bills: 账单列表
            user_id: 用户ID

        Returns:
            List[Dict]: 处理后的账单列表
        """
        # 1. 获取用户设置
        user = await self.db.get_user_by_id(user_id)
        if not user:
            self.logger.warning("[存取转账检测] 用户不存在: user_id=%d", user_id)
            return bills

        cash_account_id = user.get("cash_account_id")
        cash_transfer_category_id = user.get("cash_transfer_category_id")

        # 2. 如果没有配置，直接返回
        if not cash_account_id or not cash_transfer_category_id:
            self.logger.debug(
                "[存取转账检测] 用户未配置存取设置: cash_account_id=%s, cash_transfer_category_id=%s",
                cash_account_id,
                cash_transfer_category_id,
            )
            return bills

        # 3. 获取目标分类信息
        category = await self.db.get_category_by_id(cash_transfer_category_id, user_id)
        if not category:
            self.logger.warning("[存取转账检测] 分类不存在: category_id=%d", cash_transfer_category_id)
            return bills

        target_main_category = category.get("main_category", "")
        target_sub_category = category.get("sub_category", "")

        self.logger.info(
            "[存取转账检测] 开始检测, 目标分类='%s/%s', 现金账户ID=%d",
            target_main_category,
            target_sub_category,
            cash_account_id,
        )

        # 4. 遍历账单检测并转换
        converted_count = 0
        for bill in bills:
            bill_main_category = bill.get("main_category", "")
            bill_sub_category = bill.get("sub_category", "")

            # 检查分类是否匹配
            if bill_main_category == target_main_category and bill_sub_category == target_sub_category:
                # 获取原类型和金额
                original_type = str(bill.get("type", "")).lower()
                amount = float(bill.get("amount", 0))
                matched_account_id = bill.get("source_account_id")

                # 确定账户方向
                # 收入（amount > 0 或 type 包含收入）：现金 -> 匹配账户
                # 支出（amount < 0 或 type 包含支出）：匹配账户 -> 现金
                is_income = amount > 0 or original_type in ["收入", "income", "2"]

                if is_income:
                    # 收入：从现金账户转入匹配账户
                    bill["source_account_id"] = cash_account_id
                    bill["destination_account_id"] = matched_account_id
                    direction = "现金->账户"
                else:
                    # 支出：从匹配账户转出到现金账户
                    # source_account_id 保持不变（已是匹配账户）
                    bill["destination_account_id"] = cash_account_id
                    direction = "账户->现金"

                # 设置类型为转账
                bill["type"] = "转账"

                # 设置 destination_amount（与 amount 绝对值相等）
                bill["destination_amount"] = abs(amount)

                converted_count += 1
                self.logger.debug(
                    "[存取转账检测] 转换: date=%s, amount=%.2f, %s, source=%s, dest=%s",
                    bill.get("date", "")[:10],
                    amount,
                    direction,
                    bill.get("source_account_id"),
                    bill.get("destination_account_id"),
                )

        if converted_count > 0:
            self.logger.info("[存取转账检测] 完成, 转换 %d 条账单为转账类型", converted_count)

        return bills

    async def _validate_type_category_consistency(
        self, bills: list[dict[str, Any]], user_id: int = 1
    ) -> list[dict[str, Any]]:
        """验证类型/分类一致性，清除不匹配的分类。

        当账单 type 为支出/收入但分类属于转账类型时，清除该分类，
        避免出现 "type=支出 + category=转账" 的不一致状态。
        """
        _ = user_id
        if not bills:
            return bills

        # 构建转账类分类名称集合（从加载的规则中获取）
        transfer_category_names: set[tuple[str, str]] = set()
        if self.category_engine.is_initialized:
            for rule in self.category_engine.rules:
                if rule.get("type") == TransactionType.TRANSFER:
                    transfer_category_names.add((rule.get("main", ""), rule.get("sub", "")))

        if not transfer_category_names:
            return bills

        fixed_count = 0
        for bill in bills:
            bill_type = str(bill.get("type", "")).strip().lower()
            if bill_type in ["转账", "transfer", "4"]:
                continue  # 转账类型分配转账分类是正确的

            main_cat = bill.get("main_category", "")
            sub_cat = bill.get("sub_category", "")
            if (main_cat, sub_cat) in transfer_category_names:
                self.logger.debug(
                    "[一致性检查] 清除不一致分类: type='%s', category='%s/%s'",
                    bill.get("type"),
                    main_cat,
                    sub_cat,
                )
                bill["main_category"] = ""
                bill["sub_category"] = ""
                fixed_count += 1

        if fixed_count > 0:
            self.logger.info("[一致性检查] 修复 %d 条类型/分类不一致账单", fixed_count)

        return bills
