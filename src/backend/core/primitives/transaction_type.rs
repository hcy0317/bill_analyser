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
}
