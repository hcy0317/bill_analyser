// 中文导读：SQLite repository 层，定义导入 source 与 standard row 台账 DTO。
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
