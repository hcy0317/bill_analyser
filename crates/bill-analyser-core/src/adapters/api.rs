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
