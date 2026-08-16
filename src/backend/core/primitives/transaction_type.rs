// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和API payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ErrorCode, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Income,
    Expense,
    Transfer,
    Investment,
}

impl Serialize for TransactionType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(self.code())
    }
}

impl<'de> Deserialize<'de> for TransactionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = i32::deserialize(deserializer)?;
        Self::from_frontend_code(code).map_err(serde::de::Error::custom)
    }
}

impl TransactionType {
    pub const fn code(self) -> i32 {
        match self {
            Self::Income => 2,
            Self::Expense => 3,
            Self::Transfer => 4,
            Self::Investment => 5,
        }
    }

    pub fn from_frontend_code(value: i32) -> Result<Self, RuntimeError> {
        match value {
            2 => Ok(Self::Income),
            3 => Ok(Self::Expense),
            4 => Ok(Self::Transfer),
            5 => Ok(Self::Investment),
            _ => Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid transaction type",
            )),
        }
    }

    pub const fn backend_name(self) -> &'static str {
        match self {
            Self::Income => "收入",
            Self::Expense => "支出",
            Self::Transfer => "转账",
            Self::Investment => "投资",
        }
    }

    pub fn from_backend_name(raw_value: &str) -> Result<Self, RuntimeError> {
        match raw_value.trim().to_ascii_lowercase().as_str() {
            "收入" | "income" | "2" => Ok(Self::Income),
            "支出" | "expense" | "3" => Ok(Self::Expense),
            "转账" | "transfer" | "4" => Ok(Self::Transfer),
            "投资" | "investment" | "5" => Ok(Self::Investment),
            _ => Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid backend transaction type",
            )),
        }
    }
}
