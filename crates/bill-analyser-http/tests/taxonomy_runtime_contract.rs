use std::{error::Error, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState,
    TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS, TAXONOMY_ACCOUNT_ROUTE_PATTERNS,
    TAXONOMY_TAG_PROXIED_ROUTE_PATTERNS, TAXONOMY_TAG_ROUTE_PATTERNS,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "taxonomy-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn taxonomy_accounts_runtime_serves_crud_and_frontend_contract() -> Result<(), Box<dyn Error>>
{
    assert!(TAXONOMY_ACCOUNT_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/accounts/")));
    assert!(TAXONOMY_ACCOUNT_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("PUT", "/api/accounts/display-orders")));
    assert!(TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/accounts/{account_id}/transactions/move")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/accounts/", Body::empty()))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let accounts = list_body["result"].as_array().expect("account list");
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0]["id"], "10");
    assert_eq!(accounts[0]["name"], "工资卡");
    assert_eq!(accounts[0]["balance"], 1234);
    assert_eq!(accounts[0]["aliases"], json!(["主卡", "工资"]));
    assert_eq!(accounts[0]["hidden"], true);
    assert_eq!(accounts[0]["visible"], false);
    assert_eq!(accounts[0]["subAccounts"][0]["parentId"], "10");
    assert!(!accounts.iter().any(|account| account["name"] == "其他用户"));

    let get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/accounts/10",
            Body::empty(),
        ))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = read_json(get_response).await;
    assert_eq!(get_body["result"]["subAccounts"][0]["name"], "工资子账户");

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/",
            json!({
                "name": "现金账户",
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "balance": "2500",
                "aliases": [" 零钱 ", ""],
                "visible": true,
                "displayOrder": 5,
                "subAccounts": [{
                    "name": "现金零钱",
                    "type": 1,
                    "balance": 125,
                    "visible": false
                }]
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["result"]["name"], "现金账户");
    assert_eq!(create_body["result"]["balance"], 2500);
    assert_eq!(create_body["result"]["aliases"], json!(["零钱"]));
    assert_eq!(create_body["result"]["subAccounts"][0]["balance"], 125);
    assert_eq!(create_body["runtime"], Value::Null);
    let created_id = create_body["result"]["id"]
        .as_str()
        .expect("created id")
        .parse::<i64>()?;
    assert_eq!(account_balance(&fixture.db_path, created_id)?, 25.0);
    assert_eq!(child_count(&fixture.db_path, created_id)?, 1);

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/accounts/{created_id}"),
            json!({
                "name": "现金账户更新",
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "balance": 3099,
                "aliases": "备用, 零钱",
                "hidden": true,
                "displayOrder": 2
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["result"]["name"], "现金账户更新");
    assert_eq!(update_body["result"]["balance"], 3099);
    assert_eq!(update_body["result"]["aliases"], json!(["备用", "零钱"]));
    assert_eq!(update_body["result"]["hidden"], true);
    assert_eq!(account_balance(&fixture.db_path, created_id)?, 30.99);

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/accounts/{created_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);
    assert!(!account_exists(&fixture.db_path, created_id)?);
    assert_eq!(child_count(&fixture.db_path, created_id)?, 0);

    Ok(())
}

#[tokio::test]
async fn taxonomy_accounts_runtime_validates_payloads_and_user_scope() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let unauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/accounts/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauth_response.status(), StatusCode::UNAUTHORIZED);

    let missing_create = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/accounts/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_create).await["error"], "No data provided");

    let missing_update = app
        .clone()
        .oneshot(authed_request(
            Method::PUT,
            "/api/accounts/10",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_update.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_update).await["error"], "No data provided");

    let missing_account = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/accounts/999",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_account.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_account).await["error"],
        "Account not found"
    );

    let cross_user_delete = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/accounts/99",
            Body::empty(),
        ))
        .await?;
    assert_eq!(cross_user_delete.status(), StatusCode::NOT_FOUND);
    assert!(account_exists(&fixture.db_path, 99)?);

    let invalid_orders = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({"newDisplayOrders": [{"id": 10}]}),
        ))
        .await?;
    assert_eq!(invalid_orders.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_orders).await["error"],
        "Each item must have id and displayOrder"
    );

    let missing_orders = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({}),
        ))
        .await?;
    assert_eq!(missing_orders.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_orders).await["error"],
        "Missing newDisplayOrders parameter"
    );

    let non_list_orders = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({"newDisplayOrders": "bad"}),
        ))
        .await?;
    assert_eq!(non_list_orders.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(non_list_orders).await["error"],
        "newDisplayOrders must be a list"
    );

    let invalid_json = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/accounts/",
            Body::from("{"),
        ))
        .await?;
    assert_eq!(invalid_json.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(invalid_json).await["error"], "Invalid JSON");

    let scalar_body = app
        .clone()
        .oneshot(json_request(Method::POST, "/api/accounts/", json!(["bad"])))
        .await?;
    assert_eq!(scalar_body.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(scalar_body).await["error"],
        "Account payload must be an object"
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_account_display_orders_update_only_current_user() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({
                "newDisplayOrders": [
                    {"id": "10", "displayOrder": 20},
                    {"id": 11, "displayOrder": 10},
                    {"id": 99, "displayOrder": 1}
                ]
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read_json(response).await["result"], true);
    assert_eq!(display_order(&fixture.db_path, 10)?, 20);
    assert_eq!(display_order(&fixture.db_path, 11)?, 10);
    assert_eq!(display_order(&fixture.db_path, 99)?, 0);

    Ok(())
}

#[tokio::test]
async fn taxonomy_account_update_reconciles_direct_subaccounts() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/10",
            json!({
                "name": "工资卡更新",
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "icon": 12,
                "color": "#224466",
                "balance": 4321,
                "aliases": [true, false, null, 12, ""],
                "hidden": "0",
                "displayOrder": 7,
                "creditCardStatementDate": 18,
                "subAccounts": [
                    {
                        "id": 11,
                        "name": "工资子账户更新",
                        "parentId": 10,
                        "type": 1,
                        "balance": 999,
                        "visible": true
                    },
                    {
                        "name": "新子账户",
                        "type": 1,
                        "balance": 250,
                        "visible": false
                    }
                ]
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["result"]["name"], "工资卡更新");
    assert_eq!(body["result"]["icon"], "12");
    assert_eq!(
        body["result"]["aliases"],
        json!(["True", "False", "None", "12"])
    );
    assert_eq!(body["result"]["hidden"], false);
    assert_eq!(account_balance(&fixture.db_path, 10)?, 43.21);
    assert_eq!(account_balance(&fixture.db_path, 11)?, 9.99);
    assert_eq!(child_count(&fixture.db_path, 10)?, 2);
    assert_eq!(body["result"]["subAccounts"].as_array().unwrap().len(), 2);

    Ok(())
}

#[tokio::test]
async fn taxonomy_accounts_runtime_reports_config_errors_before_db_work(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router_without_db(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/accounts/", Body::empty()))
        .await?;
    assert_eq!(list_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(list_response).await["error"],
        "Rust taxonomy accounts DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let display_response = app
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({"newDisplayOrders": [{"id": 10, "displayOrder": 1}]}),
        ))
        .await?;
    assert_eq!(display_response.status(), StatusCode::SERVICE_UNAVAILABLE);

    Ok(())
}

#[tokio::test]
async fn taxonomy_tags_runtime_serves_crud_and_frontend_contract() -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_TAG_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/tags/")));
    assert!(TAXONOMY_TAG_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("PUT", "/api/tags/display-orders")));
    assert!(TAXONOMY_TAG_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tags/batch")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/tags/", Body::empty()))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let tags = list_body["result"].as_array().expect("tag list");
    assert_eq!(tags.len(), 2);
    assert_eq!(tags[0]["id"], "21");
    assert_eq!(tags[0]["name"], "通勤");
    assert_eq!(tags[0]["displayOrder"], 1);
    assert_eq!(tags[0]["hidden"], true);
    assert_eq!(tags[0]["visible"], false);
    assert_eq!(tags[1]["id"], "20");
    assert_eq!(tags[1]["color"], "#ff6600");
    assert!(!tags.iter().any(|tag| tag["name"] == "其他用户标签"));

    let get_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/tags/20", Body::empty()))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    assert_eq!(read_json(get_response).await["result"]["name"], "午饭");

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/tags/",
            json!({
                "name": "娱乐",
                "color": "#33aa55",
                "icon": "tag",
                "hidden": true
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["result"]["name"], "娱乐");
    assert!(!create_body["result"]["id"]
        .as_str()
        .expect("created id")
        .is_empty());
    assert_eq!(create_body["result"]["hidden"], true);
    assert_eq!(create_body["runtime"], Value::Null);
    let created_id = create_body["result"]["id"]
        .as_str()
        .expect("created id")
        .parse::<i64>()?;

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/tags/{created_id}"),
            json!({
                "id": created_id.to_string(),
                "name": "娱乐更新",
                "color": "#3355aa",
                "icon": "music",
                "hidden": false,
                "displayOrder": "6"
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["result"]["name"], "娱乐更新");
    assert_eq!(update_body["result"]["displayOrder"], 6);
    assert_eq!(update_body["result"]["hidden"], false);
    assert_eq!(tag_display_order(&fixture.db_path, created_id)?, 6);

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/tags/{created_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);
    assert!(!tag_exists(&fixture.db_path, created_id)?);

    Ok(())
}

#[tokio::test]
async fn taxonomy_tags_runtime_validates_payloads_and_user_scope() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let unauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/tags/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauth_response.status(), StatusCode::UNAUTHORIZED);

    for (method, uri, body) in [
        (Method::GET, "/api/tags/20", Body::empty()),
        (
            Method::POST,
            "/api/tags/",
            Body::from(json!({"name": "未授权"}).to_string()),
        ),
        (
            Method::PUT,
            "/api/tags/20",
            Body::from(json!({"name": "未授权"}).to_string()),
        ),
        (Method::DELETE, "/api/tags/20", Body::empty()),
        (
            Method::PUT,
            "/api/tags/display-orders",
            Body::from(json!({"newDisplayOrders": []}).to_string()),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(body)?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }

    let missing_create = app
        .clone()
        .oneshot(authed_request(Method::POST, "/api/tags/", Body::empty()))
        .await?;
    assert_eq!(missing_create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_create).await["error"], "name is required");

    let nameless_create = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/tags/",
            json!({"color": "#000"}),
        ))
        .await?;
    assert_eq!(nameless_create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(nameless_create).await["error"],
        "name is required"
    );

    let scalar_body = app
        .clone()
        .oneshot(json_request(Method::PUT, "/api/tags/20", json!(["bad"])))
        .await?;
    assert_eq!(scalar_body.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(scalar_body).await["error"],
        "Tag payload must be an object"
    );

    let invalid_display_order = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/20",
            json!({"displayOrder": "oops"}),
        ))
        .await?;
    assert_eq!(invalid_display_order.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_display_order).await["error"],
        "displayOrder must be an integer"
    );

    let nullable_update = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/20",
            json!({
                "visible": false,
                "color": null,
                "icon": null,
                "display_order": 8
            }),
        ))
        .await?;
    assert_eq!(nullable_update.status(), StatusCode::OK);
    let nullable_body = read_json(nullable_update).await;
    assert_eq!(nullable_body["result"]["hidden"], true);
    assert_eq!(nullable_body["result"]["color"], Value::Null);
    assert_eq!(nullable_body["result"]["icon"], Value::Null);
    assert_eq!(tag_display_order(&fixture.db_path, 20)?, 8);

    let missing_update = app
        .clone()
        .oneshot(authed_request(Method::PUT, "/api/tags/20", Body::empty()))
        .await?;
    assert_eq!(missing_update.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_update).await["error"], "No data provided");

    let missing_tag = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/tags/999", Body::empty()))
        .await?;
    assert_eq!(missing_tag.status(), StatusCode::NOT_FOUND);
    assert_eq!(read_json(missing_tag).await["error"], "Tag not found");

    let cross_user_delete = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/tags/98",
            Body::empty(),
        ))
        .await?;
    assert_eq!(cross_user_delete.status(), StatusCode::NOT_FOUND);
    assert!(tag_exists(&fixture.db_path, 98)?);

    let invalid_json = app
        .clone()
        .oneshot(authed_request(Method::POST, "/api/tags/", Body::from("{")))
        .await?;
    assert_eq!(invalid_json.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(invalid_json).await["error"], "Invalid JSON");

    Ok(())
}

#[tokio::test]
async fn taxonomy_tag_display_orders_update_only_current_user() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({
                "newDisplayOrders": [
                    {"id": "20", "displayOrder": 30},
                    {"id": 21, "displayOrder": 10},
                    {"id": 98, "displayOrder": 1}
                ]
            }),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(read_json(response).await["result"], true);
    assert_eq!(tag_display_order(&fixture.db_path, 20)?, 30);
    assert_eq!(tag_display_order(&fixture.db_path, 21)?, 10);
    assert_eq!(tag_display_order(&fixture.db_path, 98)?, 0);

    let missing_orders = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({}),
        ))
        .await?;
    assert_eq!(missing_orders.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_orders).await["error"],
        "newDisplayOrders is required"
    );

    let non_array_orders = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({"newDisplayOrders": "bad"}),
        ))
        .await?;
    assert_eq!(non_array_orders.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(non_array_orders).await["error"],
        "newDisplayOrders must be an array"
    );

    let missing_item_field = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({"newDisplayOrders": [{"id": 20}]}),
        ))
        .await?;
    assert_eq!(missing_item_field.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_item_field).await["error"],
        "Each item must have id and displayOrder"
    );

    let non_object_item = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({"newDisplayOrders": [20]}),
        ))
        .await?;
    assert_eq!(non_object_item.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(non_object_item).await["error"],
        "Each item must have id and displayOrder"
    );

    let invalid_item_field = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({"newDisplayOrders": [{"id": "oops", "displayOrder": 1}]}),
        ))
        .await?;
    assert_eq!(invalid_item_field.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_item_field).await["error"],
        "Invalid id or displayOrder: invalid literal for int() with base 10: 'oops'"
    );

    let invalid_display_order_value = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/tags/display-orders",
            json!({"newDisplayOrders": [{"id": 20, "displayOrder": null}]}),
        ))
        .await?;
    assert_eq!(
        invalid_display_order_value.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(invalid_display_order_value).await["error"],
        "Invalid id or displayOrder: int() argument must be a string, a bytes-like object or a real number, not 'NoneType'"
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_tags_runtime_reports_config_errors_before_db_work() -> Result<(), Box<dyn Error>>
{
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router_without_db(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(Method::GET, "/api/tags/", Body::empty()))
        .await?;
    assert_eq!(list_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(list_response).await["error"],
        "Rust taxonomy tags DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    for (method, uri, body) in [
        (Method::GET, "/api/tags/20", Body::empty()),
        (
            Method::POST,
            "/api/tags/",
            Body::from(json!({"name": "无库"}).to_string()),
        ),
        (
            Method::PUT,
            "/api/tags/20",
            Body::from(json!({"name": "无库更新"}).to_string()),
        ),
        (Method::DELETE, "/api/tags/20", Body::empty()),
        (
            Method::PUT,
            "/api/tags/display-orders",
            Body::from(json!({"newDisplayOrders": [{"id": 20, "displayOrder": 1}]}).to_string()),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(method, uri, body))
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
    }

    Ok(())
}

#[tokio::test]
async fn taxonomy_tags_runtime_reports_db_errors_for_missing_tag_schema(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    Connection::open(&fixture.db_path)?.execute("DROP TABLE tags", [])?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(authed_request(Method::GET, "/api/tags/", Body::empty()))
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        read_json(response).await["error"],
        "Rust taxonomy tags route runtime DB error"
    );

    Ok(())
}

struct RuntimeFixture {
    _temp_dir: TempDir,
    db_path: std::path::PathBuf,
}

impl RuntimeFixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("taxonomy-http.db");
        init_schema(&db_path)?;
        Ok(Self {
            _temp_dir: temp_dir,
            db_path,
        })
    }
}

fn runtime_router(fixture: &RuntimeFixture) -> Router {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:9".to_string(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
    .with_sqlite_db_path(fixture.db_path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = ProxyState::new(config).expect("proxy state");
    build_router(state)
}

fn runtime_router_without_db(fixture: &RuntimeFixture) -> Router {
    let _keep_temp_dir_alive = &fixture._temp_dir;
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:9".to_string(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )
    .expect("config")
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
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            type INTEGER NOT NULL,
            category INTEGER,
            currency TEXT DEFAULT 'CNY',
            icon TEXT,
            color TEXT,
            balance REAL DEFAULT 0,
            initial_balance REAL DEFAULT 0,
            hidden BOOLEAN DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            comment TEXT,
            aliases TEXT,
            parent_id INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        );
        CREATE TABLE tags (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            color TEXT,
            icon TEXT,
            display_order INTEGER DEFAULT 0,
            hidden BOOLEAN DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(user_id, name)
        );
        ",
    )?;
    connection.execute(
        "INSERT INTO users(id, username) VALUES (42, 'owner'), (77, 'other')",
        [],
    )?;
    connection.execute(
        "INSERT INTO accounts(
            id, user_id, name, type, category, currency, icon, color, balance,
            initial_balance, hidden, display_order, comment, aliases, parent_id,
            created_at, updated_at
        )
        VALUES
            (10, 42, '工资卡', 1, 2, 'CNY', 'card', '#336699', 12.34, 12.34, 1, 1, '主账户', '[\"主卡\",\"工资\"]', 0, 'now', 'now'),
            (11, 42, '工资子账户', 1, 2, 'CNY', 'wallet', '#336699', 0.50, 0.50, 0, 2, '', NULL, 10, 'now', 'now'),
            (99, 77, '其他用户', 1, 2, 'CNY', 'wallet', '#999999', 99.0, 99.0, 0, 0, '', NULL, 0, 'now', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO tags(
            id, user_id, name, color, icon, display_order, hidden, created_at, updated_at
        )
        VALUES
            (20, 42, '午饭', '#ff6600', 'food', 2, 0, '2026-01-01T00:00:00', 'now'),
            (21, 42, '通勤', '#0066ff', 'bus', 1, 1, '2026-01-02T00:00:00', 'now'),
            (98, 77, '其他用户标签', '#999999', 'tag', 0, 0, '2026-01-03T00:00:00', 'now')",
        [],
    )?;
    Ok(())
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

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

fn account_balance(path: &Path, account_id: i64) -> Result<f64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT balance FROM accounts WHERE id = ?1",
        [account_id],
        |row| row.get::<_, f64>(0),
    )?)
}

fn child_count(path: &Path, parent_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) FROM accounts WHERE parent_id = ?1",
        [parent_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn account_exists(path: &Path, account_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM accounts WHERE id = ?1",
        [account_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn display_order(path: &Path, account_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT display_order FROM accounts WHERE id = ?1",
        [account_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn tag_exists(path: &Path, tag_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM tags WHERE id = ?1",
        [tag_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn tag_display_order(path: &Path, tag_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT display_order FROM tags WHERE id = ?1",
        [tag_id],
        |row| row.get::<_, i64>(0),
    )?)
}
