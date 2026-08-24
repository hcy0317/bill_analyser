//! Independent import-config handlers for later assembled-router integration.

use axum::extract::rejection::JsonRejection;
use axum::{
    extract::{rejection::QueryRejection, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use bill_analyser_core::{suggest_import_config, ImportConfigDraft};
use bill_analyser_db::{
    delete_postgres_import_config, list_postgres_import_configs, match_postgres_import_config,
    save_postgres_import_config, ImportConfigRepositoryError,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    auth::{resolve_authoritative_authenticated_user_from_headers, RustRouteAuthError},
    state::HttpAppState,
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

pub const IMPORT_CONFIG_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/bills/import/configs"),
    ("POST", "/api/bills/import/configs"),
    ("POST", "/api/bills/import/configs/match"),
    ("POST", "/api/bills/import/configs/suggest"),
    ("DELETE", "/api/bills/import/configs/{config_id}"),
];

#[derive(Debug, Deserialize)]
pub struct ImportConfigListQuery {
    pub file_format: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportConfigSaveWire {
    id: Option<i64>,
    name: String,
    file_format: String,
    description: String,
    field_mappings: Value,
    sample_headers: Vec<String>,
    date_format: String,
    delimiter: Option<String>,
    encoding: String,
    skip_rows: i32,
    has_header: bool,
    custom_rules: Value,
    is_default: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportConfigMatchRequest {
    file_format: String,
    headers: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportConfigSuggestRequest {
    file_format: String,
    headers: Vec<String>,
    sample_rows: Option<Vec<Vec<String>>>,
}

pub async fn list_import_configs_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    query: Result<Query<ImportConfigListQuery>, QueryRejection>,
) -> Response {
    let Query(query) = match query {
        Ok(query) => query,
        Err(_) => return bad_request("file_format query parameter is required"),
    };
    let auth = match authenticate(&headers, &state).await {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    let runtime = match state.open_postgres_repository_runtime("import config list") {
        Ok(runtime) => runtime,
        Err(_) => return internal_error(),
    };
    match list_postgres_import_configs(runtime.pool(), auth.user_id, &query.file_format).await {
        Ok(configs) => success_data(json!(configs)),
        Err(error) => repository_error_response(error),
    }
}

pub async fn save_import_config_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Json(value) = match payload {
        Ok(payload) => payload,
        Err(_) => return bad_request("request body must be valid JSON"),
    };
    let draft = match parse_save_request(value) {
        Ok(draft) => draft,
        Err(error) => return bad_request(error),
    };
    let auth = match authenticate(&headers, &state).await {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    let runtime = match state.open_postgres_repository_runtime("import config save") {
        Ok(runtime) => runtime,
        Err(_) => return internal_error(),
    };
    match save_postgres_import_config(runtime.pool(), auth.user_id, &draft).await {
        Ok(id) => success_data(json!({ "id": id })),
        Err(error) => repository_error_response(error),
    }
}

pub async fn match_import_config_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Json(value) = match payload {
        Ok(payload) => payload,
        Err(_) => return bad_request("request body must be valid JSON"),
    };
    let request = match parse_json_request::<ImportConfigMatchRequest>(value) {
        Ok(request) => request,
        Err(error) => return bad_request(error),
    };
    let auth = match authenticate(&headers, &state).await {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    let runtime = match state.open_postgres_repository_runtime("import config match") {
        Ok(runtime) => runtime,
        Err(_) => return internal_error(),
    };
    match match_postgres_import_config(
        runtime.pool(),
        auth.user_id,
        &request.file_format,
        &request.headers,
    )
    .await
    {
        Ok(config) => success_data(json!(config)),
        Err(error) => repository_error_response(error),
    }
}

pub async fn suggest_import_config_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    payload: Result<Json<Value>, JsonRejection>,
) -> Response {
    let Json(value) = match payload {
        Ok(payload) => payload,
        Err(_) => return bad_request("request body must be valid JSON"),
    };
    let request = match parse_json_request::<ImportConfigSuggestRequest>(value) {
        Ok(request) => request,
        Err(error) => return bad_request(error),
    };
    if let Err(error) = bill_analyser_core::normalize_import_config_format(&request.file_format) {
        return bad_request(error);
    }
    if let Err(response) = authenticate(&headers, &state).await {
        return response;
    }
    match suggest_import_config(&request.headers, request.sample_rows.as_deref()) {
        Ok(suggestion) => success_data(json!(suggestion)),
        Err(error) => bad_request(error),
    }
}

pub async fn delete_import_config_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(raw_id): Path<String>,
) -> Response {
    let id = match parse_positive_id(&raw_id) {
        Ok(id) => id,
        Err(error) => return bad_request(error),
    };
    let auth = match authenticate(&headers, &state).await {
        Ok(auth) => auth,
        Err(response) => return response,
    };
    let runtime = match state.open_postgres_repository_runtime("import config delete") {
        Ok(runtime) => runtime,
        Err(_) => return internal_error(),
    };
    match delete_postgres_import_config(runtime.pool(), auth.user_id, id).await {
        Ok(id) => success_data(json!({ "id": id, "deleted": true })),
        Err(error) => repository_error_response(error),
    }
}

#[allow(clippy::result_large_err)] // Axum handlers return this response directly on auth failure.
async fn authenticate(
    headers: &HeaderMap,
    state: &HttpAppState,
) -> Result<crate::AuthenticatedUser, Response> {
    resolve_authoritative_authenticated_user_from_headers(
        headers,
        state,
        TRUSTED_USER_SECRET_HEADER,
    )
    .await
    .map_err(auth_error_response)
}

fn parse_save_request(value: Value) -> Result<ImportConfigDraft, String> {
    const REQUIRED_KEYS: &[&str] = &[
        "name",
        "fileFormat",
        "description",
        "fieldMappings",
        "sampleHeaders",
        "dateFormat",
        "delimiter",
        "encoding",
        "skipRows",
        "hasHeader",
        "customRules",
        "isDefault",
    ];
    let object = value
        .as_object()
        .ok_or_else(|| "import config request must be a JSON object".to_string())?;
    if let Some(key) = REQUIRED_KEYS.iter().find(|key| !object.contains_key(**key)) {
        return Err(format!("missing required import config field: {key}"));
    }
    let allowed = REQUIRED_KEYS
        .iter()
        .copied()
        .chain(std::iter::once("id"))
        .collect::<std::collections::BTreeSet<_>>();
    if let Some(key) = object.keys().find(|key| !allowed.contains(key.as_str())) {
        return Err(format!("unknown or read-only import config field: {key}"));
    }
    let wire: ImportConfigSaveWire = parse_json_request(value)?;
    Ok(ImportConfigDraft {
        id: wire.id,
        name: wire.name,
        file_format: wire.file_format,
        description: wire.description,
        field_mappings: wire.field_mappings,
        sample_headers: wire.sample_headers,
        date_format: wire.date_format,
        delimiter: wire.delimiter,
        encoding: wire.encoding,
        skip_rows: wire.skip_rows,
        has_header: wire.has_header,
        custom_rules: wire.custom_rules,
        is_default: wire.is_default,
    })
}

fn parse_json_request<T>(value: Value) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(value).map_err(|error| format!("invalid request: {error}"))
}

fn parse_positive_id(raw: &str) -> Result<i64, &'static str> {
    raw.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .ok_or("import config id must be a positive integer")
}

fn repository_error_response(error: ImportConfigRepositoryError) -> Response {
    match error {
        ImportConfigRepositoryError::Validation(error) => bad_request(error),
        ImportConfigRepositoryError::NotFound => {
            error_response(StatusCode::NOT_FOUND, "Import config not found")
        }
        ImportConfigRepositoryError::Conflict => {
            error_response(StatusCode::CONFLICT, "Import config already exists")
        }
        ImportConfigRepositoryError::Database(_) => internal_error(),
    }
}

fn auth_error_response(error: RustRouteAuthError) -> Response {
    error_response(status_or_internal(error.status), error.message)
}

fn success_data(data: Value) -> Response {
    json_response(StatusCode::OK, json!({ "success": true, "data": data }))
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn internal_error() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust import config route runtime DB error",
    )
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::{routing::get, Router};
    use bill_analyser_core::ImportConfigValidationError::InvalidHeaders;

    use super::*;

    fn complete_save_payload() -> Value {
        json!({
            "name": "Monthly Card",
            "fileFormat": "csv",
            "description": "Card export",
            "fieldMappings": {"columnMapping": {"1": 0, "8": 1}},
            "sampleHeaders": ["交易时间", "金额"],
            "dateFormat": "%Y-%m-%d",
            "delimiter": null,
            "encoding": "utf-8",
            "skipRows": 0,
            "hasHeader": true,
            "customRules": {},
            "isDefault": true
        })
    }

    #[test]
    fn save_request_requires_exact_fields_and_rejects_read_only_times() {
        let draft = parse_save_request(complete_save_payload()).unwrap();
        assert_eq!(draft.name, "Monthly Card");
        assert_eq!(draft.delimiter, None);

        let mut missing = complete_save_payload();
        missing.as_object_mut().unwrap().remove("delimiter");
        assert!(parse_save_request(missing)
            .unwrap_err()
            .contains("missing required import config field: delimiter"));

        let mut read_only = complete_save_payload();
        read_only
            .as_object_mut()
            .unwrap()
            .insert("createdAt".to_string(), json!("2026-07-14T00:00:00Z"));
        assert!(parse_save_request(read_only)
            .unwrap_err()
            .contains("unknown or read-only import config field: createdAt"));
    }

    #[test]
    fn match_and_suggest_requests_reject_unknown_fields_and_bad_shapes() {
        assert!(parse_json_request::<ImportConfigMatchRequest>(json!({
            "fileFormat": "csv",
            "headers": ["time"],
            "extra": true
        }))
        .is_err());
        assert!(parse_positive_id("0").is_err());
        assert!(parse_positive_id("not-an-id").is_err());
        assert_eq!(parse_positive_id("7").unwrap(), 7);
        assert!(parse_json_request::<ImportConfigSuggestRequest>(json!({
            "fileFormat": "csv",
            "headers": ["time"],
            "sampleRows": [[1]]
        }))
        .is_err());
    }

    #[tokio::test]
    async fn response_helpers_use_current_success_data_and_error_envelopes() {
        let response = success_data(json!({"id": 7}));
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(body, json!({"success": true, "data": {"id": 7}}));

        let response = repository_error_response(ImportConfigRepositoryError::Conflict);
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let response = repository_error_response(ImportConfigRepositoryError::NotFound);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let response =
            repository_error_response(ImportConfigRepositoryError::Validation(InvalidHeaders));
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = repository_error_response(ImportConfigRepositoryError::Database(
            bill_analyser_db::DbError::InvalidOperation("test".to_string()),
        ));
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let response = auth_error_response(RustRouteAuthError {
            status: 401,
            message: "unauthorized".to_string(),
        });
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(status_or_internal(1000), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn route_handoff_has_exact_five_records() {
        assert_eq!(
            IMPORT_CONFIG_ROUTE_PATTERNS,
            [
                ("GET", "/api/bills/import/configs"),
                ("POST", "/api/bills/import/configs"),
                ("POST", "/api/bills/import/configs/match"),
                ("POST", "/api/bills/import/configs/suggest"),
                ("DELETE", "/api/bills/import/configs/{config_id}"),
            ]
        );
    }

    #[test]
    fn independent_handlers_satisfy_axum_route_contracts() {
        let _: Router<HttpAppState> = Router::new()
            .route(
                "/api/bills/import/configs",
                get(list_import_configs_handler).post(save_import_config_handler),
            )
            .route(
                "/api/bills/import/configs/match",
                axum::routing::post(match_import_config_handler),
            )
            .route(
                "/api/bills/import/configs/suggest",
                axum::routing::post(suggest_import_config_handler),
            )
            .route(
                "/api/bills/import/configs/:config_id",
                axum::routing::delete(delete_import_config_handler),
            );
    }
}
