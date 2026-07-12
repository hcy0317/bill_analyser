// 中文导读：Postgres import staging 层，定义导入 source 与 standard row 台账 DTO。
// 维护重点：这些类型只描述 staging 行形状，业务合并、去重和匹配规则留在 core/http 流程。
// 不变式：standard row 金额使用分单位，source/row 查询必须保持 user/session scope。

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSourceDraft {
    pub source_index: i64,
    pub original_file_name: String,
    pub parser_id: String,
    pub parser_name: String,
    pub parser_signal: String,
    pub parser_confidence: f64,
    pub feature_signature: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSourceRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub source_index: i64,
    pub original_file_name: String,
    pub parser_id: String,
    pub parser_name: String,
    pub parser_signal: String,
    pub parser_confidence: f64,
    pub feature_signature: String,
    pub metadata: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportStandardRowDraft {
    pub source_index: i64,
    pub source_row_index: i64,
    pub occurred_at: String,
    pub amount_cents: i64,
    pub direction: String,
    pub transaction_type: String,
    pub merchant: String,
    pub payment_method: String,
    pub description: String,
    pub parser_payload: Value,
    pub standard_payload: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportStandardRow {
    pub id: i64,
    pub session_id: String,
    pub source_id: i64,
    pub user_id: i64,
    pub source_index: i64,
    pub source_row_index: i64,
    pub parser_id: String,
    pub occurred_at: String,
    pub amount_cents: i64,
    pub direction: String,
    pub transaction_type: String,
    pub merchant: String,
    pub payment_method: String,
    pub description: String,
    pub parser_payload: Value,
    pub standard_payload: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDecisionGroupDraft {
    pub group_type: String,
    pub group_key: String,
    pub decision_status: String,
    pub base_preview_row_id: Option<i64>,
    pub signal_payload: Value,
    pub members: Vec<ImportDecisionGroupMemberDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDecisionGroupMemberDraft {
    pub preview_row_id: Option<i64>,
    pub standard_row_id: Option<i64>,
    pub history_bill_id: Option<i64>,
    pub member_role: String,
    pub parser_name: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDecisionGroupRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub group_type: String,
    pub group_key: String,
    pub decision_status: String,
    pub base_preview_row_id: Option<i64>,
    pub signal_payload: Value,
    pub version: i64,
    pub members: Vec<ImportDecisionGroupMemberRow>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDecisionGroupMemberRow {
    pub id: i64,
    pub group_id: i64,
    pub preview_row_id: Option<i64>,
    pub standard_row_id: Option<i64>,
    pub history_bill_id: Option<i64>,
    pub member_role: String,
    pub parser_name: String,
    pub metadata: Value,
    pub version: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDecisionPreviewVersion {
    pub preview_row_id: i64,
    pub version: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportDecisionGroupCommand {
    pub operation_id: String,
    pub session_id: String,
    pub group_id: i64,
    pub decision: String,
    pub expected_group_version: i64,
    pub expected_preview_versions: Vec<ImportDecisionPreviewVersion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportDecisionGroupMutation {
    pub group_id: i64,
    pub group_version: i64,
    pub decision_status: String,
    pub removed_preview_ids: Vec<i64>,
    pub upserted_preview_ids: Vec<i64>,
    #[serde(default)]
    pub upserted_preview_items: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportDecisionGroupCommandResult {
    Applied(ImportDecisionGroupMutation),
    NotFound,
    MaterializationPending,
    MaterializationFailed,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportHistoryBillRow {
    pub history_bill_id: i64,
    pub history_bill_version: i64,
    pub bill: DedupBill,
    pub snapshot: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportHistoryMaterializationDraft {
    pub history_bill_id: i64,
    pub history_bill_version: i64,
    pub materialized_payload: Value,
    pub rewrite_reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportHistoryMaterializationRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub history_bill_id: i64,
    pub history_bill_version: i64,
    pub materialized_payload: Value,
    pub rewrite_reason: String,
    pub created_at: String,
}
