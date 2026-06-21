#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmPreviewResult {
    pub confirmed_count: usize,
    pub skipped_count: usize,
    pub duplicate_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportHistoryRewriteAcknowledgement {
    #[serde(default)]
    pub acknowledged: bool,
    #[serde(default, alias = "selectedPreviewIds")]
    pub selected_preview_ids: Vec<i64>,
    #[serde(default)]
    pub operations: Vec<ImportHistoryRewriteAcknowledgementOperation>,
    #[serde(default, alias = "selectionScope")]
    pub selection_scope: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportHistoryRewriteAcknowledgementOperation {
    #[serde(alias = "previewId")]
    pub preview_id: i64,
    #[serde(alias = "operationId")]
    pub operation_id: String,
    #[serde(alias = "plannedOperation")]
    pub planned_operation: String,
    #[serde(alias = "historyBillId")]
    pub history_bill_id: i64,
    #[serde(alias = "historyBillVersion")]
    pub history_bill_version: i64,
    #[serde(alias = "acknowledgementToken")]
    pub acknowledgement_token: String,
}
