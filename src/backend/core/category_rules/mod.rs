// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

mod matching;
mod parser;
mod terms;
#[cfg(test)]
mod tests;
mod types;

pub use matching::{
    match_compiled_rule, match_compiled_rule_lowercase_text, match_rule_expression,
};
pub use parser::compile_rule_expression;
pub use terms::escape_rule_expression_term;
pub use types::{CompiledRuleDto, RuleExpressionNodeDto};
