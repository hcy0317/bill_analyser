// 中文导读：核心业务合同层，负责把金额、时间、分类、导入、匹配、预算、统计等规则从 HTTP/DB 细节中隔离。
// 维护重点：在这里记录跨路由复用的业务不变式，避免 handler 或 repository 重复推导。
// 不变式：金额单位、用户可见类型和兼容 payload 在进入或离开本层时必须显式转换。

use serde::{Deserialize, Serialize};

use crate::error::{ErrorCode, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
/// Canonical money primitive stored as cents. API adapters that expose yuan or
/// front-end cent fields must convert explicitly at their boundary.
pub struct Money {
    cents: i64,
}

impl Money {
    pub const ZERO: Self = Self { cents: 0 };
    pub const MIN_TRANSACTION_AMOUNT: Self = Self { cents: 1 };
    pub const MAX_TRANSACTION_AMOUNT: Self = Self {
        cents: 99_999_999_999,
    };

    pub const fn from_cents(cents: i64) -> Self {
        Self { cents }
    }

    pub fn from_cents_text(raw_value: &str) -> Result<Self, RuntimeError> {
        let cents = parse_signed_integer(raw_value.trim(), "invalid cents amount")?;
        Ok(Self::from_cents(cents))
    }

    pub fn from_yuan_str(raw_value: &str) -> Result<Self, RuntimeError> {
        let text = raw_value.trim();
        if text.is_empty() {
            return Err(invalid_money("missing yuan amount"));
        }

        let (negative, body) = match text.as_bytes()[0] {
            b'-' => (true, &text[1..]),
            b'+' => (false, &text[1..]),
            _ => (false, text),
        };

        if body.is_empty() {
            return Err(invalid_money("invalid yuan amount"));
        }

        let mut parts = body.split('.');
        let whole_text = parts.next().unwrap_or_default();
        let fraction_text = parts.next();
        if parts.next().is_some() {
            return Err(invalid_money("invalid yuan amount"));
        }

        if whole_text.is_empty() && fraction_text.unwrap_or_default().is_empty() {
            return Err(invalid_money("invalid yuan amount"));
        }

        if !whole_text.is_empty() && !whole_text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid_money("invalid yuan amount"));
        }

        let fraction = fraction_text.unwrap_or_default();
        if !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(invalid_money("invalid yuan amount"));
        }

        let whole_cents = parse_unsigned_i128(if whole_text.is_empty() {
            "0"
        } else {
            whole_text
        })?
        .checked_mul(100)
        .ok_or_else(|| invalid_money("yuan amount is too large"))?;

        let mut fraction_digits = fraction.bytes().map(|byte| i128::from(byte - b'0'));
        let first = fraction_digits.next().unwrap_or(0);
        let second = fraction_digits.next().unwrap_or(0);
        let third = fraction_digits.next().unwrap_or(0);

        let mut cents = whole_cents
            .checked_add(first * 10 + second)
            .ok_or_else(|| invalid_money("yuan amount is too large"))?;
        if third >= 5 {
            cents = cents
                .checked_add(1)
                .ok_or_else(|| invalid_money("yuan amount is too large"))?;
        }

        let signed = if negative { -cents } else { cents };
        let cents_i64 =
            i64::try_from(signed).map_err(|_| invalid_money("yuan amount is too large"))?;
        Ok(Self::from_cents(cents_i64))
    }

    pub const fn to_cents(self) -> i64 {
        self.cents
    }

    pub const fn is_positive(self) -> bool {
        self.cents > 0
    }

    pub const fn is_negative(self) -> bool {
        self.cents < 0
    }

    pub const fn is_valid_transaction_amount(self) -> bool {
        self.cents >= Self::MIN_TRANSACTION_AMOUNT.cents
            && self.cents <= Self::MAX_TRANSACTION_AMOUNT.cents
    }

    pub fn checked_abs(self) -> Result<Self, RuntimeError> {
        if self.is_negative() {
            self.checked_negated()
        } else {
            Ok(self)
        }
    }

    pub fn checked_negated(self) -> Result<Self, RuntimeError> {
        self.cents
            .checked_neg()
            .map(Self::from_cents)
            .ok_or_else(|| invalid_money("money amount is too large to negate"))
    }

    pub fn to_yuan_string(self) -> String {
        let signed = i128::from(self.cents);
        let negative = signed < 0;
        let absolute = if negative { -signed } else { signed };
        let yuan = absolute / 100;
        let cents = absolute % 100;
        if negative && absolute != 0 {
            format!("-{yuan}.{cents:02}")
        } else {
            format!("{yuan}.{cents:02}")
        }
    }
}

fn parse_signed_integer(text: &str, message: &'static str) -> Result<i64, RuntimeError> {
    if text.is_empty() {
        return Err(invalid_money(message));
    }
    let (negative, body) = match text.as_bytes()[0] {
        b'-' => (true, &text[1..]),
        b'+' => (false, &text[1..]),
        _ => (false, text),
    };
    if body.is_empty() || !body.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_money(message));
    }
    let parsed = parse_unsigned_i128(body)?;
    let signed = if negative { -parsed } else { parsed };
    i64::try_from(signed).map_err(|_| invalid_money("cents amount is too large"))
}

fn parse_unsigned_i128(text: &str) -> Result<i128, RuntimeError> {
    let mut value = 0_i128;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(invalid_money("invalid numeric amount"));
        }
        value = value
            .checked_mul(10)
            .and_then(|current| current.checked_add(i128::from(byte - b'0')))
            .ok_or_else(|| invalid_money("amount is too large"))?;
    }
    Ok(value)
}

fn invalid_money(message: &'static str) -> RuntimeError {
    RuntimeError::new(ErrorCode::InvalidInput, message)
}
