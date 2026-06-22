use serde::de::Error as DeError;
use serde::ser::Error as SerError;
use serde::{Deserialize, Deserializer, Serializer};
use serde_json::Value;

use bill_analyser_core::Money;

/// 将 parser 内部的分单位金额序列化为旧 parser/API 合同中的元数值。
pub(crate) fn serialize_money_as_yuan_number<S>(
    amount: &Money,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let yuan_value = amount
        .to_yuan_string()
        .parse::<f64>()
        .map_err(S::Error::custom)?;
    serializer.serialize_f64(yuan_value)
}

/// 从旧 parser/API 元数值或文本恢复为内部 Money，避免 parser 边界丢失分单位。
pub(crate) fn deserialize_money_from_yuan_value<'de, D>(deserializer: D) -> Result<Money, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(number) => {
            Money::from_yuan_str(&number.to_string()).map_err(D::Error::custom)
        }
        Value::String(text) => {
            Money::from_yuan_str(&clean_amount_text(&text)).map_err(D::Error::custom)
        }
        Value::Null => Ok(Money::ZERO),
        _ => Err(D::Error::custom("invalid yuan amount")),
    }
}

/// 清理来源金额文本中的货币符号和千分位分隔符，保留元单位数字文本。
pub(crate) fn clean_amount_text(raw_amount: &str) -> String {
    raw_amount
        .replace(['¥', '$', ',', '，'], "")
        .trim()
        .to_string()
}
