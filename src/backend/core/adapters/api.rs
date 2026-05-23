// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Serialize};

use crate::primitives::Pagination;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult<T> {
    pub items: Vec<T>,
    pub total_count: u64,
    pub page: u32,
    pub page_size: u32,
    pub total: u64,
}

impl<T> PageResult<T> {
    pub fn new(items: Vec<T>, total: u64, pagination: Pagination) -> Self {
        Self {
            items,
            total_count: total,
            page: pagination.page(),
            page_size: pagination.page_size(),
            total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageResponse<T> {
    pub success: bool,
    pub result: PageResult<T>,
}

impl<T> PageResponse<T> {
    pub fn new(items: Vec<T>, total: u64, pagination: Pagination) -> Self {
        Self {
            success: true,
            result: PageResult::new(items, total, pagination),
        }
    }
}
