use std::{env, error::Error, path::Path, time::Duration};

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use bill_analyser_db::run_postgres_migrations;
use bill_analyser_http::{
    build_router, config::DatabaseBackend, HttpAppState, HttpShellConfig, ImportRouteMode,
    TAXONOMY_ACCOUNT_ROUTE_PATTERNS, TAXONOMY_ACCOUNT_RULE_ROUTE_PATTERNS,
    TAXONOMY_CATEGORY_ROUTE_PATTERNS, TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS,
    TAXONOMY_RULE_CENTER_ROUTE_PATTERNS, TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS,
    TAXONOMY_TAG_ROUTE_PATTERNS, TAXONOMY_TEMPLATE_ROUTE_PATTERNS,
};
use rusqlite::Connection;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, Row};
use tempfile::TempDir;
use tower::ServiceExt;

const TEST_AUTH_SECRET: &str = "taxonomy-route-secret";
const TEST_USER_ID: &str = "42";

#[tokio::test]
async fn taxonomy_postgres_runtime_serves_master_data_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("taxonomy-pg-{unique}"))
            .bind(format!("taxonomy-pg-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let account_id: i64 = sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents, is_active, display_order, metadata
        )
        VALUES ($1, $2, $3, 'CNY', 12345, false, 2, $4)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind("Pg Wallet")
    .bind("1")
    .bind(json!({
        "category": 7,
        "icon": "mdi-wallet",
        "color": "#336699",
        "comment": "postgres account"
    }))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents, is_active, display_order, metadata
        )
        VALUES ($1, $2, $3, 'CNY', 125, true, 1, $4)
        "#,
    )
    .bind(user_id)
    .bind("Pg Sub")
    .bind("1")
    .bind(json!({"parent_id": account_id}))
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, name, category_type, path, icon, color, display_order, is_active, metadata
        )
        VALUES ($1, '餐饮', '3', '餐饮', 'mdi-food', '#ffcc00', 1, true, $2),
               ($1, '午餐', '3', '餐饮/午餐', 'mdi-lunch', '#ffaa00', 2, false, $3)
        "#,
    )
    .bind(user_id)
    .bind(json!({"description": "food", "keywords": "eat"}))
    .bind(json!({"description": "lunch", "keywords": "meal"}))
    .execute(&pool)
    .await?;
    let tag_id: i64 = sqlx::query(
        r#"
        INSERT INTO tags (user_id, name, color, display_order, metadata)
        VALUES ($1, 'pg-tag', '#123456', 1, $2)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(json!({"icon": "mdi-tag", "hidden": true}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, transaction_type, category_id, source_account_id,
            destination_account_id, source_amount_minor_units, destination_amount_minor_units,
            hide_amount, tag_ids, comment, display_order, hidden, utc_offset
        )
        VALUES ($1, 1, 'Pg Template', '支出', '20', $2, '0', 1999, 0, false, $3, 'postgres template', 3, false, 480)
        "#,
    )
    .bind(user_id)
    .bind(account_id.to_string())
    .bind(json!([tag_id.to_string()]))
    .execute(&pool)
    .await?;

    let direct_accounts =
        bill_analyser_db::taxonomy::postgres_reads::list_postgres_accounts(&pool, user_id).await;
    assert!(
        direct_accounts.is_ok(),
        "direct postgres account projection failed: {direct_accounts:?}"
    );

    let app = postgres_runtime_router(&postgres_url)?;
    let accounts = get_result(
        app.clone()
            .oneshot(authed_request_for_user(
                Method::GET,
                "/api/accounts?visible_only=false",
                Body::empty(),
                user_id,
            ))
            .await?,
    )
    .await;
    assert_eq!(accounts[0]["name"], "Pg Wallet");
    assert_eq!(accounts[0]["balance"], 12345);
    assert_eq!(accounts[0]["hidden"], true);
    assert_eq!(accounts[0]["subAccounts"][0]["name"], "Pg Sub");

    let categories = get_result(
        app.clone()
            .oneshot(authed_request_for_user(
                Method::GET,
                "/api/categories",
                Body::empty(),
                user_id,
            ))
            .await?,
    )
    .await;
    assert_eq!(categories["3"][0]["name"], "餐饮");
    assert_eq!(categories["3"][0]["subCategories"][0]["name"], "午餐");
    assert_eq!(categories["3"][0]["subCategories"][0]["visible"], false);

    let tags = get_result(
        app.clone()
            .oneshot(authed_request_for_user(
                Method::GET,
                "/api/tags",
                Body::empty(),
                user_id,
            ))
            .await?,
    )
    .await;
    assert_eq!(tags[0]["name"], "pg-tag");
    assert_eq!(tags[0]["icon"], "mdi-tag");
    assert_eq!(tags[0]["visible"], false);

    let templates = get_result(
        app.oneshot(authed_request_for_user(
            Method::GET,
            "/api/templates?templateType=1",
            Body::empty(),
            user_id,
        ))
        .await?,
    )
    .await;
    assert_eq!(templates[0]["name"], "Pg Template");
    assert_eq!(templates[0]["sourceAmount"], 1999.0);
    assert_eq!(templates[0]["tagIds"], json!([tag_id.to_string()]));

    Ok(())
}

#[tokio::test]
async fn taxonomy_postgres_runtime_serves_category_mutations_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("taxonomy-pg-category-{unique}"))
            .bind(format!("taxonomy-pg-category-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let app = postgres_runtime_router(&postgres_url)?;
    let main_name = format!("pg-api-category-{unique}");
    let sub_name = format!("pg-api-sub-{unique}");
    let updated_sub_name = format!("pg-api-sub-updated-{unique}");

    let create_parent = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories",
            json!({
                "name": main_name,
                "type": 3,
                "comment": "postgres category",
                "keywords": "coffee",
                "icon": "mdi-food",
                "color": "#ffaa00",
                "visible": true,
                "displayOrder": 5
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_parent.status(), StatusCode::CREATED);
    let create_parent_body = read_json(create_parent).await;
    assert_eq!(create_parent_body["result"]["name"], main_name);
    assert_eq!(create_parent_body["result"]["comment"], "postgres category");
    let parent_id = create_parent_body["result"]["id"]
        .as_str()
        .expect("parent category id")
        .parse::<i64>()?;
    let parent_row = sqlx::query(
        "SELECT path, display_order, is_active, metadata FROM categories WHERE id = $1",
    )
    .bind(parent_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(parent_row.try_get::<String, _>("path")?, main_name);
    assert_eq!(parent_row.try_get::<i32, _>("display_order")?, 5);
    assert!(parent_row.try_get::<bool, _>("is_active")?);
    let parent_metadata: Value = parent_row.try_get("metadata")?;
    assert_eq!(parent_metadata["description"], "postgres category");
    assert_eq!(parent_metadata["keywords"], "coffee");

    let create_child = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories",
            json!({
                "name": sub_name,
                "parentId": parent_id.to_string(),
                "comment": "postgres subcategory",
                "visible": false,
                "displayOrder": 6
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_child.status(), StatusCode::OK);
    let create_child_body = read_json(create_child).await;
    assert_eq!(
        create_child_body["result"]["parentId"],
        parent_id.to_string()
    );
    assert_eq!(create_child_body["result"]["visible"], false);
    let child_id = create_child_body["result"]["id"]
        .as_str()
        .expect("child category id")
        .parse::<i64>()?;
    let child_row = sqlx::query("SELECT parent_id, path, is_active FROM categories WHERE id = $1")
        .bind(child_id)
        .fetch_one(&pool)
        .await?;
    assert_eq!(
        child_row.try_get::<Option<i64>, _>("parent_id")?,
        Some(parent_id)
    );
    assert_eq!(
        child_row.try_get::<String, _>("path")?,
        format!("{main_name}/{sub_name}")
    );
    assert!(!child_row.try_get::<bool, _>("is_active")?);

    let get_child = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &format!("/api/categories/{child_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(get_child.status(), StatusCode::OK);
    assert_eq!(
        read_json(get_child).await["result"]["parentId"],
        parent_id.to_string()
    );

    let virtual_main = format!("pg-virtual-{unique}");
    let virtual_child_name = format!("pg-virtual-child-{unique}");
    let create_virtual_child = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories",
            json!({
                "name": virtual_child_name,
                "parentId": format!("virtual_{virtual_main}"),
                "displayOrder": 7
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_virtual_child.status(), StatusCode::OK);
    let virtual_child_id = read_json(create_virtual_child).await["result"]["id"]
        .as_str()
        .expect("virtual child category id")
        .parse::<i64>()?;
    let get_virtual_child = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &format!("/api/categories/{virtual_child_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(get_virtual_child.status(), StatusCode::OK);
    assert_eq!(
        read_json(get_virtual_child).await["result"]["parentId"],
        format!("virtual_{virtual_main}")
    );

    let renamed_virtual_main = format!("pg-virtual-renamed-{unique}");
    let update_virtual_parent = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/categories/virtual_{virtual_main}"),
            json!({
                "name": renamed_virtual_main,
                "type": 3,
                "visible": false,
                "displayOrder": 14
            }),
            user_id,
        ))
        .await?;
    assert_eq!(update_virtual_parent.status(), StatusCode::OK);
    let update_virtual_parent_body = read_json(update_virtual_parent).await;
    assert_eq!(
        update_virtual_parent_body["result"]["name"],
        renamed_virtual_main
    );
    assert_eq!(update_virtual_parent_body["result"]["hidden"], true);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT path FROM categories WHERE id = $1")
            .bind(virtual_child_id)
            .fetch_one(&pool)
            .await?,
        format!("{renamed_virtual_main}/{virtual_child_name}")
    );

    let update_child = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/categories/{child_id}"),
            json!({
                "name": updated_sub_name,
                "comment": "postgres updated",
                "keywords": "lunch",
                "visible": true,
                "displayOrder": 2
            }),
            user_id,
        ))
        .await?;
    assert_eq!(update_child.status(), StatusCode::OK);
    let update_child_body = read_json(update_child).await;
    assert_eq!(update_child_body["result"]["name"], updated_sub_name);
    assert_eq!(update_child_body["result"]["visible"], true);
    let updated_child_row = sqlx::query(
        "SELECT path, display_order, is_active, metadata FROM categories WHERE id = $1",
    )
    .bind(child_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        updated_child_row.try_get::<String, _>("path")?,
        format!("{main_name}/{updated_sub_name}")
    );
    assert_eq!(updated_child_row.try_get::<i32, _>("display_order")?, 2);
    assert!(updated_child_row.try_get::<bool, _>("is_active")?);
    let updated_metadata: Value = updated_child_row.try_get("metadata")?;
    assert_eq!(updated_metadata["description"], "postgres updated");
    assert_eq!(updated_metadata["keywords"], "lunch");

    let move_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories/move",
            json!({"newDisplayOrders": [
                {"id": parent_id, "displayOrder": 9},
                {"id": child_id, "displayOrder": 1}
            ]}),
            user_id,
        ))
        .await?;
    assert_eq!(move_response.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT display_order FROM categories WHERE id = $1")
            .bind(child_id)
            .fetch_one(&pool)
            .await?,
        1
    );

    let batch_main = format!("pg-batch-{unique}");
    let batch_sub = format!("pg-batch-sub-{unique}");
    let batch_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories/batch",
            json!({"categories": [{
                "name": batch_main,
                "type": 3,
                "subCategories": [{"name": batch_sub, "displayOrder": 4}]
            }]}),
            user_id,
        ))
        .await?;
    assert_eq!(batch_response.status(), StatusCode::OK);
    assert!(
        serde_json::to_string(&read_json(batch_response).await)?.contains(&batch_sub),
        "batch response should include created subcategory"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM categories WHERE user_id = $1 AND path = $2",
        )
        .bind(user_id)
        .bind(format!("{batch_main}/{batch_sub}"))
        .fetch_one(&pool)
        .await?,
        1
    );

    let import_main = format!("pg-import-{unique}");
    let import_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories/import",
            json!({"categories": [
                {"main_category": import_main, "sub_category": "", "description": "import parent", "priority": 10},
                {"main_category": import_main, "sub_category": "child", "priority": 11},
                {"sub_category": "missing-main"}
            ]}),
            user_id,
        ))
        .await?;
    assert_eq!(import_response.status(), StatusCode::OK);
    let import_body = read_json(import_response).await;
    assert_eq!(import_body["result"]["imported"], 2);
    assert_eq!(import_body["result"]["skipped"], 1);

    let import_update = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/categories/import",
            json!({"result": [{
                "main_category": import_main,
                "sub_category": "child",
                "priority": 12
            }]}),
            user_id,
        ))
        .await?;
    assert_eq!(import_update.status(), StatusCode::OK);
    assert_eq!(read_json(import_update).await["result"]["updated"], 1);

    let export_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/categories/export",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(export_response.status(), StatusCode::OK);
    let export_body = read_json(export_response).await;
    let exported_categories = export_body["result"].as_array().expect("export categories");
    assert!(exported_categories
        .iter()
        .any(|category| category["main_category"] == main_name));
    assert!(exported_categories
        .iter()
        .all(|category| category.get("id").is_none()));

    sqlx::query(
        r#"
        INSERT INTO bills (
            user_id, occurred_at, amount_cents, direction, transaction_type,
            category_id, merchant, description
        )
        VALUES ($1, '2026-01-10T12:00:00Z', 1234, 'expense', 'expense', $2, 'store', 'one'),
               ($1, '2026-01-11T12:00:00Z', 566, 'expense', 'expense', $2, 'store', 'two')
        "#,
    )
    .bind(user_id)
    .bind(child_id)
    .execute(&pool)
    .await?;
    let statistics_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/categories/statistics?start_date=2026-01-01&end_date=2026-01-31",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(statistics_response.status(), StatusCode::OK);
    let statistics_body = read_json(statistics_response).await;
    assert_eq!(
        statistics_body["result"][main_name.as_str()]["total_amount"],
        18.0
    );
    assert_eq!(statistics_body["result"][main_name.as_str()]["count"], 2);
    assert_eq!(
        statistics_body["result"][main_name.as_str()]["sub_categories"][updated_sub_name.as_str()]
            ["total_amount"],
        18.0
    );

    let renamed_main_name = format!("pg-api-category-renamed-{unique}");
    let rename_parent = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/categories/{parent_id}"),
            json!({"name": renamed_main_name, "comment": "renamed parent"}),
            user_id,
        ))
        .await?;
    assert_eq!(rename_parent.status(), StatusCode::OK);
    assert_eq!(
        read_json(rename_parent).await["result"]["name"],
        renamed_main_name
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT path FROM categories WHERE id = $1")
            .bind(child_id)
            .fetch_one(&pool)
            .await?,
        format!("{renamed_main_name}/{updated_sub_name}")
    );

    let delete_parent = app
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/categories/{parent_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_parent.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM categories WHERE user_id = $1 AND (path = $2 OR path LIKE $3)",
        )
        .bind(user_id)
        .bind(&renamed_main_name)
        .bind(format!("{renamed_main_name}/%"))
        .fetch_one(&pool)
        .await?,
        0
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_postgres_runtime_serves_tag_template_mutations_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("taxonomy-pg-write-{unique}"))
            .bind(format!("taxonomy-pg-write-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let app = postgres_runtime_router(&postgres_url)?;

    let create_tag = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/tags",
            json!({
                "name": "pg-api-tag",
                "color": "#456789",
                "icon": "mdi-api",
                "hidden": true,
                "displayOrder": 5
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_tag.status(), StatusCode::CREATED);
    let create_tag_body = read_json(create_tag).await;
    assert_eq!(create_tag_body["result"]["name"], "pg-api-tag");
    assert_eq!(create_tag_body["result"]["visible"], false);
    let tag_id = create_tag_body["result"]["id"]
        .as_str()
        .expect("tag id")
        .parse::<i64>()?;
    let tag_metadata: Value = sqlx::query("SELECT metadata FROM tags WHERE id = $1")
        .bind(tag_id)
        .fetch_one(&pool)
        .await?
        .try_get("metadata")?;
    assert_eq!(tag_metadata["icon"], "mdi-api");
    assert_eq!(tag_metadata["hidden"], true);

    let update_tag = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/tags/{tag_id}"),
            json!({
                "name": "pg-api-tag-updated",
                "hidden": false,
                "displayOrder": 8
            }),
            user_id,
        ))
        .await?;
    assert_eq!(update_tag.status(), StatusCode::OK);
    let update_tag_body = read_json(update_tag).await;
    assert_eq!(update_tag_body["result"]["name"], "pg-api-tag-updated");
    assert_eq!(update_tag_body["result"]["visible"], true);
    assert_eq!(update_tag_body["result"]["displayOrder"], 8);

    let second_tag = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/tags",
            json!({"name": "pg-api-tag-second", "displayOrder": 9}),
            user_id,
        ))
        .await?;
    assert_eq!(second_tag.status(), StatusCode::CREATED);
    let second_tag_id = read_json(second_tag).await["result"]["id"]
        .as_str()
        .expect("second tag id")
        .parse::<i64>()?;
    let order_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            "/api/tags/display-orders",
            json!({"newDisplayOrders": [
                {"id": tag_id, "displayOrder": 2},
                {"id": second_tag_id, "displayOrder": 1}
            ]}),
            user_id,
        ))
        .await?;
    assert_eq!(order_response.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query("SELECT display_order FROM tags WHERE id = $1")
            .bind(second_tag_id)
            .fetch_one(&pool)
            .await?
            .try_get::<i32, _>("display_order")?,
        1
    );

    let create_template = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/templates",
            json!({
                "templateType": 1,
                "name": "pg-api-template",
                "type": 3,
                "categoryId": "31",
                "sourceAccountId": "10",
                "destinationAccountId": "0",
                "sourceAmount": 1850,
                "destinationAmount": 0,
                "hideAmount": false,
                "tagIds": [tag_id.to_string()],
                "comment": "postgres write",
                "hidden": false,
                "utcOffset": 480
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_template.status(), StatusCode::CREATED);
    let create_template_body = read_json(create_template).await;
    assert_eq!(create_template_body["result"]["name"], "pg-api-template");
    assert_eq!(create_template_body["result"]["sourceAmount"], 1850);
    assert_eq!(
        create_template_body["result"]["tagIds"],
        json!([tag_id.to_string()])
    );
    let template_id = create_template_body["result"]["id"]
        .as_str()
        .expect("template id")
        .parse::<i64>()?;
    let template_row = sqlx::query(
        "SELECT source_amount_minor_units, tag_ids FROM transaction_templates WHERE id = $1",
    )
    .bind(template_id)
    .fetch_one(&pool)
    .await?;
    assert_eq!(
        template_row.try_get::<i64, _>("source_amount_minor_units")?,
        1850
    );
    assert_eq!(
        template_row.try_get::<Value, _>("tag_ids")?,
        json!([tag_id.to_string()])
    );

    let update_template = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/templates/{template_id}?templateType=1"),
            json!({
                "name": "pg-api-template-updated",
                "sourceAmount": 2000,
                "tagIds": [second_tag_id.to_string()],
                "hidden": true,
                "displayOrder": 4
            }),
            user_id,
        ))
        .await?;
    assert_eq!(update_template.status(), StatusCode::OK);
    let update_template_body = read_json(update_template).await;
    assert_eq!(
        update_template_body["result"]["name"],
        "pg-api-template-updated"
    );
    assert_eq!(update_template_body["result"]["sourceAmount"], 2000);
    assert_eq!(
        update_template_body["result"]["tagIds"],
        json!([second_tag_id.to_string()])
    );
    assert_eq!(update_template_body["result"]["hidden"], true);

    let order_templates = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            "/api/templates/display-orders?templateType=1",
            json!({"newDisplayOrders": [{"id": template_id, "displayOrder": 1}]}),
            user_id,
        ))
        .await?;
    assert_eq!(order_templates.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query("SELECT display_order FROM transaction_templates WHERE id = $1")
            .bind(template_id)
            .fetch_one(&pool)
            .await?
            .try_get::<i32, _>("display_order")?,
        1
    );

    let delete_template = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/templates/{template_id}?templateType=1"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_template.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transaction_templates WHERE id = $1")
            .bind(template_id)
            .fetch_one(&pool)
            .await?,
        0
    );

    let delete_tag = app
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/tags/{tag_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_tag.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tags WHERE id = $1")
            .bind(tag_id)
            .fetch_one(&pool)
            .await?,
        0
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_postgres_runtime_serves_account_mutations_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("taxonomy-pg-account-{unique}"))
            .bind(format!("taxonomy-pg-account-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let app = postgres_runtime_router(&postgres_url)?;

    let create_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/accounts",
            json!({
                "name": format!("pg-api-account-{unique}"),
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "balance": 2500,
                "visible": true,
                "displayOrder": 5,
                "subAccounts": [{
                    "name": format!("pg-api-child-{unique}"),
                    "type": 1,
                    "balance": 125,
                    "visible": false
                }]
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["success"], true);
    assert_eq!(
        create_body["result"]["name"],
        format!("pg-api-account-{unique}")
    );
    assert_eq!(create_body["result"]["balance"], 2500);
    assert!(create_body["result"].get("aliases").is_none());
    assert_eq!(create_body["result"]["subAccounts"][0]["balance"], 125);
    assert_eq!(
        create_body["result"]["subAccounts"][0]["parentId"],
        create_body["result"]["id"]
    );
    let account_id = create_body["result"]["id"]
        .as_str()
        .expect("account id")
        .parse::<i64>()?;
    let child_id = create_body["result"]["subAccounts"][0]["id"]
        .as_str()
        .expect("child account id")
        .parse::<i64>()?;

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT balance_cents FROM accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await?,
        2500
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT balance_cents FROM accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(child_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await?,
        125
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM accounts WHERE user_id = $1 AND metadata->>'parent_id' = $2",
        )
        .bind(user_id)
        .bind(account_id.to_string())
        .fetch_one(&pool)
        .await?,
        1
    );

    let get_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &format!("/api/accounts/{account_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(get_response.status(), StatusCode::OK);
    let get_body = read_json(get_response).await;
    assert_eq!(
        get_body["result"]["subAccounts"][0]["name"],
        format!("pg-api-child-{unique}")
    );

    let update_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/accounts/{account_id}"),
            json!({
                "name": format!("pg-api-account-updated-{unique}"),
                "type": 1,
                "category": 2,
                "currency": "CNY",
                "balance": 3099,
                "hidden": true,
                "displayOrder": 2,
                "subAccounts": [{
                    "id": child_id,
                    "name": format!("pg-api-child-updated-{unique}"),
                    "type": 1,
                    "balance": 333,
                    "visible": true
                }]
            }),
            user_id,
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(
        update_body["result"]["name"],
        format!("pg-api-account-updated-{unique}")
    );
    assert_eq!(update_body["result"]["balance"], 3099);
    assert!(update_body["result"].get("aliases").is_none());
    assert_eq!(update_body["result"]["hidden"], true);
    assert_eq!(
        update_body["result"]["subAccounts"][0]["name"],
        format!("pg-api-child-updated-{unique}")
    );
    assert_eq!(update_body["result"]["subAccounts"][0]["balance"], 333);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT balance_cents FROM accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(account_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await?,
        3099
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT balance_cents FROM accounts WHERE id = $1 AND user_id = $2",
        )
        .bind(child_id)
        .bind(user_id)
        .fetch_one(&pool)
        .await?,
        333
    );

    let second_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/accounts",
            json!({
                "name": format!("pg-api-account-second-{unique}"),
                "type": 1,
                "balance": 0,
                "displayOrder": 9
            }),
            user_id,
        ))
        .await?;
    assert_eq!(second_response.status(), StatusCode::CREATED);
    let second_id = read_json(second_response).await["result"]["id"]
        .as_str()
        .expect("second account id")
        .parse::<i64>()?;

    let order_response = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            "/api/accounts/display-orders",
            json!({"newDisplayOrders": [
                {"id": account_id, "displayOrder": 7},
                {"id": second_id, "displayOrder": 1}
            ]}),
            user_id,
        ))
        .await?;
    assert_eq!(order_response.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT display_order FROM accounts WHERE id = $1")
            .bind(second_id)
            .fetch_one(&pool)
            .await?,
        1
    );

    let delete_response = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/accounts/{account_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM accounts WHERE user_id = $1 AND id IN ($2, $3)",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(child_id)
        .fetch_one(&pool)
        .await?,
        0
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_postgres_runtime_serves_rule_mutations_without_sqlite_fallback(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    run_postgres_migrations(&pool).await?;

    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("taxonomy-pg-rules-{unique}"))
            .bind(format!("taxonomy-pg-rules-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let other_user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("taxonomy-pg-rules-other-{unique}"))
            .bind(format!("taxonomy-pg-rules-other-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;
    let main_name = format!("pg-rules-main-{unique}");
    let sub_name = format!("pg-rules-sub-{unique}");
    let parent_id: i64 = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, display_order)
        VALUES ($1, $2, '3', $2, 1)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&main_name)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let category_id: i64 = sqlx::query(
        r#"
        INSERT INTO categories (user_id, parent_id, name, category_type, path, display_order)
        VALUES ($1, $2, $3, '3', $4, 2)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(&sub_name)
    .bind(format!("{main_name}/{sub_name}"))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let other_category_id: i64 = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, display_order)
        VALUES ($1, 'other-category', '3', 'other-category', 1)
        RETURNING id
        "#,
    )
    .bind(other_user_id)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;

    let account_id: i64 = sqlx::query(
        r#"
        INSERT INTO accounts (user_id, name, account_type, balance_cents, metadata)
        VALUES ($1, 'pg-rules-account', '1', 0, $2)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(json!({}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO accounts (user_id, name, account_type, is_active, metadata)
        VALUES ($1, 'pg-rules-hidden-account', '1', false, $2)
        "#,
    )
    .bind(user_id)
    .bind(json!({}))
    .execute(&pool)
    .await?;
    let other_account_id: i64 = sqlx::query(
        r#"
        INSERT INTO accounts (user_id, name, account_type, balance_cents)
        VALUES ($1, 'pg-rules-other-account', '1', 0)
        RETURNING id
        "#,
    )
    .bind(other_user_id)
    .fetch_one(&pool)
    .await?
    .try_get("id")?;

    let category_rule_id: i64 = sqlx::query(
        r#"
        INSERT INTO category_rules (
            user_id, category_id, name, rule_expression, priority, enabled
        )
        VALUES ($1, $2, 'pg lunch rule', $3, 10, true)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(json!({"legacy_expression": "OR={午餐,饭}", "regex_enabled": false}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let disabled_category_rule_id: i64 = sqlx::query(
        r#"
        INSERT INTO category_rules (
            user_id, category_id, name, rule_expression, priority, enabled
        )
        VALUES ($1, $2, 'pg disabled rule', $3, 20, false)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(json!({"legacy_expression": "OR={禁用}", "regex_enabled": false}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let other_category_rule_id: i64 = sqlx::query(
        r#"
        INSERT INTO category_rules (
            user_id, category_id, name, rule_expression, priority, enabled
        )
        VALUES ($1, $2, 'pg other rule', $3, 1, true)
        RETURNING id
        "#,
    )
    .bind(other_user_id)
    .bind(other_category_id)
    .bind(json!({"legacy_expression": "OR={其他}", "regex_enabled": false}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    let account_rule_id: i64 = sqlx::query(
        r#"
        INSERT INTO account_rules (
            user_id, account_id, name, account_role_scope, transaction_type_scope,
            field_scope, rule_expression, regex_enabled, priority, enabled
        )
        VALUES ($1, $2, 'pg account rule', 'source', 'expense',
            $3, $4, false, 10, true)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(json!(["payment_method"]))
    .bind(json!({"legacy_expression": "OR={招商}", "regex_enabled": false}))
    .fetch_one(&pool)
    .await?
    .try_get("id")?;
    sqlx::query(
        r#"
        INSERT INTO account_rules (
            user_id, account_id, name, account_role_scope, transaction_type_scope,
            field_scope, rule_expression, regex_enabled, priority, enabled
        )
        VALUES ($1, $2, 'pg other account rule', 'any', 'all',
            $3, $4, false, 1, true)
        "#,
    )
    .bind(other_user_id)
    .bind(other_account_id)
    .bind(json!(["counterparty"]))
    .bind(json!({"legacy_expression": "OR={其他}", "regex_enabled": false}))
    .execute(&pool)
    .await?;

    let app = postgres_runtime_router(&postgres_url)?;

    let category_list = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            &format!("/api/category-rules/?enabled_only=false&category_id={category_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(category_list.status(), StatusCode::OK);
    let category_list_body = read_json(category_list).await;
    assert_eq!(category_list_body["total"], 2);
    assert_eq!(
        category_list_body["data"][0]["rule_expression"],
        "OR={午餐,饭}"
    );
    assert_eq!(category_list_body["data"][0]["main_category"], main_name);
    assert_eq!(category_list_body["data"][0]["sub_category"], sub_name);
    assert!(!serde_json::to_string(&category_list_body)?.contains("pg other rule"));

    let legacy_rules = get_result(
        app.clone()
            .oneshot(authed_request_for_user(
                Method::GET,
                "/api/categories/rules",
                Body::empty(),
                user_id,
            ))
            .await?,
    )
    .await;
    assert_eq!(legacy_rules[0]["keywords"], "OR={午餐,饭}");
    assert_eq!(legacy_rules[0]["main"], main_name);

    let stored_legacy = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            "/api/categories/rules",
            json!({"rules": [{"main": "自定义", "sub": "规则", "keywords": "OR={x}"}]}),
            user_id,
        ))
        .await?;
    assert_eq!(stored_legacy.status(), StatusCode::OK);
    let stored_legacy_result = get_result(
        app.clone()
            .oneshot(authed_request_for_user(
                Method::GET,
                "/api/categories/rules",
                Body::empty(),
                user_id,
            ))
            .await?,
    )
    .await;
    assert_eq!(stored_legacy_result[0]["main"], "自定义");

    let create_category_rule = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/category-rules/",
            json!({
                "category_id": category_id,
                "name": "pg dinner rule",
                "priority": 5,
                "rule_expression": "OR={晚餐}",
                "regex_enabled": true,
                "enabled": true
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_category_rule.status(), StatusCode::CREATED);
    let create_category_body = read_json(create_category_rule).await;
    assert_eq!(create_category_body["data"]["regex_enabled"], 1);
    let created_category_rule_id = create_category_body["data"]["id"]
        .as_i64()
        .expect("created category rule id");

    let update_category_rule = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/category-rules/{created_category_rule_id}"),
            json!({"rule_expression": "OR={夜宵}", "enabled": false}),
            user_id,
        ))
        .await?;
    assert_eq!(update_category_rule.status(), StatusCode::OK);
    assert_eq!(read_json(update_category_rule).await["data"]["enabled"], 0);
    let stored_expression: Value =
        sqlx::query_scalar("SELECT rule_expression FROM category_rules WHERE id = $1")
            .bind(created_category_rule_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(stored_expression["legacy_expression"], "OR={夜宵}");
    assert_eq!(stored_expression["regex_enabled"], true);

    let reorder_category_rules = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/category-rules/reorder",
            json!({"rule_ids": [
                created_category_rule_id,
                category_rule_id,
                disabled_category_rule_id,
                other_category_rule_id
            ]}),
            user_id,
        ))
        .await?;
    assert_eq!(reorder_category_rules.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT priority FROM category_rules WHERE id = $1")
            .bind(created_category_rule_id)
            .fetch_one(&pool)
            .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT priority FROM category_rules WHERE id = $1")
            .bind(other_category_rule_id)
            .fetch_one(&pool)
            .await?,
        1
    );

    let category_match = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            &format!("/api/category-rules/{category_rule_id}/test"),
            json!({"text": "工作日午餐付款"}),
            user_id,
        ))
        .await?;
    assert_eq!(category_match.status(), StatusCode::OK);
    assert_eq!(read_json(category_match).await["data"]["matched"], true);

    sqlx::query("UPDATE categories SET metadata = $1 WHERE id = $2")
        .bind(json!({"keywords": "OR:午饭|套餐"}))
        .bind(category_id)
        .execute(&pool)
        .await?;
    let migrate_category_keywords = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::POST,
            "/api/category-rules/migrate",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(migrate_category_keywords.status(), StatusCode::OK);
    assert_eq!(
        read_json(migrate_category_keywords).await["data"],
        json!({"migrated": 1, "skipped": 0})
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM category_rules WHERE user_id = $1 AND name LIKE 'migrated:%'",
        )
        .bind(user_id)
        .fetch_one(&pool)
        .await?,
        1
    );

    let defaults = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::POST,
            "/api/category-rules/defaults",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(defaults.status(), StatusCode::OK);
    let defaults_body = read_json(defaults).await;
    assert_eq!(defaults_body["success"], true);
    assert!(
        defaults_body["data"]["categories"]["created"]
            .as_i64()
            .unwrap_or_default()
            > 0
    );
    assert!(
        defaults_body["data"]["rules"]["created"]
            .as_i64()
            .unwrap_or_default()
            > 0
    );

    sqlx::query(
        r#"
        INSERT INTO import_learning_lifecycle (
            user_id, recommendation_key, recommendation_type, status,
            accepted_count, auto_applied_count, metadata
        )
        VALUES ($1, 'pg-learning-rule', 'classification', 'green', 2, 1, $2)
        ON CONFLICT (user_id, recommendation_key) DO NOTHING
        "#,
    )
    .bind(user_id)
    .bind(json!({
        "match_type": "merchant",
        "match_value": "Coffee Shop",
        "learned_type": "expense",
        "learned_category_id": category_id
    }))
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, transaction_type, category_id,
            source_amount_minor_units, scheduled_frequency, scheduled_next_date,
            enabled, display_order
        )
        VALUES ($1, 2, 'Pg Recurring Rule', '支出', $2, 1299, 'monthly', '2026-04-01', true, 1)
        "#,
    )
    .bind(user_id)
    .bind(category_id.to_string())
    .execute(&pool)
    .await?;
    let overview = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/rules/overview",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(overview.status(), StatusCode::OK);
    let overview_body = read_json(overview).await;
    assert_eq!(overview_body["data"]["learningRuleCount"], 1);
    assert_eq!(
        overview_body["data"]["learningRules"][0]["matchValue"],
        "Coffee Shop"
    );
    assert_eq!(
        overview_body["data"]["recurringRules"][0]["name"],
        "Pg Recurring Rule"
    );

    let account_list = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::GET,
            "/api/account-rules/",
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(account_list.status(), StatusCode::OK);
    let account_list_body = read_json(account_list).await;
    assert_eq!(account_list_body["total"], 1);
    assert_eq!(
        account_list_body["data"][0]["accountName"],
        "pg-rules-account"
    );
    assert_eq!(
        account_list_body["data"][0]["fieldScope"],
        json!(["payment_method"])
    );
    assert!(!serde_json::to_string(&account_list_body)?.contains("pg other account rule"));

    let invalid_account_scope = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/account-rules/",
            json!({
                "account_id": account_id,
                "rule_expression": "OR={招商}",
                "account_role_scope": "wallet"
            }),
            user_id,
        ))
        .await?;
    assert_eq!(invalid_account_scope.status(), StatusCode::BAD_REQUEST);

    let create_account_rule = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/account-rules/",
            json!({
                "account_id": account_id,
                "name": "pg created account rule",
                "priority": 3,
                "rule_expression": "OR={招商}",
                "account_role_scope": "destination",
                "transaction_type_scope": "expense",
                "field_scope": ["parser", "payment_method"]
            }),
            user_id,
        ))
        .await?;
    assert_eq!(create_account_rule.status(), StatusCode::CREATED);
    let create_account_body = read_json(create_account_rule).await;
    let created_account_rule_id = create_account_body["data"]["id"]
        .as_i64()
        .expect("created account rule id");

    let update_account_rule = app
        .clone()
        .oneshot(json_request_for_user(
            Method::PUT,
            &format!("/api/account-rules/{created_account_rule_id}"),
            json!({"priority": 2, "fieldScope": "parser,payment_method"}),
            user_id,
        ))
        .await?;
    assert_eq!(update_account_rule.status(), StatusCode::OK);
    assert_eq!(
        read_json(update_account_rule).await["data"]["fieldScope"],
        json!(["parser", "payment_method"])
    );

    let duplicate_account_reorder = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/account-rules/reorder",
            json!({"rule_ids": [created_account_rule_id, created_account_rule_id]}),
            user_id,
        ))
        .await?;
    assert_eq!(duplicate_account_reorder.status(), StatusCode::BAD_REQUEST);

    let reorder_account_rules = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            "/api/account-rules/reorder",
            json!({"rule_ids": [created_account_rule_id, account_rule_id]}),
            user_id,
        ))
        .await?;
    assert_eq!(reorder_account_rules.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i32>("SELECT priority FROM account_rules WHERE id = $1")
            .bind(created_account_rule_id)
            .fetch_one(&pool)
            .await?,
        1
    );

    let account_match = app
        .clone()
        .oneshot(json_request_for_user(
            Method::POST,
            &format!("/api/account-rules/{created_account_rule_id}/test"),
            json!({
                "accountRoleScope": "destination",
                "transactionTypeScope": "expense",
                "context": {"paymentMethod": "招商银行", "parserId": "bank_csv"}
            }),
            user_id,
        ))
        .await?;
    assert_eq!(account_match.status(), StatusCode::OK);
    assert_eq!(read_json(account_match).await["data"]["matched"], true);

    let delete_category_rule = app
        .clone()
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/category-rules/{created_category_rule_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_category_rule.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM category_rules WHERE id = $1")
            .bind(created_category_rule_id)
            .fetch_one(&pool)
            .await?,
        0
    );

    let delete_account_rule = app
        .oneshot(authed_request_for_user(
            Method::DELETE,
            &format!("/api/account-rules/{created_account_rule_id}"),
            Body::empty(),
            user_id,
        ))
        .await?;
    assert_eq!(delete_account_rule.status(), StatusCode::OK);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM account_rules WHERE id = $1")
            .bind(created_account_rule_id)
            .fetch_one(&pool)
            .await?,
        0
    );

    Ok(())
}

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
    assert!(accounts[0].get("aliases").is_none());
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
    assert!(create_body["result"].get("aliases").is_none());
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
                "hidden": true,
                "displayOrder": 2
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["result"]["name"], "现金账户更新");
    assert_eq!(update_body["result"]["balance"], 3099);
    assert!(update_body["result"].get("aliases").is_none());
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
    assert!(body["result"].get("aliases").is_none());
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
        .any(|route| route == &("GET", "/api/categories/rules")));
    assert!(TAXONOMY_CATEGORY_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("PUT", "/api/categories/rules")));
    assert!(TAXONOMY_CATEGORY_ROUTE_PATTERNS
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

    let legacy_rules_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/rules",
            Body::empty(),
        ))
        .await?;
    assert_eq!(legacy_rules_response.status(), StatusCode::OK);
    let legacy_rules_body = read_json(legacy_rules_response).await;
    assert_eq!(legacy_rules_body["success"], true);
    let legacy_rules = legacy_rules_body["result"]
        .as_array()
        .expect("legacy rules");
    assert_eq!(legacy_rules.len(), 1);
    assert_eq!(legacy_rules[0]["id"], 60);
    assert_eq!(legacy_rules[0]["category_id"], 31);
    assert_eq!(legacy_rules[0]["main"], "餐饮");
    assert_eq!(legacy_rules[0]["sub"], "午餐");
    assert_eq!(legacy_rules[0]["keywords"], "OR={午餐,饭}");
    assert_eq!(legacy_rules[0]["type"], 3);
    assert!(!serde_json::to_string(&legacy_rules_body)?.contains("其他用户规则"));

    let missing_legacy_rules_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/rules",
            json!({}),
        ))
        .await?;
    assert_eq!(
        missing_legacy_rules_response.status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        read_json(missing_legacy_rules_response).await["error"],
        "rules are required"
    );

    let update_legacy_rules_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/rules",
            json!({"rules": [{"keyword": "咖啡", "category": "餐饮"}]}),
        ))
        .await?;
    assert_eq!(update_legacy_rules_response.status(), StatusCode::OK);
    let update_legacy_rules_body = read_json(update_legacy_rules_response).await;
    assert_eq!(update_legacy_rules_body["success"], true);
    assert_eq!(
        update_legacy_rules_body["message"],
        "Category rules updated successfully"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&app_setting_value(
            &fixture.db_path,
            "legacy_category_rules_config:user:42"
        )?)?,
        json!([{"keyword": "咖啡", "category": "餐饮"}])
    );

    let cached_legacy_rules_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/rules",
            Body::empty(),
        ))
        .await?;
    assert_eq!(cached_legacy_rules_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(cached_legacy_rules_response).await["result"],
        json!([{"keyword": "咖啡", "category": "餐饮"}])
    );

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
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/defaults")));
    assert!(TAXONOMY_CATEGORY_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/category-rules/migrate")));

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

    let defaults_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/defaults",
            Body::empty(),
        ))
        .await?;
    assert_eq!(defaults_response.status(), StatusCode::OK);
    let defaults_body = read_json(defaults_response).await;
    assert_eq!(defaults_body["success"], true);
    assert!(
        defaults_body["data"]["categories"]["created"]
            .as_i64()
            .expect("created categories")
            > 80
    );
    assert!(
        defaults_body["data"]["rules"]["created"]
            .as_i64()
            .expect("created rules")
            > 30
    );
    assert_eq!(defaults_body["data"]["rules"]["missingCategories"], 0);
    let seeded_delivery_id = category_id_by_name(&fixture.db_path, 42, "餐饮", "外卖")?;
    assert_eq!(
        category_rule_expression_by_name(
            &fixture.db_path,
            42,
            seeded_delivery_id,
            "default:餐饮/外卖"
        )?,
        "(OR={美团外卖,饿了么,外卖,饭团}/REGEX={(美团|饿了么).*(外卖|订单)})+NOT={退款,退货,取消,冲正}"
    );

    let defaults_repeat_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/defaults",
            Body::empty(),
        ))
        .await?;
    assert_eq!(defaults_repeat_response.status(), StatusCode::OK);
    let defaults_repeat_body = read_json(defaults_repeat_response).await;
    assert_eq!(defaults_repeat_body["data"]["categories"]["created"], 0);
    assert_eq!(defaults_repeat_body["data"]["rules"]["created"], 0);
    assert_eq!(
        defaults_repeat_body["data"]["rules"]["missingCategories"],
        0
    );

    insert_legacy_category_rule_migration_rows(&fixture.db_path)?;
    let migrate_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/migrate",
            Body::empty(),
        ))
        .await?;
    assert_eq!(migrate_response.status(), StatusCode::OK);
    let migrate_body = read_json(migrate_response).await;
    assert_eq!(migrate_body["success"], true);
    assert_eq!(migrate_body["data"], json!({"migrated": 5, "skipped": 0}));
    assert_eq!(
        category_rule_expression_by_name(&fixture.db_path, 42, 500, "migrated:迁移测试/咖啡")?,
        "OR={星巴克,咖啡}+AND={早餐}+NOT={退款}"
    );
    assert_eq!(
        category_rule_expression_by_name(
            &fixture.db_path,
            42,
            501,
            "migrated:特殊字符迁移测试/完整字面量"
        )?,
        r"OR={商户A\,咖啡\+拿铁\{热\}\|杯}"
    );
    assert_eq!(
        category_rule_expression_by_name(
            &fixture.db_path,
            42,
            504,
            "migrated:investment-recognition"
        )?,
        "OR={蚂蚁财富,天天基金}+OR={基金,ETF}+NOT={还款,账单}"
    );
    assert!(!category_rule_name_exists(
        &fixture.db_path,
        77,
        "migrated:跨用户迁移测试/隔离"
    )?);

    let migrate_repeat_response = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/migrate",
            Body::empty(),
        ))
        .await?;
    assert_eq!(migrate_repeat_response.status(), StatusCode::OK);
    assert_eq!(
        read_json(migrate_repeat_response).await["data"],
        json!({"migrated": 0, "skipped": 5})
    );

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
        (Method::POST, "/api/category-rules/defaults"),
        (Method::POST, "/api/category-rules/migrate"),
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
        .clone()
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
    let no_db_defaults_response = no_db_app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/defaults",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_db_defaults_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let no_db_migrate_response = no_db_app
        .oneshot(authed_request(
            Method::POST,
            "/api/category-rules/migrate",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_db_migrate_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_account_rules_runtime_manages_rules_without_import_cutover(
) -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_ACCOUNT_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/account-rules/")));
    assert!(!TAXONOMY_ACCOUNT_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/account-rules/migrate-aliases")));
    assert!(TAXONOMY_ACCOUNT_RULE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/account-rules/{rule_id}/test")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let list_response = app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/account-rules/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(list_response.status(), StatusCode::OK);
    let list_body = read_json(list_response).await;
    assert_eq!(list_body["success"], true);
    assert_eq!(list_body["total"], 1);
    assert_eq!(list_body["data"][0]["id"], 80);
    assert_eq!(list_body["data"][0]["accountId"], 11);
    assert_eq!(list_body["data"][0]["accountRoleScope"], "source");
    assert_eq!(list_body["data"][0]["transactionTypeScope"], "expense");
    assert_eq!(
        list_body["data"][0]["fieldScope"],
        json!(["payment_method"])
    );
    assert!(!serde_json::to_string(&list_body)?.contains("其他用户账户规则"));

    let missing_fields = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/account-rules/",
            json!({"account_id": 11}),
        ))
        .await?;
    assert_eq!(missing_fields.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(missing_fields).await["error"],
        "account_id and rule_expression are required"
    );

    let invalid_scope = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/account-rules/",
            json!({
                "account_id": 11,
                "rule_expression": "OR={招商}",
                "account_role_scope": "wallet"
            }),
        ))
        .await?;
    assert_eq!(invalid_scope.status(), StatusCode::BAD_REQUEST);
    assert!(read_json(invalid_scope).await["error"]
        .as_str()
        .unwrap_or_default()
        .contains("unsupported account_role_scope"));

    let cross_account = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/account-rules/",
            json!({"account_id": 99, "rule_expression": "OR={其他}"}),
        ))
        .await?;
    assert_eq!(cross_account.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(cross_account).await["error"],
        "Failed to create account rule"
    );

    let create_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/account-rules/",
            json!({
                "account_id": 11,
                "name": "招商账户规则",
                "priority": 4,
                "rule_expression": "OR={招商}",
                "regex_enabled": false,
                "enabled": true,
                "account_role_scope": "destination",
                "transaction_type_scope": "transfer",
                "field_scope": ["counterparty", "description"]
            }),
        ))
        .await?;
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body = read_json(create_response).await;
    assert_eq!(create_body["data"]["name"], "招商账户规则");
    assert_eq!(create_body["data"]["accountName"], "工资子账户");
    let created_rule_id = create_body["data"]["id"].as_i64().expect("rule id");

    let update_response = app
        .clone()
        .oneshot(json_request(
            Method::PUT,
            &format!("/api/account-rules/{created_rule_id}"),
            json!({
                "priority": 2,
                "fieldScope": "parser,payment_method",
                "transactionTypeScope": "expense"
            }),
        ))
        .await?;
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_body = read_json(update_response).await;
    assert_eq!(update_body["data"]["priority"], 2);
    assert_eq!(
        update_body["data"]["fieldScope"],
        json!(["parser", "payment_method"])
    );

    let duplicate_reorder = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/account-rules/reorder",
            json!({"rule_ids": [created_rule_id, created_rule_id]}),
        ))
        .await?;
    assert_eq!(duplicate_reorder.status(), StatusCode::BAD_REQUEST);

    let reorder_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/account-rules/reorder",
            json!({"rule_ids": [created_rule_id, 80]}),
        ))
        .await?;
    assert_eq!(reorder_response.status(), StatusCode::OK);
    assert_eq!(account_rule_priority(&fixture.db_path, created_rule_id)?, 1);
    assert_eq!(account_rule_priority(&fixture.db_path, 80)?, 2);

    let match_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/api/account-rules/{created_rule_id}/test"),
            json!({
                "accountRoleScope": "destination",
                "transactionTypeScope": "expense",
                "context": {
                    "paymentMethod": "招商银行",
                    "parserId": "bank_csv"
                }
            }),
        ))
        .await?;
    assert_eq!(match_response.status(), StatusCode::OK);
    let match_body = read_json(match_response).await;
    assert_eq!(match_body["data"]["matched"], true);
    assert_eq!(
        match_body["data"]["matchedFields"],
        json!(["payment_method"])
    );
    assert_eq!(match_body["data"]["fallbackUsed"], false);

    let miss_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            &format!("/api/account-rules/{created_rule_id}/test"),
            json!({
                "accountRoleScope": "destination",
                "transactionTypeScope": "expense",
                "context": {"paymentMethod": "微信零钱"}
            }),
        ))
        .await?;
    assert_eq!(miss_response.status(), StatusCode::OK);
    assert_eq!(read_json(miss_response).await["data"]["matched"], false);

    let delete_response = app
        .clone()
        .oneshot(authed_request(
            Method::DELETE,
            &format!("/api/account-rules/{created_rule_id}"),
            Body::empty(),
        ))
        .await?;
    assert_eq!(delete_response.status(), StatusCode::OK);
    assert!(!account_rule_exists(&fixture.db_path, created_rule_id)?);

    let unauthenticated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/account-rules/")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    let no_db_app = runtime_router_without_db(&fixture);
    let no_db_response = no_db_app
        .oneshot(authed_request(
            Method::GET,
            "/api/account-rules/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["error"],
        "Rust taxonomy account rules DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    Connection::open(&fixture.db_path)?.execute("DROP TABLE account_rules", [])?;
    let missing_schema_response = app
        .oneshot(authed_request(
            Method::GET,
            "/api/account-rules/",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        missing_schema_response.status(),
        StatusCode::INTERNAL_SERVER_ERROR
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_rules_overview_runtime_aggregates_user_scoped_rule_sources(
) -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_RULE_CENTER_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/rules/overview")));

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
        .any(|route| route == &("GET", "/api/settings/encryption/status")));
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/settings/bundle/export")));
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| { route == &("POST", "/api/settings/bundle/sections/{section_key}/export") }));
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/settings/bundle/import")));
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/settings/bundle/import/preview")));
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("POST", "/api/settings/bundle/sections/{section_key}/import")));

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
    assert_eq!(bundle["secretsPolicy"]["providerCredentials"], "redacted");
    assert_eq!(bundle["counts"]["accounts"], 2);
    assert_eq!(bundle["counts"]["transactionTags"], 2);
    assert_eq!(bundle["counts"]["categoryRecognitionRules"], 2);
    assert_eq!(bundle["counts"]["accountRecognitionRules"], 1);
    assert_eq!(bundle["counts"]["llmConfigs"], 1);
    assert_eq!(bundle["sections"]["accounts"][0]["name"], "工资卡");
    assert!(bundle["sections"]["accounts"][0].get("aliases").is_none());
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
    assert_eq!(
        bundle["sections"]["accountRecognitionRules"][0]["accountRef"],
        "account:11"
    );
    assert_eq!(
        bundle["sections"]["accountRecognitionRules"][0]["ruleExpression"],
        "OR={子卡}"
    );
    assert_eq!(bundle["sections"]["llmConfigs"][0]["apiKey"], "");
    assert_eq!(bundle["sections"]["llmConfigs"][0]["hasApiKey"], true);
    assert_eq!(
        bundle["sections"]["llmConfigs"][0]["credentialConfig"]["access_token"],
        "********"
    );
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
    assert_eq!(sensitive_section["secretsPolicy"]["llmApiKeys"], "included");
    assert_eq!(
        sensitive_section["secretsPolicy"]["providerCredentials"],
        "included"
    );
    assert_eq!(sensitive_section["counts"]["llmConfigs"], 1);
    assert_eq!(
        sensitive_section["sections"]["llmConfigs"][0]["hasApiKey"],
        true
    );
    assert_eq!(
        sensitive_section["sections"]["llmConfigs"][0]["apiKey"],
        "secret-key"
    );
    assert_eq!(
        sensitive_section["sections"]["llmConfigs"][0]["credentialConfig"]["access_token"],
        "secret-key"
    );

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
async fn taxonomy_settings_encryption_status_runtime_reports_rust_sqlcipher_projection(
) -> Result<(), Box<dyn Error>> {
    assert!(TAXONOMY_SETTINGS_BUNDLE_ROUTE_PATTERNS
        .iter()
        .any(|route| route == &("GET", "/api/settings/encryption/status")));

    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/settings/encryption/status")
                .body(Body::empty())
                .expect("request builds"),
        )
        .await?;

    assert_eq!(response.status(), StatusCode::OK);
    let body = read_json(response).await;
    assert_eq!(body["success"], true);
    assert_eq!(body["data"]["encrypted"], false);
    assert_eq!(body["data"]["sqlcipher_available"], false);
    assert_eq!(body["data"]["kdf_iter"], 256_000);
    assert_eq!(body["data"]["cipher_page_size"], 4096);

    Ok(())
}

#[tokio::test]
async fn taxonomy_settings_bundle_import_runtime_previews_and_upserts_sections(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);
    let bundle = json!({
        "schemaVersion": 1,
        "sections": {
            "accounts": [{
                "externalRef": "account:source",
                "name": "导入账户",
                "type": 1,
                "currency": "CNY",
                "balance": 10.25,
                "initialBalance": 10.25
            }],
            "transactionCategories": [{
                "externalRef": "category:coffee",
                "type": 3,
                "mainCategory": "导入分类",
                "subCategory": "咖啡",
                "priority": 3
            }],
            "transactionTags": [{
                "externalRef": "tag:work",
                "name": "导入标签",
                "color": "#224466",
                "icon": "tag"
            }],
            "transactionTemplates": [{
                "name": "导入模板",
                "templateType": 1,
                "type": 3,
                "categoryRef": "category:coffee",
                "sourceAccountRef": "account:source",
                "sourceAmount": 66,
                "tagRefs": ["tag:work"]
            }],
            "scheduledTransactions": [{
                "name": "导入定时模板",
                "templateType": 2,
                "type": 3,
                "categoryRef": "category:coffee",
                "sourceAccountRef": "account:source",
                "sourceAmount": 88,
                "scheduledFrequencyType": 2,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-07-01",
                "tagRefs": ["tag:work"]
            }],
            "categoryRecognitionRules": [{
                "categoryRef": "category:coffee",
                "name": "导入规则",
                "priority": 2,
                "ruleExpression": "OR={settings-bundle-import}",
                "regexEnabled": false,
                "enabled": true
            }],
            "accountRecognitionRules": [{
                "accountRef": "account:source",
                "name": "导入账户规则",
                "priority": 3,
                "ruleExpression": "OR={import-alias}",
                "regexEnabled": false,
                "enabled": true,
                "accountRoleScope": "source",
                "transactionTypeScope": "expense",
                "fieldScope": ["payment_method"],
                "source": "settings_bundle"
            }, {
                "accountRef": "account:missing",
                "name": "缺账户规则",
                "ruleExpression": "OR={missing-account}"
            }, {
                "accountRef": "account:source",
                "name": "错误角色规则",
                "ruleExpression": "OR={bad-role}",
                "accountRoleScope": "wallet"
            }, {
                "accountRef": "account:source",
                "name": "错误字段规则",
                "ruleExpression": "OR={bad-field}",
                "fieldScope": ["unknown-field"]
            }],
            "llmConfigs": [{
                "name": "导入 LLM",
                "provider": "openai",
                "model": "gpt-imported",
                "apiKey": "sk-imported-secret",
                "baseUrl": "https://example.test/v1",
                "credentialConfig": {
                    "credential_mode": "session_json",
                    "credential_json": {
                        "accessToken": "session-access",
                        "refresh_token": "session-refresh"
                    },
                    "token_endpoint": "https://example.test/oauth/token",
                    "refresh_body": {"client_id": "desktop"}
                },
                "advancedSettings": {"reasoning_depth": "medium"},
                "isActive": true
            }],
            "ocrConfig": [{
                "externalRef": "ocrConfig:receipt-recognition",
                "provider": "cloud_stub",
                "lang": "eng",
                "model": "vision-model",
                "baseUrl": "https://ocr.example.test/v1",
                "parameters": {"temperature": 0},
                "credentialConfig": {
                    "credential_mode": "access_token",
                    "credential_json": {"access_token": "ocr-access"}
                }
            }]
        }
    });

    let preview_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/import/preview",
            bundle.clone(),
        ))
        .await?;
    assert_eq!(preview_response.status(), StatusCode::OK);
    let preview = read_json(preview_response).await;
    assert_eq!(preview["success"], true);
    assert_eq!(preview["result"]["dryRun"], true);
    assert_eq!(preview["result"]["sections"]["accounts"]["created"], 1);
    assert_eq!(
        row_count_by_name(&fixture.db_path, "accounts", "导入账户")?,
        0
    );

    let import_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/import",
            bundle.clone(),
        ))
        .await?;
    assert_eq!(import_response.status(), StatusCode::OK);
    let imported = read_json(import_response).await;
    assert_eq!(imported["result"]["dryRun"], false);
    assert_eq!(
        imported["result"]["sections"]["categoryRecognitionRules"]["created"],
        1
    );
    assert_eq!(
        imported["result"]["sections"]["accountRecognitionRules"]["created"],
        1
    );
    assert_eq!(
        imported["result"]["sections"]["accountRecognitionRules"]["skipped"],
        3
    );
    assert_eq!(imported["result"]["sections"]["ocrConfig"]["updated"], 1);
    assert_eq!(
        account_balance_by_name(&fixture.db_path, "导入账户")?,
        10.25
    );
    assert_eq!(
        bill_template_amount_by_name(&fixture.db_path, "导入模板")?,
        66.0
    );
    assert_eq!(
        row_count_by_name(&fixture.db_path, "account_rules", "导入账户规则")?,
        1
    );
    assert_eq!(
        recurring_template_start_by_name(&fixture.db_path, "导入定时模板")?,
        "2026-07-01"
    );
    assert_eq!(
        llm_config_by_name(&fixture.db_path, "导入 LLM")?,
        (
            "gpt-imported".to_string(),
            "sk-imported-secret".to_string(),
            0
        )
    );
    assert!(
        llm_credential_config_by_name(&fixture.db_path, "导入 LLM")?.contains("session-refresh")
    );
    assert_eq!(
        app_setting_value(&fixture.db_path, "receipt_ocr_config")?,
        "{\"base_url\":\"https://ocr.example.test/v1\",\"credential_config\":{\"access_token\":\"ocr-access\",\"credential_json\":{\"access_token\":\"ocr-access\"},\"credential_mode\":\"access_token\"},\"lang\":\"eng\",\"model\":\"vision-model\",\"parameters\":{\"temperature\":0},\"provider\":\"cloud_stub\"}"
    );

    let second_import = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/import",
            bundle,
        ))
        .await?;
    assert_eq!(second_import.status(), StatusCode::OK);
    let second = read_json(second_import).await;
    assert_eq!(second["result"]["sections"]["accounts"]["updated"], 1);
    assert_eq!(second["result"]["sections"]["accounts"]["created"], 0);
    assert_eq!(
        second["result"]["sections"]["accountRecognitionRules"]["updated"],
        1
    );
    assert_eq!(
        second["result"]["sections"]["accountRecognitionRules"]["skipped"],
        3
    );

    let section_bundle = json!({
        "schemaVersion": 1,
        "sections": {
            "accounts": [{"name": "section-ignored-account", "type": 1}],
            "transactionTags": [{"externalRef": "tag:section", "name": "section-only-tag"}]
        }
    });
    let section_preview = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/sections/transactionTags/import/preview",
            section_bundle.clone(),
        ))
        .await?;
    assert_eq!(section_preview.status(), StatusCode::OK);
    let section_preview_body = read_json(section_preview).await;
    assert_eq!(
        section_preview_body["result"]["sections"]["transactionTags"]["created"],
        1
    );
    assert_eq!(
        section_preview_body["result"]["sections"]["accounts"]["created"],
        0
    );

    let section_import = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/sections/transactionTags/import",
            section_bundle,
        ))
        .await?;
    assert_eq!(section_import.status(), StatusCode::OK);
    assert_eq!(
        row_count_by_name(&fixture.db_path, "accounts", "section-ignored-account")?,
        0
    );
    assert_eq!(
        row_count_by_name(&fixture.db_path, "tags", "section-only-tag")?,
        1
    );

    for uri in [
        "/api/settings/bundle/sections/notASection/import",
        "/api/settings/bundle/sections/notASection/import/preview",
    ] {
        let response = app
            .clone()
            .oneshot(json_request(Method::POST, uri, json!({"schemaVersion": 1})))
            .await?;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
    }

    for uri in [
        "/api/settings/bundle/import",
        "/api/settings/bundle/import/preview",
    ] {
        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                uri,
                json!({"schemaVersion": "1", "sections": {"accounts": []}}),
            ))
            .await?;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{uri}");
        assert!(
            read_json(response).await["error"]
                .as_str()
                .expect("error")
                .contains("Unsupported settings bundle schemaVersion"),
            "{uri}"
        );
    }

    let invalid_json = app
        .clone()
        .oneshot(authed_request(
            Method::POST,
            "/api/settings/bundle/import",
            Body::from("{"),
        ))
        .await?;
    assert_eq!(invalid_json.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        read_json(invalid_json).await["error"],
        "Invalid JSON bundle"
    );

    let no_db_app = runtime_router_without_db(&fixture);
    let no_db_response = no_db_app
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/import",
            json!({"schemaVersion": 1, "sections": {"accounts": []}}),
        ))
        .await?;
    assert_eq!(no_db_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        read_json(no_db_response).await["error"],
        "Rust taxonomy settings bundle import DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    Ok(())
}

#[tokio::test]
async fn taxonomy_settings_bundle_import_runtime_preserves_llm_and_ref_collision_semantics(
) -> Result<(), Box<dyn Error>> {
    let fixture = RuntimeFixture::new()?;
    let app = runtime_router(&fixture);

    let llm_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/import",
            json!({
                "schemaVersion": 1,
                "sections": {
                    "llmConfigs": [{
                        "name": "主 LLM",
                        "provider": "openai",
                        "model": "gpt-updated",
                        "apiKey": "",
                        "baseUrl": "https://updated.test/v1",
                        "advancedSettings": {"reasoning_depth": "high"}
                    }, {
                        "name": "duplicate LLM",
                        "provider": "openai",
                        "model": "gpt-first",
                        "apiKey": "sk-first",
                        "baseUrl": "https://first.test/v1"
                    }, {
                        "name": "duplicate LLM",
                        "provider": "openai",
                        "model": "gpt-second",
                        "apiKey": "",
                        "baseUrl": "https://second.test/v1"
                    }]
                }
            }),
        ))
        .await?;
    assert_eq!(llm_response.status(), StatusCode::OK);
    let llm_body = read_json(llm_response).await;
    assert_eq!(llm_body["result"]["sections"]["llmConfigs"]["updated"], 2);
    assert_eq!(llm_body["result"]["sections"]["llmConfigs"]["created"], 1);
    assert_eq!(
        llm_config_by_name(&fixture.db_path, "主 LLM")?,
        ("gpt-updated".to_string(), "secret-key".to_string(), 1)
    );
    assert_eq!(
        llm_config_by_name(&fixture.db_path, "duplicate LLM")?,
        ("gpt-second".to_string(), "sk-first".to_string(), 0)
    );

    let collision_response = app
        .clone()
        .oneshot(json_request(
            Method::POST,
            "/api/settings/bundle/import",
            json!({
                "schemaVersion": 1,
                "sections": {
                    "transactionTemplates": [{
                        "name": "external-ref-template",
                        "templateType": 1,
                        "type": 3,
                        "categoryRef": "category:31",
                        "sourceAccountRef": "account:10",
                        "sourceAmount": 7.5,
                        "tagRefs": ["tag:20"]
                    }, {
                        "name": "legacy-id-template",
                        "templateType": 1,
                        "type": 3,
                        "categoryId": 31,
                        "sourceAccountId": 10,
                        "sourceAmount": 9.5,
                        "tagIds": [20]
                    }],
                    "categoryRecognitionRules": [{
                        "categoryRef": "category:31",
                        "name": "external-ref-rule",
                        "priority": 1,
                        "ruleExpression": "OR={external-ref}",
                        "enabled": true
                    }, {
                        "categoryId": 31,
                        "name": "legacy-id-rule",
                        "priority": 1,
                        "ruleExpression": "OR={legacy-id}",
                        "enabled": true
                    }]
                }
            }),
        ))
        .await?;
    assert_eq!(collision_response.status(), StatusCode::OK);
    let collision = read_json(collision_response).await;
    assert_eq!(
        collision["result"]["sections"]["transactionTemplates"]["created"],
        1
    );
    assert_eq!(
        collision["result"]["sections"]["transactionTemplates"]["skipped"],
        1
    );
    assert_eq!(
        collision["result"]["sections"]["categoryRecognitionRules"]["created"],
        1
    );
    assert_eq!(
        collision["result"]["sections"]["categoryRecognitionRules"]["skipped"],
        1
    );
    assert_eq!(
        row_count_by_name(&fixture.db_path, "bill_templates", "external-ref-template")?,
        0
    );
    assert_eq!(
        row_count_by_name(&fixture.db_path, "bill_templates", "legacy-id-template")?,
        1
    );
    assert_eq!(
        row_count_by_name(&fixture.db_path, "category_rules", "external-ref-rule")?,
        0
    );
    assert_eq!(
        row_count_by_name(&fixture.db_path, "category_rules", "legacy-id-rule")?,
        1
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
        (Method::GET, "/api/categories/rules"),
        (Method::PUT, "/api/categories/rules"),
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
        (Method::PUT, "/api/categories/rules"),
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
        .clone()
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

    let no_db_legacy_rules_response = no_db_app
        .clone()
        .oneshot(authed_request(
            Method::GET,
            "/api/categories/rules",
            Body::empty(),
        ))
        .await?;
    assert_eq!(
        no_db_legacy_rules_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read_json(no_db_legacy_rules_response).await["error"],
        "Rust taxonomy legacy category rules DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
    );

    let no_db_update_legacy_rules_response = no_db_app
        .oneshot(json_request(
            Method::PUT,
            "/api/categories/rules",
            json!({"rules": []}),
        ))
        .await?;
    assert_eq!(
        no_db_update_legacy_rules_response.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        read_json(no_db_update_legacy_rules_response).await["error"],
        "Rust taxonomy legacy category rules DB runtime requires BILL_ANALYSER_SQLITE_DB_PATH"
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
    .with_database_backend(DatabaseBackend::Sqlite)
    .with_require_postgres_after_cutover(false)
    .with_legacy_sqlite_runtime_for_tests()
    .with_sqlite_db_path(fixture.db_path.display().to_string())
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = HttpAppState::new(config).expect("http app state");
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
    .with_database_backend(DatabaseBackend::Sqlite)
    .with_require_postgres_after_cutover(false)
    .with_legacy_sqlite_runtime_for_tests()
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = HttpAppState::new(config).expect("http app state");
    build_router(state)
}

fn postgres_runtime_router(postgres_url: &str) -> Result<Router, Box<dyn Error>> {
    let config = HttpShellConfig::new_with_import_route_mode(
        "http://127.0.0.1:9".to_string(),
        Duration::from_secs(5),
        1024 * 1024,
        ImportRouteMode::ImportDbRuntime,
    )?
    .with_database_backend(DatabaseBackend::Postgres)
    .with_require_postgres_after_cutover(true)
    .with_postgres_url(postgres_url)?
    .with_trusted_user_header_secret(TEST_AUTH_SECRET);
    let state = HttpAppState::new(config).expect("http app state");
    Ok(build_router(state))
}

fn init_schema(path: &Path) -> Result<(), Box<dyn Error>> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "
        CREATE TABLE users(
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL,
            password_hash TEXT DEFAULT '',
            investment_platform_keywords TEXT,
            investment_product_keywords TEXT,
            investment_exclude_keywords TEXT
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
        CREATE TABLE account_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            account_id INTEGER NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 100,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            applied_count INTEGER DEFAULT 0,
            last_applied_at TEXT,
            match_count INTEGER DEFAULT 0,
            last_matched_at TEXT,
            account_role_scope TEXT NOT NULL DEFAULT 'any',
            transaction_type_scope TEXT NOT NULL DEFAULT 'all',
            field_scope TEXT NOT NULL DEFAULT '[\"counterparty\",\"payment_method\",\"description\"]',
            source TEXT NOT NULL DEFAULT 'manual',
            source_key TEXT,
            created_at TEXT,
            updated_at TEXT,
            FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
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
            is_active INTEGER DEFAULT 0,
            created_at TEXT DEFAULT 'now',
            updated_at TEXT DEFAULT 'now',
            UNIQUE(user_id, name)
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
        "INSERT INTO users(
            id, username, password_hash,
            investment_platform_keywords, investment_product_keywords, investment_exclude_keywords
        )
        VALUES
            (42, 'owner', ?1, '[\"蚂蚁财富\", \"天天基金\", \"蚂蚁财富\"]', '[\"基金\", \"ETF\"]', '[\"还款\", \"账单\"]'),
            (77, 'other', '', NULL, NULL, NULL)",
        [&password_hash],
    )?;
    connection.execute(
        "INSERT INTO accounts(
            id, user_id, name, type, category, currency, icon, color, balance,
            initial_balance, hidden, display_order, comment, parent_id,
            created_at, updated_at
        )
        VALUES
            (10, 42, '工资卡', 1, 2, 'CNY', 'card', '#336699', 12.34, 12.34, 1, 1, '主账户', 0, 'now', 'now'),
            (11, 42, '工资子账户', 1, 2, 'CNY', 'wallet', '#336699', 0.50, 0.50, 0, 2, '', 10, 'now', 'now'),
            (99, 77, '其他用户', 1, 2, 'CNY', 'wallet', '#999999', 99.0, 99.0, 0, 0, '', 0, 'now', 'now')",
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
        "INSERT INTO account_rules(
            id, user_id, account_id, name, priority, rule_expression,
            regex_enabled, enabled, applied_count, last_applied_at,
            match_count, last_matched_at,
            account_role_scope, transaction_type_scope, field_scope, source, source_key,
            created_at, updated_at
        )
        VALUES
            (80, 42, 11, '工资子账户规则', 10, 'OR={子卡}', 0, 1, 2, '2026-01-02T00:00:00',
             2, '2026-01-02T00:00:00', 'source', 'expense', '[\"payment_method\"]',
             'manual', NULL, 'now', 'now'),
            (90, 77, 99, '其他用户账户规则', 1, 'OR={其他}', 0, 1, 1, NULL,
             1, NULL, 'any', 'all', '[\"counterparty\"]', 'manual', NULL, 'now', 'now')",
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

fn json_request_for_user(method: Method, uri: &str, body: Value, user_id: i64) -> Request<Body> {
    authed_request_for_user(method, uri, Body::from(body.to_string()), user_id)
}

fn authed_request(method: Method, uri: &str, body: Body) -> Request<Body> {
    authed_request_for_user(
        method,
        uri,
        body,
        TEST_USER_ID.parse().expect("test user id"),
    )
}

fn authed_request_for_user(method: Method, uri: &str, body: Body, user_id: i64) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .header("x-user-id", user_id.to_string())
        .header("x-bill-analyser-trusted-user-secret", TEST_AUTH_SECRET)
        .body(body)
        .expect("request builds")
}

async fn get_result(response: axum::response::Response) -> Value {
    let status = response.status();
    let body = read_json(response).await;
    assert_eq!(status, StatusCode::OK, "response body: {body}");
    assert_eq!(body["success"], true);
    body["result"].clone()
}

async fn read_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body bytes");
    serde_json::from_slice(&bytes).expect("json body")
}

fn row_count_by_name(path: &Path, table_name: &str, name: &str) -> Result<i64, Box<dyn Error>> {
    assert!(
        matches!(
            table_name,
            "accounts" | "tags" | "bill_templates" | "category_rules" | "account_rules"
        ),
        "unexpected test table name"
    );
    Ok(Connection::open(path)?.query_row(
        &format!("SELECT COUNT(*) FROM {table_name} WHERE user_id = 42 AND name = ?1"),
        [name],
        |row| row.get::<_, i64>(0),
    )?)
}

fn account_balance_by_name(path: &Path, name: &str) -> Result<f64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT balance FROM accounts WHERE user_id = 42 AND name = ?1",
        [name],
        |row| row.get::<_, f64>(0),
    )?)
}

fn bill_template_amount_by_name(path: &Path, name: &str) -> Result<f64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT amount FROM bill_templates WHERE user_id = 42 AND name = ?1",
        [name],
        |row| row.get::<_, f64>(0),
    )?)
}

fn recurring_template_start_by_name(path: &Path, name: &str) -> Result<String, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT start_date FROM recurring_bills WHERE user_id = 42 AND name = ?1",
        [name],
        |row| row.get::<_, String>(0),
    )?)
}

fn llm_config_by_name(path: &Path, name: &str) -> Result<(String, String, i64), Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT model, api_key, is_active FROM llm_configs WHERE user_id = 42 AND name = ?1",
        [name],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?)
}

fn llm_credential_config_by_name(path: &Path, name: &str) -> Result<String, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT credential_config FROM llm_configs WHERE user_id = 42 AND name = ?1",
        [name],
        |row| row.get::<_, String>(0),
    )?)
}

fn app_setting_value(path: &Path, key: &str) -> Result<String, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        [key],
        |row| row.get::<_, String>(0),
    )?)
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

fn category_id_by_name(
    path: &Path,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT id FROM categories
         WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3",
        rusqlite::params![user_id, main_category, sub_category],
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

fn account_rule_exists(path: &Path, rule_id: i64) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM account_rules WHERE id = ?1",
        [rule_id],
        |row| row.get::<_, bool>(0),
    )?)
}

fn account_rule_priority(path: &Path, rule_id: i64) -> Result<i64, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT priority FROM account_rules WHERE id = ?1",
        [rule_id],
        |row| row.get::<_, i64>(0),
    )?)
}

fn category_rule_expression_by_name(
    path: &Path,
    user_id: i64,
    category_id: i64,
    name: &str,
) -> Result<String, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT rule_expression FROM category_rules
         WHERE user_id = ?1 AND category_id = ?2 AND name = ?3",
        rusqlite::params![user_id, category_id, name],
        |row| row.get::<_, String>(0),
    )?)
}

fn category_rule_name_exists(
    path: &Path,
    user_id: i64,
    name: &str,
) -> Result<bool, Box<dyn Error>> {
    Ok(Connection::open(path)?.query_row(
        "SELECT COUNT(*) > 0 FROM category_rules WHERE user_id = ?1 AND name = ?2",
        rusqlite::params![user_id, name],
        |row| row.get::<_, bool>(0),
    )?)
}

fn insert_legacy_category_rule_migration_rows(path: &Path) -> Result<(), Box<dyn Error>> {
    Connection::open(path)?.execute_batch(
        "
        INSERT INTO categories(
            id, user_id, type, main_category, sub_category, description,
            priority, keywords, hidden, icon, color, created_at
        )
        VALUES
            (500, 42, 3, '迁移测试', '咖啡', '', 11, 'OR:星巴克|咖啡&AND:早餐&NOT:退款', 0, '', '', 'now'),
            (501, 42, 3, '特殊字符迁移测试', '完整字面量', '', 10, '商户A,咖啡+拿铁{热}|杯', 0, '', '', 'now'),
            (502, 42, 3, '已有迁移', '已有规则', '', 12, 'OR:不应重复迁移', 0, '', '', 'now'),
            (503, 77, 3, '跨用户迁移测试', '隔离', '', 13, 'OR:跨用户关键词', 0, '', '', 'now'),
            (504, 42, 5, '投资理财', '基金', '', 8, '', 0, '', '', 'now');
        INSERT INTO category_rules(
            id, user_id, category_id, name, priority, rule_expression,
            regex_enabled, enabled, applied_count, last_applied_at, created_at, updated_at
        )
        VALUES
            (700, 42, 502, 'manual existing rule', 3, 'OR={手工规则}', 0, 1, 0, NULL, 'now', 'now');
        ",
    )?;
    Ok(())
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
