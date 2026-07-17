use std::{error::Error, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    routing::{delete, get, post},
    Router,
};
use bill_analyser_http::{
    delete_import_config_handler, list_import_configs_handler, match_import_config_handler,
    save_import_config_handler, suggest_import_config_handler, HttpAppState, HttpShellConfig,
};
use serde_json::{json, Value};
use tower::ServiceExt;

#[path = "../db/postgres_test_support.rs"]
mod postgres_test_support;

const TRUST_SECRET: &str = "import-config-handler-secret";

#[tokio::test]
async fn handler_only_router_honors_exact_five_records_and_six_behaviors(
) -> Result<(), Box<dyn Error>> {
    let Some(test_db) =
        postgres_test_support::isolated_postgres_database("import_config_handler").await?
    else {
        return Ok(());
    };
    let user_a = seed_user(&test_db.pool, "import-handler-a").await?;
    let user_b = seed_user(&test_db.pool, "import-handler-b").await?;
    let app = handler_router(state_for_database(&test_db.db_name)?);

    for (method, path, body) in [
        (
            Method::GET,
            "/api/bills/import/configs?file_format=csv",
            None,
        ),
        (
            Method::POST,
            "/api/bills/import/configs",
            Some(save_payload(None, "Unauthorized save", false)),
        ),
        (
            Method::POST,
            "/api/bills/import/configs/match",
            Some(json!({"fileFormat":"csv","headers":["time"]})),
        ),
        (
            Method::POST,
            "/api/bills/import/configs/suggest",
            Some(json!({"fileFormat":"csv","headers":["time"]})),
        ),
        (Method::DELETE, "/api/bills/import/configs/1", None),
    ] {
        assert_eq!(
            request_without_auth(&app, method, path, body).await?.0,
            StatusCode::UNAUTHORIZED
        );
    }

    for path in [
        "/api/bills/import/configs",
        "/api/bills/import/configs/match",
        "/api/bills/import/configs/suggest",
    ] {
        assert_eq!(
            request_raw(&app, Method::POST, path, user_a, "{", true)
                .await?
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        request_json(
            &app,
            Method::POST,
            "/api/bills/import/configs/match",
            user_a,
            Some(json!({"fileFormat":"csv"})),
        )
        .await?
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request_json(
            &app,
            Method::POST,
            "/api/bills/import/configs/suggest",
            user_a,
            Some(json!({"fileFormat":"json","headers":["time"]})),
        )
        .await?
        .0,
        StatusCode::BAD_REQUEST
    );

    let (status, created) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs",
        user_a,
        Some(save_payload(None, "Monthly Card", true)),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["success"], true);
    assert_eq!(created["data"].as_object().unwrap().len(), 1);
    let config_id = created["data"]["id"].as_i64().expect("created id");

    let (status, listed) = request_json(
        &app,
        Method::GET,
        "/api/bills/import/configs?file_format=CSV",
        user_a,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    let dto = &listed["data"][0];
    assert_exact_dto(dto);
    assert_eq!(dto["id"], config_id);
    assert_eq!(dto["fileFormat"], "csv");
    assert!(dto["delimiter"].is_null());
    for key in ["createdAt", "updatedAt"] {
        let timestamp = dto[key].as_str().expect("timestamp string");
        assert!(chrono::DateTime::parse_from_rfc3339(timestamp).is_ok());
        assert!(timestamp.ends_with('Z'));
    }

    let (status, other_list) = request_json(
        &app,
        Method::GET,
        "/api/bills/import/configs?file_format=csv",
        user_b,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other_list["data"], json!([]));

    let (status, _) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs",
        user_b,
        Some(save_payload(Some(config_id), "Hidden", false)),
    )
    .await?;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs",
        user_a,
        Some(save_payload(None, " monthly   card ", false)),
    )
    .await?;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, matched) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs/match",
        user_a,
        Some(json!({"fileFormat":"csv","headers":[" 交易时间 "," 金额 "]})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_exact_match(&matched["data"]);
    assert_eq!(matched["data"]["defaultRecommendation"], false);
    assert_eq!(matched["data"]["matchReason"], "exact_headers");
    assert_eq!(matched["data"]["matchScore"], 1.0);

    let (status, fallback) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs/match",
        user_a,
        Some(json!({"fileFormat":"csv","headers":["unknown"]})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_exact_match(&fallback["data"]);
    assert_eq!(fallback["data"]["defaultRecommendation"], true);
    assert_eq!(fallback["data"]["matchReason"], "default_template_fallback");

    let before = persistent_counts(&test_db.pool).await?;
    let (status, suggested_without_rows) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs/suggest",
        user_a,
        Some(json!({"fileFormat":"csv","headers":["交易时间","金额","备注"]})),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_exact_suggestion(&suggested_without_rows["data"]);
    let (status, suggested_with_rows) = request_json(
        &app,
        Method::POST,
        "/api/bills/import/configs/suggest",
        user_a,
        Some(json!({
            "fileFormat":"csv",
            "headers":["交易时间","金额","收支类型"],
            "sampleRows":[["2026-07-14","10.00","支出"]]
        })),
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_exact_suggestion(&suggested_with_rows["data"]);
    assert_eq!(
        suggested_with_rows["data"]["transactionTypeMapping"]["支出"],
        3
    );
    assert_eq!(persistent_counts(&test_db.pool).await?, before);

    for (method, path, user_id, body) in [
        (Method::GET, "/api/bills/import/configs", user_a, None),
        (
            Method::GET,
            "/api/bills/import/configs?file_format=json",
            user_a,
            None,
        ),
        (
            Method::POST,
            "/api/bills/import/configs/match",
            user_a,
            Some(json!({"fileFormat":"csv","headers":[]})),
        ),
        (
            Method::POST,
            "/api/bills/import/configs/suggest",
            user_a,
            Some(json!({"fileFormat":"csv","headers":[]})),
        ),
        (
            Method::DELETE,
            "/api/bills/import/configs/not-an-id",
            user_a,
            None,
        ),
    ] {
        assert_eq!(
            request_json(&app, method, path, user_id, body).await?.0,
            StatusCode::BAD_REQUEST
        );
    }

    let mut missing = save_payload(None, "Missing delimiter", false);
    missing.as_object_mut().unwrap().remove("delimiter");
    assert_eq!(
        request_json(
            &app,
            Method::POST,
            "/api/bills/import/configs",
            user_a,
            Some(missing),
        )
        .await?
        .0,
        StatusCode::BAD_REQUEST
    );
    let mut read_only = save_payload(None, "Read only time", false);
    read_only
        .as_object_mut()
        .unwrap()
        .insert("createdAt".to_string(), json!("2026-07-14T00:00:00Z"));
    assert_eq!(
        request_json(
            &app,
            Method::POST,
            "/api/bills/import/configs",
            user_a,
            Some(read_only),
        )
        .await?
        .0,
        StatusCode::BAD_REQUEST
    );

    assert_eq!(
        request_json(
            &app,
            Method::DELETE,
            &format!("/api/bills/import/configs/{config_id}"),
            user_b,
            None,
        )
        .await?
        .0,
        StatusCode::NOT_FOUND
    );
    let (status, deleted) = request_json(
        &app,
        Method::DELETE,
        &format!("/api/bills/import/configs/{config_id}"),
        user_a,
        None,
    )
    .await?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        deleted,
        json!({"success":true,"data":{"id":config_id,"deleted":true}})
    );
    assert_eq!(
        request_json(
            &app,
            Method::DELETE,
            &format!("/api/bills/import/configs/{config_id}"),
            user_a,
            None,
        )
        .await?
        .0,
        StatusCode::NOT_FOUND
    );

    test_db.cleanup().await?;
    Ok(())
}

fn handler_router(state: HttpAppState) -> Router {
    Router::new()
        .route(
            "/api/bills/import/configs",
            get(list_import_configs_handler).post(save_import_config_handler),
        )
        .route(
            "/api/bills/import/configs/match",
            post(match_import_config_handler),
        )
        .route(
            "/api/bills/import/configs/suggest",
            post(suggest_import_config_handler),
        )
        .route(
            "/api/bills/import/configs/:config_id",
            delete(delete_import_config_handler),
        )
        .with_state(state)
}

fn state_for_database(database: &str) -> Result<HttpAppState, Box<dyn Error>> {
    let base_url = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL")?;
    let mut url = url::Url::parse(&base_url)?;
    url.set_path(&format!("/{database}"));
    let config = HttpShellConfig::new("", Duration::from_secs(2), 1024 * 1024)?
        .with_postgres_url(url.to_string())?
        .with_trusted_user_header_secret(TRUST_SECRET);
    Ok(HttpAppState::new(config)?)
}

async fn request_json(
    app: &Router,
    method: Method,
    path: &str,
    user_id: i64,
    body: Option<Value>,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let text = body
        .map(|value| serde_json::to_string(&value).expect("serialize request"))
        .unwrap_or_default();
    request_raw(app, method, path, user_id, &text, true).await
}

async fn request_without_auth(
    app: &Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let text = body
        .map(|value| serde_json::to_string(&value).expect("serialize request"))
        .unwrap_or_default();
    request_raw(app, method, path, 0, &text, false).await
}

async fn request_raw(
    app: &Router,
    method: Method,
    path: &str,
    user_id: i64,
    body: &str,
    authenticated: bool,
) -> Result<(StatusCode, Value), Box<dyn Error>> {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    let request = if authenticated {
        request
            .header("x-user-id", user_id.to_string())
            .header("x-bill-analyser-trusted-user-secret", TRUST_SECRET)
    } else {
        request
    };
    let request = request.body(Body::from(body.to_string()))?;
    let response = app.clone().oneshot(request).await?;
    let status = response.status();
    let value = serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await?)?;
    Ok((status, value))
}

fn save_payload(id: Option<i64>, name: &str, is_default: bool) -> Value {
    let mut payload = json!({
        "name": name,
        "fileFormat": "csv",
        "description": "A mapping template",
        "fieldMappings": {"columnMapping":{"1":0,"8":1}},
        "sampleHeaders": ["交易时间", "金额"],
        "dateFormat": "%Y-%m-%d %H:%M:%S",
        "delimiter": null,
        "encoding": "utf-8",
        "skipRows": 0,
        "hasHeader": true,
        "customRules": {"trim":true},
        "isDefault": is_default
    });
    if let Some(id) = id {
        payload
            .as_object_mut()
            .unwrap()
            .insert("id".to_string(), json!(id));
    }
    payload
}

fn assert_exact_dto(dto: &Value) {
    let mut keys = dto
        .as_object()
        .expect("DTO object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        [
            "createdAt",
            "customRules",
            "dateFormat",
            "delimiter",
            "description",
            "encoding",
            "fieldMappings",
            "fileFormat",
            "hasHeader",
            "id",
            "isDefault",
            "name",
            "sampleHeaders",
            "skipRows",
            "updatedAt",
        ]
    );
    assert_dto_field_types(dto);
}

fn assert_dto_field_types(dto: &Value) {
    assert!(dto["id"].is_number());
    for key in [
        "name",
        "fileFormat",
        "description",
        "dateFormat",
        "encoding",
        "createdAt",
        "updatedAt",
    ] {
        assert!(dto[key].is_string(), "{key} must be a string");
    }
    assert!(dto["fieldMappings"].is_object());
    assert!(dto["sampleHeaders"].is_array());
    assert!(dto["skipRows"].is_number());
    assert!(dto["hasHeader"].is_boolean());
    assert!(dto["customRules"].is_object());
    assert!(dto["isDefault"].is_boolean());
}

fn assert_exact_match(value: &Value) {
    assert_dto_field_types(value);
    let object = value.as_object().unwrap();
    assert_eq!(object.len(), 19);
    assert!(value["descriptionSummary"].is_string());
    assert!(value["defaultRecommendation"].is_boolean());
    assert!(value["matchScore"]
        .as_f64()
        .is_some_and(|score| (0.0..=1.0).contains(&score)));
    assert!(value["matchReason"].is_string());
}

fn assert_exact_suggestion(value: &Value) {
    let object = value.as_object().expect("suggestion object");
    assert_eq!(object.len(), 4);
    assert!(value["includeHeader"].is_boolean());
    assert!(value["columnMapping"].is_object());
    assert!(value["transactionTypeMapping"].is_object());
    let suggestions = value["suggestions"].as_array().expect("suggestions array");
    for item in suggestions {
        assert_eq!(item.as_object().unwrap().len(), 4);
        assert!(item["columnType"].is_number());
        assert!(item["columnIndex"].as_i64().is_some_and(|index| index >= 0));
        assert!(item["header"].is_string());
        assert!(item["score"]
            .as_f64()
            .is_some_and(|score| (0.0..=1.0).contains(&score)));
    }
}

async fn persistent_counts(
    pool: &bill_analyser_db::PostgresPool,
) -> Result<(i64, i64), sqlx::Error> {
    let configs = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM import_configs")
        .fetch_one(pool)
        .await?;
    let learning = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM import_learning_samples) + (SELECT COUNT(*) FROM import_learning_suggestions)",
    )
    .fetch_one(pool)
    .await?;
    Ok((configs, learning))
}

async fn seed_user(
    pool: &bill_analyser_db::PostgresPool,
    prefix: &str,
) -> Result<i64, sqlx::Error> {
    let unique = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .abs();
    sqlx::query_scalar("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
        .bind(format!("{prefix}-{unique}"))
        .bind(format!("{prefix}-{unique}@example.test"))
        .fetch_one(pool)
        .await
}
