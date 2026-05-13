use std::{error::Error, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use bill_analyser_http::{
    build_router, HttpShellConfig, ImportRouteMode, ProxyState,
    TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS, TAXONOMY_ACCOUNT_ROUTE_PATTERNS,
    TAXONOMY_CATEGORY_PROXIED_ROUTE_PATTERNS, TAXONOMY_CATEGORY_ROUTE_PATTERNS,
    TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS, TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS,
    TAXONOMY_RULE_CENTER_PROXIED_ROUTE_PATTERNS, TAXONOMY_RULE_CENTER_ROUTE_PATTERNS,
    TAXONOMY_SETTINGS_BUNDLE_PROXIED_ROUTE_PATTERNS, TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS,
    TAXONOMY_TAG_PROXIED_ROUTE_PATTERNS, TAXONOMY_TAG_ROUTE_PATTERNS,
    TAXONOMY_TEMPLATE_PROXIED_ROUTE_PATTERNS, TAXONOMY_TEMPLATE_ROUTE_PATTERNS,
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
    assert!(TAXONOMY_ACCOUNT_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/accounts/sync-balances")));
    assert!(TAXONOMY_ACCOUNT_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/accounts/{account_id}/transactions/move")));
    assert!(TAXONOMY_ACCOUNT_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/accounts/{account_id}/transactions/clear")));
    assert!(TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/accounts/{account_id}/transactions/move")));
    assert!(TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/accounts/{account_id}/transactions/clear")));
    assert!(TAXONOMY_ACCOUNT_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/accounts/sync-balances")));

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

    let unauth_sync_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/accounts/sync-balances")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauth_sync_response.status(), StatusCode::UNAUTHORIZED);

    for uri in [
        "/api/accounts/10/transactions/move",
        "/api/accounts/10/transactions/clear",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }

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
async fn taxonomy_account_sync_balances_recalculates_current_user_accounts(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let connection = Connection::open(&fixture.db_path)?;
    connection.execute(
        "INSERT INTO bills(
            id, user_id, type, amount, date, counterparty, description,
            source_account_id, destination_account_id, destination_amount,
            main_category, sub_category, created_at, updated_at
        )
        VALUES
            (55, 42, '收入', 5.0, '2026-01-13', '公司', '工资入账', 10, 0, 0, '', '', 'now', 'now'),
            (98, 77, '收入', 50.0, '2026-01-13', '其他公司', '其他用户入账', 99, 0, 0, '', '', 'now', 'now')",
        [],
    )?;
    drop(connection);
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/accounts/sync-balances",
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["result"]["total_accounts"], 2);
    assert_eq!(body["result"]["synced_accounts"], 2);
    assert_eq!(body["result"]["errors"], json!([]));
    let discrepancies = body["result"]["discrepancies"]
        .as_array()
        .expect("discrepancy list");
    assert_eq!(discrepancies.len(), 1);
    assert_eq!(discrepancies[0]["account_id"], 10);
    assert_eq!(discrepancies[0]["name"], "工资卡");
    assert_eq!(discrepancies[0]["old_balance"], 12.34);
    assert_eq!(discrepancies[0]["new_balance"], 17.34);
    assert_eq!(discrepancies[0]["diff"], 5.0);
    assert_eq!(account_balance(&fixture.db_path, 10)?, 17.34);
    assert_eq!(account_balance(&fixture.db_path, 11)?, 0.50);
    assert_eq!(account_balance(&fixture.db_path, 99)?, 99.0);

    Ok(())
}

#[tokio::test]
async fn taxonomy_account_transaction_actions_move_clear_and_audit() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let connection = Connection::open(&fixture.db_path)?;
    connection.execute(
        "INSERT INTO bills(
            id, user_id, type, amount, date, counterparty, description,
            source_account_id, destination_account_id, destination_amount,
            main_category, sub_category, created_at, updated_at
        )
        VALUES
            (200, 42, '支出', -20.0, '2026-01-20', '超市', '源账户支出', 10, 0, 0, '', '', 'now', 'now'),
            (201, 42, '收入', 5.0, '2026-01-21', '退款', '目标账户收入', 0, 10, 5.0, '', '', 'now', 'now'),
            (202, 77, '支出', -99.0, '2026-01-22', '其他', '其他用户账单', 99, 0, 0, '', '', 'now', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO account_transfers(id, user_id, from_account_id, to_account_id, from_amount, to_amount, created_at)
         VALUES
            (300, 42, 10, 11, 20.0, 20.0, 'now'),
            (301, 42, 11, 10, 5.0, 5.0, 'now'),
            (302, 77, 99, 10, 99.0, 99.0, 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_tags(bill_id, tag_id, created_at)
         VALUES (200, 20, 'now'), (201, 21, 'now'), (202, 98, 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_pair_links(user_id, left_bill_id, right_bill_id)
         VALUES (42, 200, 201), (77, 202, 202)",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_transfer_pair_suppressions(user_id, left_bill_id, right_bill_id)
         VALUES (42, 200, 201)",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_investment_pair_suppressions(user_id, left_bill_id, right_bill_id)
         VALUES (42, 200, 201)",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_learning_rule_suppressions(user_id, bill_id)
         VALUES (42, 200)",
        [],
    )?;
    drop(connection);

    let app = runtime_router(&fixture);

    let missing_target_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(missing_target_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_target_response).await["error"],
        "toAccountId is required"
    );

    let invalid_target_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": "abc", "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(invalid_target_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_target_response).await["error"],
        "Account IDs must be valid integers"
    );

    let same_account_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": 10, "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(same_account_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(same_account_response).await["error"],
        "Source and target accounts must be different"
    );

    let missing_move_password_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": 11}),
        ))
        .await?;
    assert_eq!(
        missing_move_password_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_move_password_response).await["error"],
        "password is required"
    );

    for uri in [
        "/api/accounts/10/transactions/move",
        "/api/accounts/10/transactions/clear",
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(Method::POST, uri, Body::from("{")))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(read_json(response).await["error"], "Invalid JSON");
    }

    let invalid_password_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": 11, "password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(invalid_password_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(invalid_password_response).await["error"],
        "Invalid password"
    );
    assert_eq!(
        audit_log_count(&fixture.db_path, "move_transactions", "failed")?,
        1
    );

    let missing_source_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/404/transactions/move",
            json!({"toAccountId": 11, "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(
        missing_source_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(missing_source_response).await["error"],
        "Source account not found"
    );

    let move_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": "11", "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(move_response.status(), StatusCode::OK);
    let move_body = read_json(move_response).await;
    assert_eq!(move_body["success"], true);
    assert_eq!(move_body["result"], true);
    assert_eq!(move_body["moved_count"], 4);
    assert_eq!(bill_account_links(&fixture.db_path, 200)?, (11, 0));
    assert_eq!(bill_account_links(&fixture.db_path, 201)?, (0, 11));
    assert_eq!(account_transfer_links(&fixture.db_path, 300)?, (11, 11));
    assert_eq!(account_transfer_links(&fixture.db_path, 301)?, (11, 11));
    assert_eq!(
        audit_log_count(&fixture.db_path, "move_transactions", "success")?,
        1
    );

    let invalid_clear_password_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/11/transactions/clear",
            json!({"password": "wrong-password"}),
        ))
        .await?;
    assert_eq!(
        invalid_clear_password_response.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        read_json(invalid_clear_password_response).await["error"],
        "Invalid password"
    );

    let missing_clear_password_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/11/transactions/clear",
            json!({}),
        ))
        .await?;
    assert_eq!(
        missing_clear_password_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_clear_password_response).await["error"],
        "password is required"
    );

    let missing_clear_account_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/404/transactions/clear",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(
        missing_clear_account_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(missing_clear_account_response).await["error"],
        "Account not found"
    );

    let clear_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/11/transactions/clear",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(clear_response.status(), StatusCode::OK);
    let clear_body = read_json(clear_response).await;
    assert_eq!(clear_body["success"], true);
    assert_eq!(clear_body["result"], true);
    assert_eq!(clear_body["deleted_count"], 4);
    assert!(!bill_exists(&fixture.db_path, 200)?);
    assert!(!bill_exists(&fixture.db_path, 201)?);
    assert!(bill_exists(&fixture.db_path, 202)?);
    assert_eq!(bill_tag_count(&fixture.db_path, 200)?, 0);
    assert_eq!(bill_pair_link_count(&fixture.db_path, 42)?, 0);
    assert_eq!(account_transfer_count(&fixture.db_path, 42)?, 0);
    assert_eq!(account_transfer_count(&fixture.db_path, 77)?, 1);
    assert_eq!(
        audit_log_count(&fixture.db_path, "delete_transactions", "success")?,
        1
    );

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
    let new_child_id = body["result"]["subAccounts"]
        .as_array()
        .expect("sub accounts")
        .iter()
        .find(|account| account["name"] == "新子账户")
        .and_then(|account| account["id"].as_str())
        .expect("new child id")
        .parse::<i64>()?;

    let prune_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/10",
            json!({
                "name": "工资卡更新",
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "balance": 4321,
                "subAccounts": [{
                    "id": new_child_id,
                    "name": "新子账户保留",
                    "type": 1,
                    "balance": 250,
                    "visible": true
                }]
            }),
        ))
        .await?;
    assert_eq!(prune_response.status(), StatusCode::OK);
    assert_eq!(child_count(&fixture.db_path, 10)?, 0);
    assert!(!account_exists(&fixture.db_path, 11)?);
    assert!(account_exists(&fixture.db_path, new_child_id)?);

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
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({"newDisplayOrders": [{"id": 10, "displayOrder": 1}]}),
        ))
        .await?;
    assert_eq!(display_response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let sync_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/accounts/sync-balances",
            Body::empty(),
        ))
        .await?;
    assert_eq!(sync_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(sync_response).await["error"],
        "Rust taxonomy accounts DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let move_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": 11, "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(move_response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let clear_response = app
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/clear",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(clear_response.status(), StatusCode::SERVICE_UNAVAILABLE);

    Ok(())
}

#[tokio::test]
async fn taxonomy_account_sync_balances_reports_db_errors_for_missing_schema(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    Connection::open(&fixture.db_path)?.execute("DROP TABLE accounts", [])?;
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/accounts/sync-balances",
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        read_json(response).await["error"],
        "Rust taxonomy accounts route runtime DB error"
    );

    let move_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": 11, "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(move_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let clear_response = app
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/clear",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(clear_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

    Ok(())
}

#[tokio::test]
async fn taxonomy_account_transaction_actions_report_password_lookup_db_errors(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    Connection::open(&fixture.db_path)?.execute("DROP TABLE users", [])?;
    let app = runtime_router(&fixture);

    let move_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/move",
            json!({"toAccountId": 11, "password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(move_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        read_json(move_response).await["error"],
        "Rust taxonomy accounts route runtime DB error"
    );

    let clear_response = app
        .oneshot(json_request(
            Method::POST,
            "/api/accounts/10/transactions/clear",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(clear_response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        read_json(clear_response).await["error"],
        "Rust taxonomy accounts route runtime DB error"
    );

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
    assert!(TAXONOMY_TAG_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/tags/batch")));
    assert!(TAXONOMY_TAG_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/tags/batch")));

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

    let duplicate_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/tags/batch",
            json!({"tags": [{"name": "午饭"}], "skipExists": false}),
        ))
        .await?;
    assert_eq!(duplicate_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(duplicate_response).await["error"],
        "Tag already exists: 午饭"
    );

    let batch_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/tags/batch",
            json!({
                "tags": [
                    {"name": "通勤"},
                    {"name": "批量新增", "color": "#224466", "icon": "batch", "hidden": true}
                ],
                "skipExists": true
            }),
        ))
        .await?;
    assert_eq!(batch_response.status(), StatusCode::CREATED);
    let batch_body = read_json(batch_response).await;
    let batch_tags = batch_body["result"].as_array().expect("batch tags");
    assert_eq!(
        batch_tags
            .iter()
            .map(|tag| tag["name"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        vec!["通勤", "批量新增"]
    );
    assert_eq!(batch_tags[1]["color"], "#224466");
    assert_eq!(batch_tags[1]["hidden"], true);
    let batch_id = batch_tags[1]["id"]
        .as_str()
        .expect("batch id")
        .parse::<i64>()?;
    assert!(tag_exists(&fixture.db_path, batch_id)?);

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
        (
            Method::POST,
            "/api/tags/batch",
            Body::from(json!({"tags": [{"name": "未授权"}]}).to_string()),
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

    let invalid_batch_shape = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/tags/batch",
            json!({"tags": "bad"}),
        ))
        .await?;
    assert_eq!(invalid_batch_shape.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_batch_shape).await["error"],
        "tags is required and must be a non-empty array"
    );

    let invalid_batch_item = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/tags/batch",
            json!({"tags": [{}]}),
        ))
        .await?;
    assert_eq!(invalid_batch_item.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_batch_item).await["error"],
        "Each tag item must contain a non-empty name"
    );

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
        (
            Method::POST,
            "/api/tags/batch",
            Body::from(json!({"tags": [{"name": "无库"}]}).to_string()),
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

#[tokio::test]
async fn taxonomy_templates_runtime_serves_crud_and_frontend_contract() -> Result<(), Box<dyn Error>>
{
    assert!(TAXONOMY_TEMPLATE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/templates/")));
    assert!(TAXONOMY_TEMPLATE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("PUT", "/api/templates/display-orders")));
    assert!(TAXONOMY_TEMPLATE_PROXIED_ROUTE_PATTERNS.is_empty());

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/templates?templateType=1",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let templates = list_body["result"].as_array().expect("template list");
    assert_eq!(templates.len(), 1);
    assert_eq!(templates[0]["id"], "40");
    assert_eq!(templates[0]["name"], "午餐模板");
    assert_eq!(templates[0]["templateType"], 1);
    assert_eq!(templates[0]["type"], 3);
    assert_eq!(templates[0]["tagIds"], json!(["20", "21"]));
    assert!(!serde_json::to_string(&list_body)?.contains("其他用户模板"));

    let recurring_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/templates/?templateType=2",
            Body::empty(),
        ))
        .await?;
    assert_eq!(recurring_response.status(), StatusCode::OK);
    let recurring_body = read_json(recurring_response).await;
    assert_eq!(recurring_body["result"][0]["id"], "41");
    assert_eq!(recurring_body["result"][0]["scheduledFrequency"], "monthly");

    let get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/templates/40?templateType=1",
            Body::empty(),
        ))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    assert_eq!(read_json(get_response).await["result"]["categoryId"], "31");

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/templates",
            json!({
                "templateType": 1,
                "name": "咖啡模板",
                "type": 3,
                "categoryId": "31",
                "sourceAccountId": "10",
                "destinationAccountId": "0",
                "sourceAmount": 18.5,
                "destinationAmount": 0,
                "hideAmount": false,
                "tagIds": ["20"],
                "comment": "下午",
                "hidden": false,
                "utcOffset": 480
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["result"]["name"], "咖啡模板");
    assert_eq!(create_body["result"]["templateType"], 1);
    let created_id = create_body["result"]["id"]
        .as_str()
        .expect("created template id")
        .parse::<i64>()?;
    assert!(template_exists(&fixture.db_path, created_id)?);

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/templates/{created_id}?templateType=1"),
            json!({
                "name": "咖啡模板更新",
                "sourceAmount": 20.0,
                "tagIds": ["20", "21"],
                "hidden": true,
                "displayOrder": 4
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["result"]["name"], "咖啡模板更新");
    assert_eq!(update_body["result"]["sourceAmount"], 20.0);
    assert_eq!(update_body["result"]["tagIds"], json!(["20", "21"]));
    assert_eq!(update_body["result"]["hidden"], true);

    let orders_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/templates/display-orders?templateType=1",
            json!({"newDisplayOrders": [
                {"id": created_id, "displayOrder": 1},
                {"id": 40, "displayOrder": 3}
            ]}),
        ))
        .await?;
    assert_eq!(orders_response.status(), StatusCode::OK);
    assert_eq!(read_json(orders_response).await["result"], true);
    assert_eq!(template_display_order(&fixture.db_path, created_id)?, 1);
    assert_eq!(template_display_order(&fixture.db_path, 40)?, 3);

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/templates/{created_id}?templateType=1"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);
    assert!(!template_exists(&fixture.db_path, created_id)?);

    Ok(())
}

#[tokio::test]
async fn taxonomy_templates_runtime_validates_payloads_user_scope_and_db_config(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let unauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/templates/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauth_response.status(), StatusCode::UNAUTHORIZED);

    let missing_create = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/templates/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_create).await["error"], "No data provided");

    let missing_update = app
        .clone()
        .oneshot(authed_request(
            Method::PUT,
            "/api/templates/40?templateType=1",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_update.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_update).await["error"], "No data provided");

    let missing_template = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/templates/999?templateType=1",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_template.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_template).await["error"],
        "Template not found"
    );

    let cross_user_delete = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/templates/96?templateType=1",
            Body::empty(),
        ))
        .await?;
    assert_eq!(cross_user_delete.status(), StatusCode::NOT_FOUND);
    assert!(template_exists(&fixture.db_path, 96)?);

    let missing_orders = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/templates/display-orders",
            json!({"newDisplayOrders": []}),
        ))
        .await?;
    assert_eq!(missing_orders.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_orders).await["error"],
        "Missing newDisplayOrders"
    );

    let no_db_app = runtime_router_without_db(&fixture);
    for (method, uri, body) in [
        (Method::GET, "/api/templates/", Body::empty()),
        (
            Method::GET,
            "/api/templates/40?templateType=1",
            Body::empty(),
        ),
        (
            Method::POST,
            "/api/templates/",
            Body::from(r#"{"name":"NoDb","templateType":1}"#),
        ),
        (
            Method::PUT,
            "/api/templates/40?templateType=1",
            Body::from(r#"{"name":"NoDb"}"#),
        ),
        (
            Method::DELETE,
            "/api/templates/40?templateType=1",
            Body::empty(),
        ),
        (
            Method::PUT,
            "/api/templates/display-orders",
            Body::from(r#"{"newDisplayOrders":[{"id":40,"displayOrder":1}]}"#),
        ),
    ] {
        let response = no_db_app
            .clone()
            .oneshot(authed_request(method, uri, body))
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
        assert_eq!(
            read_json(response).await["error"],
            "Rust taxonomy templates DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
        );
    }

    Ok(())
}

#[tokio::test]
async fn taxonomy_templates_runtime_reports_db_errors_for_missing_schema(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    Connection::open(&fixture.db_path)?.execute("DROP TABLE bill_templates", [])?;
    let app = runtime_router(&fixture);

    for (method, uri, body) in [
        (Method::GET, "/api/templates/?templateType=1", Body::empty()),
        (
            Method::GET,
            "/api/templates/40?templateType=1",
            Body::empty(),
        ),
        (
            Method::POST,
            "/api/templates/",
            Body::from(r#"{"name":"Broken","templateType":1}"#),
        ),
        (
            Method::PUT,
            "/api/templates/40?templateType=1",
            Body::from(r#"{"name":"Broken"}"#),
        ),
        (
            Method::DELETE,
            "/api/templates/40?templateType=1",
            Body::empty(),
        ),
        (
            Method::PUT,
            "/api/templates/display-orders?templateType=1",
            Body::from(r#"{"newDisplayOrders":[{"id":40,"displayOrder":1}]}"#),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(method, uri, body))
            .await?;
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{uri}"
        );
        assert_eq!(
            read_json(response).await["error"],
            "Rust taxonomy templates route runtime DB error"
        );
    }

    Ok(())
}

#[tokio::test]
async fn taxonomy_categories_runtime_serves_master_data_contract() -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_CATEGORY_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/categories/")));
    assert!(TAXONOMY_CATEGORY_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/categories/batch")));
    assert!(TAXONOMY_CATEGORY_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/categories/statistics")));
    assert!(TAXONOMY_CATEGORY_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/categories/update-all")));
    assert!(TAXONOMY_CATEGORY_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/categories/rules")));
    assert!(!TAXONOMY_CATEGORY_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/categories/statistics")));
    assert!(!TAXONOMY_CATEGORY_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/categories/update-all")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    let expense_categories = list_body["result"]["3"].as_array().expect("type 3 list");
    assert_eq!(expense_categories.len(), 1);
    assert_eq!(expense_categories[0]["id"], "30");
    assert_eq!(expense_categories[0]["name"], "餐饮");
    assert_eq!(expense_categories[0]["subCategories"][0]["name"], "午餐");
    assert_eq!(expense_categories[0]["subCategories"][0]["hidden"], true);
    assert!(!serde_json::to_string(&list_body)?.contains("其他用户分类"));

    let get_parent_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/30",
            Body::empty(),
        ))
        .await?;
    assert_eq!(get_parent_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(get_parent_response).await["result"]["parentId"],
        "0"
    );

    let get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = read_json(get_response).await;
    assert_eq!(get_body["result"]["parentId"], "30");
    assert_eq!(get_body["result"]["name"], "午餐");

    let update_parent_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/30",
            json!({"comment": "主类更新", "displayOrder": 4}),
        ))
        .await?;
    assert_eq!(update_parent_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(update_parent_response).await["result"]["comment"],
        "主类更新"
    );

    let create_parent_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({
                "name": "出行",
                "type": 3,
                "comment": "交通主类",
                "displayOrder": 5,
                "visible": false,
                "icon": "mdi-bus",
                "color": "#336699"
            }),
        ))
        .await?;
    assert_eq!(create_parent_response.status(), StatusCode::CREATED);
    let create_parent_body = read_json(create_parent_response).await;
    assert_eq!(create_parent_body["result"]["hidden"], true);
    let parent_id = create_parent_body["result"]["id"]
        .as_str()
        .expect("created category id")
        .parse::<i64>()?;
    assert!(category_exists(&fixture.db_path, parent_id)?);

    let create_child_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({
                "name": "地铁",
                "parentId": parent_id.to_string(),
                "comment": "轨交",
                "displayOrder": 6
            }),
        ))
        .await?;
    assert_eq!(create_child_response.status(), StatusCode::OK);
    let create_child_body = read_json(create_child_response).await;
    assert_eq!(
        create_child_body["result"]["parentId"],
        parent_id.to_string()
    );
    let child_id = create_child_body["result"]["id"]
        .as_str()
        .expect("created child id")
        .parse::<i64>()?;

    let update_child_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/categories/{child_id}"),
            json!({"name": "公交", "comment": "地面公交", "displayOrder": 8, "visible": true}),
        ))
        .await?;
    assert_eq!(update_child_response.status(), StatusCode::OK);
    let update_child_body = read_json(update_child_response).await;
    assert_eq!(update_child_body["result"]["name"], "公交");
    assert_eq!(update_child_body["result"]["displayOrder"], 8);

    let move_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/move",
            json!({"newDisplayOrders": [{"id": child_id, "displayOrder": 12}]}),
        ))
        .await?;
    assert_eq!(move_response.status(), StatusCode::OK);
    assert_eq!(read_json(move_response).await["result"], true);
    assert_eq!(category_priority(&fixture.db_path, child_id)?, 12);

    let flat_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/flat",
            Body::empty(),
        ))
        .await?;
    assert_eq!(flat_response.status(), StatusCode::OK);
    let flat_body = read_json(flat_response).await;
    assert!(flat_body["result"]
        .as_array()
        .expect("flat categories")
        .iter()
        .any(|category| category["name"] == "公交"));

    let export_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(export_response.status(), StatusCode::OK);
    let export_body = read_json(export_response).await;
    assert!(export_body["result"]
        .as_array()
        .expect("exported categories")
        .iter()
        .all(|category| category.get("id").is_none()));

    let import_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/import",
            json!({"categories": [
                {"main_category": "餐饮", "sub_category": "午餐", "description": "导入更新", "priority": 9},
                {"main_category": "学习", "sub_category": "课程", "type": 3, "priority": 10},
                {"sub_category": "缺主类"}
            ]}),
        ))
        .await?;
    assert_eq!(import_response.status(), StatusCode::OK);
    let import_body = read_json(import_response).await;
    assert_eq!(import_body["result"]["updated"], 1);
    assert_eq!(import_body["result"]["imported"], 1);
    assert_eq!(import_body["result"]["skipped"], 1);

    let statistics_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/statistics?period=month&type=%E6%94%AF%E5%87%BA&start_date=2026-01-01&end_date=2026-01-31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(statistics_response.status(), StatusCode::OK);
    let statistics_body = read_json(statistics_response).await;
    assert_eq!(statistics_body["success"], true);
    assert_eq!(statistics_body["result"]["餐饮"]["total_amount"], 20.0);
    assert_eq!(statistics_body["result"]["餐饮"]["count"], 2);
    assert_eq!(
        statistics_body["result"]["餐饮"]["sub_categories"]["午餐"]["total_amount"],
        20.0
    );
    assert_eq!(
        statistics_body["result"]["餐饮"]["sub_categories"]["午餐"]["count"],
        2
    );
    assert_eq!(statistics_body["result"]["交通"]["total_amount"], 3.0);
    assert!(statistics_body["result"].get("其他用户分类").is_none());

    let recategorize_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/update-all",
            json!({"force": false}),
        ))
        .await?;
    assert_eq!(recategorize_response.status(), StatusCode::OK);
    let recategorize_body = read_json(recategorize_response).await;
    assert_eq!(recategorize_body["success"], true);
    assert_eq!(recategorize_body["result"]["total"], 6);
    assert_eq!(recategorize_body["result"]["updated"], 1);
    assert_eq!(
        bill_category_pair(&fixture.db_path, 54)?,
        ("餐饮".to_string(), "午餐".to_string())
    );
    assert_eq!(
        bill_category_pair(&fixture.db_path, 96)?,
        ("".to_string(), "".to_string())
    );

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/categories/{parent_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["result"], true);
    assert!(!category_exists(&fixture.db_path, parent_id)?);
    assert!(!category_exists(&fixture.db_path, child_id)?);

    Ok(())
}

#[tokio::test]
async fn taxonomy_category_rules_runtime_lists_canonical_rules_contract(
) -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/category-rules/")));
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/")));
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("PUT", "/api/category-rules/{rule_id}")));
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("DELETE", "/api/category-rules/{rule_id}")));
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/{rule_id}/test")));
    assert!(TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/category-rules/")));
    assert!(TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("PUT", "/api/category-rules/{rule_id}")));
    assert!(TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("DELETE", "/api/category-rules/{rule_id}")));
    assert!(TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .all(|route| route != &("POST", "/api/category-rules/reorder")));
    assert!(TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/defaults")));
    assert!(TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/migrate")));
    assert!(!TAXONOMY_CATEGORY_RULE_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/{rule_id}/test")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/category-rules/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    assert_eq!(list_body["total"], 1);
    let rules = list_body["data"].as_array().expect("rules array");
    assert_eq!(rules[0]["id"], 60);
    assert_eq!(rules[0]["category_id"], 31);
    assert_eq!(rules[0]["name"], "午餐规则");
    assert_eq!(rules[0]["main_category"], "餐饮");
    assert_eq!(rules[0]["sub_category"], "午餐");
    assert_eq!(rules[0]["category_type"], 3);
    assert!(!serde_json::to_string(&list_body)?.contains("其他用户规则"));

    let include_disabled_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/category-rules/?enabled_only=false&category_id=31",
            Body::empty(),
        ))
        .await?;
    assert_eq!(include_disabled_response.status(), StatusCode::OK);
    let include_disabled_body = read_json(include_disabled_response).await;
    assert_eq!(include_disabled_body["total"], 2);
    assert_eq!(include_disabled_body["data"][0]["id"], 60);
    assert_eq!(include_disabled_body["data"][1]["id"], 61);
    assert_eq!(include_disabled_body["data"][1]["enabled"], 0);

    let missing_category_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/category-rules/?category_id=999",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_category_response.status(), StatusCode::OK);
    assert_eq!(read_json(missing_category_response).await["total"], 0);

    let missing_create_fields_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!({"category_id": 31}),
        ))
        .await?;
    assert_eq!(
        missing_create_fields_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_create_fields_response).await["error"],
        "category_id and rule_expression are required"
    );

    let null_create_expression_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!({"category_id": 31, "rule_expression": null}),
        ))
        .await?;
    assert_eq!(
        null_create_expression_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(null_create_expression_response).await["error"],
        "category_id and rule_expression are required"
    );

    let invalid_create_json_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/",
            Body::from("{"),
        ))
        .await?;
    assert_eq!(
        invalid_create_json_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(invalid_create_json_response).await["error"],
        "Invalid JSON"
    );

    let scalar_create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!(["bad"]),
        ))
        .await?;
    assert_eq!(scalar_create_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(scalar_create_response).await["error"],
        "No data provided"
    );

    let cross_category_create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!({
                "category_id": 97,
                "rule_expression": "OR={其他用户分类}"
            }),
        ))
        .await?;
    assert_eq!(
        cross_category_create_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(cross_category_create_response).await["error"],
        "Failed to create rule"
    );

    let invalid_create_field_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!({
                "category_id": 31,
                "rule_expression": "OR={午餐}",
                "regex_enabled": ["bad"]
            }),
        ))
        .await?;
    assert_eq!(
        invalid_create_field_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(invalid_create_field_response).await["error"],
        "Rust taxonomy category rules route runtime DB error"
    );

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!({
                "category_id": 31,
                "name": "晚餐规则",
                "priority": 15,
                "rule_expression": "OR={晚餐}",
                "regex_enabled": false,
                "enabled": true
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(create_body["data"]["name"], "晚餐规则");
    assert_eq!(create_body["data"]["category_id"], 31);
    assert_eq!(create_body["data"]["main_category"], "餐饮");
    let created_rule_id = create_body["data"]["id"].as_i64().expect("rule id");

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/category-rules/{created_rule_id}"),
            json!({
                "name": "晚餐规则更新",
                "priority": 5,
                "rule_expression": "OR={晚餐,夜宵}",
                "enabled": false
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["data"]["name"], "晚餐规则更新");
    assert_eq!(update_body["data"]["priority"], 5);
    assert_eq!(update_body["data"]["enabled"], 0);

    let invalid_update_json_response = app
        .clone()
        .oneshot(authed_request(
            Method::PUT,
            &format!("/api/category-rules/{created_rule_id}"),
            Body::from("{"),
        ))
        .await?;
    assert_eq!(
        invalid_update_json_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(invalid_update_json_response).await["error"],
        "Invalid JSON"
    );

    let null_update_expression_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/category-rules/{created_rule_id}"),
            json!({"rule_expression": null}),
        ))
        .await?;
    assert_eq!(
        null_update_expression_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(null_update_expression_response).await["error"],
        "rule_expression is required"
    );

    let cross_category_update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/category-rules/{created_rule_id}"),
            json!({"category_id": 97}),
        ))
        .await?;
    assert_eq!(
        cross_category_update_response.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        read_json(cross_category_update_response).await["error"],
        "Rule not found or no change"
    );

    let invalid_update_field_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/category-rules/{created_rule_id}"),
            json!({"enabled": ["bad"]}),
        ))
        .await?;
    assert_eq!(
        invalid_update_field_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(invalid_update_field_response).await["error"],
        "Rust taxonomy category rules route runtime DB error"
    );

    let cross_user_update = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/category-rules/96",
            json!({"name": "越权更新"}),
        ))
        .await?;
    assert_eq!(cross_user_update.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(cross_user_update).await["error"],
        "Rule not found or no change"
    );

    let invalid_reorder_json_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/reorder",
            Body::from("{"),
        ))
        .await?;
    assert_eq!(
        invalid_reorder_json_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(invalid_reorder_json_response).await["error"],
        "Invalid JSON"
    );

    let missing_reorder_ids_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/reorder",
            json!({}),
        ))
        .await?;
    assert_eq!(
        missing_reorder_ids_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_reorder_ids_response).await["error"],
        "rule_ids is required"
    );

    let scalar_reorder_ids_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/reorder",
            json!({"rule_ids": 60}),
        ))
        .await?;
    assert_eq!(
        scalar_reorder_ids_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(scalar_reorder_ids_response).await["error"],
        "rule_ids must be a list"
    );

    let invalid_reorder_id_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/reorder",
            json!({"rule_ids": [60, "bad"]}),
        ))
        .await?;
    assert_eq!(
        invalid_reorder_id_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(invalid_reorder_id_response).await["error"],
        "rule_ids must be a list"
    );

    let reorder_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/reorder",
            json!({"rule_ids": [created_rule_id, 60, 61, 96]}),
        ))
        .await?;
    assert_eq!(reorder_response.status(), StatusCode::OK);
    assert_eq!(read_json(reorder_response).await["success"], true);
    assert_eq!(
        category_rule_priority(&fixture.db_path, created_rule_id)?,
        1
    );
    assert_eq!(category_rule_priority(&fixture.db_path, 60)?, 2);
    assert_eq!(category_rule_priority(&fixture.db_path, 61)?, 3);
    assert_eq!(category_rule_priority(&fixture.db_path, 96)?, 1);

    let match_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/60/test",
            json!({"text": "工作日午餐付款"}),
        ))
        .await?;
    assert_eq!(match_response.status(), StatusCode::OK);
    assert_eq!(read_json(match_response).await["data"]["matched"], true);

    let miss_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/60/test",
            json!({"text": "地铁通勤"}),
        ))
        .await?;
    assert_eq!(miss_response.status(), StatusCode::OK);
    assert_eq!(read_json(miss_response).await["data"]["matched"], false);

    let missing_rule_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/999/test",
            json!({"text": "午餐"}),
        ))
        .await?;
    assert_eq!(missing_rule_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(missing_rule_response).await["error"],
        "Rule not found"
    );

    let missing_text_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/60/test",
            json!({}),
        ))
        .await?;
    assert_eq!(missing_text_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_text_response).await["error"],
        "text is required"
    );

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/category-rules/{created_rule_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(read_json(delete_response).await["success"], true);
    assert!(!category_rule_exists(&fixture.db_path, created_rule_id)?);

    let cross_user_delete = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/category-rules/96",
            Body::empty(),
        ))
        .await?;
    assert_eq!(cross_user_delete.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        read_json(cross_user_delete).await["error"],
        "Rule not found"
    );
    assert!(category_rule_exists(&fixture.db_path, 96)?);

    let unauthenticated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/category-rules/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);
    for (method, uri) in [
        (Method::POST, "/api/category-rules/"),
        (Method::PUT, "/api/category-rules/60"),
        (Method::DELETE, "/api/category-rules/60"),
        (Method::POST, "/api/category-rules/reorder"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    let no_db_app = runtime_router_without_db(&fixture);
    let no_db_response = no_db_app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/category-rules/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["error"],
        "Rust taxonomy category rules DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );
    let no_db_test_response = no_db_app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/60/test",
            json!({"text": "午餐"}),
        ))
        .await?;
    assert_eq!(
        no_db_test_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_create_response = no_db_app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/",
            json!({"category_id": 31, "rule_expression": "OR={测试}"}),
        ))
        .await?;
    assert_eq!(
        no_db_create_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_update_response = no_db_app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/category-rules/60",
            json!({"name": "missing db"}),
        ))
        .await?;
    assert_eq!(
        no_db_update_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_delete_response = no_db_app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/category-rules/60",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_db_delete_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_reorder_response = no_db_app
        .oneshot(json_request(
            Method::POST,
            "/api/category-rules/reorder",
            json!({"rule_ids": [60]}),
        ))
        .await?;
    assert_eq!(
        no_db_reorder_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_rules_overview_runtime_aggregates_user_scoped_rule_sources(
) -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_RULE_CENTER_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/rules/overview")));
    assert!(TAXONOMY_RULE_CENTER_PROXIED_ROUTE_PATTERNS.is_empty());

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/rules/overview",
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    let data = &body["data"];
    assert_eq!(body["success"], true);
    assert_eq!(data["learningRuleCount"], 2);
    assert_eq!(data["categoryRuleCount"], 1);
    assert_eq!(data["recurringRuleCount"], 1);
    assert_eq!(data["totalRuleCount"], 4);

    let learning_rules = data["learningRules"].as_array().expect("learning rules");
    assert_eq!(learning_rules[0]["id"], 71);
    assert_eq!(learning_rules[0]["matchType"], "description");
    assert_eq!(learning_rules[0]["matchValue"], "基金定投");
    assert_eq!(learning_rules[0]["learnedCategoryId"], 31);
    assert_eq!(learning_rules[0]["enabled"], false);
    assert_eq!(learning_rules[0]["source"], "learning");

    let recurring_rules = data["recurringRules"].as_array().expect("recurring rules");
    assert_eq!(recurring_rules[0]["id"], 41);
    assert_eq!(recurring_rules[0]["name"], "房租模板");
    assert_eq!(recurring_rules[0]["amount"], 3000.0);
    assert_eq!(recurring_rules[0]["frequency"], "monthly");
    assert_eq!(recurring_rules[0]["nextDate"], "2026-02-01");
    assert_eq!(recurring_rules[0]["source"], "recurring");

    let serialized = serde_json::to_string(&body)?;
    assert!(!serialized.contains("其他用户规则"));
    assert!(!serialized.contains("其他用户学习规则"));

    Ok(())
}

#[tokio::test]
async fn taxonomy_settings_bundle_export_runtime_serves_raw_bundle_and_sensitive_sections(
) -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/settings/bundle/export")));
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| { route == &("POST", "/api/settings/bundle/sections/{section_key}/export") }));
    assert!(TAXONOMY_SETTINGS_BUNDLE_PROXIED_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/settings/bundle/import")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let export_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/settings/bundle/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(export_response.status(), StatusCode::OK);
    assert_eq!(
        export_response
            .headers()
            .get("content-disposition")
            .and_then(|value| value.to_str().ok()),
        Some("attachment; filename=\"bill-analyser-settings.json\"")
    );
    let bundle = read_json(export_response).await;
    assert_eq!(bundle["schemaVersion"], 1);
    assert_eq!(bundle["secretsPolicy"]["llmApiKeys"], "redacted");
    assert_eq!(bundle["counts"]["accounts"], 2);
    assert_eq!(bundle["counts"]["transactionTags"], 2);
    assert_eq!(bundle["counts"]["categoryRecognitionRules"], 2);
    assert_eq!(bundle["counts"]["llmConfigs"], 1);
    assert_eq!(bundle["sections"]["accounts"][0]["name"], "工资卡");
    assert_eq!(
        bundle["sections"]["accounts"][0]["aliases"],
        json!(["主卡", "工资"])
    );
    assert_eq!(
        bundle["sections"]["transactionTemplates"][0]["categoryRef"],
        "category:31"
    );
    assert_eq!(
        bundle["sections"]["transactionTemplates"][0]["tagNames"],
        json!(["午饭", "通勤"])
    );
    assert_eq!(
        bundle["sections"]["scheduledTransactions"][0]["nextDate"],
        "2026-02-01"
    );
    assert_eq!(
        bundle["sections"]["categoryRecognitionRules"][0]["ruleExpression"],
        "OR={午餐,饭}"
    );
    assert_eq!(bundle["sections"]["llmConfigs"][0]["apiKey"], "");
    assert_eq!(bundle["sections"]["llmConfigs"][0]["hasApiKey"], true);
    assert_eq!(
        bundle["sections"]["llmConfigs"][0]["advancedSettings"]["reasoning_depth"],
        "high"
    );
    assert_eq!(bundle["sections"]["ocrConfig"][0]["provider"], "cloud_stub");
    assert_eq!(bundle["sections"]["ocrConfig"][0]["lang"], "eng");
    let serialized = serde_json::to_string(&bundle)?;
    assert!(!serialized.contains("secret-key"));
    assert!(!serialized.contains("其他用户"));

    let tag_section_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/settings/bundle/sections/transactionTags/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(tag_section_response.status(), StatusCode::OK);
    let tag_section = read_json(tag_section_response).await;
    assert_eq!(
        tag_section["sections"]
            .as_object()
            .expect("section object")
            .keys()
            .collect::<Vec<_>>(),
        vec!["transactionTags"]
    );
    assert_eq!(tag_section["counts"]["transactionTags"], 2);

    let sensitive_get_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/settings/bundle/sections/llmConfigs/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(sensitive_get_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(sensitive_get_response).await["error"],
        "password is required"
    );

    let wrong_password_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/sections/llmConfigs/export",
            json!({"password": "wrong"}),
        ))
        .await?;
    assert_eq!(wrong_password_response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        read_json(wrong_password_response).await["error"],
        "Invalid password"
    );

    let sensitive_post_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/sections/llmConfigs/export",
            json!({"password": "correct horse battery staple"}),
        ))
        .await?;
    assert_eq!(sensitive_post_response.status(), StatusCode::OK);
    let sensitive_section = read_json(sensitive_post_response).await;
    assert_eq!(sensitive_section["counts"]["llmConfigs"], 1);
    assert_eq!(
        sensitive_section["sections"]["llmConfigs"][0]["hasApiKey"],
        true
    );
    assert!(!serde_json::to_string(&sensitive_section)?.contains("secret-key"));

    let invalid_section_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/settings/bundle/sections/notASection/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_section_response.status(), StatusCode::NOT_FOUND);

    let no_db_app = runtime_router_without_db(&fixture);
    let no_db_response = no_db_app
        .oneshot(authed_request(
            Method::GET,
            "/api/settings/bundle/export",
            Body::empty(),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["error"],
        "Rust taxonomy settings bundle export DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_categories_runtime_covers_legacy_aliases_virtual_and_batch_edges(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let all_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/all",
            Body::empty(),
        ))
        .await?;
    assert_eq!(all_response.status(), StatusCode::OK);
    assert!(read_json(all_response).await["result"]
        .as_array()
        .expect("raw category rows")
        .iter()
        .any(|category| category["id"] == 30));

    let update_all_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/all",
            json!({"categories": []}),
        ))
        .await?;
    assert_eq!(update_all_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(update_all_response).await["message"],
        "Categories updated successfully"
    );

    let update_all_missing = app
        .clone()
        .oneshot(json_request(Method::PUT, "/api/categories/all", json!({})))
        .await?;
    assert_eq!(update_all_missing.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(update_all_missing).await["error"],
        "categories are required"
    );

    let duplicate_parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "餐饮", "parentId": "0"}),
        ))
        .await?;
    assert_eq!(duplicate_parent.status(), StatusCode::OK);
    assert_eq!(
        read_json(duplicate_parent).await["message"],
        "Category already exists"
    );

    let duplicate_child = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "午餐", "parentId": "30"}),
        ))
        .await?;
    assert_eq!(duplicate_child.status(), StatusCode::OK);
    assert_eq!(
        read_json(duplicate_child).await["message"],
        "Category already exists"
    );

    let missing_parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "孤儿", "parentId": "99999"}),
        ))
        .await?;
    assert_eq!(missing_parent.status(), StatusCode::NOT_FOUND);

    let food_parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "Food", "type": 3, "displayOrder": 3}),
        ))
        .await?;
    assert_eq!(food_parent.status(), StatusCode::CREATED);
    let food_parent_body = read_json(food_parent).await;
    let food_parent_id = food_parent_body["result"]["id"]
        .as_str()
        .expect("food parent id")
        .parse::<i64>()?;

    let food_child = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "Lunch", "parentId": food_parent_id.to_string()}),
        ))
        .await?;
    assert_eq!(food_child.status(), StatusCode::OK);
    let food_child_body = read_json(food_child).await;
    let food_child_id = food_child_body["result"]["id"]
        .as_str()
        .expect("food child id")
        .parse::<i64>()?;

    let virtual_get = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/virtual_Food",
            Body::empty(),
        ))
        .await?;
    assert_eq!(virtual_get.status(), StatusCode::OK);
    assert_eq!(read_json(virtual_get).await["result"]["name"], "Food");

    let virtual_rename = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_Food",
            json!({
                "name": "Meals",
                "type": 3,
                "comment": "renamed parent",
                "displayOrder": 11,
                "visible": false,
                "keywords": "eat",
                "icon": "mdi-food",
                "color": "#112233"
            }),
        ))
        .await?;
    assert_eq!(virtual_rename.status(), StatusCode::OK);
    let virtual_rename_body = read_json(virtual_rename).await;
    assert_eq!(virtual_rename_body["result"]["name"], "Meals");
    assert_eq!(virtual_rename_body["result"]["hidden"], true);

    let travel_parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "Travel", "type": 3}),
        ))
        .await?;
    assert_eq!(travel_parent.status(), StatusCode::CREATED);

    let virtual_conflict = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_Meals",
            json!({"name": "Travel"}),
        ))
        .await?;
    assert_eq!(virtual_conflict.status(), StatusCode::CONFLICT);

    let virtual_create = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_NewGroup",
            json!({"name": "NewGroup", "type": 3, "displayOrder": 14}),
        ))
        .await?;
    assert_eq!(virtual_create.status(), StatusCode::OK);
    assert_eq!(
        read_json(virtual_create).await["result"]["name"],
        "NewGroup"
    );

    let update_child = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/categories/{food_child_id}"),
            json!({
                "name": "Dinner",
                "comment": "evening",
                "displayOrder": 15,
                "keywords": "night",
                "type": 3,
                "visible": false,
                "icon": "mdi-dinner",
                "color": "#445566"
            }),
        ))
        .await?;
    assert_eq!(update_child.status(), StatusCode::OK);
    let update_child_body = read_json(update_child).await;
    assert_eq!(update_child_body["result"]["name"], "Dinner");
    assert_eq!(update_child_body["result"]["hidden"], true);

    let rename_real_parent = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/categories/{food_parent_id}"),
            json!({"name": "Household", "comment": "real parent rename"}),
        ))
        .await?;
    assert_eq!(rename_real_parent.status(), StatusCode::OK);
    assert_eq!(
        read_json(rename_real_parent).await["result"]["name"],
        "Household"
    );

    for body in [
        json!({}),
        json!({"newDisplayOrders": []}),
        json!({"newDisplayOrders": [{"id": "bad"}, {"id": food_child_id}]}),
    ] {
        let move_response = app
            .clone()
            .oneshot(json_request(Method::POST, "/api/categories/move", body))
            .await?;
        assert_eq!(move_response.status(), StatusCode::OK);
        assert_eq!(read_json(move_response).await["result"], true);
    }

    let batch_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/batch",
            json!({"categories": [
                {"name": "Home", "type": 3, "subCategories": [
                    {"name": "Rent", "displayOrder": 4},
                    {"name": ""}
                ]},
                {"name": ""}
            ]}),
        ))
        .await?;
    assert_eq!(batch_response.status(), StatusCode::OK);
    assert!(serde_json::to_string(&read_json(batch_response).await)?.contains("Rent"));

    let raw_import = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/import",
            json!([
                {"main_category": "Health", "sub_category": "", "description": "checkup"},
                {"main_category": "Health", "sub_category": "Doctor", "priority": 6}
            ]),
        ))
        .await?;
    assert_eq!(raw_import.status(), StatusCode::OK);
    assert_eq!(read_json(raw_import).await["result"]["imported"], 2);

    let result_import = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/import",
            json!({"result": [{"main_category": "Health", "sub_category": "Doctor", "priority": 7}]}),
        ))
        .await?;
    assert_eq!(result_import.status(), StatusCode::OK);
    assert_eq!(read_json(result_import).await["result"]["updated"], 1);

    let invalid_import = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/import",
            json!({"categories": {}}),
        ))
        .await?;
    assert_eq!(invalid_import.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_import).await["error"],
        "Invalid format, expected list of categories"
    );

    let invalid_update_body = app
        .clone()
        .oneshot(json_request(Method::PUT, "/api/categories/30", json!([])))
        .await?;
    assert_eq!(invalid_update_body.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_update_body).await["error"],
        "Invalid request"
    );

    let invalid_delete_id = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/categories/not-a-number",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_delete_id.status(), StatusCode::BAD_REQUEST);

    let delete_virtual = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/categories/virtual_Household",
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_virtual.status(), StatusCode::OK);
    assert_eq!(read_json(delete_virtual).await["result"], true);
    assert!(!category_exists(&fixture.db_path, food_parent_id)?);
    assert!(!category_exists(&fixture.db_path, food_child_id)?);

    let deleted_get = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/categories/{food_child_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(deleted_get.status(), StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn taxonomy_categories_runtime_covers_error_edges_and_orphan_fallbacks(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    for (method, uri) in [
        (Method::GET, "/api/categories/flat"),
        (Method::GET, "/api/categories/all"),
        (Method::PUT, "/api/categories/all"),
        (Method::POST, "/api/categories/"),
        (Method::GET, "/api/categories/30"),
        (Method::PUT, "/api/categories/30"),
        (Method::DELETE, "/api/categories/30"),
        (Method::POST, "/api/categories/move"),
        (Method::POST, "/api/categories/batch"),
        (Method::GET, "/api/categories/export"),
        (Method::POST, "/api/categories/import"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))?,
            )
            .await?;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{uri}");
    }

    for (method, uri) in [
        (Method::PUT, "/api/categories/all"),
        (Method::PUT, "/api/categories/30"),
        (Method::POST, "/api/categories/move"),
        (Method::POST, "/api/categories/batch"),
        (Method::POST, "/api/categories/import"),
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(method, uri, Body::from("{")))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert_eq!(read_json(response).await["error"], "Invalid JSON");
    }

    let no_db_app = runtime_router_without_db(&fixture);
    for (method, uri, body) in [
        (Method::GET, "/api/categories/flat", Body::empty()),
        (Method::GET, "/api/categories/all", Body::empty()),
        (
            Method::POST,
            "/api/categories/",
            Body::from(r#"{"name":"NoDb"}"#),
        ),
        (Method::GET, "/api/categories/30", Body::empty()),
        (
            Method::PUT,
            "/api/categories/30",
            Body::from(r#"{"comment":"NoDb"}"#),
        ),
        (Method::DELETE, "/api/categories/30", Body::empty()),
        (
            Method::POST,
            "/api/categories/move",
            Body::from(r#"{"newDisplayOrders":[{"id":30,"displayOrder":1}]}"#),
        ),
        (
            Method::POST,
            "/api/categories/batch",
            Body::from(r#"{"categories":[{"name":"NoDb"}]}"#),
        ),
        (Method::GET, "/api/categories/export", Body::empty()),
        (
            Method::POST,
            "/api/categories/import",
            Body::from(r#"[{"main_category":"NoDb","sub_category":""}]"#),
        ),
    ] {
        let response = no_db_app
            .clone()
            .oneshot(authed_request(method, uri, body))
            .await?;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE, "{uri}");
    }

    let empty_name = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": ""}),
        ))
        .await?;
    assert_eq!(empty_name.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(empty_name).await["error"],
        "Category name is required"
    );

    let invalid_parent_text = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "BadParent", "parentId": "not-a-number"}),
        ))
        .await?;
    assert_eq!(invalid_parent_text.status(), StatusCode::NOT_FOUND);

    let virtual_parent_child = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "VirtualChild", "parentId": "virtual_VirtualParent"}),
        ))
        .await?;
    assert_eq!(virtual_parent_child.status(), StatusCode::OK);

    let invalid_update_id = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/not-a-number",
            json!({"comment": "x"}),
        ))
        .await?;
    assert_eq!(invalid_update_id.status(), StatusCode::BAD_REQUEST);

    let missing_update_with_name = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/99999",
            json!({"name": "Missing"}),
        ))
        .await?;
    assert_eq!(missing_update_with_name.status(), StatusCode::NOT_FOUND);

    let missing_update_without_name = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/99999",
            json!({"comment": "Missing"}),
        ))
        .await?;
    assert_eq!(missing_update_without_name.status(), StatusCode::NOT_FOUND);

    let alpha = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "Alpha", "type": 3}),
        ))
        .await?;
    assert_eq!(alpha.status(), StatusCode::CREATED);
    let alpha_id = read_json(alpha).await["result"]["id"]
        .as_str()
        .expect("alpha id")
        .parse::<i64>()?;

    let beta = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "Beta", "type": 3}),
        ))
        .await?;
    assert_eq!(beta.status(), StatusCode::CREATED);

    let real_rename_conflict = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/categories/{alpha_id}"),
            json!({"name": "Beta"}),
        ))
        .await?;
    assert_eq!(real_rename_conflict.status(), StatusCode::CONFLICT);

    let parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "SubConflictParent", "type": 3}),
        ))
        .await?;
    assert_eq!(parent.status(), StatusCode::CREATED);
    let parent_id = read_json(parent).await["result"]["id"]
        .as_str()
        .expect("parent id")
        .parse::<i64>()?;
    let first_child = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "First", "parentId": parent_id.to_string()}),
        ))
        .await?;
    assert_eq!(first_child.status(), StatusCode::OK);
    let second_child = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "Second", "parentId": parent_id.to_string()}),
        ))
        .await?;
    assert_eq!(second_child.status(), StatusCode::OK);
    let second_child_id = read_json(second_child).await["result"]["id"]
        .as_str()
        .expect("second child id")
        .parse::<i64>()?;
    let sub_rename_conflict = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/categories/{second_child_id}"),
            json!({"name": "First"}),
        ))
        .await?;
    assert_eq!(sub_rename_conflict.status(), StatusCode::CONFLICT);

    let orphan_id = {
        let connection = Connection::open(&fixture.db_path)?;
        connection.execute(
            "INSERT INTO categories(
                user_id, type, main_category, sub_category, description,
                priority, keywords, hidden, icon, color, created_at
            ) VALUES (42, 3, 'OrphanMain', 'OnlyChild', 'orphan', 33, '', 0, '', '', 'now')",
            [],
        )?;
        connection.last_insert_rowid()
    };

    let orphan_get = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            &format!("/api/categories/{orphan_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(orphan_get.status(), StatusCode::OK);
    assert_eq!(
        read_json(orphan_get).await["result"]["parentId"],
        "virtual_OrphanMain"
    );

    let orphan_tree = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/tree",
            Body::empty(),
        ))
        .await?;
    assert_eq!(orphan_tree.status(), StatusCode::OK);
    assert!(serde_json::to_string(&read_json(orphan_tree).await)?.contains("virtual_OrphanMain"));

    let orphan_virtual_update = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_OrphanMain",
            json!({"name": "OrphanRenamed"}),
        ))
        .await?;
    assert_eq!(orphan_virtual_update.status(), StatusCode::OK);
    assert_eq!(
        read_json(orphan_virtual_update).await["result"]["name"],
        "OrphanRenamed"
    );

    let rollback_virtual = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "RollbackVirtual", "type": 3}),
        ))
        .await?;
    assert_eq!(rollback_virtual.status(), StatusCode::CREATED);
    {
        let connection = Connection::open(&fixture.db_path)?;
        connection.execute_batch(
            "
            CREATE TRIGGER fail_virtual_category_save
            BEFORE UPDATE OF description ON categories
            WHEN NEW.main_category = 'RollbackVirtual2'
            BEGIN
                SELECT RAISE(FAIL, 'forced category update');
            END;
            ",
        )?;
    }
    let virtual_save_failure = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_RollbackVirtual",
            json!({"name": "RollbackVirtual2", "comment": "boom"}),
        ))
        .await?;
    assert_eq!(
        virtual_save_failure.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(virtual_save_failure).await["error"],
        "Failed to save category"
    );
    Connection::open(&fixture.db_path)?.execute("DROP TRIGGER fail_virtual_category_save", [])?;

    let real_rollback = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "RollbackReal", "type": 3}),
        ))
        .await?;
    assert_eq!(real_rollback.status(), StatusCode::CREATED);
    let real_rollback_id = read_json(real_rollback).await["result"]["id"]
        .as_str()
        .expect("rollback real id")
        .parse::<i64>()?;
    {
        let connection = Connection::open(&fixture.db_path)?;
        connection.execute_batch(
            "
            CREATE TRIGGER fail_real_category_save
            BEFORE UPDATE OF description ON categories
            WHEN NEW.main_category = 'RollbackReal2'
            BEGIN
                SELECT RAISE(FAIL, 'forced category update');
            END;
            ",
        )?;
    }
    let real_save_failure = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/categories/{real_rollback_id}"),
            json!({"name": "RollbackReal2", "comment": "boom"}),
        ))
        .await?;
    assert_eq!(
        real_save_failure.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(real_save_failure).await["error"],
        "Failed to update category"
    );
    Connection::open(&fixture.db_path)?.execute("DROP TRIGGER fail_real_category_save", [])?;

    let virtual_missing_old = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_MissingOldGroup",
            json!({"name": "CreatedFromMissingOld"}),
        ))
        .await?;
    assert_eq!(virtual_missing_old.status(), StatusCode::OK);
    assert_eq!(
        read_json(virtual_missing_old).await["result"]["name"],
        "CreatedFromMissingOld"
    );

    {
        let connection = Connection::open(&fixture.db_path)?;
        connection.execute_batch(
            "
            CREATE TRIGGER fail_main_category_rename
            BEFORE UPDATE OF main_category ON categories
            WHEN NEW.main_category = 'BlockedRename'
            BEGIN
                SELECT RAISE(FAIL, 'forced rename constraint');
            END;
            ",
        )?;
    }
    let virtual_rename_constraint = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/virtual_RollbackVirtual",
            json!({"name": "BlockedRename"}),
        ))
        .await?;
    assert_eq!(virtual_rename_constraint.status(), StatusCode::CONFLICT);
    assert_eq!(
        read_json(virtual_rename_constraint).await["error"],
        "Category rename conflict"
    );
    Connection::open(&fixture.db_path)?.execute("DROP TRIGGER fail_main_category_rename", [])?;

    {
        let connection = Connection::open(&fixture.db_path)?;
        connection.execute_batch(
            "
            CREATE TRIGGER fail_category_parent_insert
            BEFORE INSERT ON categories
            WHEN NEW.main_category = 'ConstraintParent'
            BEGIN
                SELECT RAISE(FAIL, 'forced insert constraint');
            END;
            ",
        )?;
    }
    let constraint_parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "ConstraintParent"}),
        ))
        .await?;
    assert_eq!(
        constraint_parent.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    Connection::open(&fixture.db_path)?.execute("DROP TRIGGER fail_category_parent_insert", [])?;

    let child_constraint_parent = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "ChildConstraintParent"}),
        ))
        .await?;
    assert_eq!(child_constraint_parent.status(), StatusCode::CREATED);
    let child_constraint_parent_id = read_json(child_constraint_parent).await["result"]["id"]
        .as_str()
        .expect("child constraint parent id")
        .parse::<i64>()?;
    {
        let connection = Connection::open(&fixture.db_path)?;
        connection.execute_batch(
            "
            CREATE TRIGGER fail_category_child_insert
            BEFORE INSERT ON categories
            WHEN NEW.sub_category = 'ConstraintChild'
            BEGIN
                SELECT RAISE(FAIL, 'forced child insert constraint');
            END;
            ",
        )?;
    }
    let constraint_child = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/",
            json!({"name": "ConstraintChild", "parentId": child_constraint_parent_id.to_string()}),
        ))
        .await?;
    assert_eq!(constraint_child.status(), StatusCode::INTERNAL_SERVER_ERROR);
    Connection::open(&fixture.db_path)?.execute("DROP TRIGGER fail_category_child_insert", [])?;

    Ok(())
}

#[tokio::test]
async fn taxonomy_categories_runtime_validates_edges_and_config() -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let unauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/categories/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauth_response.status(), StatusCode::UNAUTHORIZED);

    let update_all_unauth_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/categories/update-all")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(
        update_all_unauth_response.status(),
        StatusCode::UNAUTHORIZED
    );

    let missing_create = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/categories/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(missing_create).await["error"], "No data provided");

    let nameless_create = app
        .clone()
        .oneshot(json_request(Method::POST, "/api/categories/", json!({})))
        .await?;
    assert_eq!(nameless_create.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(nameless_create).await["error"],
        "No data provided"
    );

    let invalid_id = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/not-a-number",
            Body::empty(),
        ))
        .await?;
    assert_eq!(invalid_id.status(), StatusCode::BAD_REQUEST);
    assert_eq!(read_json(invalid_id).await["error"], "Invalid category ID");

    let missing_category = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            "/api/categories/97",
            Body::empty(),
        ))
        .await?;
    assert_eq!(missing_category.status(), StatusCode::NOT_FOUND);
    assert!(category_exists(&fixture.db_path, 97)?);

    let invalid_batch = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/batch",
            json!({}),
        ))
        .await?;
    assert_eq!(invalid_batch.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_batch).await["error"],
        "No categories provided"
    );

    let invalid_update_all_json = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/categories/update-all",
            Body::from("{"),
        ))
        .await?;
    assert_eq!(invalid_update_all_json.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_update_all_json).await["error"],
        "Invalid JSON"
    );

    let no_db_app = runtime_router_without_db(&fixture);
    let no_db_response = no_db_app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["error"],
        "Rust taxonomy categories DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let no_db_update_all_response = no_db_app
        .oneshot(json_request(
            Method::POST,
            "/api/categories/update-all",
            json!({"force": true}),
        ))
        .await?;
    assert_eq!(
        no_db_update_all_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read_json(no_db_update_all_response).await["error"],
        "Rust taxonomy categories DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_categories_runtime_reports_db_errors_for_missing_schema(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    Connection::open(&fixture.db_path)?.execute("DROP TABLE categories", [])?;
    let app = runtime_router(&fixture);

    let response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        read_json(response).await["error"],
        "Rust taxonomy categories route runtime DB error"
    );

    for (method, uri, body) in [
        (Method::GET, "/api/categories/flat", Body::empty()),
        (Method::GET, "/api/categories/all", Body::empty()),
        (
            Method::POST,
            "/api/categories/",
            Body::from(r#"{"name":"Broken"}"#),
        ),
        (
            Method::POST,
            "/api/categories/",
            Body::from(r#"{"name":"BrokenChild","parentId":"30"}"#),
        ),
        (Method::GET, "/api/categories/30", Body::empty()),
        (Method::DELETE, "/api/categories/30", Body::empty()),
        (
            Method::POST,
            "/api/categories/move",
            Body::from(r#"{"newDisplayOrders":[{"id":30,"displayOrder":1}]}"#),
        ),
        (
            Method::POST,
            "/api/categories/batch",
            Body::from(r#"{"categories":[{"name":"Broken"}]}"#),
        ),
        (Method::GET, "/api/categories/export", Body::empty()),
        (
            Method::POST,
            "/api/categories/import",
            Body::from(r#"[{"main_category":"Broken","sub_category":""}]"#),
        ),
    ] {
        let response = app
            .clone()
            .oneshot(authed_request(method, uri, body))
            .await?;
        assert_eq!(
            response.status(),
            StatusCode::INTERNAL_SERVER_ERROR,
            "{uri}"
        );
        assert_eq!(
            read_json(response).await["error"],
            "Rust taxonomy categories route runtime DB error"
        );
    }

    Connection::open(&fixture.db_path)?.execute("DROP TABLE bills", [])?;
    let update_all_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/categories/update-all",
            json!({"force": true}),
        ))
        .await?;
    assert_eq!(
        update_all_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        read_json(update_all_response).await["error"],
        "Rust taxonomy categories route runtime DB error"
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
            username TEXT NOT NULL,
            password_hash TEXT DEFAULT ''
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
        CREATE TABLE categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            type INTEGER DEFAULT 1,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL,
            description TEXT,
            priority INTEGER DEFAULT 0,
            keywords TEXT,
            hidden BOOLEAN DEFAULT 0,
            icon TEXT,
            color TEXT,
            created_at TEXT NOT NULL,
            UNIQUE(user_id, main_category, sub_category)
        );
        CREATE TABLE category_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            category_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            priority INTEGER DEFAULT 0,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            applied_count INTEGER DEFAULT 0,
            last_applied_at TEXT,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE
        );
        CREATE TABLE import_learning_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            match_type TEXT NOT NULL,
            match_value TEXT NOT NULL,
            learned_type TEXT,
            learned_category_id INTEGER,
            enabled INTEGER DEFAULT 1,
            applied_count INTEGER DEFAULT 0,
            updated_at TEXT
        );
        CREATE TABLE bill_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            description TEXT,
            type TEXT NOT NULL,
            category TEXT,
            amount REAL,
            account TEXT,
            counterparty TEXT,
            tag TEXT,
            comment TEXT,
            is_favorite BOOLEAN DEFAULT 0,
            use_count INTEGER DEFAULT 0,
            last_used_at TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            destination_amount REAL DEFAULT 0,
            hide_amount INTEGER DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            utc_offset INTEGER DEFAULT 0
        );
        CREATE TABLE recurring_bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            template_id INTEGER,
            name TEXT NOT NULL,
            description TEXT,
            type TEXT NOT NULL,
            category TEXT,
            amount REAL NOT NULL,
            account TEXT,
            counterparty TEXT,
            tag TEXT,
            comment TEXT,
            frequency TEXT NOT NULL,
            start_date TEXT NOT NULL,
            end_date TEXT,
            next_date TEXT NOT NULL,
            enabled BOOLEAN DEFAULT 1,
            auto_create BOOLEAN DEFAULT 0,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
            destination_amount REAL DEFAULT 0,
            hide_amount INTEGER DEFAULT 0,
            display_order INTEGER DEFAULT 0,
            hidden INTEGER DEFAULT 0,
            utc_offset INTEGER DEFAULT 0,
            scheduled_frequency_type INTEGER DEFAULT 0
        );
        CREATE TABLE bills (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            type TEXT,
            amount REAL NOT NULL,
            date TEXT NOT NULL,
            counterparty TEXT DEFAULT '',
            description TEXT DEFAULT '',
            payment_method TEXT DEFAULT '',
            main_category TEXT,
            sub_category TEXT,
            batch_id TEXT,
            hash TEXT,
            created_at TEXT NOT NULL DEFAULT 'now',
            updated_at TEXT NOT NULL DEFAULT 'now',
            source_account_id INTEGER DEFAULT 0,
            destination_account_id INTEGER DEFAULT 0,
            destination_amount REAL DEFAULT 0,
            created_from_template INTEGER,
            created_from_recurring INTEGER,
            import_history_id INTEGER
        );
        CREATE TABLE bill_tags (
            bill_id INTEGER NOT NULL,
            tag_id INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE account_transfers (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            from_account_id INTEGER NOT NULL,
            to_account_id INTEGER NOT NULL,
            from_amount REAL DEFAULT 0,
            to_amount REAL DEFAULT 0,
            created_at TEXT NOT NULL
        );
        CREATE TABLE bill_pair_links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL
        );
        CREATE TABLE bill_transfer_pair_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL
        );
        CREATE TABLE bill_investment_pair_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            left_bill_id INTEGER NOT NULL,
            right_bill_id INTEGER NOT NULL
        );
        CREATE TABLE bill_learning_rule_suppressions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            bill_id INTEGER NOT NULL
        );
        CREATE TABLE audit_logs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            operation_type TEXT NOT NULL,
            operation_target TEXT NOT NULL,
            target_id INTEGER,
            details TEXT,
            affected_count INTEGER DEFAULT 0,
            ip_address TEXT,
            user_agent TEXT,
            session_id TEXT,
            status TEXT,
            error_message TEXT,
            created_at TEXT
        );
        CREATE TABLE llm_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            provider TEXT DEFAULT 'openai',
            model TEXT,
            api_key TEXT,
            base_url TEXT,
            advanced_settings TEXT,
            is_active INTEGER DEFAULT 0
        );
        CREATE TABLE app_settings (
            key TEXT PRIMARY KEY,
            value TEXT,
            value_type TEXT,
            description TEXT,
            is_encrypted INTEGER DEFAULT 0,
            created_at TEXT,
            updated_at TEXT
        );
        ",
    )?;
    let password_hash = bcrypt::hash("correct horse battery staple", 4)?;
    connection.execute(
        "INSERT INTO users(id, username, password_hash) VALUES (42, 'owner', ?1), (77, 'other', '')",
        [&password_hash],
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
    connection.execute(
        "INSERT INTO categories(
            id, user_id, type, main_category, sub_category, description,
            priority, keywords, hidden, icon, color, created_at
        )
        VALUES
            (30, 42, 3, '餐饮', '', '主分类', 1, '', 0, 'mdi-food', '#ff6600', 'now'),
            (31, 42, 3, '餐饮', '午餐', '子分类', 2, '饭', 1, 'mdi-food', '#ff6600', 'now'),
            (97, 77, 3, '其他用户分类', '', '', 0, '', 0, '', '', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO category_rules(
            id, user_id, category_id, name, priority, rule_expression,
            regex_enabled, enabled, applied_count, last_applied_at, created_at, updated_at
        )
        VALUES
            (60, 42, 31, '午餐规则', 10, 'OR={午餐,饭}', 0, 1, 2, '2026-01-02T00:00:00', 'now', 'now'),
            (61, 42, 31, '禁用规则', 20, 'OR={禁用}', 0, 0, 0, NULL, 'now', 'now'),
            (96, 77, 97, '其他用户规则', 1, 'OR={其他}', 0, 1, 1, NULL, 'now', 'now')",
        [],
    )?;
    connection.execute(
        "INSERT INTO import_learning_rules(
            id, user_id, match_type, match_value, learned_type,
            learned_category_id, enabled, applied_count, updated_at
        )
        VALUES
            (70, 42, 'counterparty', '招商银行', 'expense', 31, 1, 3, '2026-01-01T00:00:00'),
            (71, 42, 'description', '基金定投', 'expense', 31, 0, 1, '2026-01-02T00:00:00'),
            (95, 77, 'description', '其他用户学习规则', 'expense', 97, 1, 5, '2026-01-03T00:00:00')",
        [],
    )?;
    connection.execute(
        "INSERT INTO bill_templates(
            id, user_id, name, description, type, category, amount, account,
            counterparty, tag, comment, is_favorite, use_count, last_used_at,
            created_at, updated_at, destination_amount, hide_amount,
            display_order, hidden, utc_offset
        )
        VALUES
            (40, 42, '午餐模板', '工作日午餐', '支出', '31', 12.5, '10', '0', '20,21', '常用', 1, 2, '2026-01-03T00:00:00', 'now', 'now', 0, 0, 2, 0, 480),
            (96, 77, '其他用户模板', '', '支出', '97', 99.0, '99', '0', '', '', 0, 0, NULL, 'now', 'now', 0, 0, 1, 0, 480)",
        [],
    )?;
    connection.execute(
        "INSERT INTO recurring_bills(
            id, user_id, template_id, name, description, type, category,
            amount, account, counterparty, tag, comment, frequency,
            scheduled_frequency_type, start_date, end_date, next_date,
            enabled, auto_create, display_order, hidden, utc_offset,
            created_at, updated_at, destination_amount, hide_amount
        )
        VALUES
            (41, 42, NULL, '房租模板', '每月房租', '支出', '30', 3000.0, '10', '0', '', '租金', 'monthly', 2, '2026-01-01', NULL, '2026-02-01', 1, 0, 1, 0, 480, 'now', 'now', 0, 0)",
        [],
    )?;
    connection.execute(
        "INSERT INTO bills(
            id, user_id, type, amount, date, counterparty, description,
            main_category, sub_category
        )
        VALUES
            (50, 42, '支出', -12.5, '2026-01-05', '食堂', '午餐套餐', '餐饮', '午餐'),
            (51, 42, '支出', -7.5, '2026-01-08', '饭馆', '午餐', '餐饮', '午餐'),
            (52, 42, '支出', -3.0, '2026-01-10', '公交', '通勤', '交通', '公交'),
            (53, 42, '支出', -99.0, '2025-12-31', '餐厅', '晚餐', '餐饮', '晚餐'),
            (54, 42, '支出', -15.0, '2026-01-11', '食堂', '午餐套餐', '', ''),
            (96, 42, '支出', -2.0, '2026-01-12', '无人匹配', '空规则', '', ''),
            (97, 77, '支出', -99.0, '2026-01-10', '其他', '午餐', '其他用户分类', '')",
        [],
    )?;
    connection.execute(
        "INSERT INTO llm_configs(
            id, user_id, name, provider, model, api_key, base_url, advanced_settings, is_active
        )
        VALUES
            (80, 42, '主 LLM', 'openai', 'gpt-test', 'secret-key', 'https://llm.example.test',
             '{\"reasoning_depth\":\"High\",\"temperature\":0.3,\"max_tokens\":2048,\"ignored\":true}', 1),
            (81, 77, '其他用户 LLM', 'openai', 'other', 'other-secret', '', '{}', 0)",
        [],
    )?;
    connection.execute(
        "INSERT INTO app_settings(
            key, value, value_type, description, is_encrypted, created_at, updated_at
        )
        VALUES
            ('receipt_ocr_config', '{\"provider\":\"cloud_stub\",\"lang\":\"eng\"}', 'json',
             'Receipt OCR runtime configuration', 0, 'now', 'now'),
            ('operation_password', 'operation-secret', 'string',
             'Sensitive operation fallback password', 0, 'now', 'now')",
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

fn template_exists(path: &Path, template_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM bill_templates WHERE id = ?1",
        [template_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn template_display_order(path: &Path, template_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT display_order FROM bill_templates WHERE id = ?1",
        [template_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn category_exists(path: &Path, category_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM categories WHERE id = ?1",
        [category_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn category_priority(path: &Path, category_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT priority FROM categories WHERE id = ?1",
        [category_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn category_rule_exists(path: &Path, rule_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM category_rules WHERE id = ?1",
        [rule_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn category_rule_priority(path: &Path, rule_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT priority FROM category_rules WHERE id = ?1",
        [rule_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn bill_category_pair(path: &Path, bill_id: i64) -> Result<(String, String), Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COALESCE(main_category, ''), COALESCE(sub_category, '') FROM bills WHERE id = ?1",
        [bill_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?)
}

fn bill_account_links(path: &Path, bill_id: i64) -> Result<(i64, i64), Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT source_account_id, destination_account_id FROM bills WHERE id = ?1",
        [bill_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?)
}

fn account_transfer_links(path: &Path, transfer_id: i64) -> Result<(i64, i64), Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT from_account_id, to_account_id FROM account_transfers WHERE id = ?1",
        [transfer_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?)
}

fn bill_exists(path: &Path, bill_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM bills WHERE id = ?1",
        [bill_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn bill_tag_count(path: &Path, bill_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) FROM bill_tags WHERE bill_id = ?1",
        [bill_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn bill_pair_link_count(path: &Path, user_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT
            (SELECT COUNT(*) FROM bill_pair_links WHERE user_id = ?1)
          + (SELECT COUNT(*) FROM bill_transfer_pair_suppressions WHERE user_id = ?1)
          + (SELECT COUNT(*) FROM bill_investment_pair_suppressions WHERE user_id = ?1)
          + (SELECT COUNT(*) FROM bill_learning_rule_suppressions WHERE user_id = ?1)",
        [user_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn account_transfer_count(path: &Path, user_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) FROM account_transfers WHERE user_id = ?1",
        [user_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn audit_log_count(path: &Path, operation_type: &str, status: &str) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) FROM audit_logs WHERE operation_type = ?1 AND status = ?2",
        [operation_type, status],
        |row| row.get::<_, i64>(0),
    )?)
}
