use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::{Body, Bytes},
    extract::{Path as AxumPath, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::{
    backup_archive_summary_from_entries, backup_restore_verify_response, build_backup_file_info,
    build_sync_config_contract, derive_backup_fernet_key, invalid_backup_archive_summary,
    is_safe_backup_archive_member, normalize_backup_job_payload, plan_backup_cleanup,
    resolve_backup_filename, BackupCleanupDecision, BackupFileCandidate, BackupFileInfoContract,
    BackupFileInfoInput, BackupRecordContract, SyncConfigContract, UserId,
};
use bill_analyser_db::{
    create_backup_audit_log_best_effort, create_or_update_backup_job, init_backup_ops_schema,
    list_backup_jobs, list_backup_records, update_backup_record_by_filename, upsert_backup_record,
    BackupAuditLogDraft, BackupJobDraft, BackupRecordDraft, BackupRecordRow, DbError,
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use chrono::{Local, Utc};
use fernet::Fernet;
use ring::hmac;
use serde_json::{json, Map, Value};
use sha2::Digest;
use tempfile::{Builder as TempFileBuilder, TempDir};
use tokio_util::io::ReaderStream;
use url::Url;
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    auth::resolve_authenticated_user_from_headers,
    backup_sync::{
        upload_backup_to_cloud, validate_sync_upload_config, CloudBackupUploadError,
        CloudBackupUploadResult,
    },
    config::HttpShellConfig,
    state::HttpAppState,
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const STEP_UP_TOKEN_HEADER: &str = "x-bill-analyser-step-up-token";
const PUBLIC_ENCRYPTED_BACKUP_SUFFIX: &str = ".zip.enc";
const RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX: &str = "_encrypted.fernet";
static BACKUP_FILENAME_COUNTER: AtomicU64 = AtomicU64::new(0);

type RouteResult<T> = Result<T, Box<Response>>;
type FileRouteResult<T> = Result<T, BackupFileRuntimeError>;

struct AuthenticatedBackupRuntime {
    runtime: SqliteRuntime,
    user_id: UserId,
    auth_kind: BackupAuthKind,
}

#[derive(Debug)]
struct PreparedCloudSync {
    config: Value,
    contract: SyncConfigContract,
    file_path: PathBuf,
    backup_info: BackupFileInfoContract,
    user_id: UserId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackupAuthKind {
    TrustedHeader,
    BearerSession,
}

#[derive(Debug)]
struct BackupFileRuntimeError {
    status: StatusCode,
    message: String,
}

impl BackupFileRuntimeError {
    fn new(status: StatusCode, message: impl ToString) -> Self {
        Self {
            status,
            message: message.to_string(),
        }
    }

    fn bad_request(message: impl ToString) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn internal(message: impl ToString) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

impl From<io::Error> for BackupFileRuntimeError {
    fn from(value: io::Error) -> Self {
        Self::internal(value)
    }
}

impl From<zip::result::ZipError> for BackupFileRuntimeError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::internal(value)
    }
}

pub const BACKUP_OPS_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/backup/"),
    ("POST", "/api/backup/cleanup"),
    ("POST", "/api/backup/create"),
    ("DELETE", "/api/backup/delete/{filename}"),
    ("GET", "/api/backup/download/{filename}"),
    ("GET", "/api/backup/jobs"),
    ("POST", "/api/backup/jobs"),
    ("POST", "/api/backup/restore/{filename}"),
    ("POST", "/api/backup/restore/verify"),
    ("POST", "/api/backup/sync"),
];

pub fn backup_ops_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route("/api/backup/", get(list_backup_files_handler))
        .route("/api/backup/cleanup", post(cleanup_backups_handler))
        .route("/api/backup/create", post(create_backup_handler))
        .route("/api/backup/sync", post(sync_backup_handler))
        .route(
            "/api/backup/delete/:filename",
            delete(delete_backup_handler),
        )
        .route(
            "/api/backup/download/:filename",
            get(download_backup_handler),
        )
        .route(
            "/api/backup/jobs",
            get(list_backup_jobs_handler).post(save_backup_job_handler),
        )
        .route(
            "/api/backup/restore/:filename",
            post(restore_backup_handler),
        )
        .route(
            "/api/backup/restore/verify",
            post(verify_backup_restore_handler),
        )
}

mod archive;
#[cfg(test)]
mod archive_tests;
mod audit;
mod auth;
mod cleanup;
mod create;
mod delete;
mod download;
mod handlers;
mod jobs;
mod payload;
mod response;
mod restore;
mod sync;

use archive::*;
use audit::*;
use auth::*;
use cleanup::*;
use create::*;
use delete::*;
use download::*;
use jobs::*;
use payload::*;
use response::*;
use restore::*;
use sync::*;

use handlers::{
    cleanup_backups_handler, create_backup_handler, delete_backup_handler, download_backup_handler,
    list_backup_files_handler, list_backup_jobs_handler, restore_backup_handler,
    save_backup_job_handler, sync_backup_handler, verify_backup_restore_handler,
};

async fn blocking_route<F>(operation: F) -> Response
where
    F: FnOnce() -> RouteResult<Response> + Send + 'static,
{
    match tokio::task::spawn_blocking(operation).await {
        Ok(Ok(response)) => response,
        Ok(Err(response)) => *response,
        Err(error) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("backup runtime task failed: {error}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_file_runtime_error_constructors_pin_status_and_message() {
        let bad_request = BackupFileRuntimeError::bad_request("bad input");
        assert_eq!(bad_request.status, StatusCode::BAD_REQUEST);
        assert_eq!(bad_request.message, "bad input");

        let internal = BackupFileRuntimeError::internal("broken");
        assert_eq!(internal.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(internal.message, "broken");

        let io_error = BackupFileRuntimeError::from(io::Error::other("io"));
        assert_eq!(io_error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(io_error.message.contains("io"));
    }

    #[tokio::test]
    async fn blocking_route_maps_returned_and_join_errors() {
        let returned = blocking_route(|| {
            Err(Box::new(error_response(
                StatusCode::BAD_REQUEST,
                "returned",
            )))
        })
        .await;
        assert_eq!(returned.status(), StatusCode::BAD_REQUEST);

        let joined = blocking_route(|| -> RouteResult<Response> {
            panic!("panic in backup blocking task");
        })
        .await;
        assert_eq!(joined.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
