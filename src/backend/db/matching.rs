// 中文导读：Postgres-only matching facade，导出当前 HTTP 层需要的查询/写入入口和请求 DTO。
// 维护重点：不再包含 non-Postgres schema/actions/reconciliation projection；所有运行态读写都走 Postgres。
// 不变式：未物化候选 action 入口不在 DB 层保留，未物化候选由 HTTP 返回明确冲突。

use bill_analyser_core::UserId;

use crate::{
    DbError, ImportPreviewExpectedState, ImportPreviewLearningApply,
    ImportPreviewRecurringCandidate,
};

pub mod postgres_reads;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MatchingRuntimeError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Db(String),
}

impl MatchingRuntimeError {
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn status_code(&self) -> u16 {
        match self {
            Self::BadRequest(_) => 400,
            Self::NotFound(_) => 404,
            Self::Conflict(_) => 409,
            Self::Db(_) => 500,
        }
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub fn message(&self) -> &str {
        match self {
            Self::BadRequest(message)
            | Self::NotFound(message)
            | Self::Conflict(message)
            | Self::Db(message) => message,
        }
    }
}

impl From<DbError> for MatchingRuntimeError {
    fn from(error: DbError) -> Self {
        Self::Db(error.to_string())
    }
}

pub type MatchingResult<T> = Result<T, MatchingRuntimeError>;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReconciliationCandidateFilters {
    pub session_id: Option<String>,
    pub preview_id: Option<i64>,
    pub existing_bill_id: Option<i64>,
    pub candidate_type: Option<String>,
    pub status: Option<String>,
    pub limit: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PreviewMatchingActionRequest {
    pub expected_state: Option<ImportPreviewExpectedState>,
    pub response_mode_preview_item: bool,
    pub reviewed_type: Option<String>,
    pub recurring_id: Option<i64>,
    pub recurring_candidate_count: i64,
    pub recurring_candidate: Option<ImportPreviewRecurringCandidate>,
    pub learning_apply: Option<ImportPreviewLearningApply>,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn _matching_user_id_marker(_user_id: UserId) {}
