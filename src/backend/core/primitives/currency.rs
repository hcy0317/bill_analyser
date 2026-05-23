// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ErrorCode, RuntimeError};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CurrencyCode {
    code: String,
}

impl CurrencyCode {
    pub fn parse(raw_value: &str) -> Result<Self, RuntimeError> {
        let code = raw_value.trim().to_ascii_uppercase();
        if code.is_empty()
            || code.len() > 8
            || !code.bytes().all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid currency code",
            ));
        }
        Ok(Self { code })
    }

    pub fn as_str(&self) -> &str {
        &self.code
    }

    pub fn symbol(&self) -> &str {
        match self.code.as_str() {
            "CNY" | "JPY" => "¥",
            "USD" => "$",
            "EUR" => "€",
            "GBP" => "£",
            "HKD" => "HK$",
            _ => self.as_str(),
        }
    }
}

impl Default for CurrencyCode {
    fn default() -> Self {
        Self {
            code: "CNY".to_string(),
        }
    }
}

impl Serialize for CurrencyCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CurrencyCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = String::deserialize(deserializer)?;
        Self::parse(&code).map_err(serde::de::Error::custom)
    }
}
