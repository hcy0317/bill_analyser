//! Stable import column-mapping template contracts.

use std::{cmp::Ordering, collections::BTreeMap, ops::Deref};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const SUPPORTED_IMPORT_CONFIG_FORMATS: &[&str] = &["csv", "excel"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfigDto {
    pub id: i64,
    pub name: String,
    pub file_format: String,
    pub description: String,
    pub field_mappings: Value,
    pub sample_headers: Vec<String>,
    pub date_format: String,
    pub delimiter: Option<String>,
    pub encoding: String,
    pub skip_rows: i32,
    pub has_header: bool,
    pub custom_rules: Value,
    pub is_default: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfigMatchDto {
    #[serde(flatten)]
    pub config: ImportConfigDto,
    pub description_summary: String,
    pub default_recommendation: bool,
    pub match_score: f64,
    pub match_reason: String,
}

impl Deref for ImportConfigMatchDto {
    type Target = ImportConfigDto;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl ImportConfigMatchDto {
    pub fn exact(config: ImportConfigDto) -> Self {
        Self::new(config, false, 1.0, "exact_headers")
    }

    pub fn default_fallback(config: ImportConfigDto) -> Self {
        Self::new(config, true, 0.0, "default_template_fallback")
    }

    fn new(
        config: ImportConfigDto,
        default_recommendation: bool,
        match_score: f64,
        match_reason: &str,
    ) -> Self {
        let description_summary = if config.description.trim().is_empty() {
            config.sample_headers.join(" / ")
        } else {
            config.description.clone()
        };
        Self {
            config,
            description_summary,
            default_recommendation,
            match_score,
            match_reason: match_reason.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfigSuggestion {
    pub include_header: bool,
    pub column_mapping: BTreeMap<String, i64>,
    pub transaction_type_mapping: BTreeMap<String, i64>,
    pub suggestions: Vec<ImportConfigColumnSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfigColumnSuggestion {
    pub column_type: i64,
    pub column_index: i64,
    pub header: String,
    pub score: f64,
}

impl ImportConfigColumnSuggestion {
    pub fn sort_key(&self) -> (i64, i64, i64, &str) {
        (
            self.column_index,
            -(self.score * 1_000_000.0).round() as i64,
            self.column_type,
            self.header.as_str(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConfigDraft {
    pub id: Option<i64>,
    pub name: String,
    pub file_format: String,
    pub description: String,
    pub field_mappings: Value,
    pub sample_headers: Vec<String>,
    pub date_format: String,
    pub delimiter: Option<String>,
    pub encoding: String,
    pub skip_rows: i32,
    pub has_header: bool,
    pub custom_rules: Value,
    pub is_default: bool,
}

impl ImportConfigDraft {
    pub fn validate_and_normalize(&self) -> Result<Self, ImportConfigValidationError> {
        if self.id.is_some_and(|id| id <= 0) {
            return Err(ImportConfigValidationError::InvalidId);
        }
        normalize_import_config_name(&self.name)?;
        let file_format = normalize_import_config_format(&self.file_format)?;
        if !self.field_mappings.is_object() {
            return Err(ImportConfigValidationError::InvalidFieldMappings);
        }
        if !self.custom_rules.is_object() {
            return Err(ImportConfigValidationError::InvalidCustomRules);
        }
        if self.skip_rows < 0 {
            return Err(ImportConfigValidationError::InvalidSkipRows);
        }
        let encoding = self.encoding.trim();
        if encoding.is_empty() {
            return Err(ImportConfigValidationError::InvalidEncoding);
        }
        let delimiter = self
            .delimiter
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        if delimiter
            .as_ref()
            .is_some_and(|value| value.chars().count() != 1)
        {
            return Err(ImportConfigValidationError::InvalidDelimiter);
        }
        if self
            .sample_headers
            .iter()
            .any(|header| header.trim().is_empty())
        {
            return Err(ImportConfigValidationError::InvalidHeaders);
        }
        Ok(Self {
            id: self.id,
            name: collapse_whitespace(self.name.trim()),
            file_format,
            description: self.description.trim().to_string(),
            field_mappings: self.field_mappings.clone(),
            sample_headers: self
                .sample_headers
                .iter()
                .map(|header| collapse_whitespace(header.trim()))
                .collect(),
            date_format: self.date_format.trim().to_string(),
            delimiter,
            encoding: encoding.to_lowercase(),
            skip_rows: self.skip_rows,
            has_header: self.has_header,
            custom_rules: self.custom_rules.clone(),
            is_default: self.is_default,
        })
    }

    pub fn normalized_name(&self) -> Result<String, ImportConfigValidationError> {
        normalize_import_config_name(&self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportConfigValidationError {
    #[error("import config id must be a positive integer")]
    InvalidId,
    #[error("import config name must not be empty")]
    InvalidName,
    #[error("unsupported import config file format")]
    UnsupportedFormat,
    #[error("import config headers must be a non-empty array of non-empty strings")]
    InvalidHeaders,
    #[error("import config fieldMappings must be an object")]
    InvalidFieldMappings,
    #[error("import config customRules must be an object")]
    InvalidCustomRules,
    #[error("import config skipRows must be non-negative")]
    InvalidSkipRows,
    #[error("import config encoding must not be empty")]
    InvalidEncoding,
    #[error("import config delimiter must contain exactly one character")]
    InvalidDelimiter,
    #[error("import config sampleRows must contain arrays of strings")]
    InvalidSampleRows,
}

pub fn normalize_import_config_format(
    file_format: &str,
) -> Result<String, ImportConfigValidationError> {
    let normalized = file_format.trim().to_lowercase();
    if SUPPORTED_IMPORT_CONFIG_FORMATS.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        Err(ImportConfigValidationError::UnsupportedFormat)
    }
}

pub fn normalize_import_config_name(name: &str) -> Result<String, ImportConfigValidationError> {
    let normalized = collapse_whitespace(name.trim()).to_lowercase();
    if normalized.is_empty() {
        Err(ImportConfigValidationError::InvalidName)
    } else {
        Ok(normalized)
    }
}

pub fn normalize_import_config_headers(
    headers: &[String],
) -> Result<Vec<String>, ImportConfigValidationError> {
    if headers.is_empty() {
        return Err(ImportConfigValidationError::InvalidHeaders);
    }
    headers
        .iter()
        .map(|header| {
            let normalized = collapse_whitespace(header.trim()).to_lowercase();
            if normalized.is_empty() {
                Err(ImportConfigValidationError::InvalidHeaders)
            } else {
                Ok(normalized)
            }
        })
        .collect()
}

pub fn suggest_import_config(
    headers: &[String],
    sample_rows: Option<&[Vec<String>]>,
) -> Result<ImportConfigSuggestion, ImportConfigValidationError> {
    let normalized_headers = normalize_import_config_headers(headers)?;
    if sample_rows.is_some_and(|rows| rows.iter().any(|row| row.len() > headers.len())) {
        return Err(ImportConfigValidationError::InvalidSampleRows);
    }

    let mut column_mapping = BTreeMap::new();
    let mut suggestions = Vec::new();
    for (column_index, normalized) in normalized_headers.iter().enumerate() {
        let Some((column_type, score)) = recognize_column_type(normalized) else {
            continue;
        };
        let column_index = i64::try_from(column_index).unwrap_or(i64::MAX);
        column_mapping
            .entry(column_type.to_string())
            .or_insert(column_index);
        suggestions.push(ImportConfigColumnSuggestion {
            column_type,
            column_index,
            header: headers[usize::try_from(column_index).unwrap_or(0)]
                .trim()
                .to_string(),
            score,
        });
    }
    suggestions.sort_by(compare_suggestions);

    let transaction_type_mapping = column_mapping
        .get("3")
        .and_then(|index| usize::try_from(*index).ok())
        .map(|index| transaction_type_values(sample_rows.unwrap_or_default(), index))
        .unwrap_or_default();

    Ok(ImportConfigSuggestion {
        include_header: true,
        column_mapping,
        transaction_type_mapping,
        suggestions,
    })
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn recognize_column_type(header: &str) -> Option<(i64, f64)> {
    const ALIASES: &[(i64, &[&str])] = &[
        (
            1,
            &[
                "transaction time",
                "datetime",
                "date",
                "time",
                "交易时间",
                "交易日期",
                "日期",
                "时间",
            ],
        ),
        (2, &["transaction timezone", "timezone", "时区"]),
        (
            3,
            &["transaction type", "type", "收支类型", "交易类型", "类型"],
        ),
        (4, &["category", "main category", "分类", "主分类"]),
        (
            5,
            &[
                "secondary category",
                "subcategory",
                "sub category",
                "二级分类",
                "子分类",
            ],
        ),
        (6, &["account name", "account", "账户", "账户名称"]),
        (7, &["account currency", "currency", "币种", "货币"]),
        (8, &["amount", "transaction amount", "金额", "交易金额"]),
        (
            9,
            &[
                "transfer in account name",
                "related account",
                "对方账户",
                "转入账户",
            ],
        ),
        (
            10,
            &[
                "transfer in currency",
                "related currency",
                "对方币种",
                "转入币种",
            ],
        ),
        (
            11,
            &[
                "transfer in amount",
                "related amount",
                "对方金额",
                "转入金额",
            ],
        ),
        (12, &["geographic location", "location", "位置", "地理位置"]),
        (13, &["tags", "tag", "标签"]),
        (
            14,
            &[
                "description",
                "memo",
                "remark",
                "note",
                "备注",
                "说明",
                "摘要",
            ],
        ),
    ];
    for (column_type, aliases) in ALIASES {
        if aliases.contains(&header) {
            return Some((*column_type, 1.0));
        }
    }
    for (column_type, aliases) in ALIASES {
        if aliases
            .iter()
            .any(|alias| header.contains(alias) || alias.contains(header))
        {
            return Some((*column_type, 0.85));
        }
    }
    None
}

fn compare_suggestions(
    left: &ImportConfigColumnSuggestion,
    right: &ImportConfigColumnSuggestion,
) -> Ordering {
    left.column_index
        .cmp(&right.column_index)
        .then_with(|| right.score.total_cmp(&left.score))
        .then_with(|| left.column_type.cmp(&right.column_type))
        .then_with(|| left.header.cmp(&right.header))
}

fn transaction_type_values(rows: &[Vec<String>], index: usize) -> BTreeMap<String, i64> {
    rows.iter()
        .filter_map(|row| row.get(index))
        .filter_map(|value| {
            let trimmed = value.trim();
            transaction_type_code(&trimmed.to_lowercase()).map(|code| (trimmed.to_string(), code))
        })
        .collect()
}

fn transaction_type_code(value: &str) -> Option<i64> {
    match value {
        "modify balance" | "balance" | "余额变更" | "调整余额" => Some(1),
        "income" | "收入" | "收" => Some(2),
        "expense" | "支出" | "付" => Some(3),
        "transfer" | "转账" | "转入" | "转出" => Some(4),
        "investment" | "投资" => Some(5),
        _ => None,
    }
}
