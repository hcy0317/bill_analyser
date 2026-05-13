use std::collections::BTreeMap;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};
use bill_analyser_core::{category_rules::match_rule_expression, UserId};
use bill_analyser_db::{
    taxonomy::{
        accounts::{AccountDisplayOrder, AccountRecord, AccountsRepository},
        categories::{CategoriesRepository, CategoryRecord, CategoryStatistic},
        category_rules::{CategoryRuleRecord, CategoryRulesRepository},
        settings_bundle::export_taxonomy_sections,
        tags::{TagDisplayOrder, TagRecord, TagsRepository},
        templates::{TemplateDisplayOrder, TemplateRecord, TemplatesRepository},
    },
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Map, Number, Value};

use crate::{
    auth::resolve_user_id_from_headers,
    config::HttpShellConfig,
    proxy::{ownership_aware_proxy_handler, ProxyState},
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";
const SETTINGS_BUNDLE_SCHEMA_VERSION: i64 = 1;
const OCR_CONFIG_SETTING_KEY: &str = "receipt_ocr_config";
const SETTINGS_BUNDLE_SECTION_KEYS: &[&str] = &[
    "accounts",
    "transactionCategories",
    "transactionTags",
    "transactionTemplates",
    "scheduledTransactions",
    "categoryRecognitionRules",
    "llmConfigs",
    "ocrConfig",
];
const SENSITIVE_EXPORT_SECTIONS: &[&str] = &["llmConfigs", "ocrConfig"];

type RouteResult<T> = Result<T, Box<Response>>;

pub const TAXONOMY_ACCOUNT_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/accounts"),
    ("GET", "/api/accounts/"),
    ("POST", "/api/accounts"),
    ("POST", "/api/accounts/"),
    ("GET", "/api/accounts/{account_id}"),
    ("PUT", "/api/accounts/{account_id}"),
    ("DELETE", "/api/accounts/{account_id}"),
    ("PUT", "/api/accounts/display-orders"),
];

pub const TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("POST", "/api/accounts/{account_id}/transactions/clear"),
    ("POST", "/api/accounts/{account_id}/transactions/move"),
    ("POST", "/api/accounts/sync-balances"),
];

pub const TAXONOMY_TAG_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/tags"),
    ("GET", "/api/tags/"),
    ("POST", "/api/tags"),
    ("POST", "/api/tags/"),
    ("POST", "/api/tags/batch"),
    ("GET", "/api/tags/{tag_id}"),
    ("PUT", "/api/tags/{tag_id}"),
    ("DELETE", "/api/tags/{tag_id}"),
    ("PUT", "/api/tags/display-orders"),
];

pub const TAXONOMY_TAG_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

pub const TAXONOMY_TEMPLATE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/templates"),
    ("GET", "/api/templates/"),
    ("POST", "/api/templates"),
    ("POST", "/api/templates/"),
    ("GET", "/api/templates/{template_id}"),
    ("PUT", "/api/templates/{template_id}"),
    ("DELETE", "/api/templates/{template_id}"),
    ("PUT", "/api/templates/display-orders"),
];

pub const TAXONOMY_TEMPLATE_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

pub const TAXONOMY_CATEGORY_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/categories"),
    ("GET", "/api/categories/"),
    ("POST", "/api/categories"),
    ("POST", "/api/categories/"),
    ("GET", "/api/categories/all"),
    ("PUT", "/api/categories/all"),
    ("POST", "/api/categories/batch"),
    ("GET", "/api/categories/export"),
    ("GET", "/api/categories/flat"),
    ("POST", "/api/categories/import"),
    ("POST", "/api/categories/move"),
    ("GET", "/api/categories/statistics"),
    ("GET", "/api/categories/tree"),
    ("GET", "/api/categories/{category_id}"),
    ("PUT", "/api/categories/{category_id}"),
    ("DELETE", "/api/categories/{category_id}"),
];

pub const TAXONOMY_CATEGORY_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/categories/rules"),
    ("PUT", "/api/categories/rules"),
    ("POST", "/api/categories/update-all"),
];

pub const TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/category-rules/"),
    ("POST", "/api/category-rules/{rule_id}/test"),
];

pub const TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("POST", "/api/category-rules/"),
    ("DELETE", "/api/category-rules/{rule_id}"),
    ("PUT", "/api/category-rules/{rule_id}"),
    ("POST", "/api/category-rules/defaults"),
    ("POST", "/api/category-rules/migrate"),
    ("POST", "/api/category-rules/reorder"),
];

pub const TAXONOMY_RULE_CENTER_ROUTE_PATTERNS: &[(&str, &str)] = &[("GET", "/api/rules/overview")];

pub const TAXONOMY_RULE_CENTER_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[];

pub const TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("GET", "/api/settings/bundle/export"),
    ("GET", "/api/settings/bundle/sections/{section_key}/export"),
    ("POST", "/api/settings/bundle/sections/{section_key}/export"),
];

pub const TAXONOMY_SETTINGS_BUNDLE_PROXIED_ROUTE_PATTERNS: &[(&str, &str)] = &[
    ("POST", "/api/settings/bundle/import"),
    ("POST", "/api/settings/bundle/import/preview"),
    ("POST", "/api/settings/bundle/sections/{section_key}/import"),
    (
        "POST",
        "/api/settings/bundle/sections/{section_key}/import/preview",
    ),
];

pub fn taxonomy_runtime_router() -> Router<ProxyState> {
    Router::new()
        .route("/api/rules/overview", get(rules_overview_handler))
        .route(
            "/api/settings/bundle/export",
            get(export_settings_bundle_handler),
        )
        .route(
            "/api/settings/bundle/import",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/settings/bundle/import/preview",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/settings/bundle/sections/:section_key/export",
            get(export_settings_bundle_section_get_handler)
                .post(export_settings_bundle_section_post_handler),
        )
        .route(
            "/api/settings/bundle/sections/:section_key/import",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/settings/bundle/sections/:section_key/import/preview",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/accounts",
            get(list_accounts_handler).post(create_account_handler),
        )
        .route(
            "/api/accounts/",
            get(list_accounts_handler).post(create_account_handler),
        )
        .route(
            "/api/accounts/display-orders",
            put(update_account_display_orders_handler),
        )
        .route(
            "/api/accounts/sync-balances",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/accounts/:account_id",
            get(get_account_handler)
                .put(update_account_handler)
                .delete(delete_account_handler),
        )
        .route("/api/tags", get(list_tags_handler).post(create_tag_handler))
        .route(
            "/api/tags/",
            get(list_tags_handler).post(create_tag_handler),
        )
        .route(
            "/api/tags/display-orders",
            put(update_tag_display_orders_handler),
        )
        .route(
            "/api/tags/batch",
            axum::routing::post(batch_create_tags_handler),
        )
        .route(
            "/api/tags/:tag_id",
            get(get_tag_handler)
                .put(update_tag_handler)
                .delete(delete_tag_handler),
        )
        .route(
            "/api/templates",
            get(list_templates_handler).post(create_template_handler),
        )
        .route(
            "/api/templates/",
            get(list_templates_handler).post(create_template_handler),
        )
        .route(
            "/api/templates/display-orders",
            put(update_template_display_orders_handler),
        )
        .route(
            "/api/templates/:template_id",
            get(get_template_handler)
                .put(update_template_handler)
                .delete(delete_template_handler),
        )
        .route(
            "/api/categories",
            get(list_categories_handler).post(create_category_handler),
        )
        .route(
            "/api/categories/",
            get(list_categories_handler).post(create_category_handler),
        )
        .route("/api/categories/tree", get(list_categories_handler))
        .route("/api/categories/flat", get(flat_categories_handler))
        .route(
            "/api/categories/all",
            get(all_categories_handler).put(update_all_categories_handler),
        )
        .route(
            "/api/categories/batch",
            axum::routing::post(batch_create_categories_handler),
        )
        .route("/api/categories/export", get(export_categories_handler))
        .route(
            "/api/categories/import",
            axum::routing::post(import_categories_handler),
        )
        .route(
            "/api/categories/move",
            axum::routing::post(move_categories_handler),
        )
        .route(
            "/api/categories/rules",
            get(ownership_aware_proxy_handler).put(ownership_aware_proxy_handler),
        )
        .route(
            "/api/categories/statistics",
            get(category_statistics_handler),
        )
        .route(
            "/api/categories/update-all",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/category-rules/",
            get(list_category_rules_handler).post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/category-rules/reorder",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/category-rules/defaults",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/category-rules/migrate",
            axum::routing::post(ownership_aware_proxy_handler),
        )
        .route(
            "/api/category-rules/:rule_id",
            put(ownership_aware_proxy_handler).delete(ownership_aware_proxy_handler),
        )
        .route(
            "/api/category-rules/:rule_id/test",
            axum::routing::post(test_category_rule_handler),
        )
        .route(
            "/api/categories/:category_id",
            get(get_category_handler)
                .put(update_category_handler)
                .delete(delete_category_handler),
        )
}

#[derive(Debug, Default, Deserialize)]
struct CategoryStatisticsQuery {
    period: Option<String>,
    start_date: Option<String>,
    end_date: Option<String>,
    #[serde(rename = "type")]
    category_type: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CategoryRulesQuery {
    category_id: Option<i64>,
    enabled_only: Option<String>,
}

async fn list_accounts_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());

    match repository.list_accounts(db_user_id(user_id)) {
        Ok(accounts) => success_result(StatusCode::OK, format_account_list_response(accounts)),
        Err(_) => db_error_response(),
    }
}

async fn get_account_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());

    match load_account_with_sub_accounts(&mut repository, account_id, db_user_id(user_id)) {
        Ok(Some(account)) => success_result(
            StatusCode::OK,
            Value::Object(backend_account_to_frontend(account)),
        ),
        Ok(None) => not_found("Account not found"),
        Err(_) => db_error_response(),
    }
}

async fn create_account_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = match frontend_account_to_backend(&body) {
        Ok(value) => Value::Object(value),
        Err(message) => return bad_request(message),
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());

    let account_id = match repository.create_account(&payload, db_user_id(user_id)) {
        Ok(value) => value,
        Err(_) => return db_error_response(),
    };
    match load_account_with_sub_accounts(&mut repository, account_id, db_user_id(user_id)) {
        Ok(Some(account)) => success_result(
            StatusCode::CREATED,
            Value::Object(backend_account_to_frontend(account)),
        ),
        Ok(None) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn update_account_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let sub_accounts = body.get("subAccounts").cloned();
    let mut payload = match frontend_account_to_backend(&body) {
        Ok(value) => value,
        Err(message) => return bad_request(message),
    };
    payload.remove("subAccounts");
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    if let Some(Value::Array(sub_accounts)) = sub_accounts.filter(|value| {
        value
            .as_array()
            .is_some_and(|sub_accounts| !sub_accounts.is_empty())
    }) {
        if let Err(response) =
            update_sub_accounts(&mut repository, account_id, user_id, &sub_accounts)
        {
            return *response;
        }
    }

    match repository.update_account(account_id, &Value::Object(payload), user_id) {
        Ok(true) => match load_account_with_sub_accounts(&mut repository, account_id, user_id) {
            Ok(Some(account)) => success_result(
                StatusCode::OK,
                Value::Object(backend_account_to_frontend(account)),
            ),
            Ok(None) => success_result(StatusCode::OK, json!({})),
            Err(_) => db_error_response(),
        },
        Ok(false) => not_found("Account not found"),
        Err(_) => db_error_response(),
    }
}

async fn delete_account_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(account_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.get_sub_accounts(account_id, user_id) {
        Ok(sub_accounts) => {
            for sub_account in sub_accounts {
                if let Some(sub_account_id) = sub_account.get("id").and_then(value_as_i64) {
                    if repository.delete_account(sub_account_id, user_id).is_err() {
                        return db_error_response();
                    }
                }
            }
        }
        Err(_) => return db_error_response(),
    }

    match repository.delete_account(account_id, user_id) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Account not found"),
        Err(_) => db_error_response(),
    }
}

async fn update_account_display_orders_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(new_display_orders) = body.get("newDisplayOrders") else {
        return bad_request("Missing newDisplayOrders parameter");
    };
    let Some(items) = new_display_orders.as_array() else {
        return bad_request("newDisplayOrders must be a list");
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(account_id) = item.get("id").and_then(value_as_i64) else {
            return bad_request("Each item must have id and displayOrder");
        };
        let Some(display_order) = item.get("displayOrder").and_then(value_as_i64) else {
            return bad_request("Each item must have id and displayOrder");
        };
        orders.push(AccountDisplayOrder {
            account_id,
            display_order,
        });
    }

    let mut runtime = match open_runtime(&state, "taxonomy accounts") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = AccountsRepository::new(runtime.connection_mut());
    match repository.update_display_orders(&orders, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => db_error_response(),
        Err(_) => db_error_response(),
    }
}

async fn list_tags_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());

    match repository.list_tags(db_user_id(user_id)) {
        Ok(tags) => success_result(StatusCode::OK, format_tag_list_response(tags)),
        Err(_) => tag_db_error_response(),
    }
}

async fn get_tag_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(tag_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());

    match repository.get_tag(tag_id, db_user_id(user_id)) {
        Ok(Some(tag)) => {
            success_result(StatusCode::OK, Value::Object(backend_tag_to_frontend(tag)))
        }
        Ok(None) => not_found("Tag not found"),
        Err(_) => tag_db_error_response(),
    }
}

async fn create_tag_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !tag_name_is_present(&body) {
        return bad_request("name is required");
    }
    let payload = match frontend_tag_to_backend(&body) {
        Ok(value) => Value::Object(value),
        Err(message) => return bad_request(message),
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let tag_id = match repository.create_tag(&payload, user_id) {
        Ok(value) => value,
        Err(_) => return tag_db_error_response(),
    };
    match repository.get_tag(tag_id, user_id) {
        Ok(Some(tag)) => success_result(
            StatusCode::CREATED,
            Value::Object(backend_tag_to_frontend(tag)),
        ),
        Ok(None) => success_result(StatusCode::CREATED, json!({ "id": tag_id.to_string() })),
        Err(_) => tag_db_error_response(),
    }
}

async fn update_tag_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(tag_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let payload = match frontend_tag_to_backend(&body) {
        Ok(value) => Value::Object(value),
        Err(message) => return bad_request(message),
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.update_tag(tag_id, &payload, user_id) {
        Ok(true) => match repository.get_tag(tag_id, user_id) {
            Ok(Some(tag)) => {
                success_result(StatusCode::OK, Value::Object(backend_tag_to_frontend(tag)))
            }
            Ok(None) => success_result(StatusCode::OK, json!({ "id": tag_id.to_string() })),
            Err(_) => tag_db_error_response(),
        },
        Ok(false) => not_found("Tag not found"),
        Err(_) => tag_db_error_response(),
    }
}

async fn delete_tag_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(tag_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());

    match repository.delete_tag(tag_id, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Tag not found"),
        Err(_) => tag_db_error_response(),
    }
}

async fn update_tag_display_orders_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(new_display_orders) = body.get("newDisplayOrders") else {
        return bad_request("newDisplayOrders is required");
    };
    let Some(items) = new_display_orders.as_array() else {
        return bad_request("newDisplayOrders must be an array");
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(object) = item.as_object() else {
            return bad_request("Each item must have id and displayOrder");
        };
        if !object.contains_key("id") || !object.contains_key("displayOrder") {
            return bad_request("Each item must have id and displayOrder");
        }
        let tag_id = match object.get("id").and_then(parse_python_int) {
            Some(value) => value,
            None => return bad_request(invalid_tag_order_value_error(object.get("id"))),
        };
        let display_order = match object.get("displayOrder").and_then(parse_python_int) {
            Some(value) => value,
            None => return bad_request(invalid_tag_order_value_error(object.get("displayOrder"))),
        };
        orders.push(TagDisplayOrder {
            tag_id,
            display_order,
        });
    }

    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    match repository.update_display_orders(&orders, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => tag_db_error_response(),
        Err(_) => tag_db_error_response(),
    }
}

async fn batch_create_tags_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(tags) = body
        .get("tags")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
    else {
        return bad_request("tags is required and must be a non-empty array");
    };
    let skip_exists = body.get("skipExists").is_some_and(value_truthy);

    let mut runtime = match open_runtime(&state, "taxonomy tags") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TagsRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);
    let mut existing_by_name = match repository.list_tags(user_id) {
        Ok(existing_tags) => existing_tags
            .into_iter()
            .map(|tag| {
                let key = tag.name.trim().to_lowercase();
                let value = Value::Object(backend_tag_to_frontend(tag));
                (key, value)
            })
            .collect::<BTreeMap<_, _>>(),
        Err(_) => return tag_db_error_response(),
    };
    let mut created_tags = Vec::new();

    for item in tags {
        let Some(normalized_name) = tag_batch_normalized_name(item) else {
            return bad_request("Each tag item must contain a non-empty name");
        };
        if let Some(existing_tag) = existing_by_name.get(&normalized_name) {
            if skip_exists {
                created_tags.push(existing_tag.clone());
                continue;
            }
            return error_response(
                StatusCode::CONFLICT,
                format!("Tag already exists: {}", tag_batch_display_name(item)),
            );
        }

        let payload = match frontend_tag_to_backend(item) {
            Ok(value) => Value::Object(value),
            Err(_) => return bad_request("Each tag item must contain a non-empty name"),
        };
        let tag_id = match repository.create_tag(&payload, user_id) {
            Ok(value) => value,
            Err(_) => return tag_db_error_response(),
        };
        match repository.get_tag(tag_id, user_id) {
            Ok(Some(tag)) => {
                let tag_value = Value::Object(backend_tag_to_frontend(tag));
                existing_by_name.insert(normalized_name, tag_value.clone());
                created_tags.push(tag_value);
            }
            Ok(None) => {}
            Err(_) => return tag_db_error_response(),
        }
    }

    success_result(StatusCode::CREATED, Value::Array(created_tags))
}

async fn list_templates_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, None, None);
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.list_templates(db_user_id(user_id), template_type) {
        Ok(templates) => success_result(StatusCode::OK, format_template_list_response(templates)),
        Err(_) => template_db_error_response(),
    }
}

async fn get_template_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    Path(template_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, None, None);
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.get_template_by_id(template_id, db_user_id(user_id), template_type) {
        Ok(Some(template)) => success_result(StatusCode::OK, Value::Object(template)),
        Ok(None) => not_found("Template not found"),
        Err(_) => template_db_error_response(),
    }
}

async fn create_template_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, Some(&body), Some(1));
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let template_id = match repository.create_template(&body, user_id) {
        Ok(value) => value,
        Err(_) => return template_db_error_response(),
    };
    match repository.get_template_by_id(template_id, user_id, template_type) {
        Ok(Some(template)) => success_result(StatusCode::CREATED, Value::Object(template)),
        Ok(None) => success_result(
            StatusCode::CREATED,
            json!({ "id": template_id.to_string() }),
        ),
        Err(_) => template_db_error_response(),
    }
}

async fn update_template_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    Path(template_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, Some(&body), Some(1));
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.update_template(template_id, &body, user_id, template_type) {
        Ok(true) => match repository.get_template_by_id(template_id, user_id, template_type) {
            Ok(Some(template)) => success_result(StatusCode::OK, Value::Object(template)),
            Ok(None) => success_result(StatusCode::OK, Value::Null),
            Err(_) => template_db_error_response(),
        },
        Ok(false) => not_found("Template not found"),
        Err(_) => template_db_error_response(),
    }
}

async fn delete_template_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    Path(template_id): Path<i64>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, None, Some(1));
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.delete_template(template_id, db_user_id(user_id), template_type) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Template not found"),
        Err(_) => template_db_error_response(),
    }
}

async fn update_template_display_orders_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<BTreeMap<String, String>>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let orders = match template_display_orders_from_body(&body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let template_type = template_type_from_query_body(&query, Some(&body), Some(1)).unwrap_or(1);
    let mut runtime = match open_runtime(&state, "taxonomy templates") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = TemplatesRepository::new(runtime.connection_mut());

    match repository.update_display_orders(&orders, template_type, db_user_id(user_id)) {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => template_db_error_response(),
        Err(_) => template_db_error_response(),
    }
}

async fn list_categories_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(StatusCode::OK, format_category_tree_response(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn flat_categories_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(StatusCode::OK, format_category_flat_response(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn all_categories_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(StatusCode::OK, categories_to_value(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn update_all_categories_handler(
    headers: HeaderMap,
    State(state): State<ProxyState>,
    body: Bytes,
) -> Response {
    if let Err(response) = user_id_from_headers(&headers, &state.config) {
        return *response;
    }
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if body
        .as_object()
        .and_then(|object| object.get("categories"))
        .is_none()
    {
        return bad_request("categories are required");
    }

    json_response(
        StatusCode::OK,
        json!({ "success": true, "message": "Categories updated successfully" }),
    )
}

async fn create_category_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(name) = category_name_from_body(&body).filter(|value| !value.is_empty()) else {
        return bad_request("Category name is required");
    };
    let parent_id = value_string(body.get("parentId"), "0");
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    if parent_id == "0" {
        if let Ok(Some(existing)) = repository.get_category_by_name(&name, "", user_id) {
            return success_result_with_message(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&existing, "0")),
                "Category already exists",
            );
        }

        let category_type = body.get("type").and_then(value_as_i64).unwrap_or(1);
        let payload = Value::Object(frontend_category_to_backend(
            &body,
            &name,
            "",
            category_type,
            CategoryPayloadMode::FrontendDefaults,
        ));
        let category_id = match repository.create_category(&payload, user_id) {
            Ok(Some(value)) => value,
            Ok(None) => return category_db_error_response(),
            Err(_) => return category_db_error_response(),
        };
        return match repository.get_category_by_id(category_id, user_id) {
            Ok(Some(category)) => success_result(
                StatusCode::CREATED,
                Value::Object(backend_category_to_frontend(&category, "0")),
            ),
            Ok(None) => category_db_error_response(),
            Err(_) => category_db_error_response(),
        };
    }

    let (main_category, parent_type) =
        match resolve_parent_category(&mut repository, &parent_id, user_id) {
            Ok(Some(value)) => value,
            Ok(None) => return not_found("Parent category not found"),
            Err(_) => return category_db_error_response(),
        };
    if let Ok(Some(existing)) = repository.get_category_by_name(&main_category, &name, user_id) {
        return success_result_with_message(
            StatusCode::OK,
            Value::Object(backend_category_to_frontend(&existing, &parent_id)),
            "Category already exists",
        );
    }

    let payload = Value::Object(frontend_category_to_backend(
        &body,
        &main_category,
        &name,
        parent_type,
        CategoryPayloadMode::FrontendDefaults,
    ));
    let category_id = match repository.create_category(&payload, user_id) {
        Ok(Some(value)) => value,
        Ok(None) => return category_db_error_response(),
        Err(_) => return category_db_error_response(),
    };
    match repository.get_category_by_id(category_id, user_id) {
        Ok(Some(category)) => success_result(
            StatusCode::OK,
            Value::Object(backend_category_to_frontend(&category, &parent_id)),
        ),
        Ok(None) => category_db_error_response(),
        Err(_) => category_db_error_response(),
    }
}

async fn get_category_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if let Some(main_category_name) = virtual_category_name(&category_id) {
        return success_result(
            StatusCode::OK,
            json!({
                "id": category_id,
                "name": main_category_name,
                "parentId": "0",
                "type": 1,
                "icon": "",
                "color": "",
                "comment": "",
                "displayOrder": 0,
                "visible": true,
                "keywords": ""
            }),
        );
    }
    let category_id = match category_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid category ID"),
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    match repository.get_category_by_id(category_id, user_id) {
        Ok(Some(category)) => {
            let parent_id = category_parent_id_for_get(&mut repository, &category, user_id);
            success_result(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&category, &parent_id)),
            )
        }
        Ok(None) => not_found("Category not found"),
        Err(_) => category_db_error_response(),
    }
}

async fn update_category_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    if !body.is_object() {
        return bad_request("Invalid request");
    }
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    if let Some(old_name) = virtual_category_name(&category_id) {
        return update_virtual_category_handler(&mut repository, user_id, &old_name, &body);
    }

    let category_id = match category_id.parse::<i64>() {
        Ok(value) => value,
        Err(_) => return bad_request("Invalid category ID"),
    };
    update_real_category_handler(&mut repository, user_id, category_id, &body)
}

async fn delete_category_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    let deleted = if let Some(main_category) = virtual_category_name(&category_id) {
        repository.delete_categories_by_main_category(&main_category, user_id)
    } else {
        match category_id.parse::<i64>() {
            Ok(category_id) => repository.delete_category(category_id, user_id),
            Err(_) => return bad_request("Invalid category ID"),
        }
    };

    match deleted {
        Ok(true) => success_result(StatusCode::OK, Value::Bool(true)),
        Ok(false) => not_found("Category not found or delete failed"),
        Err(_) => category_db_error_response(),
    }
}

async fn move_categories_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(new_display_orders) = body.get("newDisplayOrders").and_then(Value::as_array) else {
        return success_result(StatusCode::OK, Value::Bool(true));
    };
    if new_display_orders.is_empty() {
        return success_result(StatusCode::OK, Value::Bool(true));
    }

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    for item in new_display_orders {
        let Some(category_id) = item.get("id").and_then(value_as_i64) else {
            continue;
        };
        let Some(display_order) = item.get("displayOrder").and_then(value_as_i64) else {
            continue;
        };
        if repository
            .update_category(category_id, &json!({ "priority": display_order }), user_id)
            .is_err()
        {
            return category_db_error_response();
        }
    }

    success_result(StatusCode::OK, Value::Bool(true))
}

async fn batch_create_categories_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match parse_json_body(body) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(categories) = body
        .as_object()
        .and_then(|object| object.get("categories"))
        .and_then(Value::as_array)
    else {
        return bad_request("No categories provided");
    };

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);

    for category in categories {
        let Some(main_name) = category_name_from_body(category).filter(|value| !value.is_empty())
        else {
            continue;
        };
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(3);
        if repository
            .get_category_by_name(&main_name, "", user_id)
            .map(|existing| existing.is_none())
            .unwrap_or(false)
        {
            let payload = Value::Object(frontend_category_to_backend(
                category,
                &main_name,
                "",
                category_type,
                CategoryPayloadMode::FrontendDefaults,
            ));
            if repository.create_category(&payload, user_id).is_err() {
                return category_db_error_response();
            }
        }

        let Some(sub_categories) = category.get("subCategories").and_then(Value::as_array) else {
            continue;
        };
        for sub_category in sub_categories {
            let Some(sub_name) =
                category_name_from_body(sub_category).filter(|value| !value.is_empty())
            else {
                continue;
            };
            let sub_type = sub_category
                .get("type")
                .and_then(value_as_i64)
                .unwrap_or(category_type);
            if repository
                .get_category_by_name(&main_name, &sub_name, user_id)
                .map(|existing| existing.is_none())
                .unwrap_or(false)
            {
                let payload = Value::Object(frontend_category_to_backend(
                    sub_category,
                    &main_name,
                    &sub_name,
                    sub_type,
                    CategoryPayloadMode::FrontendDefaults,
                ));
                if repository.create_category(&payload, user_id).is_err() {
                    return category_db_error_response();
                }
            }
        }
    }

    match repository.list_categories(user_id) {
        Ok(categories) => success_result(StatusCode::OK, format_category_tree_response(categories)),
        Err(_) => category_db_error_response(),
    }
}

async fn export_categories_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.list_categories(db_user_id(user_id)) {
        Ok(categories) => success_result(
            StatusCode::OK,
            Value::Array(
                categories
                    .iter()
                    .map(category_export_record)
                    .map(Value::Object)
                    .collect(),
            ),
        ),
        Err(_) => category_db_error_response(),
    }
}

async fn category_statistics_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<CategoryStatisticsQuery>,
) -> Response {
    let _ignored_legacy_filters = (query.period.as_deref(), query.category_type.as_deref());
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());

    match repository.category_statistics(
        query.start_date.as_deref(),
        query.end_date.as_deref(),
        db_user_id(user_id),
    ) {
        Ok(statistics) => success_result(
            StatusCode::OK,
            format_category_statistics_response(statistics),
        ),
        Err(_) => category_db_error_response(),
    }
}

async fn list_category_rules_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Query(query): Query<CategoryRulesQuery>,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.list_rules(
        db_user_id(user_id),
        query.category_id,
        category_rules_enabled_only(&query),
    ) {
        Ok(rules) => json_response(StatusCode::OK, format_category_rules_response(rules)),
        Err(_) => category_rule_db_error_response(),
    }
}

async fn test_category_rule_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(rule_id): Path<i64>,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "text is required") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let Some(text) = body.get("text").and_then(Value::as_str) else {
        return bad_request("text is required");
    };

    let mut runtime = match open_runtime(&state, "taxonomy category rules") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoryRulesRepository::new(runtime.connection_mut());

    match repository.get_rule(rule_id, db_user_id(user_id)) {
        Ok(Some(rule)) => {
            let rule_expression = string_or_default(rule.get("rule_expression"), "");
            let regex_enabled = rule.get("regex_enabled").is_some_and(value_truthy);
            json_response(
                StatusCode::OK,
                json!({
                    "success": true,
                    "data": {
                        "matched": match_rule_expression(text, &rule_expression, regex_enabled)
                    }
                }),
            )
        }
        Ok(None) => not_found("Rule not found"),
        Err(_) => category_rule_db_error_response(),
    }
}

async fn rules_overview_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy rules overview") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = db_user_id(user_id);

    let learning_count =
        count_rules_overview_learning_rules(runtime.connection_mut(), user_id).unwrap_or(0);
    let learning_rules =
        list_rules_overview_learning_rules(runtime.connection_mut(), user_id).unwrap_or_default();
    let category_rule_count = {
        let mut repository = CategoryRulesRepository::new(runtime.connection_mut());
        repository
            .list_rules(user_id, None, true)
            .map(|rules| rules.len() as i64)
            .unwrap_or(0)
    };
    let recurring_rules =
        list_rules_overview_recurring_rules(runtime.connection_mut(), user_id).unwrap_or_default();
    let recurring_rule_count = recurring_rules.len() as i64;

    json_response(
        StatusCode::OK,
        json!({
            "success": true,
            "data": {
                "learningRules": learning_rules,
                "learningRuleCount": learning_count,
                "categoryRuleCount": category_rule_count,
                "recurringRules": recurring_rules,
                "recurringRuleCount": recurring_rule_count,
                "totalRuleCount": learning_count + category_rule_count + recurring_rule_count,
            }
        }),
    )
}

async fn export_settings_bundle_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy settings bundle export") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match build_settings_bundle(runtime.connection_mut(), db_user_id(user_id)) {
        Ok(bundle) => settings_bundle_download_response(bundle, "bill-analyser-settings.json"),
        Err(_) => settings_bundle_db_error_response(),
    }
}

async fn export_settings_bundle_section_get_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(section_key): Path<String>,
) -> Response {
    if !is_valid_settings_bundle_section(&section_key) {
        return settings_bundle_section_not_found(&section_key);
    }
    if is_sensitive_settings_export_section(&section_key) {
        return bad_request("password is required");
    }
    export_settings_bundle_section(&state, &headers, &section_key)
}

async fn export_settings_bundle_section_post_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    Path(section_key): Path<String>,
    body: Bytes,
) -> Response {
    if !is_valid_settings_bundle_section(&section_key) {
        return settings_bundle_section_not_found(&section_key);
    }

    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state, "taxonomy settings bundle export") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let user_id = db_user_id(user_id);

    if is_sensitive_settings_export_section(&section_key) {
        let payload = optional_json_body(body);
        let password = payload
            .as_ref()
            .and_then(|value| value.get("password"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        if password.is_empty() {
            return bad_request("password is required");
        }
        match verify_sensitive_export_password(runtime.connection_mut(), user_id, password) {
            Ok(true) => {}
            Ok(false) => {
                return json_response(
                    StatusCode::UNAUTHORIZED,
                    json!({"success": false, "error": "Invalid password"}),
                )
            }
            Err(_) => return settings_bundle_db_error_response(),
        }
    }

    match build_settings_bundle(runtime.connection_mut(), user_id) {
        Ok(bundle) => settings_bundle_download_response(
            filter_settings_bundle_section(&bundle, &section_key),
            &format!("bill-analyser-settings-{section_key}.json"),
        ),
        Err(_) => settings_bundle_db_error_response(),
    }
}

fn export_settings_bundle_section(
    state: &ProxyState,
    headers: &HeaderMap,
    section_key: &str,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(state, "taxonomy settings bundle export") {
        Ok(value) => value,
        Err(response) => return *response,
    };

    match build_settings_bundle(runtime.connection_mut(), db_user_id(user_id)) {
        Ok(bundle) => settings_bundle_download_response(
            filter_settings_bundle_section(&bundle, section_key),
            &format!("bill-analyser-settings-{section_key}.json"),
        ),
        Err(_) => settings_bundle_db_error_response(),
    }
}

async fn import_categories_handler(
    State(state): State<ProxyState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let body = match required_json_body(body, "No data provided") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let categories = if let Some(object) = body.as_object() {
        object
            .get("categories")
            .or_else(|| object.get("result"))
            .unwrap_or(&body)
    } else {
        &body
    };
    let Some(categories) = categories.as_array() else {
        return bad_request("Invalid format, expected list of categories");
    };

    let mut runtime = match open_runtime(&state, "taxonomy categories") {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut repository = CategoriesRepository::new(runtime.connection_mut());
    let user_id = db_user_id(user_id);
    let mut imported = 0;
    let mut updated = 0;
    let mut skipped = 0;

    for category in categories {
        let main_category = string_or_default(category.get("main_category"), "");
        if main_category.trim().is_empty() {
            skipped += 1;
            continue;
        }
        let sub_category = string_or_default(category.get("sub_category"), "");
        let payload = Value::Object(import_category_payload(
            category,
            &main_category,
            &sub_category,
        ));
        match repository.get_category_by_name(&main_category, &sub_category, user_id) {
            Ok(Some(existing)) => {
                if let Some(category_id) = existing.get("id").and_then(value_as_i64) {
                    if repository
                        .update_category(category_id, &payload, user_id)
                        .is_err()
                    {
                        return category_db_error_response();
                    }
                }
                updated += 1;
            }
            Ok(None) => {
                if repository.create_category(&payload, user_id).is_err() {
                    return category_db_error_response();
                }
                imported += 1;
            }
            Err(_) => return category_db_error_response(),
        }
    }

    success_result(
        StatusCode::OK,
        json!({ "imported": imported, "updated": updated, "skipped": skipped }),
    )
}

fn update_sub_accounts(
    repository: &mut AccountsRepository<'_>,
    account_id: i64,
    user_id: i64,
    sub_accounts: &[Value],
) -> RouteResult<()> {
    let existing_sub_accounts = repository
        .get_sub_accounts(account_id, user_id)
        .map_err(|_| Box::new(db_error_response()))?;
    let existing_sub_ids = existing_sub_accounts
        .iter()
        .filter_map(|account| account.get("id").and_then(value_as_i64))
        .collect::<Vec<_>>();
    let mut updated_sub_ids = Vec::new();

    for sub_account in sub_accounts {
        let sub_account_id = sub_account.get("id").and_then(value_as_i64);
        let mut sub_payload = frontend_account_to_backend(sub_account)
            .map_err(|message| Box::new(bad_request(message)))?;
        if let Some(sub_account_id) =
            sub_account_id.filter(|candidate| existing_sub_ids.contains(candidate))
        {
            repository
                .update_account(sub_account_id, &Value::Object(sub_payload), user_id)
                .map_err(|_| Box::new(db_error_response()))?;
            updated_sub_ids.push(sub_account_id);
            continue;
        }

        sub_payload.insert(
            "parent_id".to_string(),
            Value::Number(Number::from(account_id)),
        );
        repository
            .create_account(&Value::Object(sub_payload), user_id)
            .map_err(|_| Box::new(db_error_response()))?;
    }

    for old_sub_id in existing_sub_ids {
        if !updated_sub_ids.contains(&old_sub_id) {
            repository
                .delete_account(old_sub_id, user_id)
                .map_err(|_| Box::new(db_error_response()))?;
        }
    }
    Ok(())
}

fn load_account_with_sub_accounts(
    repository: &mut AccountsRepository<'_>,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<AccountRecord>> {
    let Some(mut account) = repository.get_account(account_id, user_id)? else {
        return Ok(None);
    };
    let sub_accounts = repository.get_sub_accounts(account_id, user_id)?;
    if !sub_accounts.is_empty() {
        account.insert(
            "subAccounts".to_string(),
            Value::Array(sub_accounts.into_iter().map(Value::Object).collect()),
        );
    }
    Ok(Some(account))
}

fn format_account_list_response(accounts: Vec<AccountRecord>) -> Value {
    let formatted = accounts
        .into_iter()
        .map(backend_account_to_frontend)
        .collect::<Vec<_>>();
    let mut children_by_parent: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut top_level = Vec::new();

    for account in formatted {
        let parent_id = account
            .get("parentId")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        if parent_id.is_empty() || parent_id == "0" {
            top_level.push(account);
        } else {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(Value::Object(account));
        }
    }

    for account in &mut top_level {
        if let Some(account_id) = account
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            if let Some(children) = children_by_parent.remove(&account_id) {
                if !children.is_empty() {
                    account.insert("subAccounts".to_string(), Value::Array(children));
                }
            }
        }
    }

    Value::Array(top_level.into_iter().map(Value::Object).collect())
}

fn frontend_account_to_backend(payload: &Value) -> Result<Map<String, Value>, String> {
    let Some(object) = payload.as_object() else {
        return Err("Account payload must be an object".to_string());
    };
    let balance_cents = object
        .get("balance")
        .or_else(|| object.get("initial_balance"))
        .and_then(value_as_f64)
        .unwrap_or_default();
    let balance_yuan = round2(balance_cents / 100.0);
    let hidden = object
        .get("hidden")
        .map(value_truthy)
        .unwrap_or_else(|| !object.get("visible").map(value_truthy).unwrap_or(true));

    let mut result = Map::new();
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(object.get("name"), "")),
    );
    result.insert(
        "parent_id".to_string(),
        Value::Number(Number::from(
            object
                .get("parentId")
                .or_else(|| object.get("parent_id"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert(
        "category".to_string(),
        object.get("category").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "type".to_string(),
        object
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(object.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(object.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(object.get("currency"), "CNY")),
    );
    result.insert("balance".to_string(), json_number(balance_yuan));
    result.insert("initial_balance".to_string(), json_number(balance_yuan));
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(object.get("comment"), "")),
    );
    result.insert(
        "aliases".to_string(),
        Value::String(
            serde_json::to_string(&parse_aliases(object.get("aliases")))
                .unwrap_or_else(|_| "[]".to_string()),
        ),
    );
    result.insert(
        "display_order".to_string(),
        Value::Number(Number::from(
            object
                .get("displayOrder")
                .or_else(|| object.get("display_order"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    if let Some(Value::Array(sub_accounts)) = object.get("subAccounts") {
        let converted_sub_accounts = sub_accounts
            .iter()
            .map(frontend_account_to_backend)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(Value::Object)
            .collect();
        result.insert(
            "subAccounts".to_string(),
            Value::Array(converted_sub_accounts),
        );
    }
    if let Some(statement_date) = object
        .get("creditCardStatementDate")
        .or_else(|| object.get("credit_card_statement_date"))
    {
        result.insert(
            "credit_card_statement_date".to_string(),
            statement_date.clone(),
        );
    }
    Ok(result)
}

fn backend_account_to_frontend(mut account: AccountRecord) -> Map<String, Value> {
    let hidden = account.get("hidden").map(value_truthy).unwrap_or(false);
    let sub_accounts = account.remove("subAccounts");
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(account.get("id"), "")),
    );
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(account.get("name"), "")),
    );
    result.insert(
        "parentId".to_string(),
        Value::String(value_string(account.get("parent_id"), "0")),
    );
    result.insert(
        "category".to_string(),
        account
            .get("category")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "type".to_string(),
        account
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(account.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(account.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(account.get("currency"), "CNY")),
    );
    result.insert(
        "balance".to_string(),
        Value::Number(Number::from(yuan_to_cents(account.get("balance")))),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(account.get("comment"), "")),
    );
    result.insert(
        "aliases".to_string(),
        Value::Array(
            parse_aliases(account.get("aliases"))
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    result.insert(
        "creditCardStatementDate".to_string(),
        account
            .get("credit_card_statement_date")
            .cloned()
            .unwrap_or(Value::Null),
    );
    result.insert(
        "displayOrder".to_string(),
        account
            .get("display_order")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));

    if let Some(Value::Array(sub_accounts)) = sub_accounts.filter(|value| {
        value
            .as_array()
            .is_some_and(|sub_accounts| !sub_accounts.is_empty())
    }) {
        result.insert(
            "subAccounts".to_string(),
            Value::Array(
                sub_accounts
                    .into_iter()
                    .filter_map(|sub_account| {
                        sub_account
                            .as_object()
                            .cloned()
                            .map(backend_account_to_frontend)
                            .map(Value::Object)
                    })
                    .collect(),
            ),
        );
    }

    result
}

fn format_tag_list_response(tags: Vec<TagRecord>) -> Value {
    Value::Array(
        tags.into_iter()
            .map(backend_tag_to_frontend)
            .map(Value::Object)
            .collect(),
    )
}

fn format_template_list_response(templates: Vec<TemplateRecord>) -> Value {
    Value::Array(templates.into_iter().map(Value::Object).collect())
}

fn backend_tag_to_frontend(tag: TagRecord) -> Map<String, Value> {
    let hidden = tag.hidden != 0;
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(tag.id.to_string()));
    result.insert("name".to_string(), Value::String(tag.name));
    result.insert(
        "color".to_string(),
        tag.color.map_or(Value::Null, Value::String),
    );
    result.insert(
        "icon".to_string(),
        tag.icon.map_or(Value::Null, Value::String),
    );
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(tag.display_order)),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));
    result
}

fn update_virtual_category_handler(
    repository: &mut CategoriesRepository<'_>,
    user_id: i64,
    old_name: &str,
    body: &Value,
) -> Response {
    let new_name = category_name_from_body(body).unwrap_or_else(|| old_name.to_string());
    let mut renamed_group = false;

    if new_name != old_name {
        let real_categories = match repository.list_categories(user_id) {
            Ok(value) => value,
            Err(_) => return category_db_error_response(),
        };
        let has_old_group = real_categories
            .iter()
            .any(|category| category_text(category, "main_category") == old_name);
        let has_target_group = real_categories
            .iter()
            .any(|category| category_text(category, "main_category") == new_name);
        if has_old_group && has_target_group {
            return error_response(StatusCode::CONFLICT, "Category rename conflict");
        }
        match repository.update_main_category_name(old_name, &new_name, user_id) {
            Ok(true) => renamed_group = has_old_group,
            Ok(false) if has_old_group => {
                return error_response(StatusCode::CONFLICT, "Category rename conflict");
            }
            Ok(false) => {}
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to rename category",
                )
            }
        }
    }

    let updates = Value::Object(virtual_category_update_payload(body, &new_name));
    let category_id = match repository.get_category_by_name(&new_name, "", user_id) {
        Ok(Some(existing)) => {
            let Some(category_id) = existing.get("id").and_then(value_as_i64) else {
                return category_db_error_response();
            };
            match repository.update_category(category_id, &updates, user_id) {
                Ok(true) => category_id,
                Ok(false) => match repository.get_category_by_name(&new_name, "", user_id) {
                    Ok(Some(refreshed)) => {
                        let Some(category_id) = refreshed.get("id").and_then(value_as_i64) else {
                            return category_db_error_response();
                        };
                        match repository.update_category(category_id, &updates, user_id) {
                            Ok(true) => category_id,
                            Ok(false) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                            Err(_) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        rollback_category_rename(
                            repository,
                            renamed_group,
                            &new_name,
                            old_name,
                            user_id,
                        );
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                    Err(_) => {
                        rollback_category_rename(
                            repository,
                            renamed_group,
                            &new_name,
                            old_name,
                            user_id,
                        );
                        return category_db_error_response();
                    }
                },
                Err(_) => {
                    rollback_category_rename(
                        repository,
                        renamed_group,
                        &new_name,
                        old_name,
                        user_id,
                    );
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to save category",
                    );
                }
            }
        }
        Ok(None) => {
            let create_payload = Value::Object(virtual_category_create_payload(body, &new_name));
            match repository.create_category(&create_payload, user_id) {
                Ok(Some(category_id)) => category_id,
                Ok(None) => match repository.get_category_by_name(&new_name, "", user_id) {
                    Ok(Some(refreshed)) => {
                        let Some(category_id) = refreshed.get("id").and_then(value_as_i64) else {
                            return category_db_error_response();
                        };
                        match repository.update_category(category_id, &updates, user_id) {
                            Ok(true) => category_id,
                            Ok(false) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                            Err(_) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        rollback_category_rename(
                            repository,
                            renamed_group,
                            &new_name,
                            old_name,
                            user_id,
                        );
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                    Err(_) => return category_db_error_response(),
                },
                Err(_) => {
                    rollback_category_rename(
                        repository,
                        renamed_group,
                        &new_name,
                        old_name,
                        user_id,
                    );
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to save category",
                    );
                }
            }
        }
        Err(_) => return category_db_error_response(),
    };

    match repository.get_category_by_id(category_id, user_id) {
        Ok(Some(category)) => success_result(
            StatusCode::OK,
            Value::Object(backend_category_to_frontend(&category, "0")),
        ),
        Ok(None) => {
            rollback_category_rename(repository, renamed_group, &new_name, old_name, user_id);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load updated category",
            )
        }
        Err(_) => category_db_error_response(),
    }
}

fn update_real_category_handler(
    repository: &mut CategoriesRepository<'_>,
    user_id: i64,
    category_id: i64,
    body: &Value,
) -> Response {
    let mut updates = category_update_payload_from_frontend(body);
    let mut renamed_main_category = false;
    let mut original_main_category = String::new();
    let mut renamed_to_main_category = String::new();

    if let Some(new_name) = category_name_from_body(body) {
        let category = match repository.get_category_by_id(category_id, user_id) {
            Ok(Some(value)) => value,
            Ok(None) => return not_found("Category not found"),
            Err(_) => return category_db_error_response(),
        };
        if category_text(&category, "sub_category").is_empty() {
            let old_name = category_text(&category, "main_category");
            if new_name != old_name {
                let real_categories = match repository.list_categories(user_id) {
                    Ok(value) => value,
                    Err(_) => return category_db_error_response(),
                };
                if real_categories
                    .iter()
                    .any(|category| category_text(category, "main_category") == new_name)
                {
                    return error_response(StatusCode::CONFLICT, "Category rename conflict");
                }
                match repository.update_main_category_name(&old_name, &new_name, user_id) {
                    Ok(true) => {
                        renamed_main_category = true;
                        original_main_category = old_name;
                        renamed_to_main_category = new_name;
                    }
                    Ok(false) => {
                        return error_response(StatusCode::CONFLICT, "Category rename conflict")
                    }
                    Err(_) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to rename category",
                        );
                    }
                }
            }
        } else {
            updates.insert("sub_category".to_string(), Value::String(new_name));
        }
    }

    let payload = Value::Object(updates);
    match repository.update_category(category_id, &payload, user_id) {
        Ok(true) => match repository.get_category_by_id(category_id, user_id) {
            Ok(Some(category)) => success_result(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&category, "0")),
            ),
            Ok(None) => {
                rollback_category_rename(
                    repository,
                    renamed_main_category,
                    &renamed_to_main_category,
                    &original_main_category,
                    user_id,
                );
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to load updated category",
                )
            }
            Err(_) => category_db_error_response(),
        },
        Ok(false) => {
            rollback_category_rename(
                repository,
                renamed_main_category,
                &renamed_to_main_category,
                &original_main_category,
                user_id,
            );
            not_found("Category not found")
        }
        Err(error) => {
            rollback_category_rename(
                repository,
                renamed_main_category,
                &renamed_to_main_category,
                &original_main_category,
                user_id,
            );
            let text = error.to_string();
            if text.contains("constraint") || text.contains("UNIQUE") {
                error_response(StatusCode::CONFLICT, "Category update conflict")
            } else {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update category",
                )
            }
        }
    }
}

fn rollback_category_rename(
    repository: &mut CategoriesRepository<'_>,
    renamed: bool,
    current_name: &str,
    previous_name: &str,
    user_id: i64,
) {
    if renamed {
        let _ = repository.update_main_category_name(current_name, previous_name, user_id);
    }
}

#[derive(Clone, Copy)]
enum CategoryPayloadMode {
    FrontendDefaults,
    ImportDefaults,
}

fn resolve_parent_category(
    repository: &mut CategoriesRepository<'_>,
    parent_id: &str,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<(String, i64)>> {
    if let Some(parent_name) = virtual_category_name(parent_id) {
        return Ok(Some((parent_name, 1)));
    }
    let Ok(parent_id) = parent_id.parse::<i64>() else {
        return Ok(None);
    };
    Ok(repository
        .get_category_by_id(parent_id, user_id)?
        .map(|parent| {
            (
                category_text(&parent, "main_category"),
                parent.get("type").and_then(value_as_i64).unwrap_or(1),
            )
        }))
}

fn category_name_from_body(payload: &Value) -> Option<String> {
    payload
        .as_object()
        .and_then(|object| object.get("name"))
        .map(|value| string_or_default(Some(value), ""))
}

fn virtual_category_name(category_id: &str) -> Option<String> {
    category_id
        .strip_prefix("virtual_")
        .map(ToString::to_string)
}

fn category_parent_id_for_get(
    repository: &mut CategoriesRepository<'_>,
    category: &CategoryRecord,
    user_id: i64,
) -> String {
    let sub_category = category_text(category, "sub_category");
    if sub_category.is_empty() {
        return "0".to_string();
    }
    let main_category = category_text(category, "main_category");
    match repository.get_category_by_name(&main_category, "", user_id) {
        Ok(Some(parent)) => parent
            .get("id")
            .map(|value| value_string(Some(value), "0"))
            .unwrap_or_else(|| format!("virtual_{main_category}")),
        Ok(None) | Err(_) => format!("virtual_{main_category}"),
    }
}

fn frontend_category_to_backend(
    payload: &Value,
    main_category: &str,
    sub_category: &str,
    category_type: i64,
    mode: CategoryPayloadMode,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert(
        "type".to_string(),
        Value::Number(Number::from(category_type)),
    );
    result.insert(
        "main_category".to_string(),
        Value::String(main_category.to_string()),
    );
    result.insert(
        "sub_category".to_string(),
        Value::String(sub_category.to_string()),
    );
    result.insert(
        "description".to_string(),
        Value::String(string_or_default(payload.get("comment"), "")),
    );
    result.insert(
        "priority".to_string(),
        Value::Number(Number::from(
            payload
                .get("displayOrder")
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert(
        "keywords".to_string(),
        Value::String(string_or_default(payload.get("keywords"), "")),
    );
    let hidden = match mode {
        CategoryPayloadMode::FrontendDefaults => payload
            .get("visible")
            .map(|visible| !value_truthy(visible))
            .or_else(|| payload.get("hidden").map(value_truthy))
            .unwrap_or(false),
        CategoryPayloadMode::ImportDefaults => {
            payload.get("hidden").map(value_truthy).unwrap_or(false)
        }
    };
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(payload.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(payload.get("color"), "")),
    );
    result
}

fn virtual_category_update_payload(payload: &Value, main_category: &str) -> Map<String, Value> {
    let category_type = payload.get("type").and_then(value_as_i64).unwrap_or(1);
    frontend_category_to_backend(
        payload,
        main_category,
        "",
        category_type,
        CategoryPayloadMode::FrontendDefaults,
    )
}

fn virtual_category_create_payload(payload: &Value, main_category: &str) -> Map<String, Value> {
    virtual_category_update_payload(payload, main_category)
}

fn category_update_payload_from_frontend(payload: &Value) -> Map<String, Value> {
    let mut result = Map::new();
    if let Some(comment) = payload.get("comment") {
        result.insert(
            "description".to_string(),
            Value::String(string_or_default(Some(comment), "")),
        );
    }
    if let Some(display_order) = payload.get("displayOrder").and_then(value_as_i64) {
        result.insert(
            "priority".to_string(),
            Value::Number(Number::from(display_order)),
        );
    }
    if let Some(keywords) = payload.get("keywords") {
        result.insert(
            "keywords".to_string(),
            Value::String(string_or_default(Some(keywords), "")),
        );
    }
    if let Some(category_type) = payload.get("type").and_then(value_as_i64) {
        result.insert(
            "type".to_string(),
            Value::Number(Number::from(category_type)),
        );
    }
    if let Some(visible) = payload.get("visible") {
        result.insert("hidden".to_string(), Value::Bool(!value_truthy(visible)));
    }
    if let Some(icon) = payload.get("icon") {
        result.insert(
            "icon".to_string(),
            Value::String(string_or_default(Some(icon), "")),
        );
    }
    if let Some(color) = payload.get("color") {
        result.insert(
            "color".to_string(),
            Value::String(string_or_default(Some(color), "")),
        );
    }
    result
}

fn import_category_payload(
    payload: &Value,
    main_category: &str,
    sub_category: &str,
) -> Map<String, Value> {
    let category_type = payload.get("type").and_then(value_as_i64).unwrap_or(3);
    let mut result = frontend_category_to_backend(
        payload,
        main_category,
        sub_category,
        category_type,
        CategoryPayloadMode::ImportDefaults,
    );
    if let Some(description) = payload.get("description") {
        result.insert(
            "description".to_string(),
            Value::String(string_or_default(Some(description), "")),
        );
    }
    if let Some(priority) = payload.get("priority").and_then(value_as_i64) {
        result.insert(
            "priority".to_string(),
            Value::Number(Number::from(priority)),
        );
    }
    result
}

fn format_category_tree_response(categories: Vec<CategoryRecord>) -> Value {
    let mut grouped: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
    let mut main_indices: BTreeMap<(i64, String), (i64, usize)> = BTreeMap::new();

    for category in categories {
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(0);
        let main_name = category_text(&category, "main_category");
        let sub_name = category_text(&category, "sub_category");
        let key = (category_type, main_name.clone());

        if !main_indices.contains_key(&key) {
            let parent_id = if sub_name.is_empty() {
                value_string(category.get("id"), &format!("virtual_{main_name}"))
            } else {
                format!("virtual_{main_name}")
            };
            let mut node = backend_category_to_frontend(&category, "0");
            node.insert("id".to_string(), Value::String(parent_id));
            node.insert("name".to_string(), Value::String(main_name.clone()));
            node.insert("parentId".to_string(), Value::String("0".to_string()));
            node.insert("comment".to_string(), Value::String(String::new()));
            node.insert("hidden".to_string(), Value::Bool(false));
            node.insert("visible".to_string(), Value::Bool(true));
            node.insert("keywords".to_string(), Value::String(String::new()));
            node.insert("subCategories".to_string(), Value::Array(Vec::new()));
            let bucket = grouped.entry(category_type).or_default();
            let index = bucket.len();
            bucket.push(Value::Object(node));
            main_indices.insert(key.clone(), (category_type, index));
        }

        let Some((bucket_key, index)) = main_indices.get(&key).copied() else {
            continue;
        };
        let Some(bucket) = grouped.get_mut(&bucket_key) else {
            continue;
        };
        let Some(node) = bucket.get_mut(index).and_then(Value::as_object_mut) else {
            continue;
        };

        if sub_name.is_empty() {
            let sub_categories = node
                .remove("subCategories")
                .unwrap_or_else(|| Value::Array(Vec::new()));
            *node = backend_category_to_frontend(&category, "0");
            node.insert("subCategories".to_string(), sub_categories);
        } else {
            let parent_id = node
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("virtual_{main_name}"));
            let sub_node = backend_category_to_frontend(&category, &parent_id);
            node.entry("subCategories".to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("subCategories must stay an array")
                .push(Value::Object(sub_node));
        }
    }

    let mut result = Map::new();
    for (category_type, values) in grouped {
        result.insert(category_type.to_string(), Value::Array(values));
    }
    Value::Object(result)
}

fn format_category_flat_response(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(
        categories
            .iter()
            .map(|category| {
                let parent_id = if category_text(category, "sub_category").is_empty() {
                    "0".to_string()
                } else {
                    format!("virtual_{}", category_text(category, "main_category"))
                };
                Value::Object(backend_category_to_frontend(category, &parent_id))
            })
            .collect(),
    )
}

fn categories_to_value(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(categories.into_iter().map(Value::Object).collect())
}

fn format_category_statistics_response(statistics: Vec<CategoryStatistic>) -> Value {
    let mut result = Map::new();

    for statistic in statistics {
        let entry = result
            .entry(statistic.main_category.clone())
            .or_insert_with(|| {
                json!({
                    "total_amount": 0.0,
                    "count": 0,
                    "sub_categories": {}
                })
            });
        let Some(entry_object) = entry.as_object_mut() else {
            continue;
        };

        let total_amount = entry_object
            .get("total_amount")
            .and_then(value_as_f64)
            .unwrap_or_default()
            + statistic.total_amount.abs();
        entry_object.insert(
            "total_amount".to_string(),
            json_number(round2(total_amount)),
        );

        let count = entry_object
            .get("count")
            .and_then(value_as_i64)
            .unwrap_or_default()
            + statistic.count;
        entry_object.insert("count".to_string(), Value::Number(Number::from(count)));

        if statistic.sub_category.is_empty() {
            continue;
        }
        let sub_categories = entry_object
            .entry("sub_categories".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(sub_categories) = sub_categories.as_object_mut() {
            sub_categories.insert(
                statistic.sub_category.clone(),
                json!({
                    "total_amount": round2(statistic.total_amount.abs()),
                    "count": statistic.count
                }),
            );
        }
    }

    Value::Object(result)
}

fn format_category_rules_response(rules: Vec<CategoryRuleRecord>) -> Value {
    let total = rules.len();
    json!({
        "success": true,
        "data": Value::Array(rules.into_iter().map(Value::Object).collect()),
        "total": total,
    })
}

fn build_settings_bundle(connection: &mut Connection, user_id: i64) -> Result<Value, String> {
    let accounts = {
        let mut repository = AccountsRepository::new(connection);
        repository
            .list_accounts(user_id)
            .map_err(|error| error.to_string())?
    };
    let categories = {
        let mut repository = CategoriesRepository::new(connection);
        repository
            .list_categories(user_id)
            .map_err(|error| error.to_string())?
    };
    let tags = {
        let mut repository = TagsRepository::new(connection);
        repository
            .list_tags(user_id)
            .map_err(|error| error.to_string())?
    };
    let templates = list_settings_templates(connection, user_id, 1)?;
    let scheduled = list_settings_templates(connection, user_id, 2)?;
    let category_refs = category_ref_map(&categories);

    let taxonomy_sections = export_taxonomy_sections(&json!({
        "accounts": records_to_array(&accounts),
        "categories": records_to_array(&categories),
        "tags": tags_to_array(&tags),
        "templates": records_to_array(&templates),
        "scheduled": records_to_array(&scheduled),
    }))
    .map_err(|error| error.to_string())?;

    let mut sections = taxonomy_sections
        .as_object()
        .cloned()
        .ok_or_else(|| "settings taxonomy sections must be an object".to_string())?;

    let category_rules = {
        let mut repository = CategoryRulesRepository::new(connection);
        repository
            .list_rules(user_id, None, false)
            .map_err(|error| error.to_string())?
    };
    sections.insert(
        "categoryRecognitionRules".to_string(),
        Value::Array(
            category_rules
                .iter()
                .map(|rule| export_settings_category_rule(rule, &category_refs))
                .collect(),
        ),
    );
    sections.insert(
        "llmConfigs".to_string(),
        Value::Array(list_settings_llm_configs(connection, user_id)?),
    );
    sections.insert(
        "ocrConfig".to_string(),
        Value::Array(vec![export_settings_ocr_config(connection)?]),
    );

    let counts = SETTINGS_BUNDLE_SECTION_KEYS
        .iter()
        .map(|key| {
            let count = sections
                .get(*key)
                .and_then(Value::as_array)
                .map(|items| items.len() as i64)
                .unwrap_or(0);
            ((*key).to_string(), Value::Number(Number::from(count)))
        })
        .collect::<Map<_, _>>();

    Ok(json!({
        "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
        "exportedAt": Utc::now().naive_utc().format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
        "secretsPolicy": {"llmApiKeys": "redacted"},
        "sections": Value::Object(sections),
        "counts": Value::Object(counts),
    }))
}

fn list_settings_templates(
    connection: &Connection,
    user_id: i64,
    template_type: i64,
) -> Result<Vec<Map<String, Value>>, String> {
    let table_name = if template_type == 2 {
        "recurring_bills"
    } else {
        "bill_templates"
    };
    let mut statement = connection
        .prepare(&format!(
            "SELECT * FROM {table_name} WHERE user_id = ?1 ORDER BY COALESCE(display_order, 0), name"
        ))
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![user_id], |row| {
            let mut item = Map::new();
            let row_ref = row.as_ref();
            for index in 0..row_ref.column_count() {
                let name = row_ref.column_name(index)?.to_string();
                item.insert(name, sqlite_ref_to_json(row.get_ref(index)?));
            }
            Ok(item)
        })
        .map_err(|error| error.to_string())?;

    rows.map(|row| {
        row.map(|raw| serialize_settings_template_row(&raw, template_type))
            .map_err(|error| error.to_string())
    })
    .collect()
}

fn serialize_settings_template_row(
    row: &Map<String, Value>,
    template_type: i64,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(row.get("id"), "")),
    );
    result.insert("timeSequenceId".to_string(), Value::String(String::new()));
    result.insert(
        "templateType".to_string(),
        Value::Number(Number::from(template_type)),
    );
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(row.get("name"), "")),
    );
    result.insert(
        "description".to_string(),
        Value::String(string_or_default(row.get("description"), "")),
    );
    result.insert(
        "type".to_string(),
        Value::Number(Number::from(normalize_template_transaction_type(
            row.get("type"),
        ))),
    );
    result.insert(
        "categoryId".to_string(),
        Value::String(value_string(row.get("category"), "")),
    );
    result.insert("time".to_string(), Value::Number(Number::from(0)));
    result.insert(
        "utcOffset".to_string(),
        Value::Number(Number::from(value_as_i64_or(row.get("utc_offset"), 0))),
    );
    result.insert(
        "sourceAccountId".to_string(),
        Value::String(value_string(row.get("account"), "0")),
    );
    result.insert(
        "destinationAccountId".to_string(),
        Value::String(value_string(row.get("counterparty"), "0")),
    );
    result.insert(
        "sourceAmount".to_string(),
        json_number(row.get("amount").and_then(value_as_f64).unwrap_or_default()),
    );
    result.insert(
        "destinationAmount".to_string(),
        json_number(
            row.get("destination_amount")
                .and_then(value_as_f64)
                .unwrap_or_default(),
        ),
    );
    result.insert(
        "hideAmount".to_string(),
        Value::Bool(row.get("hide_amount").is_some_and(value_truthy)),
    );
    result.insert(
        "tagIds".to_string(),
        Value::Array(template_tag_ids(row.get("tag"))),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(row.get("comment"), "")),
    );
    result.insert("editable".to_string(), Value::Bool(true));
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(value_as_i64_or(row.get("display_order"), 0))),
    );
    result.insert(
        "hidden".to_string(),
        Value::Bool(row.get("hidden").is_some_and(value_truthy)),
    );
    result.insert(
        "scheduledFrequencyType".to_string(),
        if template_type == 2 {
            Value::Number(Number::from(value_as_i64_or(
                row.get("scheduled_frequency_type"),
                0,
            )))
        } else {
            Value::Null
        },
    );
    result.insert(
        "scheduledFrequency".to_string(),
        recurring_string_or_null(row, template_type, "frequency"),
    );
    result.insert(
        "scheduledStartDate".to_string(),
        recurring_string_or_null(row, template_type, "start_date"),
    );
    result.insert(
        "scheduledEndDate".to_string(),
        recurring_string_or_null(row, template_type, "end_date"),
    );
    result.insert("scheduledAt".to_string(), Value::Null);
    if template_type == 2 {
        result.insert(
            "enabled".to_string(),
            Value::Bool(row.get("enabled").is_some_and(value_truthy)),
        );
        result.insert(
            "autoCreate".to_string(),
            Value::Bool(row.get("auto_create").is_some_and(value_truthy)),
        );
        result.insert(
            "nextDate".to_string(),
            row.get("next_date").cloned().unwrap_or(Value::Null),
        );
    }
    result
}

fn list_settings_llm_configs(connection: &Connection, user_id: i64) -> Result<Vec<Value>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, provider, model, api_key, base_url, advanced_settings, is_active
             FROM llm_configs
             WHERE user_id = ?1
             ORDER BY id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![user_id], |row| {
            let api_key = row.get::<_, Option<String>>("api_key")?.unwrap_or_default();
            let advanced_settings = row
                .get::<_, Option<String>>("advanced_settings")?
                .unwrap_or_default();
            Ok(json!({
                "externalRef": format!("llmConfig:{}", row.get::<_, i64>("id")?),
                "name": row.get::<_, Option<String>>("name")?.unwrap_or_default(),
                "provider": row.get::<_, Option<String>>("provider")?.unwrap_or_else(|| "openai".to_string()),
                "model": row.get::<_, Option<String>>("model")?.unwrap_or_default(),
                "apiKey": "",
                "hasApiKey": !api_key.is_empty(),
                "baseUrl": row.get::<_, Option<String>>("base_url")?.unwrap_or_default(),
                "advancedSettings": normalize_llm_advanced_settings(&advanced_settings),
                "activeInSource": row.get::<_, Option<i64>>("is_active")?.unwrap_or(0) != 0,
            }))
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn export_settings_ocr_config(connection: &Connection) -> Result<Value, String> {
    let raw_value = connection
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![OCR_CONFIG_SETTING_KEY],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .flatten()
        .unwrap_or_default();
    let loaded = serde_json::from_str::<Value>(&raw_value).unwrap_or_else(|_| json!({}));
    let normalized = normalize_ocr_config(&loaded);
    Ok(json!({
        "externalRef": "ocrConfig:receipt-recognition",
        "provider": normalized["provider"],
        "lang": normalized["lang"],
    }))
}

fn export_settings_category_rule(
    rule: &Map<String, Value>,
    category_refs: &BTreeMap<i64, String>,
) -> Value {
    let category_id = value_as_i64_or(rule.get("category_id"), 0);
    json!({
        "externalRef": format!("categoryRule:{}", value_string(rule.get("id"), "")),
        "categoryRef": category_refs.get(&category_id).cloned().unwrap_or_default(),
        "mainCategory": string_or_default(rule.get("main_category"), ""),
        "subCategory": string_or_default(rule.get("sub_category"), ""),
        "name": string_or_default(rule.get("name"), ""),
        "priority": value_as_i64_or(rule.get("priority"), 100),
        "ruleExpression": string_or_default(rule.get("rule_expression"), ""),
        "regexEnabled": rule.get("regex_enabled").is_some_and(value_truthy),
        "enabled": rule.get("enabled").map(value_truthy).unwrap_or(true),
    })
}

fn category_ref_map(categories: &[CategoryRecord]) -> BTreeMap<i64, String> {
    categories
        .iter()
        .filter_map(|category| {
            let id = category
                .get("id")
                .and_then(value_as_i64)
                .unwrap_or_default();
            (id > 0).then(|| (id, format!("category:{id}")))
        })
        .collect()
}

fn records_to_array(records: &[Map<String, Value>]) -> Value {
    Value::Array(records.iter().cloned().map(Value::Object).collect())
}

fn tags_to_array(tags: &[TagRecord]) -> Value {
    Value::Array(
        tags.iter()
            .map(|tag| serde_json::to_value(tag).unwrap_or(Value::Null))
            .collect(),
    )
}

fn filter_settings_bundle_section(bundle: &Value, section_key: &str) -> Value {
    let section_items = bundle
        .get("sections")
        .and_then(Value::as_object)
        .and_then(|sections| sections.get(section_key))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let item_count = section_items.len() as i64;
    let mut sections = Map::new();
    sections.insert(section_key.to_string(), Value::Array(section_items));
    let mut counts = Map::new();
    counts.insert(
        section_key.to_string(),
        Value::Number(Number::from(item_count)),
    );

    json!({
        "schemaVersion": bundle
            .get("schemaVersion")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(SETTINGS_BUNDLE_SCHEMA_VERSION))),
        "exportedAt": bundle.get("exportedAt").cloned().unwrap_or(Value::Null),
        "secretsPolicy": bundle
            .get("secretsPolicy")
            .cloned()
            .unwrap_or_else(|| json!({"llmApiKeys": "redacted"})),
        "sections": Value::Object(sections),
        "counts": Value::Object(counts),
    })
}

fn settings_bundle_download_response(bundle: Value, filename: &str) -> Response {
    let body = serde_json::to_string(&bundle).unwrap_or_else(|_| "{}".to_string());
    (
        StatusCode::OK,
        [
            (CONTENT_TYPE, "application/json".to_string()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response()
}

fn is_valid_settings_bundle_section(section_key: &str) -> bool {
    SETTINGS_BUNDLE_SECTION_KEYS.contains(&section_key)
}

fn is_sensitive_settings_export_section(section_key: &str) -> bool {
    SENSITIVE_EXPORT_SECTIONS.contains(&section_key)
}

fn settings_bundle_section_not_found(section_key: &str) -> Response {
    json_response(
        StatusCode::NOT_FOUND,
        json!({
            "success": false,
            "error": format!("Unsupported settings bundle section: {section_key}"),
        }),
    )
}

fn settings_bundle_db_error_response() -> Response {
    json_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({"success": false, "error": "Failed to export settings bundle"}),
    )
}

fn verify_sensitive_export_password(
    connection: &Connection,
    user_id: i64,
    password: &str,
) -> rusqlite::Result<bool> {
    let password_hash = connection.query_row(
        "SELECT password_hash FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get::<_, Option<String>>(0),
    )?;
    Ok(password_hash
        .filter(|value| !value.is_empty())
        .is_some_and(|hash| bcrypt::verify(password, &hash).unwrap_or(false)))
}

fn optional_json_body(body: Bytes) -> Option<Value> {
    if body.is_empty() {
        return None;
    }
    serde_json::from_slice(&body).ok()
}

fn template_tag_ids(value: Option<&Value>) -> Vec<Value> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(|item| Value::String(string_or_default(Some(item), "")))
            .filter(|item| item.as_str().is_some_and(|text| !text.is_empty()))
            .collect();
    }
    string_or_default(Some(value), "")
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| Value::String(item.to_string()))
        .collect()
}

fn recurring_string_or_null(row: &Map<String, Value>, template_type: i64, key: &str) -> Value {
    if template_type != 2 {
        return Value::Null;
    }
    row.get(key).cloned().unwrap_or(Value::Null)
}

fn normalize_template_transaction_type(value: Option<&Value>) -> i64 {
    let text = string_or_default(value, "").to_ascii_lowercase();
    match text.as_str() {
        "2" | "income" | "收入" => 2,
        "4" | "transfer" | "转账" => 4,
        "5" | "investment" | "投资" => 5,
        _ => 3,
    }
}

fn normalize_llm_advanced_settings(raw_value: &str) -> Value {
    let loaded = serde_json::from_str::<Value>(raw_value).unwrap_or_else(|_| json!({}));
    let object = loaded.as_object();
    let mut normalized = Map::new();

    if let Some(reasoning_depth) = object
        .and_then(|settings| settings.get("reasoning_depth"))
        .map(|value| string_or_default(Some(value), "").to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "low" | "medium" | "high"))
    {
        normalized.insert(
            "reasoning_depth".to_string(),
            Value::String(reasoning_depth),
        );
    }
    if let Some(temperature) = object
        .and_then(|settings| settings.get("temperature"))
        .and_then(value_as_f64)
        .filter(|value| (0.0..=2.0).contains(value))
    {
        normalized.insert("temperature".to_string(), json_number(temperature));
    }
    if let Some(max_tokens) = object
        .and_then(|settings| settings.get("max_tokens"))
        .and_then(value_as_i64)
        .filter(|value| (1..=200_000).contains(value))
    {
        normalized.insert(
            "max_tokens".to_string(),
            Value::Number(Number::from(max_tokens)),
        );
    }
    for key in [
        "system_prompt",
        "classification_prompt_template",
        "rule_prompt_template",
    ] {
        if let Some(text) = object
            .and_then(|settings| settings.get(key))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            normalized.insert(key.to_string(), Value::String(text.to_string()));
        }
    }
    Value::Object(normalized)
}

fn normalize_ocr_config(raw_value: &Value) -> Value {
    let provider = raw_value
        .get("provider")
        .map(|value| string_or_default(Some(value), "disabled").to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "disabled" | "tesseract" | "cloud_stub"))
        .unwrap_or_else(|| "disabled".to_string());
    let lang = raw_value
        .get("lang")
        .map(|value| string_or_default(Some(value), "chi_sim+eng"))
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 64
                && value
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '.' | '-'))
        })
        .unwrap_or_else(|| "chi_sim+eng".to_string());
    json!({
        "provider": provider,
        "lang": lang,
    })
}

fn sqlite_ref_to_json(value: rusqlite::types::ValueRef<'_>) -> Value {
    match value {
        rusqlite::types::ValueRef::Null => Value::Null,
        rusqlite::types::ValueRef::Integer(number) => Value::Number(Number::from(number)),
        rusqlite::types::ValueRef::Real(number) => {
            Number::from_f64(number).map_or(Value::Null, Value::Number)
        }
        rusqlite::types::ValueRef::Text(text) => {
            Value::String(String::from_utf8_lossy(text).to_string())
        }
        rusqlite::types::ValueRef::Blob(blob) => {
            Value::String(String::from_utf8_lossy(blob).to_string())
        }
    }
}

fn value_as_i64_or(value: Option<&Value>, default: i64) -> i64 {
    value.and_then(value_as_i64).unwrap_or(default)
}

fn count_rules_overview_learning_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<i64> {
    connection.query_row(
        "SELECT COUNT(*) FROM import_learning_rules WHERE user_id = ?1",
        params![user_id],
        |row| row.get(0),
    )
}

fn list_rules_overview_learning_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(
        "
        SELECT id, match_type, match_value, learned_type, learned_category_id,
               enabled, applied_count
        FROM import_learning_rules
        WHERE user_id = ?1
        ORDER BY updated_at DESC, id DESC
        LIMIT 500
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>("id")?,
            "matchType": row.get::<_, Option<String>>("match_type")?,
            "matchValue": row.get::<_, Option<String>>("match_value")?,
            "learnedType": row.get::<_, Option<String>>("learned_type")?,
            "learnedCategoryId": row.get::<_, Option<i64>>("learned_category_id")?,
            "enabled": row.get::<_, i64>("enabled")? != 0,
            "appliedCount": row.get::<_, Option<i64>>("applied_count")?.unwrap_or(0),
            "source": "learning",
        }))
    })?;
    rows.collect()
}

fn list_rules_overview_recurring_rules(
    connection: &Connection,
    user_id: i64,
) -> rusqlite::Result<Vec<Value>> {
    let mut statement = connection.prepare(
        "
        SELECT id, name, amount, frequency, enabled, next_date
        FROM recurring_bills
        WHERE user_id = ?1
        ORDER BY COALESCE(display_order, 0), name
        ",
    )?;
    let rows = statement.query_map(params![user_id], |row| {
        Ok(json!({
            "id": row.get::<_, i64>("id")?,
            "name": row.get::<_, Option<String>>("name")?,
            "amount": row.get::<_, Option<f64>>("amount")?,
            "frequency": row.get::<_, Option<String>>("frequency")?,
            "enabled": row.get::<_, Option<i64>>("enabled")?.unwrap_or(1) != 0,
            "nextDate": row.get::<_, Option<String>>("next_date")?,
            "source": "recurring",
        }))
    })?;
    rows.collect()
}

fn category_rules_enabled_only(query: &CategoryRulesQuery) -> bool {
    !query
        .enabled_only
        .as_deref()
        .unwrap_or("true")
        .eq_ignore_ascii_case("false")
}

fn category_export_record(category: &CategoryRecord) -> Map<String, Value> {
    let mut result = Map::new();
    for (field, default) in [
        ("type", Value::Number(Number::from(3))),
        ("main_category", Value::String(String::new())),
        ("sub_category", Value::String(String::new())),
        ("priority", Value::Number(Number::from(0))),
        ("keywords", Value::String(String::new())),
        ("description", Value::String(String::new())),
        ("icon", Value::String(String::new())),
        ("color", Value::String(String::new())),
        ("hidden", Value::Bool(false)),
    ] {
        result.insert(
            field.to_string(),
            category.get(field).cloned().unwrap_or(default),
        );
    }
    result
}

fn backend_category_to_frontend(category: &CategoryRecord, parent_id: &str) -> Map<String, Value> {
    let hidden = category.get("hidden").map(value_truthy).unwrap_or(false);
    let sub_category = category_text(category, "sub_category");
    let name = if sub_category.is_empty() {
        category_text(category, "main_category")
    } else {
        sub_category
    };
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(category.get("id"), "")),
    );
    result.insert("name".to_string(), Value::String(name));
    result.insert("parentId".to_string(), Value::String(parent_id.to_string()));
    result.insert(
        "type".to_string(),
        category
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(category.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(category.get("color"), "")),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(category.get("description"), "")),
    );
    result.insert(
        "displayOrder".to_string(),
        category
            .get("priority")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));
    result.insert(
        "keywords".to_string(),
        Value::String(string_or_default(category.get("keywords"), "")),
    );
    result
}

fn category_text(category: &CategoryRecord, key: &str) -> String {
    string_or_default(category.get(key), "")
}

fn required_json_body(body: Bytes, missing_message: &'static str) -> RouteResult<Value> {
    let value = parse_json_body(body)?;
    if matches!(value, Value::Null) || value.as_object().is_some_and(Map::is_empty) {
        return Err(Box::new(bad_request(missing_message)));
    }
    Ok(value)
}

fn parse_json_body(body: Bytes) -> RouteResult<Value> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_slice(&body).map_err(|_| Box::new(bad_request("Invalid JSON")))
}

fn open_runtime(state: &ProxyState, runtime_label: &str) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("Rust {runtime_label} DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"),
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

fn db_user_id(user_id: UserId) -> i64 {
    i64::try_from(user_id.get()).unwrap_or(i64::MAX)
}

fn json_response(status: StatusCode, body: Value) -> Response {
    (status, Json(body)).into_response()
}

fn success_result(status: StatusCode, result: Value) -> Response {
    json_response(status, json!({ "success": true, "result": result }))
}

fn success_result_with_message(status: StatusCode, result: Value, message: &str) -> Response {
    json_response(
        status,
        json!({ "success": true, "result": result, "message": message }),
    )
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
        "Rust taxonomy accounts route runtime DB error",
    )
}

fn tag_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy tags route runtime DB error",
    )
}

fn category_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy categories route runtime DB error",
    )
}

fn category_rule_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy category rules route runtime DB error",
    )
}

fn template_db_error_response() -> Response {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Rust taxonomy templates route runtime DB error",
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

fn parse_aliases(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .map(python_value_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        Some(Value::String(text)) => parse_alias_string(text),
        Some(_) => Vec::new(),
    }
}

fn parse_alias_string(text: &str) -> Vec<String> {
    let text = text.trim();
    if text.is_empty() {
        return Vec::new();
    }
    if text.starts_with('[') {
        if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(text) {
            return values
                .iter()
                .map(python_value_text)
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect();
        }
    }
    text.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn python_value_text(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::String(text) => text.clone(),
        Value::Number(_) | Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn template_type_from_query_body(
    query: &BTreeMap<String, String>,
    body: Option<&Value>,
    default: Option<i64>,
) -> Option<i64> {
    if let Some(value) = query.get("templateType") {
        return parse_template_type_text(value).or(default);
    }
    if let Some(value) = body.and_then(|value| value.get("templateType")) {
        return parse_template_type_text(&python_value_text(value)).or(default);
    }
    default
}

fn parse_template_type_text(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok()
}

fn template_display_orders_from_body(body: &Value) -> RouteResult<Vec<TemplateDisplayOrder>> {
    let Some(items) = body
        .get("newDisplayOrders")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty())
    else {
        return Err(Box::new(bad_request("Missing newDisplayOrders")));
    };
    let mut orders = Vec::with_capacity(items.len());
    for item in items {
        let Some(object) = item.as_object() else {
            return Err(Box::new(bad_request(
                "Each item must have id and displayOrder",
            )));
        };
        let Some(template_id) = object.get("id").and_then(parse_python_int) else {
            return Err(Box::new(bad_request(
                "Each item must have id and displayOrder",
            )));
        };
        let Some(display_order) = object.get("displayOrder").and_then(parse_python_int) else {
            return Err(Box::new(bad_request(
                "Each item must have id and displayOrder",
            )));
        };
        orders.push(TemplateDisplayOrder {
            template_id,
            display_order,
        });
    }
    Ok(orders)
}

fn tag_name_is_present(payload: &Value) -> bool {
    payload
        .as_object()
        .and_then(|object| object.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}

fn tag_batch_normalized_name(payload: &Value) -> Option<String> {
    let object = payload.as_object()?;
    let name = value_string(object.get("name"), "");
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_lowercase())
    }
}

fn tag_batch_display_name(payload: &Value) -> String {
    payload
        .as_object()
        .map(|object| value_string(object.get("name"), ""))
        .unwrap_or_default()
}

fn frontend_tag_to_backend(payload: &Value) -> Result<Map<String, Value>, String> {
    let Some(object) = payload.as_object() else {
        return Err("Tag payload must be an object".to_string());
    };
    let mut result = Map::new();

    if let Some(id) = object.get("id") {
        result.insert("id".to_string(), id.clone());
    }
    if let Some(name) = object.get("name") {
        result.insert(
            "name".to_string(),
            Value::String(string_or_default(Some(name), "")),
        );
    }
    if let Some(color) = object.get("color") {
        result.insert(
            "color".to_string(),
            if color.is_null() {
                Value::Null
            } else {
                Value::String(string_or_default(Some(color), "#000000"))
            },
        );
    }
    if let Some(icon) = object.get("icon") {
        result.insert(
            "icon".to_string(),
            if icon.is_null() {
                Value::Null
            } else {
                Value::String(string_or_default(Some(icon), ""))
            },
        );
    }
    if let Some(hidden) = object.get("hidden") {
        result.insert("hidden".to_string(), Value::Bool(value_truthy(hidden)));
    } else if let Some(visible) = object.get("visible") {
        result.insert("hidden".to_string(), Value::Bool(!value_truthy(visible)));
    }
    if let Some(display_order) = object
        .get("displayOrder")
        .or_else(|| object.get("display_order"))
    {
        let Some(display_order) = value_as_i64(display_order) else {
            return Err("displayOrder must be an integer".to_string());
        };
        result.insert(
            "display_order".to_string(),
            Value::Number(Number::from(display_order)),
        );
    }

    Ok(result)
}

fn value_string(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Null) | None => default.to_string(),
        Some(value @ (Value::Array(_) | Value::Object(_))) => value.to_string(),
    }
}

fn string_or_default(value: Option<&Value>, default: &str) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => default.to_string(),
        Some(value) => value.to_string(),
    }
}

fn parse_python_int(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.trim().parse::<i64>().ok(),
        Value::Bool(flag) => Some(i64::from(*flag)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn invalid_tag_order_value_error(value: Option<&Value>) -> String {
    let reason = match value {
        Some(Value::String(text)) => {
            format!("invalid literal for int() with base 10: '{text}'")
        }
        Some(Value::Null) => {
            "int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
                .to_string()
        }
        Some(value) => format!("unsupported integer value: {value}"),
        None => "missing value".to_string(),
    };
    format!("Invalid id or displayOrder: {reason}")
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.trim().parse::<i64>().ok())
    })
}

fn value_as_f64(value: &Value) -> Option<f64> {
    value.as_f64().or_else(|| {
        value
            .as_str()
            .and_then(|text| text.trim().parse::<f64>().ok())
    })
}

fn value_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_i64().unwrap_or_default() != 0,
        Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty()
                && !trimmed.eq_ignore_ascii_case("false")
                && trimmed != "0"
                && !trimmed.eq_ignore_ascii_case("none")
                && !trimmed.eq_ignore_ascii_case("null")
        }
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => !values.is_empty(),
        Value::Null => false,
    }
}

fn yuan_to_cents(value: Option<&Value>) -> i64 {
    let yuan = value.and_then(value_as_f64).unwrap_or_default();
    (yuan * 100.0).round() as i64
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn json_number(value: f64) -> Value {
    Number::from_f64(value).map_or(Value::Null, Value::Number)
}
