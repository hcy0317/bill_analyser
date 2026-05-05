"""CategoryEngine rule loading, matching, and cache lifecycle."""

from typing import Any

from ...utils.constants import TransactionType
from ...utils.logger import get_logger, log_method, log_step
from bill_analyser.core.investment.matching import is_ordinary_bank_interest_income
from .matcher import CompiledRule, KeywordMatcher


_VALID_CATEGORY_RULE_TYPES = {
    int(TransactionType.INCOME),
    int(TransactionType.EXPENSE),
    int(TransactionType.TRANSFER),
    int(TransactionType.INVESTMENT),
}
_LEGACY_EXPENSE_CATEGORY_TYPE = 1


def _normalize_category_rule_type(value: Any) -> int | None:
    """Normalize stored category type values into current TransactionType ids."""
    try:
        raw_type = int(value)
    except (TypeError, ValueError):
        return None

    if raw_type == _LEGACY_EXPENSE_CATEGORY_TYPE:
        return int(TransactionType.EXPENSE)
    if raw_type in _VALID_CATEGORY_RULE_TYPES:
        return raw_type
    return None


def _coerce_sort_int(value: Any, default: int = 999_999) -> int:
    """Return an integer sort key while tolerating DB/null/string values."""
    if value is None or value == "":
        return default
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


class CategoryEngine:
    """账单分类引擎 V2"""

    def __init__(self):
        """初始化分类引擎"""
        self.logger = get_logger("CategoryEngine")
        self.keyword_matcher = KeywordMatcher()
        self.rules: list[dict[str, Any]] = []
        self._initialized = False
        self._current_user_id: int = 1  # 当前加载规则的用户ID
        # v6.73: 预编译的规则缓存（keywords -> CompiledRule）
        self._compiled_rules: dict[str, CompiledRule] = {}
        # Lock已移除 - SQLite自带线程安全

    @staticmethod
    def _rule_match_sort_key(
        rule: dict[str, Any],
        fallback_order: int = 0,
    ) -> tuple[int, int, int, int]:
        """Sort runtime matching by category priority, then stable identifiers.

        ``category_rules.priority`` is kept as rule metadata for compatibility,
        but it is intentionally not part of the runtime matching order.
        """
        category_priority = rule.get("category_priority", rule.get("priority"))
        return (
            _coerce_sort_int(category_priority),
            _coerce_sort_int(rule.get("category_id")),
            _coerce_sort_int(rule.get("id"), fallback_order),
            fallback_order,
        )

    def _precompile_rules(self):
        """预编译所有规则关键词

        v6.73新增：在规则加载后调用，将所有规则的关键词字符串
        预编译为CompiledRule对象，避免每次匹配时重复解析。

        v6.74改进：在预编译前先清空 KeywordMatcher 的缓存，确保：
        - 用户修改分类关键词后，缓存中的旧规则被清除
        - 重新分类功能能使用最新的关键词规则

        性能优化效果：
        - 13106条账单 × 77条规则 = 1,009,162次匹配
        - 优化前：每次匹配都解析规则字符串
        - 优化后：规则只解析一次，直接使用预编译对象匹配
        """
        # v6.74: 先清空 KeywordMatcher 的缓存，确保使用最新规则
        self.keyword_matcher.clear_cache()

        # 清空 CategoryEngine 的规则缓存
        self._compiled_rules.clear()

        for rule in self.rules:
            keywords = rule.get("keywords", "")
            if keywords and keywords not in self._compiled_rules:
                self._compiled_rules[keywords] = self.keyword_matcher.compile_rule(keywords)

        self.logger.info("[分类引擎] 预编译了 %d 条规则关键词", len(self._compiled_rules))

    def _match_keywords_fast(self, text: str, keywords: str) -> bool:
        """使用预编译规则快速匹配关键词

        v6.73新增：使用预编译的CompiledRule进行匹配。
        如果规则未预编译，会自动编译并缓存。

        Args:
            text: 要匹配的文本
            keywords: 关键词规则字符串

        Returns:
            bool: 是否匹配
        """
        if not keywords or not text:
            return False

        # 获取预编译规则
        compiled = self._compiled_rules.get(keywords)
        if compiled is None:
            # 动态编译并缓存
            compiled = self.keyword_matcher.compile_rule(keywords)
            self._compiled_rules[keywords] = compiled

        return self.keyword_matcher.match_compiled(text, compiled)

    @property
    def is_initialized(self) -> bool:
        """是否已初始化"""
        return self._initialized

    @property
    def current_user_id(self) -> int:
        """当前加载规则的用户ID"""
        return self._current_user_id

    def invalidate_cache(self):
        """使规则缓存失效

        v6.74新增：当分类关键词被修改后，调用此方法使缓存失效。
        下次调用 load_rules_from_db() 时会重新加载和预编译规则。

        使用场景：
        - 用户修改了分类规则的关键词
        - 用户添加或删除了分类规则
        - 需要强制刷新分类引擎

        注意：此方法只是清空缓存，不会自动重新加载规则。
        需要调用 load_rules_from_db() 来重新加载规则。
        """
        cache_count = len(self._compiled_rules)
        self._compiled_rules.clear()
        self.keyword_matcher.clear_cache()
        self.logger.info("[分类引擎] 缓存已失效: 规则缓存=%d条", cache_count)

    @log_method
    @log_step("加载分类规则(DB)")
    async def load_rules_from_db(self, db, user_id: int = 1, types: list[int] | None = None):
        """从数据库加载分类规则（委托给 v2 实现）。"""
        await self.load_rules_from_db_v2(db, user_id=user_id, types=types)

    @log_method
    async def load_rules_from_db_v2(self, db, user_id: int = 1, types: list[int] | None = None):
        """从 category_rules canonical source 加载规则。

        1. 从 category_rules 加载已启用的规则（JOIN categories 获取分类元数据）
        2. 不再回退到 categories.keywords，避免运行时双规则源
        3. 按分类 priority 排序并预编译
        """
        # pylint: disable=too-many-locals
        try:
            types_str = str(types) if types else "all"
            self.logger.info(
                "从数据库加载分类规则 v2 (user_id=%s, types=%s)", user_id, types_str
            )

            type_filter: set = set(types) if types else set()

            valid_rules: list[dict[str, Any]] = []

            # --- Canonical source: category_rules table ---
            try:
                try:
                    cr_rows = await db.get_category_rules(
                        user_id=user_id,
                        enabled_only=True,
                        include_category_priority=True,
                    )
                except TypeError as exc:
                    if "include_category_priority" not in str(exc):
                        raise
                    cr_rows = await db.get_category_rules(
                        user_id=user_id,
                        enabled_only=True,
                    )
            except Exception:  # pylint: disable=broad-exception-caught
                # Table might not exist yet (pre-migration) or the query failed.
                cr_rows = []

            for row_index, row in enumerate(cr_rows):
                rule_type = _normalize_category_rule_type(
                    row.get("category_type", TransactionType.EXPENSE)
                )
                if rule_type is None:
                    self.logger.warning(
                        "[分类规则] 跳过未知类型规则: row_id=%s category_type=%s",
                        row.get("id"),
                        row.get("category_type"),
                    )
                    continue

                if types and rule_type not in type_filter:
                    continue

                category_id = _coerce_sort_int(row.get("category_id"), 0)
                if category_id <= 0:
                    self.logger.warning(
                        "[分类规则] 跳过缺失分类ID规则: row_id=%s main=%s sub=%s",
                        row.get("id"),
                        row.get("main_category", ""),
                        row.get("sub_category", ""),
                    )
                    continue

                main_category = str(row.get("main_category", "") or "").strip()
                sub_category = str(row.get("sub_category", "") or "").strip()
                if not main_category or not sub_category:
                    self.logger.warning(
                        "[分类规则] 跳过分类名称不完整规则: row_id=%s category_id=%s",
                        row.get("id"),
                        category_id,
                    )
                    continue

                expr = row.get("rule_expression", "")
                regex_enabled = bool(row.get("regex_enabled", False))

                # Pre-compile using the new v2 compiler
                compiled = self.keyword_matcher.compile_rule_expression(
                    expr, regex_enabled=regex_enabled,
                )

                valid_rules.append(
                    {
                        "id": row.get("id"),
                        "category_id": category_id,
                        "main": main_category,
                        "sub": sub_category,
                        "priority": row.get("category_priority", row.get("priority", 100)),
                        "category_priority": row.get(
                            "category_priority",
                            row.get("priority", 100),
                        ),
                        "rule_priority": row.get("priority", 100),
                        "keywords": expr,
                        "type": rule_type,
                        "_load_order": row_index,
                        "_compiled_v2": compiled,
                    }
                )

            # Sort by transaction-category priority; rule priority is metadata only.
            valid_rules.sort(
                key=lambda r: self._rule_match_sort_key(
                    r,
                    fallback_order=_coerce_sort_int(r.get("_load_order"), 0),
                )
            )

            self.rules = valid_rules
            self._initialized = True
            self._current_user_id = user_id

            # Pre-compile rules (old-style ones that lack _compiled_v2)
            self._precompile_rules()

            self.logger.info(
                "从数据库加载了 %d 条分类规则 v2 (user_id=%s)", len(valid_rules), user_id
            )
            if valid_rules:
                for i, rule in enumerate(valid_rules[:3]):
                    self.logger.debug(
                        "规则#%d: %s/%s -> '%s...'",
                        i + 1,
                        rule["main"],
                        rule["sub"],
                        rule["keywords"][:50],
                    )

        except Exception as exc:  # pylint: disable=broad-exception-caught
            self.logger.error("从数据库加载分类规则失败: %s", exc, exc_info=True)
            self.rules = []

    @log_method
    def match_category(
        self, bill: dict[str, Any], types: list[int] | None = None
    ) -> tuple[str | None, str | None]:
        """
        匹配账单分类

        v6.44改进：优先通过关键词匹配所有类型的分类规则，
        如果匹配到投资类分类，会同时更新账单的type字段为'投资'。

        v6.53改进：添加types参数，支持只在指定类型的分类中匹配。

        v6.72改进：基于账单金额正负自动选择对应类型的分类规则。
        - 收入账单（amount > 0）：只使用收入类(2)和投资类(5)分类规则匹配
        - 支出账单（amount < 0）：只使用支出类(3)和投资类(5)分类规则匹配
        - 转账账单（dedup_type='transfer'）：只使用转账类(4)分类规则匹配

        匹配逻辑：
        1. 根据账单金额正负确定适用的分类类型
        2. 在对应类型的分类规则中按关键词匹配
        3. 如果匹配到投资类分类，更新账单type字段为'投资'
        4. 如果没有匹配到，返回None

        Args:
            bill: 账单数据字典，必须包含amount字段
            types: 可选的类型过滤列表，只在指定类型的规则中匹配
                   例如: [TransactionType.INVESTMENT] 只匹配投资类型规则
                   None表示根据账单金额自动选择类型

        Returns:
            Tuple[Optional[str], Optional[str]]: (主分类, 子分类)
        """
        # pylint: disable=too-many-locals,too-many-branches,too-many-statements
        if not self._initialized:
            self.logger.warning("分类引擎未初始化")
            return None, None

        if not bill:
            return None, None

        # 获取账单字段用于匹配
        counterparty = str(bill.get("counterparty", ""))
        description = str(bill.get("description", ""))
        original_category = str(bill.get("original_category", ""))
        combined_text = f"{counterparty} {description} {original_category}"

        # 获取金额和原始类型
        amount = float(bill.get("amount", 0))
        original_type = str(bill.get("type", "")).strip()
        dedup_type = str(bill.get("_dedup_type", "")).lower()
        suppress_investment_rules = bool(
            bill.get("_suppress_investment_signal")
        ) or is_ordinary_bank_interest_income(bill)

        self.logger.debug(
            (
                "[分类匹配] counterparty='%s', description='%s', amount=%.2f, "
                "type='%s', dedup_type='%s'"
            ),
            counterparty[:30],
            description[:30],
            amount,
            original_type,
            dedup_type,
        )

        # v6.75: 智能类型过滤 - 根据账单特征自动选择适用的分类类型
        # 修复：预览表金额使用绝对值（正数），不能用金额符号判断支出/收入
        # 应该优先使用账单的 type 字段来判断类型
        if types:
            type_filter = set(types)
            self.logger.debug("[分类匹配] 调用方指定类型过滤: %s", types)
        else:
            # 根据账单特征自动选择类型
            if dedup_type == "transfer":
                # 转账配对的账单只使用转账类规则
                type_filter = {TransactionType.TRANSFER}
            else:
                # v6.75: 优先使用 type 字段判断，而不是金额符号
                # 因为预览表金额使用绝对值（正数），金额符号判断会失效
                type_str = original_type.lower()
                if type_str in ["支出", "expense", "3"]:
                    # 支出账单：使用支出类和投资类规则
                    type_filter = (
                        {TransactionType.EXPENSE}
                        if suppress_investment_rules
                        else {TransactionType.EXPENSE, TransactionType.INVESTMENT}
                    )
                elif type_str in ["收入", "income", "2"]:
                    # 收入账单：使用收入类和投资类规则
                    type_filter = (
                        {TransactionType.INCOME}
                        if suppress_investment_rules
                        else {TransactionType.INCOME, TransactionType.INVESTMENT}
                    )
                elif type_str in ["转账", "transfer", "4"]:
                    # 转账账单：使用转账类规则
                    type_filter = {TransactionType.TRANSFER}
                elif type_str in ["投资", "investment", "5"]:
                    # 投资账单：使用投资类规则
                    type_filter = {TransactionType.INVESTMENT}
                elif amount < 0:
                    # 回退：type字段无效时，使用金额符号判断（兼容旧数据）
                    type_filter = (
                        {TransactionType.EXPENSE}
                        if suppress_investment_rules
                        else {TransactionType.EXPENSE, TransactionType.INVESTMENT}
                    )
                elif amount > 0:
                    type_filter = (
                        {TransactionType.INCOME}
                        if suppress_investment_rules
                        else {TransactionType.INCOME, TransactionType.INVESTMENT}
                    )
                else:
                    # 金额为0且type无效，使用所有类型
                    type_filter = {
                        TransactionType.INCOME,
                        TransactionType.EXPENSE,
                        TransactionType.TRANSFER,
                        TransactionType.INVESTMENT,
                    }
            self.logger.debug(
                "[分类匹配] 自动类型过滤: %s (type='%s', amount=%.2f)",
                type_filter,
                original_type,
                amount,
            )

        # ===== 第一步：在对应类型的分类规则中按关键词匹配（分类优先级排序）=====
        # v6.72: 始终根据类型过滤规则，确保收入账单只匹配收入类规则，支出账单只匹配支出类规则
        rules_to_match = [r for r in self.rules if r.get("type") in type_filter]
        self.logger.debug("[分类匹配] 过滤后规则数: %d/%d", len(rules_to_match), len(self.rules))

        sorted_rules = [
            rule
            for _, rule in sorted(
                enumerate(rules_to_match),
                key=lambda item: self._rule_match_sort_key(
                    item[1],
                    fallback_order=item[0],
                ),
            )
        ]

        for rule in sorted_rules:
            keywords = rule.get("keywords")
            if not keywords:
                continue

            # v6.73: 使用预编译规则进行快速匹配
            compiled_v2 = rule.get("_compiled_v2")
            if compiled_v2 is not None:
                matched = self.keyword_matcher.match_compiled(combined_text, compiled_v2)
            else:
                matched = self._match_keywords_fast(combined_text, keywords)

            if matched:
                rule_type = rule.get("type")
                main_cat = rule["main"]
                sub_cat = rule["sub"]

                # v6.63: 单条匹配日志改为 DEBUG，减少批量导入时的冗余输出
                self.logger.debug(
                    "[分类匹配] '%s' -> %s/%s (type=%s)",
                    combined_text[:30],
                    main_cat,
                    sub_cat,
                    rule_type,
                )

                # v6.52: 修复分类匹配逻辑
                # 只有 dedup_type='transfer' 的账单才能匹配转账类型分类
                # 转账类型的判断应该在去重阶段通过转账配对机制完成
                # 关键词匹配不应该改变账单的type为转账

                # 获取账单的去重类型
                dedup_type = str(bill.get("_dedup_type", "")).lower()

                # 如果匹配到投资类分类，更新账单type为'投资'
                if rule_type == TransactionType.INVESTMENT:
                    bill["type"] = "投资"
                # 如果匹配到转账类分类，只有 dedup_type='transfer' 时才更新type
                elif rule_type == TransactionType.TRANSFER:
                    if dedup_type == "transfer":
                        bill["type"] = "转账"
                    else:
                        # 不更新type，跳过此规则继续查找其他规则
                        self.logger.debug("[分类匹配] 跳过转账规则(dedup_type='%s')", dedup_type)
                        continue

                return main_cat, sub_cat

        # ===== 第二步：如果没有匹配到任何规则，根据金额正负确定默认类型搜索 =====
        # 这部分保持原有逻辑，用于没有关键词匹配的账单
        self.logger.debug("[分类匹配] 关键词未匹配，使用默认类型逻辑")

        # 根据金额正负和原始类型确定要匹配的分类类型
        if original_type in ["转账", "转出", "转入"]:
            target_types = [TransactionType.TRANSFER]
        elif original_type in ["投资", "投资理财", "理财"]:
            target_types = [TransactionType.INVESTMENT]
        elif amount > 0:
            target_types = [TransactionType.INCOME]
        elif amount < 0:
            target_types = [TransactionType.EXPENSE]
        else:
            target_types = [
                TransactionType.EXPENSE,
                TransactionType.INCOME,
                TransactionType.TRANSFER,
                TransactionType.INVESTMENT,
            ]

        # 过滤目标类型的规则（此时可能有无关键词的默认分类）
        filtered_rules = [rule for rule in self.rules if rule.get("type") in target_types]

        self.logger.debug(
            "[分类匹配] 默认类型过滤: target_types=%s, 规则数=%d/%d",
            target_types,
            len(filtered_rules),
            len(self.rules),
        )

        return None, None

    @log_method
    async def batch_match_categories(
        self, bills: list[dict[str, Any]], types: list[int] | None = None
    ) -> list[dict[str, Any]]:
        """
        批量匹配账单分类

        Args:
            bills: 账单列表
            types: 可选的类型过滤列表，只在指定类型的规则中匹配
                   例如: [TransactionType.INVESTMENT] 只匹配投资类型规则
                   None表示在所有类型中匹配

        Returns:
            List[Dict]: 添加了分类信息的账单列表
        """
        if not self._initialized:
            self.logger.warning("分类引擎未初始化,请先调用 load_rules()")
            return bills

        types_str = str(types) if types else "all"
        self.logger.info("开始批量分类 %d 条账单 (types=%s)", len(bills), types_str)

        categorized_bills = []
        matched_count = 0

        for bill in bills:
            # v6.53: 传递types参数给match_category
            main_cat, sub_cat = self.match_category(bill, types=types)

            # 添加分类信息
            categorized_bill = bill.copy()
            categorized_bill["main_category"] = main_cat
            categorized_bill["sub_category"] = sub_cat

            categorized_bills.append(categorized_bill)

            if main_cat:
                matched_count += 1

        match_rate = (matched_count / len(bills) * 100) if bills else 0
        self.logger.info(
            "批量分类完成: 成功匹配 %d/%d 条 (%.1f%%)",
            matched_count,
            len(bills),
            match_rate,
        )

        return categorized_bills

    @log_method
    def get_categories_tree(self) -> dict[str, dict[str, list[str]]]:
        """
        获取分类树结构(用于UI显示)
        按支出/收入/转账/投资分组

        Returns:
            Dict[str, Dict[str, List[str]]]:
            {
                "支出": {主分类: [子分类列表]},
                "收入": {主分类: [子分类列表]},
                "转账": {主分类: [子分类列表]},
                "投资": {主分类: [子分类列表]}
            }
        """
        tree = {"支出": {}, "收入": {}, "转账": {}, "投资": {}}

        for rule in self.rules:
            main = rule.get("main")
            sub = rule.get("sub")

            if not main or not sub:
                continue

            # 根据主分类名称判断类型
            if main == "收入":
                bill_type = "收入"
            elif main == "转账":
                bill_type = "转账"
            elif main == "投资":
                bill_type = "投资"
            else:
                bill_type = "支出"

            if main not in tree[bill_type]:
                tree[bill_type][main] = []

            if sub not in tree[bill_type][main]:
                tree[bill_type][main].append(sub)

        self.logger.debug(
            "生成分类树: 支出 %d 个, 收入 %d 个, 转账 %d 个, 投资 %d 个",
            len(tree["支出"]),
            len(tree["收入"]),
            len(tree["转账"]),
            len(tree["投资"]),
        )
        return tree


__all__ = ["CategoryEngine"]
