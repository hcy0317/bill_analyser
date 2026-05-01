"""Compiled category-rule data structures."""

from dataclasses import dataclass, field

from .expression import RuleExpressionNode


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


__all__ = ["CompiledRule"]
