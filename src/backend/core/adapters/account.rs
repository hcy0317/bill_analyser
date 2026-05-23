// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use serde_json::Value;

pub fn parse_aliases_text(raw_value: &str) -> Vec<String> {
    let text = raw_value.trim();
    if text.is_empty() {
        return Vec::new();
    }

    if text.starts_with('[') {
        if let Ok(values) = serde_json::from_str::<Vec<Value>>(text) {
            return values
                .into_iter()
                .filter_map(|value| {
                    let alias = match value {
                        Value::String(text) => text,
                        other => other.to_string(),
                    };
                    let alias = alias.trim().to_string();
                    (!alias.is_empty()).then_some(alias)
                })
                .collect();
        }
    }

    text.split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}
