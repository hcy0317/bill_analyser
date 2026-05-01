"""Public facade for the category engine package.

Keeps the historic ``bill_analyser.core.category_engine`` import path stable
while the implementation lives in functional shards.
"""

from .engine import CategoryEngine
from .expression import RuleExpressionNode, escape_rule_expression_term
from .compiled_rule import CompiledRule
from .matcher import KeywordMatcher


_category_engine_v2 = CategoryEngine()


async def get_category_engine(
    db=None,
    user_id: int = 1,
    types: list[int] | None = None,
) -> CategoryEngine:
    """Get the process-wide category engine instance.

    Args:
        db: 数据库实例
        user_id: 用户ID，用于加载用户特定的分类规则
        types: 可选的类型过滤列表，只加载指定类型的规则

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
        await _category_engine_v2.load_rules_from_db(db, user_id=user_id, types=types)
    return _category_engine_v2


__all__ = [
    "CategoryEngine",
    "CompiledRule",
    "KeywordMatcher",
    "RuleExpressionNode",
    "escape_rule_expression_term",
    "get_category_engine",
]
