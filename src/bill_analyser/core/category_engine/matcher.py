"""Keyword and rule-expression matcher implementation."""

import re

from ...utils.logger import get_logger
from .compiled_rule import CompiledRule
from .expression import (
    _RULE_EXPRESSION_AND_CONNECTORS,
    _RULE_EXPRESSION_ESCAPABLE_CHARS,
    _RULE_EXPRESSION_FACTOR_TERMINATORS,
    _RULE_EXPRESSION_NOT_CONNECTORS,
    _RULE_EXPRESSION_OR_CONNECTORS,
    RuleExpressionNode,
    _split_rule_expression_terms,
)


class KeywordMatcher:
    """复杂关键词匹配器

    支持格式:
    - 简单: "滴滴出行"
    - OR: "OR:滴滴|快的|优步"
    - 正则: "REGEX:^滴滴.*出行$"
    - 组合: "OR:滴滴|快的&NOT:退款|取消&AND:打车"
    - v6.67 多OR块: "OR:地铁|公交&OR:一卡通|交通卡" (两个OR块都必须匹配)

    v6.73性能优化:
    - compile_rule(): 预编译规则，返回CompiledRule对象
    - match_compiled(): 使用预编译规则快速匹配
    - 规则缓存: _compiled_rules_cache 避免重复编译
    """

    def __init__(self):
        self.logger = get_logger("KeywordMatcher")
        # 缓存编译后的正则表达式
        self._regex_cache: dict[str, re.Pattern] = {}
        # v6.73: 缓存编译后的规则
        self._compiled_rules_cache: dict[str, CompiledRule] = {}

    def clear_cache(self):
        """清空所有缓存

        v6.74新增：当分类关键词被修改后，需要调用此方法清空缓存，
        确保后续匹配使用最新的规则。

        清空的缓存包括：
        - _compiled_rules_cache: 预编译的规则缓存
        - _regex_cache: 正则表达式编译缓存
        """
        cache_count = len(self._compiled_rules_cache)
        regex_count = len(self._regex_cache)
        self._compiled_rules_cache.clear()
        self._regex_cache.clear()
        self.logger.info(
            "[KeywordMatcher] 缓存已清空: 规则缓存=%d条, 正则缓存=%d条",
            cache_count,
            regex_count,
        )

    def compile_rule(self, rule: str) -> CompiledRule:
        """预编译关键词规则

        v6.73新增：将规则字符串解析为CompiledRule对象，避免每次匹配时重复解析。
        结果会被缓存，相同规则只解析一次。

        参数：
            rule: 关键词规则字符串

        返回：
            CompiledRule: 预编译的规则对象
        """
        # pylint: disable=too-many-branches,too-many-locals
        if not rule:
            return CompiledRule(is_empty=True)

        # 检查缓存
        if rule in self._compiled_rules_cache:
            return self._compiled_rules_cache[rule]

        # 解析规则
        or_blocks: list[list[str]] = []
        not_patterns: list[str] = []
        and_patterns: list[str] = []
        simple_patterns: list[str] = []

        parts = rule.split("&")

        for part in parts:
            part = part.strip()

            if part.upper().startswith("OR:"):
                # OR逻辑: 每个OR块独立保存
                keywords = part[3:].split("|")
                block = [k.strip().lower() for k in keywords if k.strip()]
                if block:
                    or_blocks.append(block)

            elif part.upper().startswith("NOT:"):
                # NOT逻辑: 不能包含
                keywords = part[4:].split("|")
                not_patterns.extend([k.strip().lower() for k in keywords if k.strip()])

            elif part.upper().startswith("AND:"):
                # AND逻辑: 必须全部包含
                keywords = part[4:].split("|")
                and_patterns.extend([k.strip().lower() for k in keywords if k.strip()])

            elif part.upper().startswith("REGEX:"):
                # 正则表达式: 作为独立OR块处理
                regex_pattern = part[6:].strip()
                if regex_pattern:
                    or_blocks.append([f"regex:{regex_pattern}"])
                    # 预编译正则表达式
                    try:
                        if regex_pattern not in self._regex_cache:
                            self._regex_cache[regex_pattern] = re.compile(
                                regex_pattern,
                                re.IGNORECASE,
                            )
                    except re.error as exc:
                        self.logger.warning("无效的正则表达式 '%s': %s", regex_pattern, exc)
            else:
                # 简单匹配
                if part:
                    simple_patterns.append(part.lower())

        # 简单关键词组成一个OR块
        if simple_patterns:
            or_blocks.append(simple_patterns)

        compiled = CompiledRule(
            or_blocks=or_blocks,
            not_patterns=not_patterns,
            and_patterns=and_patterns,
            is_empty=False,
        )

        # 缓存结果
        self._compiled_rules_cache[rule] = compiled

        return compiled

    def match_compiled(self, text: str, compiled: CompiledRule) -> bool:
        """使用预编译规则匹配文本

        v6.73新增：使用预编译的CompiledRule对象进行快速匹配，
        避免每次匹配时重复解析规则字符串。

        参数：
            text: 要匹配的文本
            compiled: 预编译的规则对象

        返回：
            bool: 是否匹配
        """
        # pylint: disable=too-many-return-statements
        if compiled.is_empty or not text:
            return False

        text_lower = text.lower()

        if compiled.expression_ast is not None:
            return self._match_expression_node(text_lower, compiled.expression_ast)

        # 1. 检查NOT条件(排除) - 任一匹配则失败
        for pattern in compiled.not_patterns:
            if self._match_pattern(text_lower, pattern):
                return False

        # 2. 检查AND条件(必须全部包含)
        for pattern in compiled.and_patterns:
            if not self._match_pattern(text_lower, pattern):
                return False

        # 3. 检查每个OR块（所有OR块都必须匹配）
        if compiled.or_blocks:
            for block in compiled.or_blocks:
                block_matched = False
                for pattern in block:
                    if self._match_pattern(text_lower, pattern):
                        block_matched = True
                        break
                if not block_matched:
                    return False
            return True

        # 如果只有AND/NOT条件，且都通过了，则匹配成功
        if compiled.and_patterns or compiled.not_patterns:
            return True

        return False

    def _match_expression_node(self, text_lower: str, node: RuleExpressionNode) -> bool:
        """Evaluate a compiled rule-expression AST against normalized text."""
        if node.kind == "all":
            result = bool(node.children) and all(
                self._match_expression_node(text_lower, child) for child in node.children
            )
        elif node.kind == "any":
            result = any(
                self._match_expression_node(text_lower, child) for child in node.children
            )
        elif node.kind == "not":
            result = len(node.children) == 1 and not self._match_expression_node(
                text_lower,
                node.children[0],
            )
        elif node.kind != "clause":
            result = False
        elif node.operator == "OR":
            result = any(self._match_pattern(text_lower, pattern) for pattern in node.patterns)
        elif node.operator == "AND":
            result = bool(node.patterns) and all(
                self._match_pattern(text_lower, pattern) for pattern in node.patterns
            )
        elif node.operator == "NOT":
            result = not any(
                self._match_pattern(text_lower, pattern) for pattern in node.patterns
            )
        else:
            result = False
        return result

    def _match_pattern(self, text_lower: str, pattern: str) -> bool:
        """匹配单个模式（支持正则表达式）

        Args:
            text_lower: 小写文本
            pattern: 匹配模式（可能是普通字符串或正则表达式）

        Returns:
            bool: 是否匹配
        """
        if pattern.startswith("regex:"):
            # 正则表达式匹配
            regex_pattern = pattern[6:]  # 移除 'regex:' 前缀
            try:
                if regex_pattern not in self._regex_cache:
                    self._regex_cache[regex_pattern] = re.compile(regex_pattern, re.IGNORECASE)
                return bool(self._regex_cache[regex_pattern].search(text_lower))
            except re.error as exc:
                self.logger.warning("无效的正则表达式 '%s': %s", regex_pattern, exc)
                return False
        else:
            # 普通子串匹配
            return pattern in text_lower

    def compile_rule_expression(self, expr: str, regex_enabled: bool = False) -> CompiledRule:
        """Compile a category rule expression into a ``CompiledRule``.

        Composite grammar:
        ``expression := and_expr (('|' | '/') and_expr)*``
        ``and_expr := factor (('+' factor) | (('×' | 'NOT') factor))*``
        ``factor := clause | '(' expression ')'``
        ``clause := OR={terms} | AND={terms} | NOT={terms} | REGEX={patterns}``
        ``term`` may escape delimiters with ``\\`` (for example ``\\,``).

        ``+`` keeps the existing no-parentheses syntax as a conjunction:
        ``OR={k1,k2}+AND={k3}+NOT={k4}`` means
        ``(k1 or k2) and k3 and not k4``. ``×`` and bare ``NOT`` are accepted
        as visible block-level NOT connectors (for example ``×OR={k4}`` or
        ``NOT OR={k4}`` means AND NOT the next block), ``/`` is accepted as the
        visible OR connector inside an expression, and ``|`` is the canonical
        separator between top-level expression groups. Both OR connectors parse
        to logical OR; parentheses preserve the intended grouping. Old
        ``OR:k1|k2&AND:k3&NOT:k4`` strings still fall back to ``compile_rule``.
        """
        if not expr:
            return CompiledRule(is_empty=True)

        # Backward compatible: if expression doesn't contain ={}, use old parser
        if "={" not in expr:
            return self.compile_rule(expr)

        cache_key = f"v2:{expr}:{regex_enabled}"
        if cache_key in self._compiled_rules_cache:
            return self._compiled_rules_cache[cache_key]

        try:
            expression_ast, index = self._parse_rule_or_expression(expr, 0, regex_enabled)
            index = self._skip_expression_space(expr, index)
            if index != len(expr):
                raise ValueError(f"unexpected token at offset {index}: {expr[index:index + 10]!r}")
        except ValueError as exc:
            self.logger.warning("无效的分类规则表达式 '%s': %s", expr, exc)
            compiled = CompiledRule(is_empty=True)
            self._compiled_rules_cache[cache_key] = compiled
            return compiled

        or_blocks: list[list[str]] = []
        not_patterns: list[str] = []
        and_patterns: list[str] = []
        self._collect_legacy_compiled_fields(
            expression_ast,
            or_blocks,
            and_patterns,
            not_patterns,
        )

        compiled = CompiledRule(
            or_blocks=or_blocks,
            not_patterns=not_patterns,
            and_patterns=and_patterns,
            is_empty=expression_ast is None,
            expression_ast=expression_ast,
        )
        self._compiled_rules_cache[cache_key] = compiled
        return compiled

    def _parse_rule_or_expression(
        self,
        expr: str,
        index: int,
        regex_enabled: bool,
    ) -> tuple[RuleExpressionNode | None, int]:
        """Parse expression-level OR chains, stopping at a closing parenthesis."""
        children: list[RuleExpressionNode] = []
        left, index = self._parse_rule_and_expression(expr, index, regex_enabled)
        if left is not None:
            children.append(left)

        while True:
            index = self._skip_expression_space(expr, index)
            if index >= len(expr) or expr[index] not in _RULE_EXPRESSION_OR_CONNECTORS:
                break
            index += 1
            right, index = self._parse_rule_and_expression(expr, index, regex_enabled)
            if right is not None:
                children.append(right)

        if not children:
            return None, index
        if len(children) == 1:
            return children[0], index
        return RuleExpressionNode(kind="any", children=tuple(children)), index

    def _parse_rule_and_expression(
        self,
        expr: str,
        index: int,
        regex_enabled: bool,
    ) -> tuple[RuleExpressionNode | None, int]:
        """Parse ``+``-joined chains and visible block-level negation."""
        children: list[RuleExpressionNode] = []
        pending_negated_connector = False

        while True:
            index = self._skip_expression_space(expr, index)
            if (
                index >= len(expr)
                or expr[index] == ")"
                or expr[index] in _RULE_EXPRESSION_OR_CONNECTORS
            ):
                break

            connector = self._read_rule_and_connector(
                expr,
                index,
                allow_word_not=bool(children),
            )
            if connector is not None:
                pending_negated_connector, index = connector
                continue

            child, index = self._parse_rule_factor(expr, index, regex_enabled)
            if child is not None:
                if pending_negated_connector:
                    child = self._apply_not_connector(child)
                    pending_negated_connector = False
                children.append(child)

            index = self._skip_expression_space(expr, index)
            if (
                index >= len(expr)
                or expr[index] == ")"
                or expr[index] in _RULE_EXPRESSION_OR_CONNECTORS
            ):
                break
            connector = self._read_rule_and_connector(expr, index)
            if connector is not None:
                pending_negated_connector, index = connector
                continue
            raise ValueError(f"expected AND/OR connector at offset {index}")

        if not children:
            return None, index
        if len(children) == 1:
            return children[0], index
        return RuleExpressionNode(kind="all", children=tuple(children)), index

    @staticmethod
    def _apply_not_connector(child: RuleExpressionNode) -> RuleExpressionNode:
        """Apply a visible NOT connector without mutating clause operators.

        Older UI states could serialize the same intent as ``×NOT={term}``;
        keep that accepted spelling as a single NOT clause instead of turning
        it into a double-negative.
        """
        if child.kind == "clause" and child.operator == "NOT":
            return child
        return RuleExpressionNode(kind="not", children=(child,))

    def _parse_rule_factor(
        self,
        expr: str,
        index: int,
        regex_enabled: bool,
    ) -> tuple[RuleExpressionNode | None, int]:
        """Parse a parenthesized expression or one rule clause."""
        index = self._skip_expression_space(expr, index)
        if index >= len(expr):
            return None, index

        if expr[index] == "(":
            node, index = self._parse_rule_or_expression(expr, index + 1, regex_enabled)
            index = self._skip_expression_space(expr, index)
            if index >= len(expr) or expr[index] != ")":
                raise ValueError("missing closing ')' in rule expression")
            return node, index + 1

        start = index
        brace_depth = 0
        while index < len(expr):
            char = expr[index]
            if (
                char == "\\"
                and brace_depth
                and index + 1 < len(expr)
                and expr[index + 1] in _RULE_EXPRESSION_ESCAPABLE_CHARS
            ):
                index += 2
                continue
            if char == "{":
                brace_depth += 1
            elif char == "}" and brace_depth:
                brace_depth -= 1
            elif brace_depth == 0:
                if char in _RULE_EXPRESSION_FACTOR_TERMINATORS:
                    break
                if index > start and self._read_rule_not_connector(expr, index) is not None:
                    break
            index += 1

        block = expr[start:index].strip()
        if not block:
            return None, index
        return self._parse_rule_clause(block, regex_enabled), index

    def _parse_rule_clause(
        self,
        block: str,
        regex_enabled: bool,
    ) -> RuleExpressionNode:
        """Parse a single ``OR={...}``/``AND={...}``/``NOT={...}`` clause."""
        eq_idx = block.find("={")
        if eq_idx == -1:
            pattern = f"regex:{block}" if regex_enabled else block.lower()
            if regex_enabled:
                self._precompile_regex(block)
            return RuleExpressionNode(kind="clause", operator="OR", patterns=(pattern,))

        prefix = block[:eq_idx].strip().upper()
        content = block[eq_idx + 2 :].strip()
        if not content.endswith("}"):
            raise ValueError(f"missing closing '}}' in clause {block!r}")
        content = content[:-1]

        keywords = _split_rule_expression_terms(content)
        operator = prefix if prefix in {"OR", "AND", "NOT"} else "OR"
        force_regex = prefix == "REGEX"

        patterns: list[str] = []
        for keyword in keywords:
            if regex_enabled or force_regex:
                patterns.append(f"regex:{keyword}")
                self._precompile_regex(keyword)
            else:
                patterns.append(keyword.lower())

        return RuleExpressionNode(
            kind="clause",
            operator=operator,
            patterns=tuple(patterns),
        )

    @staticmethod
    def _skip_expression_space(expr: str, index: int) -> int:
        """Skip whitespace while parsing a rule expression."""
        while index < len(expr) and expr[index].isspace():
            index += 1
        return index

    def _read_rule_and_connector(
        self,
        expr: str,
        index: int,
        allow_word_not: bool = True,
    ) -> tuple[bool, int] | None:
        """Read an AND or visible NOT connector from an AND-expression."""
        if expr[index] in _RULE_EXPRESSION_AND_CONNECTORS:
            return False, index + 1
        if expr[index] in _RULE_EXPRESSION_NOT_CONNECTORS:
            return True, index + 1
        if allow_word_not:
            not_connector_index = self._read_rule_not_connector(expr, index)
            if not_connector_index is not None:
                return True, not_connector_index
        return None

    @staticmethod
    def _read_rule_not_connector(expr: str, index: int) -> int | None:
        """Return the offset after a bare ``NOT`` connector, if present."""
        if expr[index : index + 3].upper() != "NOT":
            return None

        before = expr[index - 1] if index > 0 else ""
        after = expr[index + 3] if index + 3 < len(expr) else ""
        if before and (before.isalnum() or before == "_"):
            return None
        if after == "=" or (after and (after.isalnum() or after == "_")):
            return None

        next_index = index + 3
        while next_index < len(expr) and expr[next_index].isspace():
            next_index += 1
        return next_index

    def _collect_legacy_compiled_fields(
        self,
        node: RuleExpressionNode | None,
        or_blocks: list[list[str]],
        and_patterns: list[str],
        not_patterns: list[str],
    ) -> None:
        """Populate legacy CompiledRule fields for diagnostics/back-compat tests."""
        if node is None:
            return
        if node.kind in {"all", "any"}:
            for child in node.children:
                self._collect_legacy_compiled_fields(
                    child,
                    or_blocks,
                    and_patterns,
                    not_patterns,
                )
            return
        if node.kind == "not":
            return
        if node.kind != "clause":
            return

        patterns = list(node.patterns)
        if node.operator == "OR":
            if patterns:
                or_blocks.append(patterns)
        elif node.operator == "AND":
            and_patterns.extend(patterns)
        elif node.operator == "NOT":
            not_patterns.extend(patterns)

    def _precompile_regex(self, pattern: str):
        """Pre-compile a regex pattern into the cache."""
        if pattern not in self._regex_cache:
            try:
                self._regex_cache[pattern] = re.compile(pattern, re.IGNORECASE)
            except re.error as exc:
                self.logger.warning("无效的正则表达式 '%s': %s", pattern, exc)

    def parse_and_match(self, text: str, rule: str) -> bool:
        """
        解析并匹配复杂规则

        v6.73优化：内部使用预编译规则进行匹配，结果会被缓存。

        v6.67 重构：多个OR块之间是AND关系，每个OR块独立评估
        例如: "OR:地铁|公交&OR:一卡通|交通卡&NOT:退款"
        - 必须包含 (地铁 OR 公交) 中的至少一个
        - 并且必须包含 (一卡通 OR 交通卡) 中的至少一个
        - 并且不能包含 退款

        Args:
            text: 要匹配的文本
            rule: 匹配规则

        Returns:
            bool: 是否匹配
        """
        # v6.73: 使用预编译规则进行匹配
        compiled = self.compile_rule(rule)
        return self.match_compiled(text, compiled)

    def extract_positive_keywords(self, rule: str) -> list[str]:
        """从规则中提取正向关键词（OR和AND，不包含NOT）

        用于配对识别时提取可用于匹配的关键词列表。
        v6.67: 支持REGEX前缀

        Args:
            rule: 关键词规则字符串

        Returns:
            List[str]: 关键词列表（小写）
        """
        if not rule:
            return []

        keywords = []
        parts = rule.split("&")

        for part in parts:
            part = part.strip()

            if part.upper().startswith("OR:"):
                # OR逻辑: 任意一个
                kws = part[3:].split("|")
                keywords.extend([k.strip().lower() for k in kws if k.strip()])

            elif part.upper().startswith("AND:"):
                # AND逻辑: 必须全部包含
                kws = part[4:].split("|")
                keywords.extend([k.strip().lower() for k in kws if k.strip()])

            elif part.upper().startswith("NOT:"):
                # NOT逻辑: 跳过（排除词不用于正向匹配）
                pass

            elif part.upper().startswith("REGEX:"):
                # REGEX逻辑: 以 'regex:' 前缀保留
                regex_pattern = part[6:].strip()
                if regex_pattern:
                    keywords.append(f"regex:{regex_pattern.lower()}")

            else:
                # 简单关键词
                if part:
                    keywords.append(part.lower())

        return keywords


__all__ = ["CompiledRule", "KeywordMatcher"]
