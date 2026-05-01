"""Rule-expression AST and escaping helpers for category matching."""

from dataclasses import dataclass

_RULE_EXPRESSION_ESCAPABLE_CHARS = frozenset("\\,+{}|()/×")
_RULE_EXPRESSION_AND_CONNECTORS = frozenset({"+"})
_RULE_EXPRESSION_NOT_CONNECTORS = frozenset({"×"})
_RULE_EXPRESSION_OR_CONNECTORS = frozenset({"|", "/"})
_RULE_EXPRESSION_FACTOR_TERMINATORS = (
    _RULE_EXPRESSION_AND_CONNECTORS
    | _RULE_EXPRESSION_NOT_CONNECTORS
    | _RULE_EXPRESSION_OR_CONNECTORS
    | frozenset({")"})
)


def escape_rule_expression_term(term: str) -> str:
    """Escape one literal term for ``OR={...}``-style rule expressions.

    Terms are comma-separated inside ``{...}``, while braces participate in
    clause scanning and ``+``/``×``/``|``/``/``/parentheses are expression
    delimiters outside a clause. Escaping all expression delimiters keeps
    migrated legacy keywords round-trippable even when a literal keyword
    contains those chars.
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
    ``expression := and_expr (('|' | '/') and_expr)*``
    ``and_expr := factor (('+' factor) | (('×' | 'NOT') factor))*``
    ``factor := clause | '(' expression ')'``
    ``clause := OR={terms} | AND={terms} | NOT={terms} | REGEX={patterns}``
    ``term`` may escape delimiters with ``\\`` (for example ``\\,`` or ``\\{``).

    ``OR={a,b}`` means any term matches; ``AND={a,b}`` means all terms match;
    ``NOT={a,b}`` means no term may match. ``+`` combines clauses/groups with
    logical AND, ``×`` and bare ``NOT`` are visible block-level NOT connectors
    (AND NOT the following clause/group), ``/`` is the visible OR connector,
    and ``|`` is the canonical separator between top-level expression groups.
    Both OR connectors parse to logical OR; parentheses preserve the intended
    grouping. Legacy ``OR:a|b&AND:c&NOT:d`` is still compiled by ``compile_rule``.
    """

    kind: str
    operator: str = ""
    patterns: tuple[str, ...] = ()
    children: tuple["RuleExpressionNode", ...] = ()


__all__ = [
    "RuleExpressionNode",
    "escape_rule_expression_term",
]
