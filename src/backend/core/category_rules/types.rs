// 中文导读：分类规则表达式的数据传输结构。
// 维护重点：这些类型是 HTTP、DB 和规则匹配层共享的稳定合同，字段名不得随拆分变化。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleExpressionNodeDto {
    pub kind: String,
    pub operator: String,
    pub patterns: Vec<String>,
    pub children: Vec<RuleExpressionNodeDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledRuleDto {
    pub or_blocks: Vec<Vec<String>>,
    pub not_patterns: Vec<String>,
    pub and_patterns: Vec<String>,
    pub is_empty: bool,
    pub expression_ast: Option<RuleExpressionNodeDto>,
}

impl CompiledRuleDto {
    pub(super) fn empty() -> Self {
        Self {
            or_blocks: Vec::new(),
            not_patterns: Vec::new(),
            and_patterns: Vec::new(),
            is_empty: true,
            expression_ast: None,
        }
    }
}
