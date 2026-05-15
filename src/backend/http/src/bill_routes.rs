use std::{fmt::Write as _, fs, path::Path as FsPath};

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use bill_analyser_core::adapters::transaction::{
    apply_create_category_contract, apply_legacy_modify_preserved_fields,
    apply_manual_create_defaults, batch_create_persist_error_route_response,
    batch_create_prepare_error_route_response, batch_create_success_route_response,
    batch_create_transaction_items, batch_delete_success_payload, batch_update_response,
    build_reconciliation_transactions, calculate_reconciliation_summary,
    delete_bill_success_payload, frontend_transaction_from_backend,
    frontend_transaction_mutation_to_backend, frontend_transaction_type_from_backend,
    invalid_transaction_picture_file_response, is_allowed_transaction_picture_filename,
    legacy_delete_bill_success_payload, legacy_modify_bill_success_payload,
    missing_transaction_picture_file_response, missing_unused_transaction_picture_id_response,
    normalize_bill_create_aliases, parse_reconciliation_query,
    reconciliation_account_not_found_response, reconciliation_category_filters,
    reconciliation_internal_error_response, reconciliation_opening_balance,
    reconciliation_result_payload, reconciliation_type_filter,
    remove_unused_transaction_picture_success_response, serialize_optional_export_cell,
    transaction_list_type_filter, transaction_picture_data_url_from_base64,
    transaction_picture_delete_path, transaction_picture_internal_error_response,
    transaction_picture_upload_id, transaction_picture_upload_success_response,
    unsupported_transaction_picture_type_response, BackendTransactionView, FrontendTransactionTag,
    ReconciliationBill, ReconciliationCategoryRecord, ReconciliationOpeningBalanceSnapshot,
    ReconciliationQueryParams, RouteResponseContract, EXPORT_COLUMNS,
};
use bill_analyser_core::category_rules::match_rule_expression;
use bill_analyser_core::{Money, RuntimeError, UserId, UtcOffsetMinutes};
use bill_analyser_db::{
    batch_create_bills, batch_delete_bills, batch_update_bills, bind_bill_to_recurring,
    create_bill, delete_bill, get_bill_by_id, get_bill_recurring_candidates, get_bill_tags,
    get_bill_update_snapshot, get_first_account_id, list_bills, query_bills,
    unbind_bill_from_recurring, update_bill, BillCategoryFilter, BillCreateDraft, BillFilters,
    BillRecord, BillUpdateDraft, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use chrono::{DateTime, Local};
use ring::rand::{SecureRandom, SystemRandom};
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use crate::{auth::resolve_user_id_from_headers, config::HttpShellConfig, state::HttpAppState};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const DEFAULT_UTC_OFFSET_MINUTES: i32 = 480;

type RouteResult<T> = Result<T, Box<Response>>;

pub const BILL_CRUD_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/bills"),
    ("GET", "/api/bills/"),
    ("POST", "/api/bills"),
    ("POST", "/api/bills/"),
    ("GET", "/api/bills/by-month"),
    ("GET", "/api/bills/get"),
    ("GET", "/api/bills/{bill_id}"),
    ("PUT", "/api/bills/{bill_id}"),
    ("DELETE", "/api/bills/{bill_id}"),
    ("POST", "/api/bills/modify"),
    ("POST", "/api/bills/delete"),
    ("POST", "/api/bills/batch"),
    ("PUT", "/api/bills/batch/update"),
    ("DELETE", "/api/bills/batch/delete"),
    ("GET", "/api/bills/export"),
    ("GET", "/api/bills/reconciliation_statements"),
    ("POST", "/api/bills/category/quick-add-keyword"),
    ("POST", "/api/bills/category/refresh"),
    ("GET", "/api/bills/{bill_id}/recurring-candidates"),
    ("PUT", "/api/bills/{bill_id}/recurring-match"),
    ("DELETE", "/api/bills/{bill_id}/recurring-match"),
    ("POST", "/api/bills/pictures"),
    ("POST", "/api/bills/pictures/unused"),
];

pub fn bill_runtime_router() -> Router<HttpAppState> {
    Router::new()
        .route("/api/bills/export", get(export_bills_handler))
        .route(
            "/api/bills/pictures",
            post(upload_transaction_picture_handler),
        )
        .route(
            "/api/bills/pictures/unused",
            post(remove_unused_transaction_picture_handler),
        )
        .route(
            "/api/bills/reconciliation_statements",
            get(reconciliation_statements_handler),
        )
        .route(
            "/api/bills/category/quick-add-keyword",
            post(quick_add_category_keyword_handler),
        )
        .route(
            "/api/bills/category/refresh",
            post(refresh_bill_categories_handler),
        )
        .route(
            "/api/bills/:bill_id/recurring-candidates",
            get(recurring_candidates_handler),
        )
        .route(
            "/api/bills/:bill_id/recurring-match",
            put(bind_recurring_match_handler).delete(unbind_recurring_match_handler),
        )
        .route(
            "/api/bills",
            get(list_bills_handler).post(create_bill_handler),
        )
        .route(
            "/api/bills/",
            get(list_bills_handler).post(create_bill_handler),
        )
        .route("/api/bills/by-month", get(bills_by_month_handler))
        .route("/api/bills/get", get(get_bill_query_handler))
        .route("/api/bills/modify", post(legacy_modify_bill_handler))
        .route("/api/bills/delete", post(legacy_delete_bill_handler))
        .route("/api/bills/batch", post(batch_create_bills_handler))
        .route("/api/bills/batch/update", put(batch_update_bills_handler))
        .route(
            "/api/bills/batch/delete",
            delete(batch_delete_bills_handler),
        )
        .route(
            "/api/bills/:bill_id",
            get(get_bill_path_handler)
                .put(update_bill_path_handler)
                .delete(delete_bill_path_handler),
        )
}

#[derive(Debug, Default, Deserialize)]
struct BillsListQuery {
    page: Option<usize>,
    count: Option<usize>,
    page_size: Option<usize>,
    #[serde(rename = "pageSize")]
    page_size_camel: Option<usize>,
    #[serde(rename = "type")]
    transaction_type: Option<String>,
    main_category: Option<String>,
    sub_category: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    min_time: Option<i64>,
    max_time: Option<i64>,
    keyword: Option<String>,
    batch_id: Option<String>,
    counterparty: Option<String>,
    description: Option<String>,
    account_ids: Option<String>,
    #[serde(rename = "accountIds")]
    account_ids_camel: Option<String>,
    category_ids: Option<String>,
    #[serde(rename = "categoryIds")]
    category_ids_camel: Option<String>,
    tag_ids: Option<String>,
    #[serde(rename = "tagIds")]
    tag_ids_camel: Option<String>,
    amount_filter: Option<String>,
    #[serde(rename = "amountFilter")]
    amount_filter_camel: Option<String>,
}

impl BillsListQuery {
    fn page(&self) -> usize {
        self.page.unwrap_or(1).max(1)
    }

    fn page_size(&self) -> usize {
        self.page_size
            .or(self.page_size_camel)
            .or(self.count)
            .unwrap_or(20)
            .clamp(1, 500)
    }

    fn account_ids(&self) -> Vec<i64> {
        parse_csv_i64(
            self.account_ids
                .as_ref()
                .or(self.account_ids_camel.as_ref()),
        )
    }

    fn category_ids(&self) -> Vec<i64> {
        parse_csv_i64(
            self.category_ids
                .as_ref()
                .or(self.category_ids_camel.as_ref()),
        )
    }

    fn tag_ids(&self) -> Vec<i64> {
        parse_csv_i64(self.tag_ids.as_ref().or(self.tag_ids_camel.as_ref()))
    }

    fn amount_filter(&self) -> Option<String> {
        non_empty_string(
            self.amount_filter
                .as_ref()
                .or(self.amount_filter_camel.as_ref()),
        )
    }
}

#[derive(Debug, Deserialize)]
struct BillsByMonthQuery {
    year: i32,
    month: u32,
    #[serde(flatten)]
    common: BillsListQuery,
}

#[derive(Debug, Default, Deserialize)]
struct BillIdQuery {
    id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct BillsExportQuery {
    format: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ReconciliationStatementsQuery {
    account_id: Option<String>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    category_ids: Option<String>,
    #[serde(rename = "type")]
    transaction_type: Option<i64>,
    keyword: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RecurringCandidatesQuery {
    #[serde(rename = "toleranceDays")]
    tolerance_days: Option<i64>,
}

#[derive(Debug, Clone)]
struct CategoryRuleRuntimeRecord {
    id: i64,
    category_id: i64,
    main_category: String,
    sub_category: String,
    category_type: i32,
    category_priority: i32,
    rule_expression: String,
    regex_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CategoryRefreshResult {
    total: usize,
    categorized: usize,
    still_uncategorized: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CategoryRecategorizeResult {
    pub total: usize,
    pub updated: usize,
}

async fn list_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsListQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let filters = match filters_from_query(runtime.connection(), user_id, &query) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let page = query.page();
    let page_size = query.page_size();
    match query_bills(runtime.connection(), user_id, page, page_size, &filters).and_then(
        |bill_page| page_to_frontend(runtime.connection(), user_id, page, page_size, bill_page),
    ) {
        Ok(body) => json_response(StatusCode::OK, body),
        Err(_) => db_error_response(),
    }
}

async fn bills_by_month_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsByMonthQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let (start_date, end_date) = match bill_analyser_core::adapters::transaction::month_date_range(
        query.year,
        query.month,
    ) {
        Ok(value) => value,
        Err(error) => return bad_request(error.to_string()),
    };
    let mut filters = match filters_from_query(runtime.connection(), user_id, &query.common) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    filters.date_from = Some(start_date);
    filters.date_to = Some(end_date);
    match query_bills(runtime.connection(), user_id, 1, 100_000, &filters).and_then(|bill_page| {
        page_to_frontend(runtime.connection(), user_id, 1, 100_000, bill_page)
    }) {
        Ok(body) => json_response(StatusCode::OK, body),
        Err(_) => db_error_response(),
    }
}

async fn export_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillsExportQuery>,
) -> Response {
    let export_format = match BillExportFormat::normalize(query.format.as_deref()) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bills = match list_bills(runtime.connection(), user_id, &BillFilters::default()) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    if bills.is_empty() {
        return not_found("No bills to export");
    }

    match export_format {
        BillExportFormat::Csv => match render_bills_csv_export(&bills) {
            Ok(body) => {
                export_file_response("text/csv; charset=utf-8", export_filename("csv"), body)
            }
            Err(_) => db_error_response(),
        },
        BillExportFormat::Excel => export_file_response(
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            export_filename("xlsx"),
            render_bills_xlsx_export(&bills),
        ),
    }
}

async fn reconciliation_statements_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<ReconciliationStatementsQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let params = match parse_reconciliation_query(
        query.account_id.as_deref(),
        query.start_time,
        query.end_time,
        query.category_ids.as_deref(),
        query.transaction_type,
        query.keyword.as_deref(),
    ) {
        Ok(value) => value,
        Err(response) => return route_contract_response(response),
    };

    match reconciliation_statement_payload(runtime.connection(), user_id, &params) {
        Ok(Some(result)) => success_result(StatusCode::OK, result),
        Ok(None) => route_contract_response(reconciliation_account_not_found_response()),
        Err(error) => {
            route_contract_response(reconciliation_internal_error_response(error.to_string()))
        }
    }
}

async fn quick_add_category_keyword_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let payload = match required_json_object_from_body(&body, "Request body is required") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(main_category) = value_string(payload.get("main_category")) else {
        return bad_request("main_category and keyword are required");
    };
    let Some(keyword) = value_string(payload.get("keyword")) else {
        return bad_request("main_category and keyword are required");
    };
    let sub_category = value_string(payload.get("sub_category"));
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match quick_add_category_keyword(
        runtime.connection_mut(),
        user_id,
        &main_category,
        sub_category.as_deref(),
        &keyword,
    ) {
        Ok(true) => json_response(
            StatusCode::OK,
            json!({"success": true, "message": "Keyword added successfully"}),
        ),
        Ok(false) => bad_request("Failed to add keyword"),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn refresh_bill_categories_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let payload = optional_json_object_from_body(&body);
    let bill_ids = extract_bill_ids(&payload).filter(|ids| !ids.is_empty());
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match refresh_category_for_bills(runtime.connection_mut(), user_id, bill_ids.as_deref()) {
        Ok(result) => json_response(
            StatusCode::OK,
            json!({
                "success": true,
                "result": {
                    "success": true,
                    "total": result.total,
                    "categorized": result.categorized,
                    "still_uncategorized": result.still_uncategorized,
                },
            }),
        ),
        Err(error) => error_response(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

async fn recurring_candidates_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Query(query): Query<RecurringCandidatesQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let tolerance_days = query.tolerance_days.unwrap_or(3).clamp(0, 31);
    match get_bill_recurring_candidates(runtime.connection(), user_id, bill_id, tolerance_days) {
        Ok(Some(result)) => success_result(
            StatusCode::OK,
            json!({
                "billId": bill_id,
                "linkedRecurringId": result.linked_recurring_id,
                "linkedRecurringName": result.linked_recurring_name,
                "candidates": result.candidates,
            }),
        ),
        Ok(None) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

async fn bind_recurring_match_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    let Some(recurring_id) = payload.get("recurringId").and_then(value_to_positive_i64) else {
        return bad_request("Missing recurringId");
    };
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match bind_bill_to_recurring(runtime.connection_mut(), user_id, bill_id, recurring_id) {
        Ok(Some(result)) => success_result(
            StatusCode::OK,
            json!({
                "billId": result.bill_id,
                "recurringId": result.recurring_id,
                "nextScheduledDate": result.next_scheduled_date,
            }),
        ),
        Ok(None) => not_found("Bill or recurring template not found"),
        Err(_) => db_error_response(),
    }
}

async fn unbind_recurring_match_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match unbind_bill_from_recurring(runtime.connection_mut(), user_id, bill_id) {
        Ok(Some(true)) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(Some(false)) | Ok(None) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

async fn upload_transaction_picture_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return *response;
    }

    loop {
        let next_field = match multipart.next_field().await {
            Ok(value) => value,
            Err(error) => {
                return route_contract_response(transaction_picture_internal_error_response(
                    error.to_string(),
                ));
            }
        };
        let Some(field) = next_field else {
            return route_contract_response(missing_transaction_picture_file_response());
        };
        if field.name() != Some("picture") {
            continue;
        }

        let Some(filename) = field
            .file_name()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
        else {
            return route_contract_response(invalid_transaction_picture_file_response());
        };
        if !is_allowed_transaction_picture_filename(&filename) {
            return route_contract_response(unsupported_transaction_picture_type_response());
        }

        let picture_bytes = match field.bytes().await {
            Ok(value) => value,
            Err(error) => {
                return route_contract_response(transaction_picture_internal_error_response(
                    error.to_string(),
                ));
            }
        };
        let picture_uuid = match random_picture_uuid_hex() {
            Ok(value) => value,
            Err(response) => return *response,
        };
        let picture_id = match transaction_picture_upload_id(&picture_uuid, &filename) {
            Ok(value) => value,
            Err(error) => return bad_request(error.to_string()),
        };
        let upload_root = FsPath::new(&state.config.uploads_dir);
        if let Err(error) = fs::create_dir_all(upload_root) {
            return route_contract_response(transaction_picture_internal_error_response(
                error.to_string(),
            ));
        }
        let file_path = upload_root.join(&picture_id);
        if let Err(error) = fs::write(&file_path, picture_bytes.as_ref()) {
            return route_contract_response(transaction_picture_internal_error_response(
                error.to_string(),
            ));
        }

        let encoded = general_purpose::STANDARD.encode(picture_bytes.as_ref());
        let original_url = transaction_picture_data_url_from_base64(&picture_id, encoded);
        return route_contract_response(transaction_picture_upload_success_response(
            picture_id,
            original_url,
        ));
    }
}

async fn remove_unused_transaction_picture_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return *response;
    }

    let payload = if body.is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_slice(&body).unwrap_or_else(|_| Value::Object(Map::new()))
    };
    let Some(picture_id) = picture_id_from_payload(&payload) else {
        return route_contract_response(missing_unused_transaction_picture_id_response());
    };

    let file_path =
        transaction_picture_delete_path(FsPath::new(&state.config.uploads_dir), &picture_id);
    if file_path.is_file() {
        if let Err(error) = fs::remove_file(&file_path) {
            return route_contract_response(transaction_picture_internal_error_response(
                error.to_string(),
            ));
        }
    }

    route_contract_response(remove_unused_transaction_picture_success_response())
}

async fn create_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let draft = match create_draft_from_payload(runtime.connection(), user_id, &payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let bill_id = match create_bill(runtime.connection_mut(), user_id, &draft) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match get_frontend_bill(runtime.connection(), user_id, bill_id) {
        Ok(Some(value)) => success_result(StatusCode::CREATED, value),
        Ok(None) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn batch_create_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let item_objects = match batch_create_transaction_items(&payload) {
        Ok(value) => value,
        Err(error) => {
            return route_contract_response(batch_create_prepare_error_route_response(
                error.to_string(),
                0,
            ))
        }
    };
    let mut drafts = Vec::with_capacity(item_objects.len());
    for (index, item) in item_objects.iter().enumerate() {
        let item_value = Value::Object((*item).clone());
        match create_draft_from_payload(runtime.connection(), user_id, &item_value) {
            Ok(draft) => drafts.push(draft),
            Err(response) => {
                let error = response_error_text(*response)
                    .unwrap_or_else(|| "Invalid bill payload".to_string());
                return route_contract_response(batch_create_prepare_error_route_response(
                    error, index,
                ));
            }
        }
    }
    let bill_ids = match batch_create_bills(runtime.connection_mut(), user_id, &drafts) {
        Ok(value) => value,
        Err(error) => {
            return route_contract_response(batch_create_persist_error_route_response(
                error.to_string(),
                0,
                Vec::new(),
                Vec::new(),
            ))
        }
    };
    let mut items = Vec::with_capacity(bill_ids.len());
    let mut ids = Vec::with_capacity(bill_ids.len());
    for bill_id in bill_ids {
        match get_frontend_bill(runtime.connection(), user_id, bill_id) {
            Ok(Some(value)) => {
                ids.push(bill_id.to_string());
                items.push(value);
            }
            Ok(None) | Err(_) => return db_error_response(),
        }
    }
    route_contract_response(batch_create_success_route_response(items, ids))
}

async fn get_bill_query_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Query(query): Query<BillIdQuery>,
) -> Response {
    let Some(bill_id) = query.id.as_deref().and_then(parse_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    get_bill_response(state, headers, bill_id).await
}

async fn get_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    get_bill_response(state, headers, bill_id).await
}

async fn get_bill_response(state: HttpAppState, headers: HeaderMap, bill_id: i64) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match get_frontend_bill(runtime.connection(), user_id, bill_id) {
        Ok(Some(value)) => success_result(StatusCode::OK, value),
        Ok(None) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

async fn update_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if get_bill_by_id(runtime.connection(), user_id, bill_id)
        .map_err(|_| ())
        .ok()
        .flatten()
        .is_none()
    {
        return not_found("Bill not found");
    }
    let draft = match update_draft_from_payload(runtime.connection(), user_id, &payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match update_bill(runtime.connection_mut(), user_id, bill_id, &draft) {
        Ok(true) => match get_frontend_bill(runtime.connection(), user_id, bill_id) {
            Ok(Some(value)) => success_result(StatusCode::OK, value),
            Ok(None) => not_found("Bill not found"),
            Err(_) => db_error_response(),
        },
        Ok(false) => not_found("Bill not found or update failed"),
        Err(_) => db_error_response(),
    }
}

async fn legacy_modify_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let Some(bill_id) = payload.get("id").and_then(value_to_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(old_snapshot) = (match get_bill_update_snapshot(runtime.connection(), user_id, bill_id)
    {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    }) else {
        return not_found("Bill not found");
    };
    let mut draft = match update_draft_from_payload(runtime.connection(), user_id, &payload) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    apply_legacy_modify_preserved_fields(&mut draft.fields, &payload, &old_snapshot);
    match update_bill(runtime.connection_mut(), user_id, bill_id, &draft) {
        Ok(true) => json_response(StatusCode::OK, legacy_modify_bill_success_payload(bill_id)),
        Ok(false) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn delete_bill_path_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Path(bill_id): Path<i64>,
) -> Response {
    delete_bill_response(state, headers, bill_id, delete_bill_success_payload()).await
}

async fn legacy_delete_bill_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let Some(bill_id) = payload.get("id").and_then(value_to_positive_i64) else {
        return bad_request("Missing id parameter");
    };
    delete_bill_response(
        state,
        headers,
        bill_id,
        legacy_delete_bill_success_payload(),
    )
    .await
}

async fn delete_bill_response(
    state: HttpAppState,
    headers: HeaderMap,
    bill_id: i64,
    success_body: Value,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    match delete_bill(runtime.connection_mut(), user_id, bill_id) {
        Ok(true) => json_response(StatusCode::OK, success_body),
        Ok(false) => not_found("Bill not found"),
        Err(_) => db_error_response(),
    }
}

async fn batch_update_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(bill_ids) = extract_bill_ids(&payload).filter(|ids| !ids.is_empty()) else {
        return bad_request("No bill IDs provided");
    };
    let mut fields = match payload_object(&payload) {
        Ok(value) => value.clone(),
        Err(response) => return *response,
    };
    let _ = fields.remove("bill_ids");
    let _ = fields.remove("billIds");
    let _ = fields.remove("ids");
    if let Some(Value::Object(updates)) = fields.remove("updates") {
        fields = updates;
    }
    sanitize_backend_update_fields(&mut fields, &payload);
    if fields.is_empty() {
        return bad_request("No fields to update");
    }
    match batch_update_bills(runtime.connection_mut(), user_id, &bill_ids, &fields) {
        Ok(result) => success_result(
            StatusCode::OK,
            serde_json::to_value(batch_update_response(
                result.success_count,
                result.failed_count,
                result.failed_ids,
            ))
            .expect("batch update response serializes"),
        ),
        Err(error) => bad_request(error.to_string()),
    }
}

async fn batch_delete_bills_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(bill_ids) = extract_bill_ids(&payload).filter(|ids| !ids.is_empty()) else {
        return bad_request("No bill IDs provided");
    };
    match batch_delete_bills(runtime.connection_mut(), user_id, &bill_ids) {
        Ok(deleted_count) => {
            json_response(StatusCode::OK, batch_delete_success_payload(deleted_count))
        }
        Err(_) => db_error_response(),
    }
}

fn create_draft_from_payload(
    connection: &Connection,
    user_id: UserId,
    payload: &Value,
) -> RouteResult<BillCreateDraft> {
    let fallback_account_id =
        get_first_account_id(connection, user_id).map_err(|_| Box::new(db_error_response()))?;
    let (mut fields, tag_ids, category_id) = if is_frontend_mutation(payload) {
        let (mut backend_data, metadata) = frontend_transaction_mutation_to_backend(
            payload,
            UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        )
        .map_err(|error| Box::new(bad_request(error.to_string())))?;
        apply_manual_create_defaults(&mut backend_data, payload, fallback_account_id)
            .map_err(|error| Box::new(bad_request(error.to_string())))?;
        (backend_data, metadata.tag_ids, metadata.category_id)
    } else {
        let mut fields = payload_object(payload)?.clone();
        let tag_ids = extract_tag_ids(&fields)?;
        let category_id = value_string(
            fields
                .get("categoryId")
                .or_else(|| fields.get("category_id")),
        )
        .unwrap_or_default();
        strip_route_only_keys(&mut fields);
        normalize_bill_create_aliases(&mut fields);
        apply_manual_create_defaults(&mut fields, payload, fallback_account_id)
            .map_err(|error| Box::new(bad_request(error.to_string())))?;
        (fields, tag_ids, category_id)
    };

    apply_category_id(connection, user_id, &mut fields, &category_id)?;
    let category_pair = category_pair_from_fields(&fields);
    apply_create_category_contract(
        &mut fields,
        category_pair
            .as_ref()
            .map(|(main, sub)| (main.as_str(), sub.as_str())),
        None,
    );
    ensure_create_defaults(&mut fields);
    Ok(BillCreateDraft { fields, tag_ids })
}

fn update_draft_from_payload(
    connection: &Connection,
    user_id: UserId,
    payload: &Value,
) -> RouteResult<BillUpdateDraft> {
    let (mut fields, tag_ids, category_id) = if is_frontend_mutation(payload) {
        let (backend_data, metadata) = frontend_transaction_mutation_to_backend(
            payload,
            UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        )
        .map_err(|error| Box::new(bad_request(error.to_string())))?;
        (backend_data, Some(metadata.tag_ids), metadata.category_id)
    } else {
        let mut fields = payload_object(payload)?.clone();
        let tag_ids = tag_ids_for_update(&fields)?;
        let category_id = value_string(
            fields
                .get("categoryId")
                .or_else(|| fields.get("category_id")),
        )
        .unwrap_or_default();
        strip_route_only_keys(&mut fields);
        sanitize_backend_update_fields(&mut fields, payload);
        (fields, tag_ids, category_id)
    };
    apply_category_id(connection, user_id, &mut fields, &category_id)?;
    Ok(BillUpdateDraft { fields, tag_ids })
}

fn filters_from_query(
    connection: &Connection,
    user_id: UserId,
    query: &BillsListQuery,
) -> RouteResult<BillFilters> {
    let category_ids = query.category_ids();
    let categories = if category_ids.is_empty() {
        Vec::new()
    } else {
        category_filters_for_ids(connection, user_id, &category_ids)
            .map_err(|_| Box::new(db_error_response()))?
    };
    Ok(BillFilters {
        date_from: query
            .start_date
            .clone()
            .or_else(|| query.min_time.and_then(date_from_timestamp)),
        date_to: query
            .end_date
            .clone()
            .or_else(|| query.max_time.and_then(date_from_timestamp)),
        transaction_type: transaction_list_type_filter(query.transaction_type.as_deref()),
        main_category: non_empty_string(query.main_category.as_ref()),
        sub_category: non_empty_string(query.sub_category.as_ref()),
        batch_id: non_empty_string(query.batch_id.as_ref()),
        counterparty: non_empty_string(query.counterparty.as_ref()),
        description: non_empty_string(query.description.as_ref()),
        keyword: non_empty_string(query.keyword.as_ref()),
        account_ids: query.account_ids(),
        categories,
        tag_ids: query.tag_ids(),
        amount_filter: query.amount_filter(),
        ..BillFilters::default()
    })
}

struct ReconciliationAccountRow {
    name: String,
    initial_balance: Money,
}

const RECONCILIATION_QUERY_LIMIT: usize = 10_000;
const RECONCILIATION_QUERY_PAGE_SIZE: usize = 500;

fn reconciliation_statement_payload(
    connection: &Connection,
    user_id: UserId,
    params: &ReconciliationQueryParams,
) -> bill_analyser_db::DbResult<Option<Value>> {
    let Some(account) = load_reconciliation_account(connection, user_id, params.account_id_int)?
    else {
        return Ok(None);
    };
    let categories = load_reconciliation_category_records(connection, user_id)?;
    let category_filters =
        reconciliation_category_filters(params.category_ids.as_deref(), &categories);
    let filters = BillFilters {
        date_from: params.start_date.clone(),
        date_to: params.end_date.clone(),
        transaction_type: reconciliation_type_filter(params.transaction_type_code)
            .map(ToString::to_string),
        keyword: non_empty_string(params.keyword.as_ref()),
        account_ids: vec![params.account_id_int],
        categories: category_filters
            .iter()
            .map(|category| BillCategoryFilter {
                main: category.main.clone(),
                sub: non_empty_string(Some(&category.sub)),
            })
            .collect(),
        ..BillFilters::default()
    };
    let bills = query_reconciliation_bill_records(connection, user_id, &filters)?;
    let opening_snapshots = load_reconciliation_opening_snapshots(connection, user_id, params)?;
    let opening_balance =
        reconciliation_opening_balance(params, account.initial_balance, &opening_snapshots);
    let reconciliation_bills = bills
        .iter()
        .map(record_to_reconciliation_bill)
        .collect::<bill_analyser_db::DbResult<Vec<_>>>()?;
    let summary = calculate_reconciliation_summary(
        params.account_id_int,
        opening_balance,
        &reconciliation_bills,
    )
    .map_err(runtime_error)?;
    let mut frontend_transactions = Vec::with_capacity(summary.balance_history.len());
    for bill in bills {
        let bill_id = record_i64(&bill, "id").unwrap_or_default().to_string();
        if summary.balance_history.contains_key(&bill_id) {
            frontend_transactions.push(record_to_frontend_value(connection, user_id, bill)?);
        }
    }
    let transactions =
        build_reconciliation_transactions(frontend_transactions, &summary.balance_history);
    reconciliation_result_payload(params, account.name, &summary, transactions)
        .map(Some)
        .map_err(runtime_error)
}

fn quick_add_category_keyword(
    connection: &mut Connection,
    user_id: UserId,
    main_category: &str,
    sub_category: Option<&str>,
    keyword: &str,
) -> bill_analyser_db::DbResult<bool> {
    if !table_exists(connection, "categories")? {
        return Ok(false);
    }
    let sub_category = sub_category.unwrap_or("");
    let row = connection
        .query_row(
            concat!(
                "SELECT id, COALESCE(keywords, '') FROM categories ",
                "WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3 LIMIT 1"
            ),
            params![user_id.get() as i64, main_category, sub_category],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((category_id, current_keywords)) = row else {
        return Ok(false);
    };

    let mut keyword_list = current_keywords
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if keyword_list.iter().any(|value| value == keyword) {
        return Ok(false);
    }
    keyword_list.push(keyword.to_string());
    let new_keywords = keyword_list.join(",");
    let updated = connection.execute(
        "UPDATE categories SET keywords = ?1 WHERE id = ?2 AND user_id = ?3",
        params![new_keywords, category_id, user_id.get() as i64],
    )?;
    Ok(updated > 0)
}

fn refresh_category_for_bills(
    connection: &mut Connection,
    user_id: UserId,
    bill_ids: Option<&[i64]>,
) -> bill_analyser_db::DbResult<CategoryRefreshResult> {
    let rules = load_category_runtime_rules(connection, user_id)?;
    let bills = load_category_refresh_bills(connection, user_id, bill_ids)?;
    let total = bills.len();
    let mut categorized = 0_usize;
    let mut still_uncategorized = 0_usize;

    for bill in bills {
        if let Some((main_category, sub_category)) = match_category_for_bill(&bill, &rules) {
            let bill_id = record_i64(&bill, "id").unwrap_or_default();
            let mut fields = Map::new();
            fields.insert("main_category".to_string(), Value::String(main_category));
            fields.insert("sub_category".to_string(), Value::String(sub_category));
            update_bill(
                connection,
                user_id,
                bill_id,
                &BillUpdateDraft {
                    fields,
                    tag_ids: None,
                },
            )?;
            categorized += 1;
        } else {
            still_uncategorized += 1;
        }
    }

    Ok(CategoryRefreshResult {
        total,
        categorized,
        still_uncategorized,
    })
}

pub(crate) fn recategorize_bills_with_category_rules(
    connection: &mut Connection,
    user_id: UserId,
    force: bool,
) -> bill_analyser_db::DbResult<CategoryRecategorizeResult> {
    let rules = load_category_runtime_rules(connection, user_id)?;
    let bills = load_all_category_refresh_bills(connection, user_id)?;
    let total = bills.len();
    let mut updated = 0_usize;

    for (bill_id, bill) in bills {
        if !force && !record_text(&bill, "main_category").trim().is_empty() {
            continue;
        }
        if let Some((main_category, sub_category)) = match_category_for_bill(&bill, &rules) {
            let mut fields = Map::new();
            fields.insert("main_category".to_string(), Value::String(main_category));
            fields.insert("sub_category".to_string(), Value::String(sub_category));
            let draft = BillUpdateDraft {
                fields,
                tag_ids: None,
            };
            update_bill(connection, user_id, bill_id, &draft)?;
            updated += 1;
        }
    }

    Ok(CategoryRecategorizeResult { total, updated })
}

fn load_category_runtime_rules(
    connection: &Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<CategoryRuleRuntimeRecord>> {
    if !table_exists(connection, "categories")? || !table_exists(connection, "category_rules")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(concat!(
        "SELECT cr.id, cr.category_id, c.main_category, c.sub_category, ",
        "COALESCE(c.type, 1), COALESCE(c.priority, cr.priority, 100), ",
        "COALESCE(cr.priority, 100), cr.rule_expression, COALESCE(cr.regex_enabled, 0) ",
        "FROM category_rules cr ",
        "JOIN categories c ON cr.category_id = c.id ",
        "WHERE cr.user_id = ?1 AND cr.enabled = 1 ",
        "ORDER BY COALESCE(c.priority, cr.priority, 100) ASC, c.id ASC, cr.id ASC"
    ))?;
    let rows = statement.query_map(params![user_id.get() as i64], |row| {
        Ok(CategoryRuleRuntimeRecord {
            id: row.get(0)?,
            category_id: row.get(1)?,
            main_category: row.get::<_, String>(2)?.trim().to_string(),
            sub_category: row.get::<_, String>(3)?.trim().to_string(),
            category_type: normalize_category_rule_type(row.get::<_, Option<i32>>(4)?).unwrap_or(3),
            category_priority: row.get::<_, Option<i32>>(5)?.unwrap_or(100),
            rule_expression: row.get::<_, String>(7)?,
            regex_enabled: row.get::<_, Option<i64>>(8)?.unwrap_or(0) != 0,
        })
    })?;
    let mut rules = Vec::new();
    for row in rows {
        let rule = row?;
        if rule.category_id > 0
            && !rule.main_category.is_empty()
            && !rule.sub_category.is_empty()
            && !rule.rule_expression.is_empty()
        {
            rules.push(rule);
        }
    }
    rules.sort_by_key(|rule| (rule.category_priority, rule.category_id, rule.id));
    Ok(rules)
}

fn load_category_refresh_bills(
    connection: &Connection,
    user_id: UserId,
    bill_ids: Option<&[i64]>,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    if let Some(bill_ids) = bill_ids {
        let mut bills = Vec::new();
        for bill_id in bill_ids.iter().copied().filter(|value| *value > 0) {
            if let Some(bill) = get_bill_by_id(connection, user_id, bill_id)? {
                bills.push(bill);
            }
        }
        return Ok(bills);
    }

    let mut statement = connection.prepare(concat!(
        "SELECT id, user_id, date, type, amount, counterparty, description, ",
        "payment_method, main_category, sub_category, batch_id, hash, created_at, updated_at, ",
        "source_account_id, destination_account_id, destination_amount, created_from_template, ",
        "created_from_recurring, import_history_id ",
        "FROM bills WHERE user_id = ?1 ",
        "AND (main_category IS NULL OR main_category = '') ORDER BY date DESC"
    ))?;
    let rows = statement.query_map(params![user_id.get() as i64], row_to_bill_record)?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(bills)
}

fn load_all_category_refresh_bills(
    connection: &Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<(i64, BillRecord)>> {
    let mut statement = connection.prepare(concat!(
        "SELECT id, user_id, date, type, amount, counterparty, description, ",
        "payment_method, main_category, sub_category, batch_id, hash, created_at, updated_at, ",
        "source_account_id, destination_account_id, destination_amount, created_from_template, ",
        "created_from_recurring, import_history_id ",
        "FROM bills WHERE user_id = ?1 ORDER BY date DESC"
    ))?;
    let rows = statement.query_map(params![user_id.get() as i64], |row| {
        Ok((row.get::<_, i64>(0)?, row_to_bill_record(row)?))
    })?;
    let mut bills = Vec::new();
    for row in rows {
        bills.push(row?);
    }
    Ok(bills)
}

fn row_to_bill_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<BillRecord> {
    let mut record = Map::new();
    record.insert("id".to_string(), json!(row.get::<_, i64>(0)?));
    record.insert("user_id".to_string(), json!(row.get::<_, i64>(1)?));
    record.insert("date".to_string(), json!(row.get::<_, String>(2)?));
    record.insert("type".to_string(), json!(row.get::<_, String>(3)?));
    record.insert("amount".to_string(), json!(row.get::<_, f64>(4)?));
    record.insert("counterparty".to_string(), json!(row.get::<_, String>(5)?));
    record.insert("description".to_string(), json!(row.get::<_, String>(6)?));
    record.insert(
        "payment_method".to_string(),
        json!(row.get::<_, Option<String>>(7)?.unwrap_or_default()),
    );
    record.insert(
        "main_category".to_string(),
        optional_string_value(row.get::<_, Option<String>>(8)?),
    );
    record.insert(
        "sub_category".to_string(),
        optional_string_value(row.get::<_, Option<String>>(9)?),
    );
    record.insert(
        "batch_id".to_string(),
        optional_string_value(row.get::<_, Option<String>>(10)?),
    );
    record.insert(
        "hash".to_string(),
        optional_string_value(row.get::<_, Option<String>>(11)?),
    );
    record.insert("created_at".to_string(), json!(row.get::<_, String>(12)?));
    record.insert("updated_at".to_string(), json!(row.get::<_, String>(13)?));
    record.insert(
        "source_account_id".to_string(),
        json!(row.get::<_, Option<i64>>(14)?.unwrap_or_default()),
    );
    record.insert(
        "destination_account_id".to_string(),
        json!(row.get::<_, Option<i64>>(15)?.unwrap_or_default()),
    );
    record.insert(
        "destination_amount".to_string(),
        json!(row.get::<_, Option<f64>>(16)?.unwrap_or_default()),
    );
    record.insert(
        "created_from_template".to_string(),
        optional_i64_value(row.get::<_, Option<i64>>(17)?),
    );
    record.insert(
        "created_from_recurring".to_string(),
        optional_i64_value(row.get::<_, Option<i64>>(18)?),
    );
    record.insert(
        "import_history_id".to_string(),
        optional_i64_value(row.get::<_, Option<i64>>(19)?),
    );
    Ok(record)
}

fn optional_string_value(value: Option<String>) -> Value {
    value.map(Value::String).unwrap_or(Value::Null)
}

fn optional_i64_value(value: Option<i64>) -> Value {
    value.map(|value| json!(value)).unwrap_or(Value::Null)
}

fn match_category_for_bill(
    bill: &BillRecord,
    rules: &[CategoryRuleRuntimeRecord],
) -> Option<(String, String)> {
    let type_filter = category_rule_type_filter(bill);
    let combined_text = [
        record_text(bill, "counterparty"),
        record_text(bill, "description"),
        record_text(bill, "original_category"),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(" ");

    rules
        .iter()
        .filter(|rule| type_filter.contains(&rule.category_type))
        .find(|rule| {
            if rule.category_type == 4 && !bill_is_transfer_refresh_candidate(bill) {
                return false;
            }
            match_rule_expression(&combined_text, &rule.rule_expression, rule.regex_enabled)
        })
        .map(|rule| (rule.main_category.clone(), rule.sub_category.clone()))
}

fn category_rule_type_filter(bill: &BillRecord) -> Vec<i32> {
    let bill_type = record_text(bill, "type").to_lowercase();
    let amount = record_f64(bill, "amount").unwrap_or_default();
    let suppress_investment = bill_analyser_core::is_ordinary_bank_interest_income(bill, None);

    match bill_type.as_str() {
        "支出" | "expense" | "3" => expense_category_types(suppress_investment),
        "收入" | "income" | "2" => income_category_types(suppress_investment),
        "转账" | "transfer" | "4" => vec![4],
        "投资" | "investment" | "5" => vec![5],
        _ if amount < 0.0 => expense_category_types(suppress_investment),
        _ if amount > 0.0 => income_category_types(suppress_investment),
        _ => vec![2, 3, 4, 5],
    }
}

fn expense_category_types(suppress_investment: bool) -> Vec<i32> {
    if suppress_investment {
        vec![3]
    } else {
        vec![3, 5]
    }
}

fn income_category_types(suppress_investment: bool) -> Vec<i32> {
    if suppress_investment {
        vec![2]
    } else {
        vec![2, 5]
    }
}

fn bill_is_transfer_refresh_candidate(bill: &BillRecord) -> bool {
    let bill_type = record_text(bill, "type").to_lowercase();
    matches!(bill_type.as_str(), "转账" | "transfer" | "4")
}

fn normalize_category_rule_type(value: Option<i32>) -> Option<i32> {
    match value {
        Some(1) => Some(3),
        Some(value @ 2..=5) => Some(value),
        _ => None,
    }
}

fn load_reconciliation_account(
    connection: &Connection,
    user_id: UserId,
    account_id: i64,
) -> bill_analyser_db::DbResult<Option<ReconciliationAccountRow>> {
    let row = connection
        .query_row(
            "SELECT name, initial_balance FROM accounts WHERE id = ?1 AND user_id = ?2",
            params![account_id, user_id.get() as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<f64>>(1)?.unwrap_or(0.0),
                ))
            },
        )
        .optional()?;
    row.map(|(name, initial_balance)| {
        Ok(ReconciliationAccountRow {
            name,
            initial_balance: Money::from_yuan_str(&python_float_text(initial_balance))
                .map_err(runtime_error)?,
        })
    })
    .transpose()
}

fn load_reconciliation_category_records(
    connection: &Connection,
    user_id: UserId,
) -> bill_analyser_db::DbResult<Vec<ReconciliationCategoryRecord>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "SELECT id, main_category, sub_category FROM categories WHERE user_id = ?1 ORDER BY id ASC",
    )?;
    let rows = statement.query_map(params![user_id.get() as i64], |row| {
        Ok(ReconciliationCategoryRecord {
            id: row.get::<_, i64>(0)?,
            main_category: row.get::<_, String>(1)?,
            sub_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        })
    })?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row?);
    }
    Ok(categories)
}

fn query_reconciliation_bill_records(
    connection: &Connection,
    user_id: UserId,
    filters: &BillFilters,
) -> bill_analyser_db::DbResult<Vec<BillRecord>> {
    let mut bills = Vec::new();
    let mut page = 1;
    while bills.len() < RECONCILIATION_QUERY_LIMIT {
        let remaining = RECONCILIATION_QUERY_LIMIT - bills.len();
        let page_size = remaining.min(RECONCILIATION_QUERY_PAGE_SIZE);
        let bill_page = query_bills(connection, user_id, page, page_size, filters)?;
        let fetched = bill_page.bills.len();
        let total = usize::try_from(bill_page.total.max(0)).unwrap_or(usize::MAX);
        bills.extend(bill_page.bills);
        if fetched == 0 || fetched < page_size || bills.len() >= total {
            break;
        }
        page += 1;
    }
    Ok(bills)
}

fn load_reconciliation_opening_snapshots(
    connection: &Connection,
    user_id: UserId,
    params: &ReconciliationQueryParams,
) -> bill_analyser_db::DbResult<Vec<ReconciliationOpeningBalanceSnapshot>> {
    let Some(start_date) = params.start_date.clone() else {
        return Ok(Vec::new());
    };
    let previous_page = query_bills(
        connection,
        user_id,
        1,
        1,
        &BillFilters {
            date_to: Some(start_date),
            account_ids: vec![params.account_id_int],
            ..BillFilters::default()
        },
    )?;
    let snapshot = previous_page
        .bills
        .first()
        .map(|record| optional_money_from_record(record, "account_balance"))
        .transpose()?
        .flatten()
        .map(|account_balance| ReconciliationOpeningBalanceSnapshot { account_balance });
    Ok(snapshot.into_iter().collect())
}

fn record_to_reconciliation_bill(
    record: &BillRecord,
) -> bill_analyser_db::DbResult<ReconciliationBill> {
    Ok(ReconciliationBill {
        id: record_i64(record, "id").unwrap_or_default().to_string(),
        date: record_text(record, "date"),
        transaction_type: frontend_transaction_type_from_backend(&record_text(record, "type")).ok(),
        amount: money_from_record(record, "amount")?,
        source_account_id: positive_record_i64(record, "source_account_id"),
        destination_account_id: positive_record_i64(record, "destination_account_id"),
    })
}

fn page_to_frontend(
    connection: &Connection,
    user_id: UserId,
    page: usize,
    page_size: usize,
    bill_page: bill_analyser_db::BillPage,
) -> bill_analyser_db::DbResult<Value> {
    let mut items = Vec::with_capacity(bill_page.bills.len());
    for bill in bill_page.bills {
        items.push(record_to_frontend_value(connection, user_id, bill)?);
    }
    let total = usize::try_from(bill_page.total.max(0)).unwrap_or(usize::MAX);
    Ok(json!({
        "success": true,
        "result": {
            "items": items,
            "totalCount": bill_page.total,
            "page": page,
            "pageSize": page_size,
            "total": bill_page.total,
            "page_size": page_size,
            "total_pages": total.div_ceil(page_size),
        }
    }))
}

fn get_frontend_bill(
    connection: &Connection,
    user_id: UserId,
    bill_id: i64,
) -> bill_analyser_db::DbResult<Option<Value>> {
    get_bill_by_id(connection, user_id, bill_id)?
        .map(|record| record_to_frontend_value(connection, user_id, record))
        .transpose()
}

fn record_to_frontend_value(
    connection: &Connection,
    user_id: UserId,
    record: BillRecord,
) -> bill_analyser_db::DbResult<Value> {
    let bill_id = record_i64(&record, "id").unwrap_or(0);
    let tags = get_bill_tags(connection, user_id, bill_id)?;
    let frontend_tags = tags
        .iter()
        .filter_map(frontend_tag_from_value)
        .collect::<Vec<_>>();
    let tag_ids = frontend_tags.iter().map(|tag| tag.id.clone()).collect();
    let bill = BackendTransactionView {
        id: bill_id.to_string(),
        time_sequence_id: None,
        transaction_type: frontend_transaction_type_from_backend(&record_text(&record, "type"))
            .map_err(runtime_error)?,
        category_id: category_id_for_record(connection, user_id, &record)?,
        main_category: record_text(&record, "main_category"),
        sub_category: record_text(&record, "sub_category"),
        date: record_text(&record, "date"),
        amount: money_from_record(&record, "amount")?,
        destination_amount: Some(money_from_record(&record, "destination_amount")?),
        source_account_id: positive_record_i64(&record, "source_account_id"),
        destination_account_id: positive_record_i64(&record, "destination_account_id"),
        utc_offset: UtcOffsetMinutes::new(DEFAULT_UTC_OFFSET_MINUTES),
        hide_amount: false,
        tag_ids,
        tags: frontend_tags,
        category: None,
        source_account: None,
        destination_account: None,
        description: record_text(&record, "description"),
    };
    serde_json::to_value(frontend_transaction_from_backend(&bill))
        .map_err(|error| bill_analyser_db::DbError::InvalidOperation(error.to_string()))
}

fn category_filters_for_ids(
    connection: &Connection,
    user_id: UserId,
    category_ids: &[i64],
) -> rusqlite::Result<Vec<BillCategoryFilter>> {
    if !table_exists(connection, "categories")? {
        return Ok(Vec::new());
    }
    let category_ids = category_ids
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat_n("?", category_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut params = category_ids
        .iter()
        .copied()
        .map(SqlValue::Integer)
        .collect::<Vec<_>>();
    params.push(SqlValue::Integer(user_id.get() as i64));
    let mut statement = connection.prepare(&format!(
        "SELECT main_category, sub_category FROM categories WHERE id IN ({placeholders}) AND user_id = ?"
    ))?;
    let rows = statement.query_map(params_from_iter(params), |row| {
        Ok(BillCategoryFilter {
            main: row.get::<_, String>(0)?,
            sub: row
                .get::<_, Option<String>>(1)?
                .filter(|value| !value.is_empty()),
        })
    })?;
    let mut filters = Vec::new();
    for row in rows {
        filters.push(row?);
    }
    Ok(filters)
}

fn category_id_for_record(
    connection: &Connection,
    user_id: UserId,
    record: &BillRecord,
) -> bill_analyser_db::DbResult<Option<String>> {
    if !table_exists(connection, "categories").map_err(bill_analyser_db::DbError::from)? {
        return Ok(None);
    }
    let main = record_text(record, "main_category");
    if main.trim().is_empty() {
        return Ok(None);
    }
    let sub = record_text(record, "sub_category");
    let id = connection
        .query_row(
            "SELECT id FROM categories WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3 LIMIT 1",
            params![user_id.get() as i64, main, sub],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(bill_analyser_db::DbError::from)?;
    Ok(id.map(|value| value.to_string()))
}

fn apply_category_id(
    connection: &Connection,
    user_id: UserId,
    fields: &mut Map<String, Value>,
    category_id: &str,
) -> RouteResult<()> {
    let Some(category_id) = parse_positive_i64(category_id) else {
        return Ok(());
    };
    let category = resolve_category_by_id(connection, user_id, category_id)
        .map_err(|_| Box::new(db_error_response()))?;
    if let Some((main, sub)) = category {
        fields.insert("main_category".to_string(), Value::String(main));
        fields.insert("sub_category".to_string(), Value::String(sub));
    }
    Ok(())
}

fn resolve_category_by_id(
    connection: &Connection,
    user_id: UserId,
    category_id: i64,
) -> rusqlite::Result<Option<(String, String)>> {
    if !table_exists(connection, "categories")? {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT main_category, sub_category FROM categories WHERE id = ?1 AND user_id = ?2",
            params![category_id, user_id.get() as i64],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
}

fn table_exists(connection: &Connection, table_name: &str) -> rusqlite::Result<bool> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map(|value| value.is_some())
}

fn random_picture_uuid_hex() -> RouteResult<String> {
    let rng = SystemRandom::new();
    let mut bytes = [0_u8; 16];
    rng.fill(&mut bytes).map_err(|_| {
        Box::new(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Rust bills picture runtime random generation failed",
        ))
    })?;

    let mut hex = String::with_capacity(32);
    for byte in bytes {
        write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(hex)
}

fn picture_id_from_payload(payload: &Value) -> Option<String> {
    let value = payload.as_object()?.get("id")?;
    let raw_value = match value {
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => String::new(),
    };
    let picture_id = raw_value.trim().to_string();
    (!picture_id.is_empty()).then_some(picture_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BillExportFormat {
    Csv,
    Excel,
}

impl BillExportFormat {
    fn normalize(value: Option<&str>) -> RouteResult<Self> {
        let normalized = value.unwrap_or("csv").trim().to_ascii_lowercase();
        match normalized.as_str() {
            "" | "csv" => Ok(Self::Csv),
            "excel" | "xlsx" | "xls" => Ok(Self::Excel),
            _ => Err(Box::new(bad_request("Unsupported export format"))),
        }
    }
}

fn export_filename(extension: &str) -> String {
    format!(
        "bills_export_{}.{}",
        Local::now().format("%Y%m%d_%H%M%S"),
        extension
    )
}

fn export_file_response(content_type: &'static str, filename: String, body: Vec<u8>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header(
            "content-disposition",
            format!("attachment; filename={filename}"),
        )
        .body(Body::from(body))
        .unwrap_or_else(|_| db_error_response())
}

fn render_bills_csv_export(bills: &[BillRecord]) -> Result<Vec<u8>, csv::Error> {
    let mut writer = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(Vec::new());
    writer.write_record(EXPORT_COLUMNS.iter().map(|(label, _)| *label))?;
    for bill in bills {
        writer.write_record(
            EXPORT_COLUMNS
                .iter()
                .map(|(_, key)| export_bill_value(bill, key)),
        )?;
    }
    let mut body = "\u{feff}".as_bytes().to_vec();
    body.extend(writer.into_inner().map_err(|error| error.into_error())?);
    Ok(body)
}

fn render_bills_xlsx_export(bills: &[BillRecord]) -> Vec<u8> {
    let sheet_xml = render_bills_xlsx_sheet(bills);
    let entries = [
        (
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#.as_slice(),
        ),
        (
            "_rels/.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.as_slice(),
        ),
        (
            "xl/workbook.xml",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Bills" sheetId="1" r:id="rId1"/></sheets></workbook>"#.as_slice(),
        ),
        (
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#.as_slice(),
        ),
        ("xl/worksheets/sheet1.xml", sheet_xml.as_bytes()),
    ];
    render_stored_zip(&entries)
}

fn render_bills_xlsx_sheet(bills: &[BillRecord]) -> String {
    let mut sheet = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>"#,
    );
    render_xlsx_row(
        &mut sheet,
        1,
        EXPORT_COLUMNS.iter().map(|(label, _)| label.to_string()),
    );
    for (index, bill) in bills.iter().enumerate() {
        render_xlsx_row(
            &mut sheet,
            index + 2,
            EXPORT_COLUMNS
                .iter()
                .map(|(_, key)| export_bill_value(bill, key)),
        );
    }
    sheet.push_str("</sheetData></worksheet>");
    sheet
}

fn render_xlsx_row<I>(sheet: &mut String, row_number: usize, cells: I)
where
    I: IntoIterator<Item = String>,
{
    let _ = write!(sheet, r#"<row r="{row_number}">"#);
    for (column_index, cell) in cells.into_iter().enumerate() {
        let reference = xlsx_cell_reference(column_index, row_number);
        let _ = write!(
            sheet,
            r#"<c r="{reference}" t="inlineStr"><is><t xml:space="preserve">{}</t></is></c>"#,
            xml_escape(&cell)
        );
    }
    sheet.push_str("</row>");
}

fn xlsx_cell_reference(mut column_index: usize, row_number: usize) -> String {
    let mut column = Vec::new();
    loop {
        let remainder = column_index % 26;
        column.push((b'A' + u8::try_from(remainder).unwrap_or(0)) as char);
        column_index /= 26;
        if column_index == 0 {
            break;
        }
        column_index -= 1;
    }
    column.iter().rev().collect::<String>() + &row_number.to_string()
}

fn xml_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

fn export_bill_value(bill: &BillRecord, key: &str) -> String {
    match bill.get(key) {
        Some(Value::String(text)) => serialize_optional_export_cell(key, Some(text)),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(value) => serialize_optional_export_cell(key, Some(value.to_string())),
    }
}

fn render_stored_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut body = Vec::new();
    let mut central_directory = Vec::new();
    for (name, data) in entries {
        let offset = u32::try_from(body.len()).unwrap_or(u32::MAX);
        write_zip_local_file(&mut body, name, data);
        write_zip_central_directory_file(&mut central_directory, name, data, offset);
    }
    let central_directory_offset = u32::try_from(body.len()).unwrap_or(u32::MAX);
    let central_directory_size = u32::try_from(central_directory.len()).unwrap_or(u32::MAX);
    body.extend(central_directory);
    write_zip_end_of_central_directory(
        &mut body,
        u16::try_from(entries.len()).unwrap_or(u16::MAX),
        central_directory_size,
        central_directory_offset,
    );
    body
}

fn write_zip_local_file(body: &mut Vec<u8>, name: &str, data: &[u8]) {
    write_u32_le(body, 0x0403_4b50);
    write_u16_le(body, 20);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_zip_data_descriptor(body, name, data);
    body.extend(name.as_bytes());
    body.extend(data);
}

fn write_zip_central_directory_file(
    body: &mut Vec<u8>,
    name: &str,
    data: &[u8],
    local_header_offset: u32,
) {
    write_u32_le(body, 0x0201_4b50);
    write_u16_le(body, 20);
    write_u16_le(body, 20);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_zip_data_descriptor(body, name, data);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u32_le(body, 0);
    write_u32_le(body, local_header_offset);
    body.extend(name.as_bytes());
}

fn write_zip_data_descriptor(body: &mut Vec<u8>, name: &str, data: &[u8]) {
    write_u32_le(body, crc32(data));
    let data_len = u32::try_from(data.len()).unwrap_or(u32::MAX);
    write_u32_le(body, data_len);
    write_u32_le(body, data_len);
    write_u16_le(body, u16::try_from(name.len()).unwrap_or(u16::MAX));
    write_u16_le(body, 0);
}

fn write_zip_end_of_central_directory(
    body: &mut Vec<u8>,
    entry_count: u16,
    central_directory_size: u32,
    central_directory_offset: u32,
) {
    write_u32_le(body, 0x0605_4b50);
    write_u16_le(body, 0);
    write_u16_le(body, 0);
    write_u16_le(body, entry_count);
    write_u16_le(body, entry_count);
    write_u32_le(body, central_directory_size);
    write_u32_le(body, central_directory_offset);
    write_u16_le(body, 0);
}

fn write_u16_le(body: &mut Vec<u8>, value: u16) {
    body.extend(value.to_le_bytes());
}

fn write_u32_le(body: &mut Vec<u8>, value: u32) {
    body.extend(value.to_le_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn open_runtime(state: &HttpAppState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust bills DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
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

fn user_id_from_headers(headers: &HeaderMap, config: &HttpShellConfig) -> RouteResult<UserId> {
    resolve_user_id_from_headers(headers, config, TRUSTED_USER_SECRET_HEADER).map_err(|error| {
        Box::new(error_response(
            status_or_internal(error.status),
            error.message,
        ))
    })
}

fn route_contract_response(response: RouteResponseContract) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    json_response(status, response.body)
}

fn response_error_text(response: Response) -> Option<String> {
    let status = response.status();
    if status == StatusCode::BAD_REQUEST {
        Some("Invalid bill payload".to_string())
    } else {
        None
    }
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn bad_request(message: impl ToString) -> Response {
    error_response(StatusCode::BAD_REQUEST, message)
}

fn not_found(message: impl ToString) -> Response {
    error_response(StatusCode::NOT_FOUND, message)
}

fn db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust bills route runtime DB error",
    )
}

fn error_response(status: StatusCode, message: impl ToString) -> Response {
    json_response(
        status,
        json!({ "success": false, "error": message.to_string() }),
    )
}

fn status_or_internal(status: u16) -> StatusCode {
    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

fn payload_object(payload: &Value) -> RouteResult<&Map<String, Value>> {
    payload
        .as_object()
        .filter(|object| !object.is_empty())
        .ok_or_else(|| Box::new(bad_request("No data provided")))
}

fn required_json_object_from_body(
    body: &Bytes,
    empty_message: &str,
) -> Result<Value, Box<Response>> {
    if body.is_empty() {
        return Err(Box::new(bad_request(empty_message)));
    }
    let payload = serde_json::from_slice::<Value>(body).map_err(|error| {
        Box::new(error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("JSON parse error: {error}"),
        ))
    })?;
    if payload
        .as_object()
        .filter(|object| !object.is_empty())
        .is_none()
    {
        return Err(Box::new(bad_request(empty_message)));
    }
    Ok(payload)
}

fn optional_json_object_from_body(body: &Bytes) -> Value {
    if body.is_empty() {
        return Value::Object(Map::new());
    }
    serde_json::from_slice::<Value>(body).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn is_frontend_mutation(payload: &Value) -> bool {
    payload.as_object().is_some_and(|object| {
        object.contains_key("sourceAmount")
            || object.contains_key("destinationAmount")
            || object.contains_key("sourceAccountId")
            || object.contains_key("destinationAccountId")
            || object.contains_key("categoryId")
            || object.contains_key("tagIds")
            || object.get("type").is_some_and(Value::is_number)
    })
}

fn sanitize_backend_update_fields(fields: &mut Map<String, Value>, original: &Value) {
    if !fields.contains_key("payment_method") {
        if let Some(value) = fields.remove("channel").filter(is_non_empty_value) {
            fields.insert("payment_method".to_string(), value);
        }
    }
    if !fields.contains_key("main_category") {
        if let Some(value) = fields.remove("category").filter(is_non_empty_value) {
            fields.insert("main_category".to_string(), value);
        }
    }
    if !fields.contains_key("description") {
        if let Some(value) = original
            .get("remark")
            .or_else(|| original.get("comment"))
            .and_then(|value| value_string(Some(value)))
        {
            fields.insert("description".to_string(), Value::String(value));
        }
    }
    let _ = fields.remove("remark");
    let _ = fields.remove("comment");
}

fn strip_route_only_keys(fields: &mut Map<String, Value>) {
    for key in [
        "id",
        "tagIds",
        "tag_ids",
        "tags",
        "categoryId",
        "category_id",
        "clientSessionId",
        "client_session_id",
    ] {
        let _ = fields.remove(key);
    }
}

fn ensure_create_defaults(fields: &mut Map<String, Value>) {
    fields
        .entry("payment_method".to_string())
        .or_insert_with(|| Value::String("manual".to_string()));
    fields
        .entry("destination_account_id".to_string())
        .or_insert_with(|| Value::Number(Number::from(0)));
    fields
        .entry("destination_amount".to_string())
        .or_insert_with(|| json_number(0.0));
}

fn category_pair_from_fields(fields: &Map<String, Value>) -> Option<(String, String)> {
    let main = value_string(fields.get("main_category"))?;
    if main.trim().is_empty() {
        return None;
    }
    let sub = value_string(fields.get("sub_category")).unwrap_or_default();
    Some((main, sub))
}

fn extract_tag_ids(fields: &Map<String, Value>) -> RouteResult<Vec<i64>> {
    match fields.get("tagIds").or_else(|| fields.get("tag_ids")) {
        Some(value) => tag_ids_from_value(value),
        None => Ok(Vec::new()),
    }
}

fn tag_ids_for_update(fields: &Map<String, Value>) -> RouteResult<Option<Vec<i64>>> {
    match fields.get("tagIds").or_else(|| fields.get("tag_ids")) {
        Some(value) => Ok(Some(tag_ids_from_value(value)?)),
        None => Ok(None),
    }
}

fn tag_ids_from_value(value: &Value) -> RouteResult<Vec<i64>> {
    match value {
        Value::Array(items) => Ok(items.iter().filter_map(value_to_positive_i64).collect()),
        Value::String(text) => Ok(parse_csv_i64(Some(text))),
        Value::Null => Ok(Vec::new()),
        other => value_to_positive_i64(other)
            .map(|value| vec![value])
            .ok_or_else(|| Box::new(bad_request("Invalid tag IDs"))),
    }
}

fn extract_bill_ids(payload: &Value) -> Option<Vec<i64>> {
    let object = payload.as_object()?;
    object
        .get("bill_ids")
        .or_else(|| object.get("billIds"))
        .or_else(|| object.get("ids"))
        .and_then(|value| match value {
            Value::Array(items) => Some(items.iter().filter_map(value_to_positive_i64).collect()),
            Value::String(text) => Some(parse_csv_i64(Some(text))),
            other => value_to_positive_i64(other).map(|value| vec![value]),
        })
}

fn parse_csv_i64(value: Option<&String>) -> Vec<i64> {
    value
        .map(|text| {
            text.split(',')
                .filter_map(|part| parse_positive_i64(part.trim()))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|value| *value > 0)
}

fn value_to_positive_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64().filter(|value| *value > 0),
        Value::String(text) => parse_positive_i64(text),
        _ => None,
    }
}

fn value_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(text)) => Some(text.trim().to_string()),
        Some(Value::Number(number)) => Some(number.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

fn non_empty_string(value: Option<&String>) -> Option<String> {
    value
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn record_text(record: &Map<String, Value>, key: &str) -> String {
    value_string(record.get(key)).unwrap_or_default()
}

fn record_i64(record: &Map<String, Value>, key: &str) -> Option<i64> {
    record.get(key).and_then(value_to_i64)
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn positive_record_i64(record: &Map<String, Value>, key: &str) -> Option<i64> {
    record_i64(record, key).filter(|value| *value > 0)
}

fn record_f64(record: &Map<String, Value>, key: &str) -> bill_analyser_db::DbResult<f64> {
    match record.get(key) {
        Some(Value::Number(number)) => number.as_f64().ok_or_else(|| {
            bill_analyser_db::DbError::InvalidOperation(format!("invalid numeric field: {key}"))
        }),
        Some(Value::String(text)) => text.trim().parse::<f64>().map_err(|_| {
            bill_analyser_db::DbError::InvalidOperation(format!("invalid numeric field: {key}"))
        }),
        _ => Err(bill_analyser_db::DbError::InvalidOperation(format!(
            "missing numeric field: {key}"
        ))),
    }
}

fn money_from_record(record: &Map<String, Value>, key: &str) -> bill_analyser_db::DbResult<Money> {
    let value = record_f64(record, key)?;
    Money::from_yuan_str(&python_float_text(value)).map_err(runtime_error)
}

fn optional_money_from_record(
    record: &Map<String, Value>,
    key: &str,
) -> bill_analyser_db::DbResult<Option<Money>> {
    match record.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(_) => money_from_record(record, key).map(Some),
    }
}

fn frontend_tag_from_value(value: &Value) -> Option<FrontendTransactionTag> {
    let object = value.as_object()?;
    Some(FrontendTransactionTag {
        id: value_string(object.get("id"))?,
        name: value_string(object.get("name")).unwrap_or_default(),
    })
}

fn date_from_timestamp(value: i64) -> Option<String> {
    let seconds = if value.abs() >= 100_000_000_000 {
        value / 1000
    } else {
        value
    };
    DateTime::from_timestamp(seconds, 0)
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

fn json_number(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}

fn is_non_empty_value(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::String(text) => !text.trim().is_empty(),
        _ => true,
    }
}

fn python_float_text(value: f64) -> String {
    let text = value.to_string();
    if value.is_finite() && !text.contains('.') && !text.contains('e') && !text.contains('E') {
        format!("{text}.0")
    } else {
        text
    }
}

fn runtime_error(error: RuntimeError) -> bill_analyser_db::DbError {
    bill_analyser_db::DbError::InvalidOperation(error.to_string())
}
