// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

pub const DEFAULT_PARENT_ID: &str = "0";

pub fn virtual_parent_id(main_category: &str) -> String {
    format!("virtual_{}", main_category)
}
