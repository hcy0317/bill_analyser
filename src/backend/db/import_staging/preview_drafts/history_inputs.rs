#[derive(Debug, Clone)]
pub struct ImportHistoryDuplicatePreviewInput<'a> {
    pub imported_bill: &'a DedupBill,
    pub history_bill: &'a ImportHistoryBillRow,
    pub candidate_id: &'a str,
    pub group_key: &'a str,
    pub time_diff_seconds: i64,
    pub score_percent: u8,
    pub level: &'a str,
    pub reason: &'a str,
}

#[derive(Debug, Clone)]
pub struct ImportHistoryTransferPreviewInput<'a> {
    pub imported_bill: &'a DedupBill,
    pub history_bill: &'a ImportHistoryBillRow,
    pub candidate_id: &'a str,
    pub group_key: &'a str,
    pub time_diff_seconds: i64,
    pub score_percent: u8,
    pub level: &'a str,
    pub reason: &'a str,
}
