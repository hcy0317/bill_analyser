// 中文导读：核心业务合同层，负责账户识别规则的表达式匹配和匹配解释。
// 维护重点：本模块只处理与请求无关的规则语义，仓储和 HTTP 层不得重复实现匹配规则。
// 不变式：账户规则复用分类规则表达式语法；导入账户识别由稳定后的预览上下文决定角色、类型和字段包。

mod engine;
#[cfg(test)]
mod tests;

pub use engine::*;

#[cfg(test)]
use engine::{
    account_rule_cross_field_match, account_rule_plain_pattern_matches,
    match_account_rule_expression, match_account_rule_expression_node,
};
