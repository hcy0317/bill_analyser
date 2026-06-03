// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde_json::{Map, Value};

pub(super) fn first_non_empty_field(
    object: Option<&Map<String, Value>>,
    key: &str,
) -> Option<String> {
    object
        .and_then(|item| item.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn string_field_or(
    object: Option<&Map<String, Value>>,
    key: &str,
    default: &str,
) -> String {
    first_non_empty_field(object, key).unwrap_or_else(|| default.to_string())
}
