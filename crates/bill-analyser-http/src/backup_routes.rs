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
    derive_backup_fernet_key, invalid_backup_archive_summary, is_safe_backup_archive_member,
    normalize_backup_job_payload, plan_backup_cleanup, resolve_backup_filename,
    BackupCleanupDecision, BackupFileCandidate, BackupFileInfoContract, BackupFileInfoInput,
    BackupRecordContract, UserId,
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
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    auth::resolve_authenticated_user_from_headers, config::HttpShellConfig, proxy::ProxyState,
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
];

pub const BACKUP_OPS_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

pub fn backup_ops_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route("/api/backup/", get(list_backup_files_handler))
        .route("/api/backup/cleanup", post(cleanup_backups_handler))
        .route("/api/backup/create", post(create_backup_handler))
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

async fn list_backup_files_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    blocking_route(move || list_backup_files_response(&state, &headers)).await
}

async fn create_backup_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    blocking_route(move || create_backup_response(&state, &headers)).await
}

async fn verify_backup_restore_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    blocking_route(move || verify_backup_restore_response(&state, &headers, body)).await
}

async fn download_backup_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    match prepare_download_backup_response(&state, &headers, &filename) {
        Ok((file, safe_filename)) => stream_backup_download_response(file, &safe_filename),
        Err(response) => *response,
    }
}

async fn delete_backup_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    blocking_route(move || delete_backup_response(&state, &headers, &filename)).await
}

async fn restore_backup_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    AxumPath(filename): AxumPath<String>,
) -> Response {
    blocking_route(move || restore_backup_response(&state, &headers, &filename)).await
}

async fn cleanup_backups_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    blocking_route(move || cleanup_backups_response(&state, &headers, body)).await
}

async fn list_backup_jobs_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    blocking_route(move || list_backup_jobs_response(&state, &headers)).await
}

async fn save_backup_job_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    blocking_route(move || save_backup_job_response(&state, &headers, body)).await
}

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

fn list_backup_files_response(state: &ProxyState, headers: &HeaderMap) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let mut infos = Vec::new();
    for file_path in list_local_backup_files(&backup_dir).map_err(file_error_response)? {
        let info =
            build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
                .map_err(file_error_response)?;
        infos.push(info);
    }

    let records = list_backup_records(auth_runtime.runtime.connection())
        .map_err(|_| Box::new(db_error_response()))?;
    let record_map = records
        .into_iter()
        .map(|record| (record.backup_name.clone(), record))
        .collect::<BTreeMap<_, _>>();

    let mut payload = infos
        .into_iter()
        .map(|info| {
            let filename = info.filename.clone();
            backup_info_with_record(info, record_map.get(&filename))
        })
        .collect::<Vec<_>>();
    payload.sort_by(|left, right| {
        json_string_field(right, "created_at").cmp(&json_string_field(left, "created_at"))
    });

    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": payload }),
    ))
}

fn create_backup_response(state: &ProxyState, headers: &HeaderMap) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let data_dir = data_dir(&state.config);
    let sqlite_db_path = state.config.sqlite_db_path.as_deref().map(PathBuf::from);

    let user_id = auth_runtime.user_id;
    drop(auth_runtime);

    let result = create_backup_file(
        &data_dir,
        &backup_dir,
        sqlite_db_path.as_deref(),
        state.config.backup_encryption_key.as_deref(),
    );
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let file_path = match result {
        Ok(Some(path)) => path,
        Ok(None) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                user_id,
                headers,
                "backup_created",
                json!({"filename": "", "path": ""}),
                0,
                "failed",
                Some("数据未变化，无需备份".to_string()),
            );
            return Ok(error_response(
                StatusCode::BAD_REQUEST,
                "数据未变化，无需备份",
            ));
        }
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                user_id,
                headers,
                "backup_created",
                json!({"filename": "", "path": ""}),
                0,
                "failed",
                Some(error.message.clone()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };

    let backup_info =
        build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
            .map_err(|error| {
                write_backup_audit_event(
                    auth_runtime.runtime.connection(),
                    user_id,
                    headers,
                    "backup_created",
                    json!({"filename": backup_filename(&file_path), "path": public_backup_reference(&file_path)}),
                    0,
                    "failed",
                    Some(error.message.clone()),
                );
                Box::new(error_response(error.status, error.message))
            })?;

    if !(backup_info.valid_zip && backup_info.ready_to_restore) {
        let error_message = if backup_info.error.is_empty() {
            "backup archive is not ready to restore".to_string()
        } else {
            backup_info.error.clone()
        };
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            user_id,
            headers,
            "backup_created",
            json!({"filename": backup_info.filename, "path": backup_info.path}),
            0,
            "failed",
            Some(error_message.clone()),
        );
        return Ok(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            error_message,
        ));
    }

    upsert_backup_record_from_info(auth_runtime.runtime.connection(), &backup_info)
        .map_err(|_| Box::new(db_error_response()))?;
    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        user_id,
        headers,
        "backup_created",
        json!({
            "filename": backup_info.filename,
            "path": backup_info.path,
            "checksum": backup_info.checksum,
        }),
        1,
        "success",
        None,
    );

    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": backup_info }),
    ))
}

fn verify_backup_restore_response(
    state: &ProxyState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_restore_verified",
                json!({}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, Some(&payload))?;
    let filename = payload
        .get("filename")
        .and_then(value_to_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if filename.is_empty() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_restore_verified",
            json!({"filename": audit_safe_json_field(&payload, "filename")}),
            0,
            "failed",
            Some("filename is required".to_string()),
        );
        return Ok(error_response(
            StatusCode::BAD_REQUEST,
            "filename is required",
        ));
    }

    let backup_dir = backup_dir(&state.config)?;
    let file_path = match resolve_backup_path(&backup_dir, &filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_restore_verified",
                json!({"filename": audit_safe_value(&Value::String(filename))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_restore_verified",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Ok(error_response(StatusCode::NOT_FOUND, "文件不存在"));
    }

    let backup_info =
        build_runtime_backup_info(&file_path, state.config.backup_encryption_key.as_deref())
            .map_err(file_error_response)?;
    let restore_ready = backup_info.valid_zip && backup_info.ready_to_restore;
    let restore_error = if backup_info.error.is_empty() {
        "backup archive is not ready to restore".to_string()
    } else {
        backup_info.error.clone()
    };
    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        "backup_restore_verified",
        json!({
            "filename": safe_filename,
            "checksum": backup_info.checksum,
            "valid_zip": backup_info.valid_zip,
            "ready_to_restore": backup_info.ready_to_restore,
            "metadata_checksum_matched": backup_info.metadata_checksum_matched,
        }),
        0,
        if restore_ready { "success" } else { "failed" },
        (!restore_ready).then_some(restore_error.clone()),
    );

    Ok(json_response(
        if restore_ready {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        },
        backup_restore_verify_response(&backup_info),
    ))
}

fn prepare_download_backup_response(
    state: &ProxyState,
    headers: &HeaderMap,
    filename: &str,
) -> RouteResult<(File, String)> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let file_path = match resolve_backup_path(&backup_dir, filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_downloaded",
                json!({"filename": audit_safe_value(&Value::String(filename.to_string()))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Err(Box::new(error_response(error.status, error.message)));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_downloaded",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Err(Box::new(error_response(
            StatusCode::NOT_FOUND,
            "文件不存在",
        )));
    }

    let file = match File::open(&file_path) {
        Ok(file) => file,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_downloaded",
                json!({"filename": safe_filename}),
                0,
                "failed",
                Some(error.to_string()),
            );
            return Err(Box::new(error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                error,
            )));
        }
    };
    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        "backup_downloaded",
        json!({"filename": safe_filename, "path": public_backup_reference(&file_path)}),
        1,
        "success",
        None,
    );
    Ok((file, safe_filename))
}

fn stream_backup_download_response(file: File, safe_filename: &str) -> Response {
    let stream = ReaderStream::new(tokio::fs::File::from_std(file));
    let mut response = Response::new(Body::from_stream(stream));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{safe_filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

fn delete_backup_response(
    state: &ProxyState,
    headers: &HeaderMap,
    filename: &str,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let file_path = match resolve_backup_path(&backup_dir, filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_deleted",
                json!({"filename": audit_safe_value(&Value::String(filename.to_string()))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_deleted",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Ok(error_response(StatusCode::NOT_FOUND, "文件不存在"));
    }

    let result = (|| -> FileRouteResult<()> {
        fs::remove_file(&file_path)?;
        remove_metadata_file(&file_path)?;
        update_backup_record_by_filename(
            auth_runtime.runtime.connection(),
            &safe_filename,
            Some("deleted"),
            json!({
                "deleted_reason": "manual_delete",
                "deleted_at": now_iso(),
            }),
        )
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        Ok(())
    })();

    if let Err(error) = result {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_deleted",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some(error.message.clone()),
        );
        return Ok(error_response(error.status, error.message));
    }

    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        "backup_deleted",
        json!({"filename": safe_filename}),
        1,
        "success",
        None,
    );
    Ok(json_response(
        StatusCode::OK,
        json!({"success": true, "data": {"filename": safe_filename}}),
    ))
}

fn restore_backup_response(
    state: &ProxyState,
    headers: &HeaderMap,
    filename: &str,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, None)?;
    let backup_dir = backup_dir(&state.config)?;
    let data_dir = data_dir(&state.config);
    let file_path = match resolve_backup_path(&backup_dir, filename) {
        Ok(path) => path,
        Err(error) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_restored",
                json!({"filename": audit_safe_value(&Value::String(filename.to_string()))}),
                0,
                "failed",
                Some("invalid backup filename".to_string()),
            );
            return Ok(error_response(error.status, error.message));
        }
    };
    let safe_filename = backup_filename(&file_path);
    if !file_path.exists() {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_restored",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some("backup file does not exist".to_string()),
        );
        return Ok(error_response(StatusCode::NOT_FOUND, "文件不存在"));
    }

    let restored_at = now_iso();
    let user_id = auth_runtime.user_id;
    drop(auth_runtime);
    let result = restore_data_dir_from_backup(
        &file_path,
        &backup_dir,
        &data_dir,
        state.config.backup_encryption_key.as_deref(),
    );
    let runtime = open_backup_ops_runtime(state)?;

    if let Err(error) = result {
        write_backup_audit_event(
            runtime.connection(),
            user_id,
            headers,
            "backup_restored",
            json!({"filename": safe_filename}),
            0,
            "failed",
            Some(error.message.clone()),
        );
        return Ok(error_response(error.status, error.message));
    }
    update_backup_record_by_filename(
        runtime.connection(),
        &safe_filename,
        Some("restored"),
        json!({"restored_at": restored_at}),
    )
    .map_err(|error| Box::new(db_write_error_response(error)))?;

    write_backup_audit_event(
        runtime.connection(),
        user_id,
        headers,
        "backup_restored",
        json!({"filename": safe_filename, "restored_at": restored_at}),
        1,
        "success",
        None,
    );
    Ok(json_response(
        StatusCode::OK,
        json!({"success": true, "data": {"filename": safe_filename, "restored_at": restored_at}}),
    ))
}

fn cleanup_backups_response(
    state: &ProxyState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_cleanup",
                json!({"keep_count": "", "deleted_count": 0}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };
    ensure_sensitive_backup_auth(&auth_runtime, state, headers, Some(&payload))?;
    let keep_count = match parse_keep_count(&payload) {
        Ok(value) => value,
        Err(message) => {
            write_backup_audit_event(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                "backup_cleanup",
                json!({"keep_count": audit_safe_json_field(&payload, "keep_count"), "deleted_count": 0}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };

    let backup_dir = backup_dir(&state.config)?;
    let records = list_backup_records(auth_runtime.runtime.connection())
        .map_err(|_| Box::new(db_error_response()))?;
    let record_contracts = records
        .iter()
        .map(|record| {
            let file_exists = resolve_backup_path(&backup_dir, &record.backup_name)
                .map(|path| path.exists())
                .unwrap_or(false);
            BackupRecordContract {
                id: record.id,
                backup_name: record.backup_name.clone(),
                storage_type: record.storage_type.clone(),
                status: record.status.clone(),
                created_at: record.created_at.clone(),
                file_exists,
            }
        })
        .collect::<Vec<_>>();
    let stray_files = list_local_backup_files(&backup_dir)
        .map_err(file_error_response)?
        .into_iter()
        .map(|path| {
            Ok(BackupFileCandidate {
                filename: backup_filename(&path),
                modified_at: file_modified_at(&path)?,
            })
        })
        .collect::<FileRouteResult<Vec<_>>>()
        .map_err(file_error_response)?;

    let plan = plan_backup_cleanup(&record_contracts, &stray_files, keep_count);
    let result = apply_cleanup_plan(
        auth_runtime.runtime.connection(),
        &backup_dir,
        &plan.decisions,
    );
    if let Err(error) = result {
        write_backup_audit_event(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            "backup_cleanup",
            json!({"keep_count": keep_count, "deleted_count": plan.deleted_count}),
            i64::try_from(plan.deleted_count).unwrap_or(i64::MAX),
            "failed",
            Some(error.message.clone()),
        );
        return Ok(error_response(error.status, error.message));
    }

    write_backup_audit_event(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        "backup_cleanup",
        json!({"keep_count": keep_count, "deleted_count": plan.deleted_count}),
        i64::try_from(plan.deleted_count).unwrap_or(i64::MAX),
        "success",
        None,
    );

    Ok(json_response(
        StatusCode::OK,
        json!({"success": true, "data": {"deleted_count": plan.deleted_count, "kept_count": plan.kept_count}}),
    ))
}

fn list_backup_jobs_response(state: &ProxyState, headers: &HeaderMap) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let jobs = list_backup_jobs(auth_runtime.runtime.connection(), auth_runtime.user_id)
        .map_err(|_| Box::new(db_error_response()))?;
    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": jobs }),
    ))
}

fn save_backup_job_response(
    state: &ProxyState,
    headers: &HeaderMap,
    body: Bytes,
) -> RouteResult<Response> {
    let auth_runtime = authenticated_backup_runtime(state, headers)?;
    let payload = match optional_json_body(&body) {
        Ok(payload) => payload,
        Err(message) => {
            write_backup_job_audit(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                json!({}),
                0,
                "failed",
                Some(message.clone()),
            );
            return Ok(error_response(StatusCode::BAD_REQUEST, message));
        }
    };
    let normalized = match normalize_backup_job_payload(&payload) {
        Ok(job) => job,
        Err(error) => {
            write_backup_job_audit(
                auth_runtime.runtime.connection(),
                auth_runtime.user_id,
                headers,
                validation_audit_details(&payload),
                0,
                "failed",
                Some(error.error.clone()),
            );
            return Ok(error_response(
                status_or_internal(error.status_code),
                error.message,
            ));
        }
    };
    let response_payload = json!({
        "id": normalized.id,
        "job_type": normalized.job_type,
        "schedule_expr": normalized.schedule_expr,
        "retention_days": normalized.retention_days,
        "retention_count": normalized.retention_count,
        "enabled": normalized.enabled,
        "last_status": normalized.last_status,
    });

    let job_id = create_or_update_backup_job(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        BackupJobDraft::from(normalized.clone()),
    )
    .map_err(|error| {
        write_backup_job_audit(
            auth_runtime.runtime.connection(),
            auth_runtime.user_id,
            headers,
            validation_audit_details(&payload),
            0,
            "failed",
            Some(error.to_string()),
        );
        Box::new(db_write_error_response(error))
    })?;

    write_backup_job_audit(
        auth_runtime.runtime.connection(),
        auth_runtime.user_id,
        headers,
        json!({
            "user_id": auth_runtime.user_id.get(),
            "job_id": job_id,
            "job_type": normalized.job_type,
            "retention_days": normalized.retention_days,
            "retention_count": normalized.retention_count,
        }),
        1,
        "success",
        None,
    );

    let mut response_payload = response_payload;
    if let Some(object) = response_payload.as_object_mut() {
        object.insert("id".to_string(), json!(job_id));
    }
    Ok(json_response(
        StatusCode::OK,
        json!({ "success": true, "data": response_payload }),
    ))
}

fn authenticated_backup_runtime(
    state: &ProxyState,
    headers: &HeaderMap,
) -> RouteResult<AuthenticatedBackupRuntime> {
    let authenticated =
        resolve_authenticated_user_from_headers(headers, &state.config, TRUSTED_USER_SECRET_HEADER)
            .map_err(|error| Box::new(auth_error_response(error)))?;
    let auth_kind = if authenticated.session_id.is_some() {
        BackupAuthKind::BearerSession
    } else {
        BackupAuthKind::TrustedHeader
    };
    let runtime = open_backup_ops_runtime(state)?;
    Ok(AuthenticatedBackupRuntime {
        runtime,
        user_id: authenticated.user_id,
        auth_kind,
    })
}

fn open_backup_ops_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let runtime = open_runtime(state)?;
    init_backup_ops_schema(runtime.connection()).map_err(|_| Box::new(db_error_response()))?;
    Ok(runtime)
}

fn ensure_sensitive_backup_auth(
    auth_runtime: &AuthenticatedBackupRuntime,
    state: &ProxyState,
    headers: &HeaderMap,
    payload: Option<&Value>,
) -> RouteResult<()> {
    if auth_runtime.auth_kind == BackupAuthKind::TrustedHeader {
        return Ok(());
    }
    let Some(token) = backup_step_up_token(headers, payload) else {
        return Err(Box::new(error_response(
            StatusCode::UNAUTHORIZED,
            "step-up token is required for backup file operation",
        )));
    };
    validate_backup_step_up_token(&token, state, auth_runtime.user_id)
        .map_err(|message| Box::new(error_response(StatusCode::UNAUTHORIZED, message)))
}

fn backup_step_up_token(headers: &HeaderMap, payload: Option<&Value>) -> Option<String> {
    header_text(headers, STEP_UP_TOKEN_HEADER).or_else(|| {
        payload
            .and_then(|value| {
                value
                    .get("stepUpToken")
                    .or_else(|| value.get("step_up_token"))
                    .and_then(Value::as_str)
            })
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn validate_backup_step_up_token(
    token: &str,
    state: &ProxyState,
    expected_user_id: UserId,
) -> Result<(), &'static str> {
    let secret = state
        .config
        .auth_jwt_secret
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or("step-up token validation is not configured")?;
    let mut parts = token.split('.');
    let encoded_header = parts.next().ok_or("Invalid step-up token")?;
    let encoded_payload = parts.next().ok_or("Invalid step-up token")?;
    let encoded_signature = parts.next().ok_or("Invalid step-up token")?;
    if parts.next().is_some() {
        return Err("Invalid step-up token");
    }

    let header = decode_backup_jwt_part(encoded_header)?;
    let payload = decode_backup_jwt_part(encoded_payload)?;
    let configured_algorithm = normalize_jwt_algorithm(&state.config.auth_jwt_algorithm);
    let header_algorithm = normalize_jwt_algorithm(
        header
            .get("alg")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    if header_algorithm != configured_algorithm {
        return Err("Invalid step-up token");
    }
    let hmac_algorithm = jwt_hmac_algorithm(&configured_algorithm)?;
    let signature = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded_signature)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded_signature))
        .map_err(|_| "Invalid step-up token")?;
    let signing_input = format!("{encoded_header}.{encoded_payload}");
    let key = hmac::Key::new(hmac_algorithm, secret.as_bytes());
    hmac::verify(&key, signing_input.as_bytes(), &signature)
        .map_err(|_| "Invalid step-up token")?;
    if payload.get("type").and_then(Value::as_str) != Some("step_up") {
        return Err("Invalid step-up token");
    }
    let exp = payload
        .get("exp")
        .and_then(Value::as_i64)
        .ok_or("Invalid step-up token")?;
    if exp <= Utc::now().timestamp() {
        return Err("Invalid step-up token");
    }
    let user_id = payload
        .get("user_id")
        .and_then(Value::as_u64)
        .ok_or("Invalid step-up token")?;
    if user_id != expected_user_id.get() {
        return Err("Invalid step-up token");
    }
    Ok(())
}

fn decode_backup_jwt_part(encoded: &str) -> Result<Value, &'static str> {
    let decoded = general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .or_else(|_| general_purpose::URL_SAFE.decode(encoded))
        .map_err(|_| "Invalid step-up token")?;
    serde_json::from_slice(&decoded).map_err(|_| "Invalid step-up token")
}

fn normalize_jwt_algorithm(algorithm: &str) -> String {
    algorithm.trim().to_ascii_uppercase()
}

fn jwt_hmac_algorithm(algorithm: &str) -> Result<hmac::Algorithm, &'static str> {
    match normalize_jwt_algorithm(algorithm).as_str() {
        "HS256" => Ok(hmac::HMAC_SHA256),
        "HS384" => Ok(hmac::HMAC_SHA384),
        "HS512" => Ok(hmac::HMAC_SHA512),
        _ => Err("Unsupported JWT algorithm for backup step-up token"),
    }
}

fn create_backup_file(
    data_dir: &Path,
    backup_dir: &Path,
    sqlite_db_path: Option<&Path>,
    encryption_key: Option<&str>,
) -> FileRouteResult<Option<PathBuf>> {
    if !data_dir.exists() {
        return Ok(None);
    }
    fs::create_dir_all(backup_dir).map_err(|error| {
        io_context_error(
            error,
            format!("create backup directory {}", backup_dir.display()),
        )
    })?;
    remove_stale_plaintext_backup_temps(backup_dir)?;
    let source = prepare_backup_source(data_dir, backup_dir, sqlite_db_path)?;
    let backup_path = unique_backup_zip_path(backup_dir);
    create_backup_zip(source.data_dir(), &backup_path)?;
    if let Some(secret) = encryption_key.filter(|value| !value.trim().is_empty()) {
        Ok(Some(encrypt_backup_file(&backup_path, secret)?))
    } else {
        Ok(Some(backup_path))
    }
}

struct BackupSource {
    data_dir: PathBuf,
    _temp: Option<TempDir>,
}

impl BackupSource {
    fn borrowed(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
            _temp: None,
        }
    }

    fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

fn prepare_backup_source(
    data_dir: &Path,
    backup_dir: &Path,
    sqlite_db_path: Option<&Path>,
) -> FileRouteResult<BackupSource> {
    let Some(sqlite_db_path) = sqlite_db_path else {
        return Ok(BackupSource::borrowed(data_dir));
    };
    if !sqlite_db_path.starts_with(data_dir) {
        return Ok(BackupSource::borrowed(data_dir));
    }

    let sqlite_relative = sqlite_db_path
        .strip_prefix(data_dir)
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?
        .to_path_buf();
    let temp = TempFileBuilder::new()
        .prefix("backup_source_")
        .tempdir_in(backup_dir)?;
    let staged_data_dir = temp.path().join(
        data_dir
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("data"),
    );
    let sqlite_sidecars = sqlite_sidecar_relative_paths(&sqlite_relative);
    copy_dir_all_filtered(data_dir, &staged_data_dir, &|relative| {
        relative == sqlite_relative.as_path()
            || sqlite_sidecars
                .iter()
                .any(|sidecar| sidecar.as_path() == relative)
    })?;
    if sqlite_db_path.exists() {
        snapshot_sqlite_database(sqlite_db_path, &staged_data_dir.join(&sqlite_relative))?;
    }
    Ok(BackupSource {
        data_dir: staged_data_dir,
        _temp: Some(temp),
    })
}

fn sqlite_sidecar_relative_paths(sqlite_relative: &Path) -> Vec<PathBuf> {
    let raw = sqlite_relative.to_string_lossy();
    [format!("{raw}-wal"), format!("{raw}-shm")]
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn snapshot_sqlite_database(source: &Path, target: &Path) -> FileRouteResult<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if target.exists() {
        fs::remove_file(target)?;
    }
    let connection = rusqlite::Connection::open(source)
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    connection
        .execute("VACUUM INTO ?1", [target.to_string_lossy().as_ref()])
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    Ok(())
}

fn unique_backup_zip_path(backup_dir: &Path) -> PathBuf {
    let counter = BACKUP_FILENAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    backup_dir.join(format!(
        "backup_{}_{}.zip",
        Local::now().format("%Y%m%d_%H%M%S_%f"),
        counter
    ))
}

fn create_backup_zip(data_dir: &Path, backup_path: &Path) -> FileRouteResult<()> {
    let data_parent = data_dir
        .parent()
        .ok_or_else(|| BackupFileRuntimeError::internal("data directory has no parent"))?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(backup_path)
        .map_err(|error| {
            io_context_error(
                error,
                format!("create backup archive {}", backup_path.display()),
            )
        })?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let mut files = WalkDir::new(data_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort();

    for file_path in files {
        let archive_name = relative_zip_name(data_parent, &file_path)?;
        zip.start_file(&archive_name, options).map_err(|error| {
            BackupFileRuntimeError::internal(format!(
                "start backup archive member {archive_name}: {error}"
            ))
        })?;
        let mut source = File::open(&file_path).map_err(|error| {
            io_context_error(error, format!("open backup source {}", file_path.display()))
        })?;
        io::copy(&mut source, &mut zip).map_err(|error| {
            io_context_error(
                error,
                format!("write backup source {}", file_path.display()),
            )
        })?;
    }
    let archive_file = zip.finish().map_err(|error| {
        BackupFileRuntimeError::internal(format!(
            "finish backup archive {}: {error}",
            backup_path.display()
        ))
    })?;
    drop(archive_file);
    Ok(())
}

fn relative_zip_name(base_dir: &Path, file_path: &Path) -> FileRouteResult<String> {
    let relative = file_path
        .strip_prefix(base_dir)
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    let name = relative
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if name.is_empty() {
        Err(BackupFileRuntimeError::internal(
            "empty backup archive path",
        ))
    } else {
        Ok(name)
    }
}

fn encrypt_backup_file(file_path: &Path, secret: &str) -> FileRouteResult<PathBuf> {
    let key = derive_backup_fernet_key(secret)
        .ok_or_else(|| BackupFileRuntimeError::bad_request("备份加密密钥未配置"))?;
    let fernet = Fernet::new(&key)
        .ok_or_else(|| BackupFileRuntimeError::internal("invalid derived backup key"))?;
    let archive_bytes = fs::read(file_path).map_err(|error| {
        io_context_error(
            error,
            format!("read backup archive {}", file_path.display()),
        )
    })?;
    let encrypted = fernet.encrypt(&archive_bytes);
    let encrypted_path = encrypted_storage_path_for_zip(file_path);
    let mut encrypted_file = File::create(&encrypted_path).map_err(|error| {
        io_context_error(
            error,
            format!(
                "create encrypted backup archive {}",
                encrypted_path.display()
            ),
        )
    })?;
    encrypted_file
        .write_all(encrypted.as_bytes())
        .map_err(|error| {
            io_context_error(
                error,
                format!(
                    "write encrypted backup archive {}",
                    encrypted_path.display()
                ),
            )
        })?;
    encrypted_file.flush().map_err(|error| {
        io_context_error(
            error,
            format!(
                "flush encrypted backup archive {}",
                encrypted_path.display()
            ),
        )
    })?;
    drop(encrypted_file);
    if let Err(error) = fs::remove_file(file_path) {
        let _ = fs::remove_file(&encrypted_path);
        return Err(io_context_error(
            error,
            format!("remove plaintext backup {}", file_path.display()),
        ));
    }
    Ok(encrypted_path)
}

fn build_runtime_backup_info(
    file_path: &Path,
    encryption_key: Option<&str>,
) -> FileRouteResult<BackupFileInfoContract> {
    let stat = fs::metadata(file_path)?;
    let filename = public_backup_filename(file_path);
    let checksum = calculate_file_checksum(file_path)?;
    let mut metadata = read_backup_metadata(file_path);
    let metadata_checksum = metadata
        .get("checksum")
        .and_then(Value::as_str)
        .map(str::to_string);
    let archive_summary = archive_summary_for_backup_file(file_path, encryption_key);

    if metadata
        .get("size")
        .and_then(Value::as_u64)
        .is_none_or(|size| size != stat.len())
    {
        metadata = build_backup_metadata(file_path, &checksum, &archive_summary)?;
    }

    Ok(build_backup_file_info(BackupFileInfoInput {
        filename,
        size: stat.len(),
        created_at: system_time_iso(stat.modified().unwrap_or_else(|_| SystemTime::now())),
        path: public_backup_reference(file_path),
        checksum,
        metadata_checksum: metadata
            .get("checksum")
            .and_then(Value::as_str)
            .map(str::to_string)
            .or(metadata_checksum),
        archive_summary,
    }))
}

fn public_backup_reference(file_path: &Path) -> String {
    format!("backup/{}", public_backup_filename(file_path))
}

fn archive_summary_for_backup_file(
    file_path: &Path,
    encryption_key: Option<&str>,
) -> bill_analyser_core::BackupArchiveSummary {
    if is_encrypted_backup_storage_path(file_path) {
        let Some(secret) = encryption_key.filter(|value| !value.trim().is_empty()) else {
            return invalid_backup_archive_summary("backup encryption key is not configured");
        };
        return match decrypted_backup_bytes(file_path, secret) {
            Ok(bytes) => inspect_zip_reader(io::Cursor::new(bytes)),
            Err(error) => invalid_backup_archive_summary(&error.message),
        };
    }
    match File::open(file_path) {
        Ok(file) => inspect_zip_reader(file),
        Err(error) => invalid_backup_archive_summary(&error.to_string()),
    }
}

fn decrypted_backup_bytes(file_path: &Path, secret: &str) -> FileRouteResult<Vec<u8>> {
    let key = derive_backup_fernet_key(secret)
        .ok_or_else(|| BackupFileRuntimeError::bad_request("备份加密密钥未配置"))?;
    let fernet = Fernet::new(&key)
        .ok_or_else(|| BackupFileRuntimeError::internal("invalid derived backup key"))?;
    let token = String::from_utf8(fs::read(file_path)?)
        .map_err(|_| BackupFileRuntimeError::bad_request("备份解密失败"))?;
    fernet
        .decrypt(&token)
        .map_err(|_| BackupFileRuntimeError::bad_request("备份解密失败"))
}

fn inspect_zip_reader<R: Read + Seek>(reader: R) -> bill_analyser_core::BackupArchiveSummary {
    let mut archive = match ZipArchive::new(reader) {
        Ok(archive) => archive,
        Err(error) => return invalid_backup_archive_summary(&error.to_string()),
    };
    let mut names = Vec::new();
    for index in 0..archive.len() {
        match archive.by_index(index) {
            Ok(file) => names.push(file.name().to_string()),
            Err(error) => return invalid_backup_archive_summary(&error.to_string()),
        }
    }
    backup_archive_summary_from_entries(names)
}

fn build_backup_metadata(
    file_path: &Path,
    checksum: &str,
    archive_summary: &bill_analyser_core::BackupArchiveSummary,
) -> FileRouteResult<Value> {
    let stat = fs::metadata(file_path)?;
    let metadata = json!({
        "filename": backup_filename(file_path),
        "checksum": checksum,
        "size": stat.len(),
        "created_at": system_time_iso(stat.modified().unwrap_or_else(|_| SystemTime::now())),
        "valid_zip": archive_summary.valid_zip,
        "contains_data_dir": archive_summary.contains_data_dir,
        "entry_count": archive_summary.entry_count,
        "top_level_entries": archive_summary.top_level_entries,
        "ready_to_restore": archive_summary.ready_to_restore,
    });
    Ok(metadata)
}

fn read_backup_metadata(file_path: &Path) -> Value {
    fs::read_to_string(backup_metadata_path(file_path))
        .ok()
        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
        .unwrap_or_else(|| Value::Object(Map::new()))
}

fn backup_metadata_path(file_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.meta.json", file_path.to_string_lossy()))
}

fn calculate_file_checksum(file_path: &Path) -> FileRouteResult<String> {
    let mut file = File::open(file_path).map_err(|error| {
        io_context_error(
            error,
            format!("open backup checksum source {}", file_path.display()),
        )
    })?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn io_context_error(error: io::Error, context: impl ToString) -> BackupFileRuntimeError {
    BackupFileRuntimeError::internal(format!("{}: {error}", context.to_string()))
}

fn upsert_backup_record_from_info(
    connection: &rusqlite::Connection,
    backup_info: &BackupFileInfoContract,
) -> Result<i64, DbError> {
    upsert_backup_record(
        connection,
        BackupRecordDraft {
            backup_name: backup_info.filename.clone(),
            file_path: backup_info.path.clone(),
            checksum: backup_info.checksum.clone(),
            encrypted: backup_info.encrypted,
            status: "created".to_string(),
            metadata: json!({
                "valid_zip": backup_info.valid_zip,
                "contains_data_dir": backup_info.contains_data_dir,
                "entry_count": backup_info.entry_count,
                "top_level_entries": backup_info.top_level_entries,
                "ready_to_restore": backup_info.ready_to_restore,
            }),
        },
    )
}

fn backup_info_with_record(
    info: BackupFileInfoContract,
    record: Option<&BackupRecordRow>,
) -> Value {
    let mut value = serde_json::to_value(info).unwrap_or(Value::Null);
    if let (Value::Object(object), Some(record)) = (&mut value, record) {
        object.insert("recordId".to_string(), json!(record.id));
        object.insert("recordStatus".to_string(), json!(record.status));
        object.insert("recordCreatedAt".to_string(), json!(record.created_at));
    }
    value
}

fn restore_data_dir_from_backup(
    file_path: &Path,
    backup_dir: &Path,
    data_dir: &Path,
    encryption_key: Option<&str>,
) -> FileRouteResult<()> {
    validate_data_dir_for_restore(data_dir)?;
    if is_encrypted_backup_storage_path(file_path) {
        let Some(secret) = encryption_key.filter(|value| !value.trim().is_empty()) else {
            return Err(BackupFileRuntimeError::bad_request("备份加密密钥未配置"));
        };
        let bytes = decrypted_backup_bytes(file_path, secret)?;
        restore_data_dir_from_archive(io::Cursor::new(bytes), backup_dir, data_dir)
    } else {
        restore_data_dir_from_archive(File::open(file_path)?, backup_dir, data_dir)
    }
}

fn restore_data_dir_from_archive<R: Read + Seek>(
    reader: R,
    backup_dir: &Path,
    data_dir: &Path,
) -> FileRouteResult<()> {
    let mut archive = ZipArchive::new(reader)
        .map_err(|error| BackupFileRuntimeError::bad_request(error.to_string()))?;
    let names = archive.file_names().map(str::to_string).collect::<Vec<_>>();
    if let Some(unsafe_name) = names
        .iter()
        .find(|name| !is_safe_backup_archive_member(name))
    {
        return Err(BackupFileRuntimeError::bad_request(format!(
            "备份文件包含不安全路径: {unsafe_name}"
        )));
    }
    let summary = backup_archive_summary_from_entries(names);
    if !(summary.valid_zip && summary.ready_to_restore) {
        let error = if summary.error.is_empty() {
            "backup archive is not ready to restore".to_string()
        } else {
            summary.error
        };
        return Err(BackupFileRuntimeError::bad_request(error));
    }

    let temp_restore_dir = TempFileBuilder::new()
        .prefix("restore_temp_")
        .tempdir_in(backup_dir)?;
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let member_name = file.name().replace('\\', "/");
        if !is_safe_backup_archive_member(&member_name) {
            return Err(BackupFileRuntimeError::bad_request(format!(
                "备份文件包含不安全路径: {member_name}"
            )));
        }
        let out_path = temp_restore_dir.path().join(&member_name);
        if file.is_dir() || member_name.ends_with('/') {
            fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&out_path)?;
        io::copy(&mut file, &mut output)?;
    }

    let extracted_data_dir = temp_restore_dir.path().join("data");
    if !extracted_data_dir.exists() {
        return Err(BackupFileRuntimeError::bad_request(
            "备份文件缺少 data/ 目录",
        ));
    }
    if !directory_contains_file(&extracted_data_dir) {
        return Err(BackupFileRuntimeError::bad_request(
            "备份文件缺少可恢复的 data/ 文件",
        ));
    }

    let data_parent = data_dir
        .parent()
        .ok_or_else(|| BackupFileRuntimeError::internal("data directory has no parent"))?;
    let staged_parent = TempFileBuilder::new()
        .prefix("restore_stage_")
        .tempdir_in(data_parent)?;
    let staged_data_dir = staged_parent.path().join("data");
    copy_dir_all(&extracted_data_dir, &staged_data_dir)?;

    let rollback_dir = unique_directory_path(data_parent, "data_rollback_");
    if data_dir.exists() {
        let backup_current = unique_directory_path(backup_dir, "before_restore_");
        copy_dir_all(data_dir, &backup_current)?;
        fs::rename(data_dir, &rollback_dir).map_err(|error| {
            io_context_error(
                error,
                format!(
                    "stage current data directory {}",
                    data_dir.to_string_lossy()
                ),
            )
        })?;
    }
    if let Err(error) = fs::rename(&staged_data_dir, data_dir) {
        if rollback_dir.exists() && !data_dir.exists() {
            let _ = fs::rename(&rollback_dir, data_dir);
        }
        return Err(io_context_error(
            error,
            format!("activate restored data directory {}", data_dir.display()),
        ));
    }
    if rollback_dir.exists() {
        let _ = fs::remove_dir_all(&rollback_dir);
    }
    Ok(())
}

fn validate_data_dir_for_restore(data_dir: &Path) -> FileRouteResult<()> {
    if data_dir.file_name().and_then(|value| value.to_str()) != Some("data") {
        return Err(BackupFileRuntimeError::internal(
            "Rust backup restore data dir must end with data",
        ));
    }
    if data_dir.parent().is_none() {
        return Err(BackupFileRuntimeError::internal(
            "Rust backup restore data dir must have a parent",
        ));
    }
    Ok(())
}

fn copy_dir_all(from: &Path, to: &Path) -> FileRouteResult<()> {
    copy_dir_all_filtered(from, to, &|_| false)
}

fn copy_dir_all_filtered(
    from: &Path,
    to: &Path,
    should_skip_relative: &dyn Fn(&Path) -> bool,
) -> FileRouteResult<()> {
    fs::create_dir_all(to)?;
    for entry in WalkDir::new(from) {
        let entry = entry.map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(from)
            .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if should_skip_relative(relative) {
            continue;
        }
        let target = to.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn unique_directory_path(parent: &Path, prefix: &str) -> PathBuf {
    let counter = BACKUP_FILENAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        "{}{}_{}",
        prefix,
        Local::now().format("%Y%m%d_%H%M%S_%f"),
        counter
    ))
}

fn directory_contains_file(directory: &Path) -> bool {
    directory.exists()
        && WalkDir::new(directory)
            .into_iter()
            .filter_map(Result::ok)
            .any(|entry| entry.file_type().is_file())
}

fn parse_keep_count(payload: &Value) -> Result<usize, String> {
    let Some(value) = payload.get("keep_count") else {
        return Ok(10);
    };
    let parsed = match value {
        Value::Null => 10,
        Value::Number(number) => number
            .as_i64()
            .ok_or_else(|| "keep_count must be an integer".to_string())?,
        Value::String(text) => text
            .trim()
            .parse::<i64>()
            .map_err(|_| "keep_count must be an integer".to_string())?,
        _ => return Err("keep_count must be an integer".to_string()),
    };
    if parsed < 0 {
        return Err("keep_count must be greater than or equal to 0".to_string());
    }
    usize::try_from(parsed).map_err(|_| "keep_count must be an integer".to_string())
}

fn apply_cleanup_plan(
    connection: &rusqlite::Connection,
    backup_dir: &Path,
    decisions: &[BackupCleanupDecision],
) -> FileRouteResult<()> {
    let mut seen = BTreeSet::new();
    for decision in decisions {
        if !seen.insert(decision.filename.clone()) {
            continue;
        }
        match decision.action.as_str() {
            "mark_deleted" => {
                update_backup_record_by_filename(
                    connection,
                    &decision.filename,
                    Some("deleted"),
                    json!({"deleted_reason": decision.reason, "deleted_at": now_iso()}),
                )
                .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
            }
            "delete_file_and_mark_deleted" => {
                let path = resolve_backup_path(backup_dir, &decision.filename)?;
                if path.exists() {
                    fs::remove_file(&path)?;
                }
                remove_metadata_file(&path)?;
                update_backup_record_by_filename(
                    connection,
                    &decision.filename,
                    Some("deleted"),
                    json!({"deleted_reason": decision.reason, "deleted_at": now_iso()}),
                )
                .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
            }
            "delete_stray_file" => {
                let path = resolve_backup_path(backup_dir, &decision.filename)?;
                if path.exists() {
                    fs::remove_file(&path)?;
                }
                remove_metadata_file(&path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn list_local_backup_files(backup_dir: &Path) -> FileRouteResult<Vec<PathBuf>> {
    fs::create_dir_all(backup_dir)?;
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let filename = backup_filename(&path);
        if filename.starts_with("backup_")
            && (filename.ends_with(".zip") || filename.ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX))
        {
            files.insert(filename, path);
        }
    }
    Ok(files.into_values().collect())
}

fn remove_stale_plaintext_backup_temps(backup_dir: &Path) -> FileRouteResult<()> {
    if !backup_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        let filename = physical_backup_filename(&path);
        if path.is_file()
            && filename.starts_with("bill-analyser-backup-")
            && filename.ends_with(".zip")
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn remove_metadata_file(file_path: &Path) -> FileRouteResult<()> {
    let metadata_path = backup_metadata_path(file_path);
    if metadata_path.exists() {
        fs::remove_file(metadata_path)?;
    }
    Ok(())
}

fn resolve_backup_path(backup_dir: &Path, filename: &str) -> FileRouteResult<PathBuf> {
    let resolution = resolve_backup_filename(filename).map_err(|error| {
        BackupFileRuntimeError::new(status_or_internal(error.status_code), error.message)
    })?;
    let backup_dir = backup_dir
        .canonicalize()
        .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
    let path = backup_dir.join(&resolution.sanitized);
    let path = if path.exists() {
        path
    } else if resolution
        .sanitized
        .ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX)
    {
        let storage_path =
            backup_dir.join(encrypted_storage_filename_for_public(&resolution.sanitized));
        if storage_path.exists() {
            storage_path
        } else {
            path
        }
    } else {
        path
    };
    if path.exists() {
        let canonical = path
            .canonicalize()
            .map_err(|error| BackupFileRuntimeError::internal(error.to_string()))?;
        if !canonical.starts_with(&backup_dir) {
            return Err(BackupFileRuntimeError::bad_request("无效的文件名"));
        }
        Ok(canonical)
    } else {
        Ok(path)
    }
}

fn backup_dir(config: &HttpShellConfig) -> RouteResult<PathBuf> {
    let backup_dir = PathBuf::from(&config.backup_dir);
    fs::create_dir_all(&backup_dir)
        .map_err(|error| Box::new(error_response(StatusCode::INTERNAL_SERVER_ERROR, error)))?;
    Ok(backup_dir)
}

fn data_dir(config: &HttpShellConfig) -> PathBuf {
    PathBuf::from(&config.data_dir)
}

fn backup_filename(path: &Path) -> String {
    public_backup_filename(path)
}

fn public_backup_filename(path: &Path) -> String {
    let filename = physical_backup_filename(path);
    if filename.ends_with(RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX) {
        return format!(
            "{}{}",
            filename.trim_end_matches(RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX),
            PUBLIC_ENCRYPTED_BACKUP_SUFFIX
        );
    }
    filename
}

fn physical_backup_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_string()
}

fn is_encrypted_backup_storage_path(path: &Path) -> bool {
    let filename = physical_backup_filename(path);
    filename.ends_with(PUBLIC_ENCRYPTED_BACKUP_SUFFIX)
        || filename.ends_with(RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX)
}

fn encrypted_storage_path_for_zip(file_path: &Path) -> PathBuf {
    let raw = file_path.to_string_lossy();
    PathBuf::from(format!(
        "{}{}",
        raw.trim_end_matches(".zip"),
        RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX
    ))
}

fn encrypted_storage_filename_for_public(filename: &str) -> String {
    format!(
        "{}{}",
        filename.trim_end_matches(PUBLIC_ENCRYPTED_BACKUP_SUFFIX),
        RUST_ENCRYPTED_BACKUP_STORAGE_SUFFIX
    )
}

fn file_modified_at(path: &Path) -> FileRouteResult<i64> {
    Ok(path
        .metadata()?
        .modified()
        .unwrap_or_else(|_| SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64)
}

fn system_time_iso(value: SystemTime) -> String {
    let datetime: chrono::DateTime<Local> = value.into();
    datetime.format("%Y-%m-%dT%H:%M:%S%.f").to_string()
}

fn now_iso() -> String {
    Utc::now()
        .naive_utc()
        .format("%Y-%m-%dT%H:%M:%S%.f")
        .to_string()
}

fn optional_json_body(body: &Bytes) -> Result<Value, String> {
    if body.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let payload: Value =
        serde_json::from_slice(body).map_err(|_| "invalid JSON body".to_string())?;
    if payload.is_object() {
        Ok(payload)
    } else {
        Err("JSON body must be an object".to_string())
    }
}

fn validation_audit_details(payload: &Value) -> Value {
    json!({
        "job_type": audit_safe_json_field(payload, "job_type"),
        "id": audit_safe_json_field(payload, "id"),
        "retention_days": audit_safe_json_field(payload, "retention_days"),
        "retention_count": audit_safe_json_field(payload, "retention_count"),
    })
}

fn write_backup_job_audit(
    connection: &rusqlite::Connection,
    user_id: UserId,
    headers: &HeaderMap,
    details: Value,
    affected_count: i64,
    status: &str,
    error_message: Option<String>,
) {
    write_backup_audit_event(
        connection,
        user_id,
        headers,
        "backup_job_saved",
        details,
        affected_count,
        status,
        error_message,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_backup_audit_event(
    connection: &rusqlite::Connection,
    user_id: UserId,
    headers: &HeaderMap,
    operation_type: &str,
    details: Value,
    affected_count: i64,
    status: &str,
    error_message: Option<String>,
) {
    let details = with_audit_actor(details, user_id);
    create_backup_audit_log_best_effort(
        connection,
        BackupAuditLogDraft {
            operation_type: operation_type.to_string(),
            details,
            affected_count,
            ip_address: client_ip(headers),
            user_agent: header_text(headers, "user-agent"),
            status: status.to_string(),
            error_message,
        },
    );
}

fn with_audit_actor(mut details: Value, user_id: UserId) -> Value {
    let Value::Object(object) = &mut details else {
        return json!({ "user_id": user_id.get(), "details": details });
    };
    object.insert("user_id".to_string(), json!(user_id.get()));
    details
}

fn client_ip(headers: &HeaderMap) -> Option<String> {
    let forwarded_for = header_text(headers, "x-forwarded-for");
    if let Some(value) = forwarded_for {
        let first = value.split(',').next().unwrap_or_default().trim();
        if !first.is_empty() {
            return Some(first.to_string());
        }
    }
    header_text(headers, "x-real-ip")
}

fn header_text(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn audit_safe_json_field(payload: &Value, key: &str) -> String {
    payload.get(key).map(audit_safe_value).unwrap_or_default()
}

fn audit_safe_value(value: &Value) -> String {
    let raw = value.as_str().map(str::to_string).unwrap_or_else(|| {
        if value.is_null() {
            String::new()
        } else {
            value.to_string()
        }
    });
    raw.trim().chars().take(128).collect()
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn json_string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn open_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust backup ops DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
        ))
    })?;
    let db_path = SqliteDbPath::application_file(db_path).map_err(|error| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            error.to_string(),
        ))
    })?;
    SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: state.config.timeout,
    })
    .map_err(|_| Box::new(db_error_response()))
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust backup ops route runtime DB error",
    )
}

fn auth_error_response(error: crate::auth::RustRouteAuthError) -> Response {
    error_response(status_or_internal(error.status), error.message)
}

fn db_write_error_response(error: DbError) -> Response {
    match error {
        DbError::InvalidOperation(message) if message == "backup job not found" => {
            error_response(StatusCode::NOT_FOUND, message)
        }
        _ => db_error_response(),
    }
}

fn file_error_response(error: BackupFileRuntimeError) -> Box<Response> {
    Box::new(error_response(error.status, error.message))
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({
            "success": false,
            "error": message.to_string(),
        }),
    )
}
