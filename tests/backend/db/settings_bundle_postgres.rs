use std::{env, error::Error, str::FromStr};

use bill_analyser_db::{
    run_postgres_migrations, taxonomy::settings_bundle::import_postgres_settings_bundle,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor, Row};

#[tokio::test]
async fn settings_bundle_import_upserts_templates_when_postgres_available(
) -> Result<(), Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        return Ok(());
    };
    let unique = chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default();
    let test_db = format!("settings_bundle_test_{unique}");
    let base_options = PgConnectOptions::from_str(&postgres_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.clone().database("postgres"))
        .await?;
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{}""#, test_db).as_str())
        .await?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.database(&test_db))
        .await?;
    run_postgres_migrations(&pool).await?;

    let user_id: i64 =
        sqlx::query("INSERT INTO users (username, email) VALUES ($1, $2) RETURNING id")
            .bind(format!("settings-bundle-{unique}"))
            .bind(format!("settings-bundle-{unique}@example.test"))
            .fetch_one(&pool)
            .await?
            .try_get("id")?;

    let bundle = settings_bundle_payload(unique, 1234, 250000, false);
    let dry_run = import_postgres_settings_bundle(&pool, &bundle, user_id, true).await?;
    assert_eq!(dry_run["dryRun"], true);
    assert_eq!(dry_run["sections"]["transactionTemplates"]["created"], 1);
    assert_eq!(dry_run["sections"]["scheduledTransactions"]["created"], 1);
    assert_no_unsupported_template_warning(&dry_run);

    let template_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM transaction_templates WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await?;
    assert_eq!(template_count, 0);

    let imported = import_postgres_settings_bundle(&pool, &bundle, user_id, false).await?;
    assert_eq!(imported["sections"]["transactionTemplates"]["created"], 1);
    assert_eq!(imported["sections"]["scheduledTransactions"]["created"], 1);
    assert_no_unsupported_template_warning(&imported);

    assert_template_row(&pool, user_id, 1, "午餐模板", 1234, 0, false).await?;
    assert_template_row(&pool, user_id, 2, "房租计划", 250000, 250000, false).await?;

    let updated_bundle = settings_bundle_payload(unique, 1999, 300000, true);
    let updated = import_postgres_settings_bundle(&pool, &updated_bundle, user_id, false).await?;
    assert_eq!(updated["sections"]["transactionTemplates"]["updated"], 1);
    assert_eq!(updated["sections"]["scheduledTransactions"]["updated"], 1);
    assert_no_unsupported_template_warning(&updated);

    assert_template_row(&pool, user_id, 1, "午餐模板", 1999, 0, true).await?;
    assert_template_row(&pool, user_id, 2, "房租计划", 300000, 300000, true).await?;

    pool.close().await;
    admin_pool
        .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, test_db).as_str())
        .await?;

    Ok(())
}

fn settings_bundle_payload(
    unique: i64,
    template_amount: i64,
    scheduled_amount: i64,
    hidden: bool,
) -> Value {
    json!({
        "schemaVersion": 1,
        "sections": {
            "accounts": [
                {
                    "externalRef": format!("account:cash:{unique}"),
                    "name": format!("现金-{unique}"),
                    "type": 1,
                    "currency": "CNY",
                    "balance": 12.34
                },
                {
                    "externalRef": format!("account:bank:{unique}"),
                    "name": format!("工资卡-{unique}"),
                    "type": 1,
                    "currency": "CNY",
                    "balance": 0
                }
            ],
            "transactionCategories": [{
                "externalRef": format!("category:lunch:{unique}"),
                "type": 3,
                "mainCategory": format!("餐饮-{unique}"),
                "subCategory": "午餐"
            }],
            "transactionTags": [{
                "externalRef": format!("tag:project:{unique}"),
                "name": format!("项目-{unique}")
            }],
            "transactionTemplates": [{
                "name": "午餐模板",
                "type": 3,
                "categoryRef": format!("category:lunch:{unique}"),
                "sourceAccountRef": format!("account:cash:{unique}"),
                "sourceAmount": template_amount,
                "destinationAmount": 0,
                "hideAmount": false,
                "tagRefs": [format!("tag:project:{unique}")],
                "comment": "工作日午餐",
                "displayOrder": 4,
                "hidden": hidden
            }],
            "scheduledTransactions": [{
                "name": "房租计划",
                "type": 4,
                "categoryRef": format!("category:lunch:{unique}"),
                "sourceAccountRef": format!("account:cash:{unique}"),
                "destinationAccountRef": format!("account:bank:{unique}"),
                "sourceAmount": scheduled_amount,
                "destinationAmount": scheduled_amount,
                "hideAmount": false,
                "tagRefs": [format!("tag:project:{unique}")],
                "scheduledFrequencyType": 3,
                "scheduledFrequency": "1",
                "scheduledStartDate": "2026-06-01",
                "enabled": true,
                "autoCreate": true,
                "displayOrder": 5,
                "hidden": hidden
            }]
        }
    })
}

fn assert_no_unsupported_template_warning(result: &Value) {
    let warnings = result
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    assert!(warnings.iter().all(|warning| {
        !warning
            .as_str()
            .unwrap_or_default()
            .contains("not supported")
    }));
}

async fn assert_template_row(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    template_type: i32,
    name: &str,
    source_amount_minor_units: i64,
    destination_amount_minor_units: i64,
    hidden: bool,
) -> Result<(), Box<dyn Error>> {
    let row = sqlx::query(
        r#"
        SELECT template_type, name, category_id, source_account_id, destination_account_id,
            source_amount_minor_units::BIGINT AS source_amount_minor_units,
            destination_amount_minor_units::BIGINT AS destination_amount_minor_units,
            tag_ids, scheduled_frequency_type, scheduled_frequency, scheduled_start_date,
            scheduled_next_date, enabled, auto_create, display_order, hidden
        FROM transaction_templates
        WHERE user_id = $1 AND template_type = $2 AND name = $3
        "#,
    )
    .bind(user_id)
    .bind(template_type)
    .bind(name)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.try_get::<i32, _>("template_type")?, template_type);
    assert_eq!(
        row.try_get::<i64, _>("source_amount_minor_units")?,
        source_amount_minor_units
    );
    assert_eq!(
        row.try_get::<i64, _>("destination_amount_minor_units")?,
        destination_amount_minor_units
    );
    assert!(!row
        .try_get::<Option<String>, _>("category_id")?
        .unwrap_or_default()
        .is_empty());
    assert_ne!(
        row.try_get::<Option<String>, _>("source_account_id")?
            .unwrap_or_default(),
        "0"
    );
    assert!(row
        .try_get::<Value, _>("tag_ids")?
        .as_array()
        .is_some_and(|items| items.len() == 1));
    assert_eq!(row.try_get::<bool, _>("hidden")?, hidden);

    if template_type == 2 {
        assert_eq!(
            row.try_get::<Option<i32>, _>("scheduled_frequency_type")?,
            Some(3)
        );
        assert_eq!(
            row.try_get::<Option<String>, _>("scheduled_start_date")?
                .as_deref(),
            Some("2026-06-01")
        );
        assert_eq!(
            row.try_get::<Option<String>, _>("scheduled_next_date")?
                .as_deref(),
            Some("2026-06-01")
        );
        assert!(row.try_get::<bool, _>("enabled")?);
        assert!(row.try_get::<bool, _>("auto_create")?);
        assert_ne!(
            row.try_get::<Option<String>, _>("destination_account_id")?
                .unwrap_or_default(),
            "0"
        );
    } else {
        assert!(row
            .try_get::<Option<i32>, _>("scheduled_frequency_type")?
            .is_none());
    }

    Ok(())
}
