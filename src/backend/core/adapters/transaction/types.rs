#[derive(Debug, Clone, PartialEq)]
pub struct BackendTransactionView {
    pub id: String,
    pub time_sequence_id: Option<String>,
    pub transaction_type: TransactionType,
    pub category_id: Option<String>,
    pub main_category: String,
    pub sub_category: String,
    pub date: String,
    pub amount: Money,
    pub destination_amount: Option<Money>,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
    pub utc_offset: UtcOffsetMinutes,
    pub hide_amount: bool,
    pub tag_ids: Vec<String>,
    pub tags: Vec<FrontendTransactionTag>,
    pub category: Option<Value>,
    pub source_account: Option<Value>,
    pub destination_account: Option<Value>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendTransactionTag {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrontendTransactionView {
    pub id: String,
    pub time_sequence_id: String,
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    pub category_id: String,
    pub category_name: String,
    pub sub_category_name: String,
    pub time: i64,
    pub utc_offset: i32,
    pub source_account_id: String,
    pub destination_account_id: String,
    pub amount_cents: i64,
    pub source_amount_cents: i64,
    pub destination_amount_cents: i64,
    pub hide_amount: bool,
    pub tag_ids: Vec<String>,
    pub tags: Vec<FrontendTransactionTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_account: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_account: Option<Value>,
    pub comment: String,
    pub editable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gregorian_calendar_year_dash_month_dash_day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gregorian_calendar_day_of_month: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_day_of_week: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillMutationMetadata {
    pub category_id: String,
    pub source_account_id: i64,
    pub destination_account_id: i64,
    pub tag_ids: Vec<i64>,
    pub auto_invest_account: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BillAccountSyncSnapshot {
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountBalanceBill {
    pub transaction_type: TransactionType,
    pub amount: Money,
    pub destination_amount: Money,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BatchUpdateResponse {
    pub updated_count: usize,
    pub failed_count: usize,
    pub failed_ids: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_index: Option<usize>,
    pub created_count: usize,
    pub items: Vec<Value>,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteResponseContract {
    pub status_code: u16,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionPictureUploadResult {
    pub picture_id: String,
    pub original_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationQueryParams {
    pub account_id: String,
    pub account_id_int: i64,
    pub start_time: i64,
    pub end_time: i64,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub category_ids: Option<String>,
    pub transaction_type_code: Option<i64>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationCategoryRecord {
    pub id: i64,
    pub main_category: String,
    pub sub_category: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReconciliationCategoryFilter {
    pub main: String,
    pub sub: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconciliationOpeningBalanceSnapshot {
    pub account_balance: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationBill {
    pub id: String,
    pub date: String,
    pub transaction_type: Option<TransactionType>,
    pub amount: Money,
    pub destination_amount: Option<Money>,
    pub source_account_id: Option<i64>,
    pub destination_account_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconciliationBalanceEntry {
    pub opening: Money,
    pub closing: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconciliationSummary {
    pub opening_balance: Money,
    pub closing_balance: Money,
    pub total_inflows: Money,
    pub total_outflows: Money,
    pub balance_history: BTreeMap<String, ReconciliationBalanceEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationResult {
    pub account_id: String,
    pub account_name: String,
    pub start_time: i64,
    pub end_time: i64,
    pub opening_balance_cents: i64,
    pub closing_balance_cents: i64,
    pub total_inflows_cents: i64,
    pub total_outflows_cents: i64,
    pub net_flow_cents: i64,
    pub transactions: Vec<Value>,
    pub item_count: usize,
}
