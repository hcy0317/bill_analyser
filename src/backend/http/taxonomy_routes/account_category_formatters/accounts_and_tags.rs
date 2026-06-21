// 中文导读：账户与标签响应/请求格式化 facade，聚合账户子账户、金额、类型分类归一化和标签模板格式化。
// 维护重点：保持前端 DTO 字段、legacy 账户分类兼容和 cents/minor units 金额合同不变。

include!("accounts_and_tags/sub_accounts.rs");
include!("accounts_and_tags/account_responses.rs");
include!("accounts_and_tags/account_payload.rs");
include!("accounts_and_tags/account_normalization.rs");
include!("accounts_and_tags/tag_template_responses.rs");
#[cfg(test)]
include!("accounts_and_tags/tests.rs");
