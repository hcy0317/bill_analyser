#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSessionDraft {
    pub session_id: String,
    pub user_id: UserId,
    pub file_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSessionStatusUpdate {
    pub session_id: String,
    pub user_id: UserId,
    pub status: String,
    pub total_parsed: Option<i64>,
    pub total_preview: Option<i64>,
    pub total_confirmed: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportParseStagingResult {
    pub inserted_count: usize,
    pub total_parsed: i64,
    pub session_found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSessionRow {
    pub id: i64,
    pub session_id: String,
    pub user_id: i64,
    pub status: String,
    pub file_count: i64,
    pub total_parsed: i64,
    pub total_preview: i64,
    pub total_confirmed: i64,
    pub created_at: String,
    pub updated_at: String,
}
