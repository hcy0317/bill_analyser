use std::{error::Error, fs, net::SocketAddr, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::any,
    Router,
};
use base64::Engine as _;
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState, BILL_CRUD_PROXIED_ROUTE_PATTERNS,
    BILL_CRUD_ROUTE_PATTERNS,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "bills-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn bills_crud_runtime_serves_owned_routes_and_writes_db() -> Result<(), Box<dyn Error>> {
    assert!(BILL_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/bills")));
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills",
            json!({
                "date": "2026-05-08 09:00:00",
                "type": "收入",
                "amount": 123.45,
                "counterparty": "ACME",
                "description": "Salary",
                "source_account_id": 10,
                "category_id": 1,
                "tag_ids": [1]
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["result"]["amount"], 12345);
    assert_eq!(create_body["result"]["categoryName"], "工资");
    let bill_id = create_body["result"]["id"]
        .as_str()
        .expect("created id")
        .to_string();

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/?page=1&page_size=20&type=2&keyword=Salary&tagIds=1",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    assert_eq!(
        list_body["result"]["items"]
            .as_array()
            .expect("items")
            .len(),
        1
    );
    assert_eq!(list_body["result"]["total"], 1);

    let get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/bills/get?id={bill_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    assert_eq!(read_json(get_response).await["result"]["comment"], "Salary");

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/bills/{bill_id}"),
            json!({
                "amount": 150.0,
                "description": "Bonus"
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["result"]["amount"], 15000);
    assert_eq!(account_balance(&fixture.db_path, 10)?, 150.0);

    let legacy_modify_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/modify",
            json!({
                "id": bill_id,
                "remark": "Updated remark"
            }),
        ))
        .await?;
    assert_eq!(legacy_modify_response.status(), StatusCode::OK);
    assert_eq!(read_json(legacy_modify_response).await["success"], true);

    let delete_response = app
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/bills/{bill_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);
    assert_eq!(bill_count(&fixture.db_path)?, 0);
    assert_eq!(account_balance(&fixture.db_path, 10)?, 0.0);

    Ok(())
}

#[tokio::test]
async fn bills_runtime_covers_batch_month_filters_and_error_edges() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let batch_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/batch",
            json!({
                "transactions": [
                    backend_bill_payload("2026-05-08 10:00:00", "收入", 12.5, "ACME", "May salary"),
                    backend_bill_payload("2026-06-08 10:00:00", "收入", 18.0, "ACME", "June salary")
                ]
            }),
        ))
        .await?;
    assert_eq!(batch_response.status(), StatusCode::CREATED);
    let batch_body = read_json(batch_response).await;
    assert_eq!(batch_body["success"], true);
    assert_eq!(batch_body["result"]["createdCount"], 2);
    let ids = batch_body["result"]["ids"].as_array().expect("ids");
    let first_id = ids[0].as_str().expect("first id").to_string();
    let second_id = ids[1].as_str().expect("second id").to_string();

    let month_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/by-month?year=2026&month=5&categoryIds=1&amountFilter=between:10:13&accountIds=10",
            Body::empty(),
        ))
        .await?;
    assert_eq!(month_response.status(), StatusCode::OK);
    let month_body = read_json(month_response).await;
    assert_eq!(month_body["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(month_body["result"]["items"][0]["comment"], "May salary");

    let path_get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/bills/{first_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(path_get_response.status(), StatusCode::OK);
    assert_eq!(read_json(path_get_response).await["result"]["amount"], 1250);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills?page=1&count=5&type=2&main_category=%E5%B7%A5%E8%B5%84&sub_category=&batch_id=batch-a&counterparty=ACME&description=salary&keyword=May&accountIds=10&categoryIds=1&tagIds=1&amountFilter=gte:10&min_time=1778198400000&max_time=1778284800000",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    assert_eq!(read_json(list_response).await["result"]["total"], 1);

    let batch_update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/bills/batch/update",
            json!({
                "billIds": [first_id, "999"],
                "updates": {
                    "amount": 42.0,
                    "description": "Batch updated"
                }
            }),
        ))
        .await?;
    assert_eq!(batch_update_response.status(), StatusCode::OK);
    let batch_update_body = read_json(batch_update_response).await;
    assert_eq!(batch_update_body["result"]["updated_count"], 1);
    assert_eq!(batch_update_body["result"]["failed_count"], 1);

    let legacy_delete_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/delete",
            json!({"id": first_id}),
        ))
        .await?;
    assert_eq!(legacy_delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(legacy_delete_response).await["success"], true);

    let batch_delete_response = app
        .clone()
        .oneshot(json_request(
            Method::DELETE,
            "/api/bills/batch/delete",
            json!({"ids": second_id}),
        ))
        .await?;
    assert_eq!(batch_delete_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(batch_delete_response).await["result"]["deleted_count"],
        1
    );

    for (method, path, body, status) in [
        (
            Method::GET,
            "/api/bills/by-month?year=2026&month=13",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/bills/get",
            Value::Null,
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::GET,
            "/api/bills/999",
            Value::Null,
            StatusCode::NOT_FOUND,
        ),
        (
            Method::PUT,
            "/api/bills/batch/update",
            json!({"updates": {"amount": 1.0}}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::DELETE,
            "/api/bills/batch/delete",
            json!({"ids": []}),
            StatusCode::BAD_REQUEST,
        ),
        (
            Method::POST,
            "/api/bills/batch",
            json!({"transactions": [false]}),
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let request = if body.is_null() {
            authed_request(method, path, Body::empty())
        } else {
            json_request(method, path, body)
        };
        let response = app.clone().oneshot(request).await?;
        assert_eq!(response.status(), status, "{path}");
    }

    let missing_db_state = ProxyState::new(
        HttpShellConfig::new_with_import_route_mode(
            "http://127.0.0.1:9".to_string(),
            Duration::from_secs(1),
            1024 * 1024,
            ImportRouteMode::ImportDbRuntime,
        )?
        .with_trusted_user_header_secret(TEST_AUTH_SECRET),
    )?;
    let missing_db_response = build_router(missing_db_state)
        .oneshot(authed_request(Method::GET, "/api/bills", Body::empty()))
        .await?;
    assert_eq!(
        missing_db_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let unauthenticated_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/bills")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn bills_runtime_exports_csv_and_xlsx_without_python_proxy() -> Result<(), Box<dyn Error>> {
    assert!(BILL_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/bills/export")));
    assert!(BILL_CRUD_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("GET", "/api/bills/export")));
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let unsupported_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/export?format=pdf",
            Body::empty(),
        ))
        .await?;
    assert_eq!(unsupported_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(unsupported_response).await["error"],
        "Unsupported export format"
    );

    let empty_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/export?format=csv",
            Body::empty(),
        ))
        .await?;
    assert_eq!(empty_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(empty_response).await["error"],
        "No bills to export"
    );

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills",
            json!({
                "date": "2026-05-09 08:30:00",
                "type": "支出",
                "amount": 8.8,
                "counterparty": "=Formula Shop",
                "description": "  -csv injection guard",
                "payment_method": "@card",
                "main_category": "+food",
                "sub_category": "早餐",
                "source_account_id": 10
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let csv_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/export?format=csv",
            Body::empty(),
        ))
        .await?;
    assert_eq!(csv_response.status(), StatusCode::OK);
    assert!(csv_response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .expect("content type")
        .starts_with("text/csv"));
    assert!(csv_response
        .headers()
        .get("content-disposition")
        .and_then(|value| value.to_str().ok())
        .expect("content disposition")
        .contains("bills_export_"));
    let csv_text = read_text(csv_response).await;
    assert!(csv_text.starts_with('\u{feff}'));
    assert!(csv_text.contains("date,type,amount,counterparty,description,payment_method,main_category,sub_category,source_account_id,destination_account_id,destination_amount"));
    assert!(csv_text.contains("'=Formula Shop"));
    assert!(csv_text.contains("'  -csv injection guard"));
    assert!(csv_text.contains("'@card"));
    assert!(csv_text.contains("'+food"));

    let xlsx_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/export?format=xls",
            Body::empty(),
        ))
        .await?;
    assert_eq!(xlsx_response.status(), StatusCode::OK);
    assert!(xlsx_response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .expect("xlsx content type")
        .starts_with("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"));
    assert!(xlsx_response
        .headers()
        .get("content-disposition")
        .and_then(|value| value.to_str().ok())
        .expect("xlsx content disposition")
        .contains(".xlsx"));
    let xlsx_bytes = read_bytes(xlsx_response).await;
    assert!(xlsx_bytes.starts_with(b"PK"));
    let xlsx_text = String::from_utf8_lossy(&xlsx_bytes);
    assert!(xlsx_text.contains("xl/worksheets/sheet1.xml"));
    assert!(xlsx_text.contains("'=Formula Shop"));

    let unauthenticated_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/bills/export")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn bills_runtime_serves_recurring_candidates_and_match_without_python_proxy(
) -> Result<(), Box<dyn Error>> {
    for route in [
        ("GET", "/api/bills/{bill_id}/recurring-candidates"),
        ("PUT", "/api/bills/{bill_id}/recurring-match"),
        ("DELETE", "/api/bills/{bill_id}/recurring-match"),
    ] {
        assert!(BILL_CRUD_ROUTE_PATTERNS.iter().any(|item| item == &route));
        assert!(BILL_CRUD_PROXIED_ROUTE_PATTERNS
            .iter()
            .all(|item| item != &route));
    }

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);
    insert_recurring_template(&fixture.db_path, 500, "rent", 4321.0, "2026-03-01", "8")?;

    let missing_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/bills/999/recurring-candidates",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(read_json(missing_response).await["error"], "Bill not found");

    let missing_recurring_id_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/bills/999/recurring-match",
            json!({}),
        ))
        .await?;
    assert_eq!(
        missing_recurring_id_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_recurring_id_response).await["error"],
        "Missing recurringId"
    );

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills",
            json!({
                "date": "2026-03-08 08:30:00",
                "type": "支出",
                "amount": 43.21,
                "counterparty": "房东",
                "description": "monthly rent",
                "source_account_id": 10,
                "main_category": "房租"
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let bill_id = read_json(create_response).await["result"]["id"]
        .as_str()
        .expect("bill id")
        .to_string();

    let candidates_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/bills/{bill_id}/recurring-candidates?toleranceDays=2"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(candidates_response.status(), StatusCode::OK);
    let candidates_body = read_json(candidates_response).await;
    assert_eq!(candidates_body["success"], true);
    assert_eq!(candidates_body["result"]["billId"], bill_id.parse::<i64>()?);
    assert_eq!(candidates_body["result"]["linkedRecurringId"], Value::Null);
    assert_eq!(
        candidates_body["result"]["candidates"][0]["id"],
        Value::String("500".to_string())
    );
    assert_eq!(
        candidates_body["result"]["candidates"][0]["matchedOccurrenceDate"],
        "2026-03-08"
    );
    assert!(
        candidates_body["result"]["candidates"][0]["matchScore"]
            .as_i64()
            .unwrap()
            >= 90
    );
    assert_eq!(candidates_body["result"]["candidates"][0]["linked"], false);

    let bind_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/bills/{bill_id}/recurring-match"),
            json!({"recurringId": 500}),
        ))
        .await?;
    assert_eq!(bind_response.status(), StatusCode::OK);
    let bind_body = read_json(bind_response).await;
    assert_eq!(bind_body["success"], true);
    assert_eq!(bind_body["result"]["recurringId"], 500);
    assert_eq!(bind_body["result"]["nextScheduledDate"], "2026-04-08");
    assert_eq!(recurring_next_date(&fixture.db_path, 500)?, "2026-04-08");
    assert_eq!(
        bill_created_from_recurring(&fixture.db_path, bill_id.parse::<i64>()?)?,
        Some(500)
    );

    let linked_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/bills/{bill_id}/recurring-candidates"),
            Body::empty(),
        ))
        .await?;
    let linked_body = read_json(linked_response).await;
    assert_eq!(linked_body["result"]["linkedRecurringId"], 500);
    assert_eq!(linked_body["result"]["linkedRecurringName"], "rent");
    assert_eq!(linked_body["result"]["candidates"][0]["linked"], true);

    let unbind_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/bills/{bill_id}/recurring-match"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(unbind_response.status(), StatusCode::OK);
    assert_eq!(read_json(unbind_response).await["result"], true);
    assert_eq!(recurring_next_date(&fixture.db_path, 500)?, "2026-03-08");
    assert_eq!(
        bill_created_from_recurring(&fixture.db_path, bill_id.parse::<i64>()?)?,
        None
    );

    let unauthenticated_response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/bills/{bill_id}/recurring-candidates"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn bills_runtime_keeps_unmigrated_bill_subdomains_proxied() -> Result<(), Box<dyn Error>> {
    assert!(BILL_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/bills/pictures")));
    assert!(BILL_CRUD_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/bills/pictures")));
    assert!(BILL_CRUD_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/bills/pictures/unused")));
    assert!(BILL_CRUD_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/bills/pictures/unused")));
    assert!(BILL_CRUD_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/bills/reconciliation_statements")));
    assert!(BILL_CRUD_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("DELETE", "/api/bills/{bill_id}/recurring-match")));
    let upstream = spawn_fake_upstream().await;
    let fixture = RuntimeFixture::new_with_upstream(upstream.url())?;
    let app = runtime_router(&fixture);

    for (method, path) in [
        (Method::GET, "/api/bills/reconciliation_statements"),
        (Method::POST, "/api/bills/category/quick-add-keyword"),
        (Method::POST, "/api/bills/category/refresh"),
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(method, path, Body::empty()))
            .await?;
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        let body = read_json(response).await;
        assert_eq!(body["runtime"], "python-sidecar", "{path}");
        assert_eq!(
            body["path"],
            path.split('?').next().unwrap_or(path),
            "{path}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn bills_runtime_serves_transaction_picture_upload_and_cleanup() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let unauthenticated_upload_response = app
        .clone()
        .oneshot(unauthenticated_multipart_picture_request(
            "/api/bills/pictures",
            "picture",
            Some("receipt.png"),
            b"not-authed",
        ))
        .await?;
    assert_eq!(
        unauthenticated_upload_response.status(),
        StatusCode::UNAUTHORIZED
    );

    let missing_response = app
        .clone()
        .oneshot(multipart_picture_request(
            "/api/bills/pictures",
            "not_picture",
            Some("receipt.png"),
            b"ignored",
        ))
        .await?;
    assert_eq!(missing_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_response).await["error"],
        "Missing picture file"
    );

    let invalid_name_response = app
        .clone()
        .oneshot(multipart_picture_request(
            "/api/bills/pictures",
            "picture",
            Some(""),
            b"empty-name",
        ))
        .await?;
    assert_eq!(invalid_name_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_name_response).await["error"],
        "Invalid picture file"
    );

    let unsupported_response = app
        .clone()
        .oneshot(multipart_picture_request(
            "/api/bills/pictures",
            "picture",
            Some("receipt.txt"),
            b"plain text",
        ))
        .await?;
    assert_eq!(unsupported_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(unsupported_response).await["error"],
        "Picture type not allowed. Supported: bmp, gif, jpeg, jpg, png, webp"
    );

    let malformed_response = app
        .clone()
        .oneshot(malformed_multipart_picture_request("/api/bills/pictures"))
        .await?;
    assert_eq!(
        malformed_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let malformed_header_response = app
        .clone()
        .oneshot(malformed_multipart_header_request("/api/bills/pictures"))
        .await?;
    assert_eq!(
        malformed_header_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let blocked_upload_app = runtime_router_with_uploads_dir(&fixture, &fixture.db_path);
    let blocked_upload_response = blocked_upload_app
        .oneshot(multipart_picture_request(
            "/api/bills/pictures",
            "picture",
            Some("receipt.png"),
            b"blocked",
        ))
        .await?;
    assert_eq!(
        blocked_upload_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    let picture_bytes = b"\x89PNG";
    let upload_response = app
        .clone()
        .oneshot(multipart_picture_request(
            "/api/bills/pictures",
            "picture",
            Some("../receipt.png"),
            picture_bytes,
        ))
        .await?;
    assert_eq!(upload_response.status(), StatusCode::OK);
    let upload_body = read_json(upload_response).await;
    assert_eq!(upload_body["success"], true);
    let picture_id = upload_body["result"]["pictureId"]
        .as_str()
        .expect("picture id");
    assert!(picture_id.ends_with(".png"));
    assert_eq!(
        upload_body["result"]["originalUrl"],
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(picture_bytes)
        )
    );
    let stored_path = fixture.uploads_dir.join(picture_id);
    assert_eq!(fs::read(&stored_path)?, picture_bytes);

    let unauthenticated_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/bills/pictures/unused")
                .header("content-type", "application/json")
                .body(Body::from("{}"))?,
        )
        .await?;
    assert_eq!(
        unauthenticated_delete_response.status(),
        StatusCode::UNAUTHORIZED
    );

    let empty_body_delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/bills/pictures/unused",
            Body::empty(),
        ))
        .await?;
    assert_eq!(empty_body_delete_response.status(), StatusCode::BAD_REQUEST);

    let delete_missing_id_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/pictures/unused",
            json!({}),
        ))
        .await?;
    assert_eq!(delete_missing_id_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(delete_missing_id_response).await["error"],
        "Missing picture id"
    );

    let object_id_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/pictures/unused",
            json!({"id": {}}),
        ))
        .await?;
    assert_eq!(object_id_response.status(), StatusCode::BAD_REQUEST);

    fs::create_dir_all(&fixture.uploads_dir)?;
    let numeric_picture_path = fixture.uploads_dir.join("123");
    fs::write(&numeric_picture_path, b"stale")?;
    let numeric_delete_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/pictures/unused",
            json!({"id": 123}),
        ))
        .await?;
    assert_eq!(numeric_delete_response.status(), StatusCode::OK);
    assert!(!numeric_picture_path.exists());

    let boolean_picture_path = fixture.uploads_dir.join("true");
    fs::write(&boolean_picture_path, b"stale")?;
    let boolean_delete_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/bills/pictures/unused",
            json!({"id": true}),
        ))
        .await?;
    assert_eq!(boolean_delete_response.status(), StatusCode::OK);
    assert!(!boolean_picture_path.exists());

    let delete_response = app
        .oneshot(json_request(
            Method::POST,
            "/api/bills/pictures/unused",
            json!({"id": picture_id}),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);
    assert!(!stored_path.exists());

    Ok(())
}

#[tokio::test]
async fn runtime_metadata_declares_import_and_bills_crud_boundary() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let metadata_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/runtime", Body::empty()))
        .await?;
    assert_eq!(metadata_response.status(), StatusCode::OK);
    let metadata = read_json(metadata_response).await;
    assert_eq!(
        metadata["runtime_boundary"],
        "rust-http-shell:import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-recurring-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime+auth-login-register-token-account-recovery-profile-cloud-external-auth-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime"
    );
    assert_eq!(
        metadata["business_migration"],
        "import-db-runtime+bills-crud-runtime+bills-picture-runtime+bills-export-runtime+bills-recurring-runtime+budgets-crud-execution-forecast-history-import-runtime+statistics-read-runtime-partial+auth-login-register-token-session-personal-refresh-logout-account-recovery-oauth2-authorize-profile-cloud-external-auth-system-user-data-statistics-2fa-status-verify-recovery-write-step-up-export-clear-runtime"
    );

    let health_response = app
        .oneshot(authed_request(Method::GET, "/api/health", Body::empty()))
        .await?;
    assert_eq!(health_response.status(), StatusCode::OK);
    let health = read_json(health_response).await;
    assert!(health["details"]["owned_routes"]
        .as_str()
        .expect("owned routes")
        .contains("bills CRUD runtime routes"));
    assert!(health["details"]["owned_routes"]
        .as_str()
        .expect("owned routes")
        .contains("bills picture runtime routes"));
    assert!(health["details"]["owned_routes"]
        .as_str()
        .expect("owned routes")
        .contains("bills export runtime route"));
    assert!(health["details"]["owned_routes"]
        .as_str()
        .expect("owned routes")
        .contains("bills recurring runtime routes"));
    assert!(health["details"]["bills_crud_runtime"].as_str().is_some());
    assert!(health["details"]["budgets_crud_runtime"].as_str().is_some());
    assert!(health["details"]["statistics_read_runtime"]
        .as_str()
        .is_some());
    assert!(health["details"]["auth_token_runtime"].as_str().is_some());

    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
    uploads_dir: std::path::PathBuf,
    upstream: String,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        Self::new_with_upstream("http://127.0.0.1:9".to_string())
    }

    fn new_with_upstream(upstream: String) -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("bills-http.db");
        let uploads_dir = temp_dir.path().join("uploads");
        init_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
            uploads_dir,
            upstream,
        })
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    runtime_router_with_uploads_dir(fixture, &fixture.uploads_dir)
}

fn runtime_router_with_uploads_dir(fixture: &RuntimeFixture, uploads_dir: &Path) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        fixture.upstream.clone(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.display().to_string())
    .with_uploads_dir(uploads_dir.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn init_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL
        );
        CREATE TABLE accounts(
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            type INTEGER NOT NULL,
            category INTEGER,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            date TEXT NOT NULL,
            type TEXT NOT NULL,
            amount REAL NOT NULL,
            counterparty TEXT NOT NULL,
            description TEXT NOT NULL,
            payment_method TEXT DEFAULT '',
            main_category TEXT,
            sub_category TEXT,
            batch_id TEXT,
            hash TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER
        );
        CREATE UNIQUE INDEX idx_bills_user_hash_unique ON bills(user_id, hash);
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY (bill_id, tag_id)
        );
        CREATE TABLE recurring_bills (
            id INTEGER PRIMARY KEY,
            user_id INTEGER NOT NULL DEFAULT 1,
            template_id INTEGER,
            name TEXT NOT NULL,
            description TEXT,
            type INTEGER,
            category TEXT,
            amount REAL,
            account TEXT,
            counterparty TEXT,
            destination_amount REAL,
            hide_amount INTEGER DEFAULT 0,
            tag TEXT,
            comment TEXT,
            frequency TEXT,
            scheduled_frequency_type INTEGER,
            start_date TEXT,
            end_date TEXT,
            next_date TEXT,
            enabled INTEGER DEFAULT 1,
            auto_create INTEGER DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            utc_offset INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )?;
    connection.execute("INSERT INTO users(id, username) VALUES (42, 'owner')", [])?;
    connection.execute(
        "INSERT INTO accounts(id, user_id, name, type, balance, initial_balance, created_at, updated_at)
         VALUES (10, 42, 'cash', 1, 0.0, 0.0, 'now', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO categories(id, user_id, type, main_category, sub_category, created_at)
         VALUES (1, 42, 2, '工资', '', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO tags(id, user_id, name, created_at, updated_at)
         VALUES (1, 42, 'salary', 'now', 'now')",
        [],
    )?;
    Ok(())
}

fn insert_recurring_template(
    path: &Path,
    recurring_id: i64,
    name: &str,
    amount: f64,
    start_date: &str,
    frequency: &str,
) -> Result<(), Box<dyn Error>> {
    Connection::open(path)?.execute(
        "INSERT INTO recurring_bills(
            id, user_id, name, type, category, amount, account, counterparty,
            destination_amount, frequency, scheduled_frequency_type, start_date,
            end_date, next_date, enabled, display_order, created_at, updated_at
         ) VALUES (?1, 42, ?2, 3, '1', ?3, '10', '0', 0, ?4, 2, ?5, '', ?5, 1, 0, 'now', 'now')",
        rusqlite::params![recurring_id, name, amount, frequency, start_date],
    )?;
    Ok(())
}

fn recurring_next_date(path: &Path, recurring_id: i64) -> Result<String, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT next_date FROM recurring_bills WHERE id = ?1",
        [recurring_id],
        |row| row.get::<_, String>(0),
    )?)
}

fn bill_created_from_recurring(path: &Path, bill_id: i64) -> Result<Option<i64>, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT created_from_recurring FROM bills WHERE id = ?1",
        [bill_id],
        |row| row.get::<_, Option<i64>>(0),
    )?)
}

fn account_balance(path: &Path, account_id: i64) -> Result<f64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT balance FROM accounts WHERE id = ?1",
        [account_id],
        |row| row.get::<_, f64>(0),
    )?)
}

fn bill_count(path: &Path) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row("SELECT COUNT(*) FROM bills", [], |row| row.get(0))?)
}

fn json_request(method: Method, uri: &str, body: Value) -> Request<Body> {
    authed_request(method, uri, Body::from(body.to_string()))
}

fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", TEST_USER_ID)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
}

fn multipart_picture_request(
    uri: &str,
    field_name: &str,
    filename: Option<&str>,
    file_bytes: &[u8],
) -> Request<Body> {
    let boundary = "bill-analyser-picture-test-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{field_name}\"").as_bytes(),
    );
    if let Some(filename) = filename {
        body.extend_from_slice(format!("; filename=\"{filename}\"").as_bytes());
    }
    body.extend_from_slice(b"\r\nContent-Type: image/png\r\n\r\n");
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("x-user-id", TEST_USER_ID)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(Body::from(body))
        .expect("multipart request builds")
}

fn unauthenticated_multipart_picture_request(
    uri: &str,
    field_name: &str,
    filename: Option<&str>,
    file_bytes: &[u8],
) -> Request<Body> {
    let boundary = "bill-analyser-picture-test-boundary";
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{field_name}\"").as_bytes(),
    );
    if let Some(filename) = filename {
        body.extend_from_slice(format!("; filename=\"{filename}\"").as_bytes());
    }
    body.extend_from_slice(b"\r\nContent-Type: image/png\r\n\r\n");
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .expect("multipart request builds")
}

fn malformed_multipart_picture_request(uri: &str) -> Request<Body> {
    let boundary = "bill-analyser-broken-boundary";
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("x-user-id", TEST_USER_ID)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(Body::from(format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"picture\"; filename=\"receipt.png\"\r\n\r\nbroken"
        )))
        .expect("multipart request builds")
}

fn malformed_multipart_header_request(uri: &str) -> Request<Body> {
    let boundary = "bill-analyser-bad-header-boundary";
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .header("x-user-id", TEST_USER_ID)
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(Body::from(format!(
            "--{boundary}\r\nContent-Disposition\r\n\r\nbroken\r\n--{boundary}--\r\n"
        )))
        .expect("multipart request builds")
}

struct FakeUpstream {
    addr: SocketAddr,
}

impl FakeUpstream {
    fn url(&self) -> String {
        format!("http://{}", self.addr)
    }
}

fn backend_bill_payload(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
) -> Value {
    json!({
        "date": date,
        "type": bill_type,
        "amount": amount,
        "counterparty": counterparty,
        "description": description,
        "payment_method": "manual",
        "main_category": "工资",
        "sub_category": "",
        "batch_id": "batch-a",
        "source_account_id": 10,
        "category_id": 1,
        "tag_ids": [1]
    })
}

async fn spawn_fake_upstream() -> FakeUpstream {
    let app = Router::new().fallback(any(echo_handler));
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener binds");
    let addr = listener.local_addr().expect("local addr");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("fake upstream serves");
    });

    FakeUpstream { addr }
}

async fn echo_handler(request: Request<Body>) -> impl IntoResponse {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 1024 * 1024).await.expect("body bytes");
    axum::Json(json!({
        "runtime": "python-sidecar",
        "method": parts.method.as_str(),
        "path": parts.uri.path(),
        "query": parts.uri.query().unwrap_or(""),
        "body": String::from_utf8_lossy(&body).to_string(),
    }))
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = read_bytes(response).await;
    serde_json::from_slice(&bytes).expect("json body")
}

async fn read_text(response: axum::response::Response) -> String {
    String::from_utf8(read_bytes(response).await).expect("utf8 body")
}

async fn read_bytes(response: axum::response::Response) -> Vec<u8> {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    bytes.to_vec()
}
