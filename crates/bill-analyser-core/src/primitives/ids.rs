use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{ErrorCode, RuntimeError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId {
    value: u64,
}

impl UserId {
    pub fn new(value: u64) -> Result<Self, RuntimeError> {
        if value == 0 {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "invalid user id",
            ));
        }
        Ok(Self { value })
    }

    pub fn parse_positive(raw_value: &str) -> Result<Self, RuntimeError> {
        let value = raw_value
            .trim()
            .parse::<u64>()
            .map_err(|_| RuntimeError::new(ErrorCode::InvalidInput, "invalid user id"))?;
        Self::new(value)
    }

    pub const fn get(self) -> u64 {
        self.value
    }
}

impl Serialize for UserId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.get())
    }
}

impl<'de> Deserialize<'de> for UserId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId {
    value: String,
}

impl EntityId {
    pub fn parse(raw_value: &str) -> Result<Self, RuntimeError> {
        let value = raw_value.trim();
        if value.is_empty() {
            return Err(RuntimeError::new(
                ErrorCode::InvalidInput,
                "missing entity id",
            ));
        }
        Ok(Self {
            value: value.to_string(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl Serialize for EntityId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for EntityId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}
