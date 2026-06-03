// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortField {
    Date,
    Amount,
    Type,
    Category,
    Account,
    CreatedAt,
    UpdatedAt,
}

impl SortField {
    pub fn parse(raw_value: &str) -> Result<Self, RuntimeError> {
        match raw_value.trim() {
            "date" => Ok(Self::Date),
            "amount" => Ok(Self::Amount),
            "type" => Ok(Self::Type),
            "category" => Ok(Self::Category),
            "account" => Ok(Self::Account),
            "created_at" => Ok(Self::CreatedAt),
            "updated_at" => Ok(Self::UpdatedAt),
            _ => Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid sort field",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Amount => "amount",
            Self::Type => "type",
            Self::Category => "category",
            Self::Account => "account",
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
    Asc,
    Desc,
}

impl SortOrder {
    pub fn parse(raw_value: &str) -> Result<Self, RuntimeError> {
        match raw_value.trim().to_ascii_lowercase().as_str() {
            "asc" => Ok(Self::Asc),
            "desc" => Ok(Self::Desc),
            _ => Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid sort order",
            )),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}
