use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const OCR_DISABLED_PROVIDER_NAME: &str = "disabled";
pub const OCR_DEFAULT_LANG: &str = "chi_sim+eng";
pub const LLM_SYSTEM_PROMPT: &str = "你是 Bill Analyser 的智能分类助手。Bill Analyser 是一个个人/家庭账单管理系统，支持收入、支出、转账三种交易类型。\n每笔交易包含：日期、金额（单位：元）、交易对方、描述、支付方式、主分类、子分类。\n你的任务是根据交易信息推断最合适的分类，或根据已分类样本归纳关键词匹配规则。\n请始终以 JSON 格式返回结果，不要包含额外的解释文字。";
pub const OCR_AVAILABLE_PROVIDERS: [&str; 3] = ["cloud_stub", "tesseract", "local_json_ocr"];
pub const LLM_AVAILABLE_PROVIDERS: [&str; 13] = [
    "openai",
    "claude",
    "anthropic",
    "deepseek",
    "ollama",
    "xai",
    "google",
    "openrouter",
    "openai_compatible",
    "openai-compatible",
    "azure",
    "azure_openai",
    "azure-openai",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OcrConfigContract {
    pub provider: String,
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaymentScreenshotParseContract {
    /// Yuan-unit OCR extraction for Python API parity; storage conversions to cents happen later.
    pub amount: Option<f64>,
    pub trade_time: Option<String>,
    pub description: Option<String>,
    pub payment_platform: Option<String>,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AiRouteResponse {
    pub status_code: u16,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrProviderTextResult {
    pub text: String,
    pub confidence: f64,
    pub model: String,
    pub raw_provider_response: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<OcrProviderTextLine>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OcrProviderTextLine {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bbox: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReceiptDraftField {
    pub value: Value,
    pub confidence: f64,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReceiptTransactionDraft {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub auto_fill: BTreeMap<String, ReceiptDraftField>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub candidates: BTreeMap<String, Vec<ReceiptDraftField>>,
}

impl ReceiptTransactionDraft {
    pub fn is_empty(&self) -> bool {
        self.auto_fill.is_empty() && self.candidates.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReceiptDraftContext {
    pub categories: Vec<ReceiptDraftCategory>,
    pub category_rules: Vec<ReceiptDraftCategoryRule>,
    pub accounts: Vec<ReceiptDraftAccount>,
    pub tags: Vec<ReceiptDraftTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptDraftCategory {
    pub id: String,
    pub type_code: i64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptDraftCategoryRule {
    pub id: String,
    pub category_id: String,
    pub category_type: i64,
    pub label: String,
    pub priority: i64,
    pub rule_expression: String,
    pub regex_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptDraftAccount {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptDraftTag {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmProviderConfigContract {
    pub provider: String,
    pub normalized_provider: String,
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub provider_name: String,
}
