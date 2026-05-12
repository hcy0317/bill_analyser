use std::collections::BTreeMap;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};
use bill_analyser_core::UserId;
use bill_analyser_db::{
    taxonomy::accounts::{AccountDisplayOrder, AccountRecord, AccountsRepository},
    SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use serde_json::{json, Map, Number, Value};

use crate::{
    auth::resolve_user_id_from_headers,
    config::HttpShellConfig,
    proxy::{ownership_aware_proxy_handler, ProxyState},
};

const TRUSTED_USER_SECRET_HEADER: &str = "x-bill-analyser-trusted-user-secret";

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

pub fn taxonomy_runtime_router() -> Router<ProxyState> {
    Router::new()
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
}

async fn list_accounts_handler(State(state): State<ProxyState>, headers: HeaderMap) -> Response {
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(value) => value,
        Err(response) => return *response,
    };
    let mut runtime = match open_runtime(&state) {
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
    let mut runtime = match open_runtime(&state) {
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
    let mut runtime = match open_runtime(&state) {
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
    let mut runtime = match open_runtime(&state) {
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
    let mut runtime = match open_runtime(&state) {
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

    let mut runtime = match open_runtime(&state) {
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

fn open_runtime(state: &ProxyState) -> RouteResult<SqliteRuntime> {
    let db_path = state.config.sqlite_db_path.as_deref().ok_or_else(|| {
        Box::new(error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "Rust taxonomy accounts DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH",
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
