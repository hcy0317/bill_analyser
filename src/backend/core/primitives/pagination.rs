// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, RuntimeError};

pub const DEFAULT_PAGE_SIZE: u32 = 50;
pub const MAX_PAGE_SIZE: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaginationWire {
    page: u32,
    page_size: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    page: u32,
    page_size: u32,
}

impl Pagination {
    pub fn new(page: u32, page_size: u32) -> Result<Self, RuntimeError> {
        if page == 0 || page_size == 0 || page_size > MAX_PAGE_SIZE {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid pagination",
            ));
        }
        Ok(Self { page, page_size })
    }

    pub fn from_request(page: Option<u32>, page_size: Option<u32>) -> Self {
        let normalized_page = page.unwrap_or(1).max(1);
        let normalized_page_size = match page_size {
            Some(0) | None => DEFAULT_PAGE_SIZE,
            Some(size) => size.min(MAX_PAGE_SIZE),
        };
        Self {
            page: normalized_page,
            page_size: normalized_page_size,
        }
    }

    pub const fn page(self) -> u32 {
        self.page
    }

    pub const fn page_size(self) -> u32 {
        self.page_size
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl<'de> Deserialize<'de> for Pagination {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = PaginationWire::deserialize(deserializer)?;
        Self::new(wire.page, wire.page_size).map_err(serde::de::Error::custom)
    }
}
