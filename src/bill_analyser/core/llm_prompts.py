"""Prompt templates for LLM-driven classification and rule induction."""

from __future__ import annotations

import json
from typing import Any

SYSTEM_PROMPT = (
    "你是 Bill Analyser 的智能分类助手。"
    "Bill Analyser 是一个个人/家庭账单管理系统，支持收入、支出、转账三种交易类型。\n"
    "每笔交易包含：日期、金额（单位：元）、交易对方、描述、支付方式、主分类、子分类。\n"
    "你的任务是根据交易信息推断最合适的分类，或根据已分类样本归纳关键词匹配规则。\n"
    "请始终以 JSON 格式返回结果，不要包含额外的解释文字。"
)


def build_classification_prompt(transactions: list[dict[str, Any]]) -> str:
    """构建分类建议 prompt，要求 LLM 对未分类交易推荐分类。"""
    txn_lines: list[str] = []
    for i, txn in enumerate(transactions, 1):
        txn_lines.append(
            f"  {i}. id={txn.get('id')}, "
            f"date={txn.get('date', '')}, "
            f"amount={txn.get('amount', 0)}元, "
            f"counterparty=\"{txn.get('counterparty', '')}\", "
            f"description=\"{txn.get('description', '')}\", "
            f"payment_method=\"{txn.get('payment_method', '')}\""
        )

    transactions_block = "\n".join(txn_lines)

    return f"""以下是一批未分类的交易记录，请为每笔交易推荐最合适的主分类和子分类。

交易列表：
{transactions_block}

请以如下 JSON 格式返回（数组，每个元素对应一笔交易）：
[
  {{
    "bill_id": <交易ID>,
    "suggested_main_category": "<推荐主分类>",
    "suggested_sub_category": "<推荐子分类>",
    "confidence": <0.0-1.0之间的置信度>
  }}
]

分类应尽可能贴合中文个人财务常见分类体系（如：餐饮美食、交通出行、日用百货、住房物业、医疗健康、教育培训、休闲娱乐、人情往来、工资薪酬等）。
只返回 JSON，不要有其他文字。"""


def render_prompt_template(
    template: str,
    *,
    default_prompt: str,
    transactions: list[dict[str, Any]],
    category_name: str = "",
) -> str:
    """Render an optional user prompt template with safe token replacement."""
    if not template.strip():
        return default_prompt

    transactions_json = json.dumps(transactions, ensure_ascii=False, default=str)
    transactions_text = "\n".join(
        json.dumps(item, ensure_ascii=False, default=str) for item in transactions
    )

    return (
        template
        .replace("{default_prompt}", default_prompt)
        .replace("{transactions_json}", transactions_json)
        .replace("{transactions_text}", transactions_text)
        .replace("{category_name}", category_name)
    )


def build_rule_induction_prompt(
    category_name: str,
    transactions: list[dict[str, Any]],
) -> str:
    """构建规则归纳 prompt，根据已分类样本推断关键词规则。"""
    txn_lines: list[str] = []
    for i, txn in enumerate(transactions, 1):
        txn_lines.append(
            f"  {i}. counterparty=\"{txn.get('counterparty', '')}\", "
            f"description=\"{txn.get('description', '')}\", "
            f"payment_method=\"{txn.get('payment_method', '')}\""
        )

    samples_block = "\n".join(txn_lines)

    return f"""以下是已被归类为「{category_name}」的交易样本，请分析它们的共同模式，归纳出一组关键词匹配规则。

样本列表：
{samples_block}

规则表达式语法说明：
- OR={{关键词1,关键词2}} 表示匹配任一关键词
- AND={{关键词1,关键词2}} 表示必须同时包含所有关键词
- NOT={{关键词1}} 表示排除包含这些关键词的交易
- 多个条件用 + 连接，如：OR={{美团,饿了么}}+NOT={{退款}}

请以如下 JSON 格式返回（可返回多条规则建议）：
[
  {{
    "rule_name": "<规则名称>",
    "rule_expression": "<规则表达式>",
    "confidence": <0.0-1.0之间的置信度>,
    "explanation": "<简短说明为什么归纳出这条规则>"
  }}
]

只返回 JSON，不要有其他文字。"""
