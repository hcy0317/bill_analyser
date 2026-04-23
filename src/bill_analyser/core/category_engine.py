"""
分类引擎 V2 模块

支持复杂关键词匹配: OR:|&NOT:|&AND:
支持按金额正负匹配对应类型分类（正=收入, 负=支出）
v6.53: 添加类型过滤支持，允许选择性加载和匹配特定类型的分类规则
v6.67: 修复多OR块匹配逻辑 - 每个OR块独立评估，所有OR块必须都匹配（AND关系）
       添加正则表达式支持（REGEX:前缀）
v6.73: 性能优化 - 规则预编译机制，避免每次匹配都重新解析规则字符串
       - 添加CompiledRule数据类存储预编译的规则
       - 添加compile_rule方法一次性解析规则
       - 批量匹配时使用预编译规则，性能提升10-50倍
v6.74: 缓存失效机制 - 确保修改分类关键词后能使用最新规则
       - KeywordMatcher.clear_cache(): 清空规则缓存和正则缓存
       - CategoryEngine.invalidate_cache(): 公开方法，供外部调用
       - _precompile_rules(): 在预编译前先清空缓存
作者：Bill Analyser Team
更新时间：2025-12-06
"""

import re
from dataclasses import dataclass, field
from typing import Any

# pylint: disable=too-many-lines

from ..utils.constants import TransactionType
from ..utils.logger import get_logger, log_method, log_step


_RULE_EXPRESSION_ESCAPABLE_CHARS = frozenset("\\,+{}|()")


def escape_rule_expression_term(term: str) -> str:
    """Escape one literal term for ``OR={...}``-style rule expressions.

    Terms are comma-separated inside ``{...}``, while braces participate in
    clause scanning and ``+``/``|``/parentheses are expression delimiters
    outside a clause. Escaping all expression delimiters keeps migrated legacy
    keywords round-trippable even when a literal keyword contains those chars.
    """
    return "".join(
        f"\\{char}" if char in _RULE_EXPRESSION_ESCAPABLE_CHARS else char
        for char in term
    )


def _unescape_rule_expression_term(term: str) -> str:
    """Unescape one term from ``OR={...}`` content."""
    chars: list[str] = []
    index = 0
    while index < len(term):
        char = term[index]
        if (
            char == "\\"
            and index + 1 < len(term)
            and term[index + 1] in _RULE_EXPRESSION_ESCAPABLE_CHARS
        ):
            chars.append(term[index + 1])
            index += 2
            continue
        chars.append(char)
        index += 1
    return "".join(chars)


def _split_rule_expression_terms(content: str) -> list[str]:
    """Split comma-separated rule-expression terms while honoring escapes."""
    terms: list[str] = []
    current: list[str] = []
    index = 0
    while index < len(content):
        char = content[index]
        if (
            char == "\\"
            and index + 1 < len(content)
            and content[index + 1] in _RULE_EXPRESSION_ESCAPABLE_CHARS
        ):
            current.append(char)
            current.append(content[index + 1])
            index += 2
            continue
        if char == ",":
            term = _unescape_rule_expression_term("".join(current).strip())
            if term:
                terms.append(term)
            current = []
            index += 1
            continue
        current.append(char)
        index += 1

    term = _unescape_rule_expression_term("".join(current).strip())
    if term:
        terms.append(term)
    return terms


@dataclass
class RuleExpressionNode:
    """Boolean AST node for category rule expressions.

    Grammar supported by ``compile_rule_expression``:
    ``expression := and_expr ('|' and_expr)*``
    ``and_expr := factor ('+' factor)*``
    ``factor := clause | '(' expression ')'``
    ``clause := OR={terms} | AND={terms} | NOT={terms} | REGEX={patterns}``
    ``term`` may escape delimiters with ``\\`` (for example ``\\,`` or ``\\{``).

    ``OR={a,b}`` means any term matches; ``AND={a,b}`` means all terms match;
    ``NOT={a,b}`` means no term may match. ``+`` combines clauses/groups with
    logical AND, while top-level ``|`` is an optional expression-level OR for
    parenthesized priority. Legacy ``OR:a|b&AND:c&NOT:d`` is still compiled by
    ``compile_rule``.
    """

    kind: str
    operator: str = ""
    patterns: tuple[str, ...] = ()
    children: tuple["RuleExpressionNode", ...] = ()


@dataclass
class CompiledRule:
    """预编译的关键词规则

    将规则字符串解析为结构化数据，避免每次匹配时重复解析。

    属性：
        or_blocks: OR块列表，每个OR块是一个关键词列表，所有OR块必须匹配
        not_patterns: NOT模式列表，任一匹配则失败
        and_patterns: AND模式列表，必须全部匹配
        is_empty: 规则是否为空
    """

    or_blocks: list[list[str]] = field(default_factory=list)
    not_patterns: list[str] = field(default_factory=list)
    and_patterns: list[str] = field(default_factory=list)
    is_empty: bool = True
    expression_ast: RuleExpressionNode | None = None


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
        ``expression := and_expr ('|' and_expr)*``
        ``and_expr := factor ('+' factor)*``
        ``factor := clause | '(' expression ')'``
        ``clause := OR={terms} | AND={terms} | NOT={terms} | REGEX={patterns}``
        ``term`` may escape delimiters with ``\\`` (for example ``\\,``).

        ``+`` keeps the existing no-parentheses syntax as a conjunction:
        ``OR={k1,k2}+AND={k3}+NOT={k4}`` means
        ``(k1 or k2) and k3 and not k4``. Parentheses can group any nested
        expression, and top-level ``|`` is supported as an expression-level OR
        where grouping is needed. Old ``OR:k1|k2&AND:k3&NOT:k4`` strings still
        fall back to ``compile_rule``.
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
            if index >= len(expr) or expr[index] != "|":
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
        """Parse ``+``-joined clause/group chains."""
        children: list[RuleExpressionNode] = []

        while True:
            index = self._skip_expression_space(expr, index)
            if index >= len(expr) or expr[index] in ")|":
                break
            if expr[index] == "+":
                index += 1
                continue

            child, index = self._parse_rule_factor(expr, index, regex_enabled)
            if child is not None:
                children.append(child)

            index = self._skip_expression_space(expr, index)
            if index >= len(expr) or expr[index] in ")|":
                break
            if expr[index] == "+":
                index += 1
                continue
            raise ValueError(f"expected '+' or '|' at offset {index}")

        if not children:
            return None, index
        if len(children) == 1:
            return children[0], index
        return RuleExpressionNode(kind="all", children=tuple(children)), index

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
            elif brace_depth == 0 and char in "+|)":
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
        3. 按 priority 排序并预编译
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
                cr_rows = await db.get_category_rules(
                    user_id=user_id, enabled_only=True
                )
            except Exception:  # pylint: disable=broad-exception-caught
                # Table might not exist yet (pre-migration) or the query failed.
                cr_rows = []

            for row in cr_rows:
                rule_type = row.get("category_type", TransactionType.EXPENSE)
                if types and rule_type not in type_filter:
                    continue

                expr = row.get("rule_expression", "")
                regex_enabled = bool(row.get("regex_enabled", False))

                # Pre-compile using the new v2 compiler
                compiled = self.keyword_matcher.compile_rule_expression(
                    expr, regex_enabled=regex_enabled,
                )

                valid_rules.append(
                    {
                        "main": row.get("main_category", ""),
                        "sub": row.get("sub_category", ""),
                        "priority": row.get("priority", 100),
                        "keywords": expr,
                        "type": rule_type,
                        "_compiled_v2": compiled,
                    }
                )

            # Sort by priority
            valid_rules.sort(key=lambda r: r.get("priority", 999))

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
                    type_filter = {TransactionType.EXPENSE, TransactionType.INVESTMENT}
                elif type_str in ["收入", "income", "2"]:
                    # 收入账单：使用收入类和投资类规则
                    type_filter = {TransactionType.INCOME, TransactionType.INVESTMENT}
                elif type_str in ["转账", "transfer", "4"]:
                    # 转账账单：使用转账类规则
                    type_filter = {TransactionType.TRANSFER}
                elif type_str in ["投资", "investment", "5"]:
                    # 投资账单：使用投资类规则
                    type_filter = {TransactionType.INVESTMENT}
                elif amount < 0:
                    # 回退：type字段无效时，使用金额符号判断（兼容旧数据）
                    type_filter = {TransactionType.EXPENSE, TransactionType.INVESTMENT}
                elif amount > 0:
                    type_filter = {TransactionType.INCOME, TransactionType.INVESTMENT}
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

        # ===== 第一步：在对应类型的分类规则中按关键词匹配（优先级排序）=====
        # v6.72: 始终根据类型过滤规则，确保收入账单只匹配收入类规则，支出账单只匹配支出类规则
        rules_to_match = [r for r in self.rules if r.get("type") in type_filter]
        self.logger.debug("[分类匹配] 过滤后规则数: %d/%d", len(rules_to_match), len(self.rules))

        sorted_rules = sorted(rules_to_match, key=lambda r: r.get("priority", 999))

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


# 全局分类引擎实例
_category_engine_v2 = CategoryEngine()


async def get_category_engine(
    db=None,
    user_id: int = 1,
    types: list[int] | None = None,
) -> CategoryEngine:
    """获取分类引擎实例

    Args:
        db: 数据库实例
        user_id: 用户ID，用于加载用户特定的分类规则
        types: 可选的类型过滤列表，只加载指定类型的规则
               例如: [TransactionType.INVESTMENT] 只加载投资类型规则
               None表示加载所有类型

    Returns:
        CategoryEngine: 分类引擎实例
    """
    if not _category_engine_v2.is_initialized and db:
        await _category_engine_v2.load_rules_from_db(db, user_id=user_id, types=types)
    elif (
        _category_engine_v2.is_initialized
        and _category_engine_v2.current_user_id != user_id
        and db
    ):
        # 如果用户ID变化，重新加载规则
        await _category_engine_v2.load_rules_from_db(db, user_id=user_id, types=types)
    return _category_engine_v2
