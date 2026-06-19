// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::Value;

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_llm_classification_prompt(transactions: &[Value]) -> String {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_llm_classification_prompt",
        "business operation entered"
    );
    let transactions_block = transactions
        .iter()
        .enumerate()
        .map(|(index, txn)| {
            format!(
                "  {}. id={}, date={}, amount={}元, counterparty=\"{}\", description=\"{}\", payment_method=\"{}\"",
                index + 1,
                prompt_value(txn, "id"),
                prompt_value(txn, "date"),
                prompt_value(txn, "amount"),
                prompt_value(txn, "counterparty"),
                prompt_value(txn, "description"),
                prompt_value(txn, "payment_method"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "以下是一批未分类的交易记录，请为每笔交易推荐最合适的主分类和子分类。\n\n交易列表：\n{transactions_block}\n\n请以如下 JSON 格式返回（数组，每个元素对应一笔交易）：\n[\n  {{\n    \"bill_id\": <交易ID>,\n    \"suggested_main_category\": \"<推荐主分类>\",\n    \"suggested_sub_category\": \"<推荐子分类>\",\n    \"confidence\": <0.0-1.0之间的置信度>\n  }}\n]\n\n分类应尽可能贴合中文个人财务常见分类体系（如：餐饮美食、交通出行、日用百货、住房物业、医疗健康、教育培训、休闲娱乐、人情往来、工资薪酬等）。\n只返回 JSON，不要有其他文字。"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_llm_rule_induction_prompt(category_name: &str, transactions: &[Value]) -> String {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_llm_rule_induction_prompt",
        "business operation entered"
    );
    let samples_block = transactions
        .iter()
        .enumerate()
        .map(|(index, txn)| {
            format!(
                "  {}. counterparty=\"{}\", description=\"{}\", payment_method=\"{}\"",
                index + 1,
                prompt_value(txn, "counterparty"),
                prompt_value(txn, "description"),
                prompt_value(txn, "payment_method"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "以下是已被归类为「{category_name}」的交易样本，请分析它们的共同模式，归纳出一组关键词匹配规则。\n\n样本列表：\n{samples_block}\n\n规则表达式语法说明：\n- OR={{关键词1,关键词2}} 表示匹配任一关键词\n- AND={{关键词1,关键词2}} 表示必须同时包含所有关键词\n- NOT={{关键词1}} 表示排除包含这些关键词的交易\n- 多个条件用 + 连接，如：OR={{美团,饿了么}}+NOT={{退款}}\n\n请以如下 JSON 格式返回（可返回多条规则建议）：\n[\n  {{\n    \"rule_name\": \"<规则名称>\",\n    \"rule_expression\": \"<规则表达式>\",\n    \"confidence\": <0.0-1.0之间的置信度>,\n    \"explanation\": \"<简短说明为什么归纳出这条规则>\"\n  }}\n]\n\n只返回 JSON，不要有其他文字。"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_llm_import_preview_recommendation_prompt(
    transactions: &[Value],
    existing_categories: &[String],
    existing_accounts: &[String],
    memory_context: &[Value],
) -> String {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_llm_import_preview_recommendation_prompt",
        "business operation entered"
    );
    let transactions_block = transactions
        .iter()
        .enumerate()
        .map(|(index, txn)| {
            format!(
                "  {}. preview_id={}, date=\"{}\", amount={}元, type={}, counterparty=\"{}\", description=\"{}\", payment_method=\"{}\"",
                index + 1,
                prompt_value(txn, "id"),
                prompt_value(txn, "date"),
                prompt_value(txn, "amount"),
                prompt_value(txn, "type"),
                prompt_value(txn, "counterparty"),
                prompt_value(txn, "description"),
                prompt_value(txn, "payment_method"),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let categories_block = if existing_categories.is_empty() {
        String::new()
    } else {
        format!(
            "\n已有分类体系（优先从中选择）：\n{}\n",
            existing_categories
                .iter()
                .take(50)
                .map(|category| format!("  - {category}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let accounts_block = if existing_accounts.is_empty() {
        String::new()
    } else {
        format!(
            "\n已有账户（若需要给出账户路由，请优先使用这些账户名）：\n{}\n",
            existing_accounts
                .iter()
                .take(50)
                .map(|account| format!("  - {account}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    let memory_lines = memory_context
        .iter()
        .take(20)
        .filter_map(|memory| {
            let decision = prompt_value(memory, "decision");
            let category = prompt_value(memory, "suggested_main_category");
            if decision.is_empty() || category.is_empty() {
                return None;
            }
            Some(format!(
                "  - {decision}: \"{}\" -> {}/{}",
                prompt_value(memory, "description_hint"),
                category,
                prompt_value(memory, "suggested_sub_category"),
            ))
        })
        .collect::<Vec<_>>();
    let memory_block = if memory_lines.is_empty() {
        String::new()
    } else {
        format!(
            "\n历史记忆（你过去的推荐和用户反馈，请从中学习）：\n{}\n",
            memory_lines.join("\n")
        )
    };

    format!(
        "以下是一批待导入的交易记录，请为每笔交易推荐最合适的类型、主分类、子分类和账户路由。\n{categories_block}{accounts_block}{memory_block}\n交易列表：\n{transactions_block}\n\n请以如下 JSON 格式返回（数组，每个元素对应一笔交易）：\n[\n  {{\n    \"preview_id\": <预览行ID>,\n    \"suggested_type\": \"<收入|支出|投资|转账之一；只有交易本身明确是账户间转移时才填转账>\",\n    \"suggested_category_id\": <已有分类 ID，无法从候选分类选择时填 null>,\n    \"suggested_main_category\": \"<推荐主分类>\",\n    \"suggested_sub_category\": \"<推荐子分类>\",\n    \"suggested_source_account\": \"<推荐来源账户，可为空>\",\n    \"suggested_destination_account\": \"<推荐目标账户，可为空>\",\n    \"confidence\": <0.0-1.0之间的置信度>,\n    \"reason\": \"<简短推荐理由>\"\n  }}\n]\n\n如已提供分类候选，应优先按 ID 选择，并让主分类/子分类与该 ID 对应路径一致。如果历史记忆中有相似交易的反馈，优先参考用户的纠正。\n只返回 JSON，不要有其他文字。"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn build_llm_rule_expression_synthesis_prompt(
    knowledge_summary_pack: &Value,
    max_candidates: usize,
) -> String {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "build_llm_rule_expression_synthesis_prompt",
        "business operation entered"
    );
    let summary_json =
        serde_json::to_string_pretty(knowledge_summary_pack).unwrap_or_else(|_| "{}".to_string());
    format!(
        "以下是 Bill Analyser 的长期学习知识摘要（KnowledgeSummaryPack）。\n请基于这些长期学习证据，为规则中心归纳出可人工审核的分类规则候选。\n\n约束：\n- 只能输出“候选规则”，不要假设会自动写入正式规则系统。\n- 候选必须符合现有规则表达式语法：\n  - OR={{关键词1,关键词2}}\n  - AND={{关键词1,关键词2}}\n  - NOT={{关键词1}}\n  - REGEX={{模式1,模式2}}\n  - 可以使用 +、/、|、× 和括号组合\n- 不要输出无效语法、空表达式或与知识摘要明显冲突的规则。\n- 优先覆盖证据稳定、反馈正向、可复用的模式。\n- 推荐分类必须严格来自 knowledge_summary_pack.existing_categories 中已有的分类路径。\n- 如果证据不足，请少提，不要为了凑数量强行生成。\n- 最多输出 {max_candidates} 条候选。\n\nKnowledgeSummaryPack:\n{summary_json}\n\n请以如下 JSON 格式返回：\n[\n  {{\n    \"rule_name\": \"<候选名称>\",\n    \"suggested_main_category\": \"<主分类>\",\n    \"suggested_sub_category\": \"<子分类，可为空>\",\n    \"rule_expression\": \"<规则表达式>\",\n    \"confidence\": <0.0-1.0之间的置信度>,\n    \"reason\": \"<简短说明归纳依据>\"\n  }}\n]\n\n只返回 JSON，不要有其他文字。"
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn render_llm_prompt_template(
    template: &str,
    default_prompt: &str,
    transactions: &[Value],
    category_name: &str,
) -> String {
    if template.trim().is_empty() {
        return default_prompt.to_string();
    }
    let transactions_json =
        serde_json::to_string(transactions).unwrap_or_else(|_| "[]".to_string());
    let transactions_text = transactions
        .iter()
        .map(|item| serde_json::to_string(item).unwrap_or_else(|_| "{}".to_string()))
        .collect::<Vec<_>>()
        .join("\n");
    template
        .replace("{default_prompt}", default_prompt)
        .replace("{transactions_json}", &transactions_json)
        .replace("{transactions_text}", &transactions_text)
        .replace("{category_name}", category_name)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_llm_json_array_response(content: &str) -> Result<Vec<Value>, String> {
    let stripped = strip_json_code_fence(content.trim());
    let candidate = if let (Some(start), Some(end)) = (stripped.find('['), stripped.rfind(']')) {
        if start <= end {
            &stripped[start..=end]
        } else {
            stripped
        }
    } else {
        stripped
    };
    serde_json::from_str::<Vec<Value>>(candidate)
        .map_err(|error| format!("Unable to parse LLM JSON array response: {error}"))
}

fn strip_json_code_fence(content: &str) -> &str {
    if !content.starts_with("```") {
        return content;
    }
    let Some(first_newline) = content.find('\n') else {
        return content;
    };
    let body = &content[first_newline + 1..];
    if let Some(last_fence) = body.rfind("```") {
        body[..last_fence].trim()
    } else {
        body.trim()
    }
}

fn prompt_value(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|value| match value {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            Value::Array(_) | Value::Object(_) => Some(value.to_string()),
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_preview_recommendation_prompt_requests_suggested_type() {
        let prompt = build_llm_import_preview_recommendation_prompt(
            &[serde_json::json!({
                "id": 7,
                "date": "2026-01-01",
                "amount": "12.34",
                "type": "支出",
                "counterparty": "基金平台",
                "description": "定投扣款",
                "payment_method": "招商卡",
            })],
            &["ID=55 | 投资 | 投资交易/基金买入".to_string()],
            &["招商卡".to_string()],
            &[],
        );

        assert!(prompt.contains("\"suggested_type\""));
        assert!(prompt.contains("\"suggested_category_id\""));
        assert!(prompt.contains("收入|支出|投资|转账"));
        assert!(prompt.contains("ID=55 | 投资 | 投资交易/基金买入"));
    }
}
