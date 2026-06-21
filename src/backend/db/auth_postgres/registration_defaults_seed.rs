#[doc(hidden)]
#[tracing::instrument(level = "debug", skip_all)]
pub async fn seed_postgres_standard_daily_defaults_for_user(
    pool: &PostgresPool,
    user_id: i64,
    draft: &RegisterUserDraft,
) -> DbResult<RegisterDefaultSeedSummary> {
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let summary = insert_postgres_standard_daily_package(&mut transaction, user_id, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(summary)
}

async fn insert_postgres_standard_daily_package(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    draft: &RegisterUserDraft,
) -> DbResult<RegisterDefaultSeedSummary> {
    let mut summary =
        RegisterDefaultSeedSummary::empty(RegisterDefaultSeedPackage::StandardDailyV1);

    for category in STANDARD_DAILY_V1_CATEGORIES {
        insert_postgres_standard_category_tree(transaction, user_id, category, draft, &mut summary)
            .await?;
    }
    for account in STANDARD_DAILY_V1_ACCOUNTS {
        insert_postgres_standard_account(transaction, user_id, account, draft, &mut summary)
            .await?;
    }
    for rule in STANDARD_DAILY_V1_CATEGORY_RULES {
        insert_postgres_standard_category_rule(transaction, user_id, rule, &mut summary).await?;
    }
    for rule in STANDARD_DAILY_V1_ACCOUNT_RULES {
        insert_postgres_standard_account_rule(transaction, user_id, rule, &mut summary).await?;
    }

    Ok(summary)
}

async fn insert_postgres_standard_category_tree(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    category: &DefaultCategory,
    draft: &RegisterUserDraft,
    summary: &mut RegisterDefaultSeedSummary,
) -> DbResult<()> {
    let parent_id = upsert_postgres_standard_category(
        transaction,
        user_id,
        None,
        category.name,
        category.type_code,
        category.name,
        category.icon,
        category.color,
        category.priority,
        draft,
        summary,
    )
    .await?;

    for (offset, sub_category) in category.sub_categories.iter().enumerate() {
        let display_order = category
            .priority
            .saturating_add(i64::try_from(offset + 1).unwrap_or(i64::MAX));
        let path = format!("{}/{}", category.name, sub_category.name);
        upsert_postgres_standard_category(
            transaction,
            user_id,
            Some(parent_id),
            sub_category.name,
            category.type_code,
            &path,
            sub_category.icon,
            sub_category.color,
            display_order,
            draft,
            summary,
        )
        .await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn upsert_postgres_standard_category(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
    type_code: i64,
    path: &str,
    icon: &str,
    color: &str,
    display_order: i64,
    draft: &RegisterUserDraft,
    summary: &mut RegisterDefaultSeedSummary,
) -> DbResult<i64> {
    if let Some(category_id) =
        find_postgres_register_category(transaction, user_id, parent_id, name).await?
    {
        sqlx::query(
            r#"
            UPDATE categories
            SET category_type = $1,
                path = $2,
                icon = $3,
                color = $4,
                display_order = $5,
                is_active = TRUE,
                metadata = $6,
                updated_at = $7::timestamptz,
                version = version + 1
            WHERE id = $8 AND user_id = $9
            "#,
        )
        .bind(type_code.to_string())
        .bind(path)
        .bind(icon)
        .bind(color)
        .bind(auth_i64_to_i32(display_order))
        .bind(Json(
            json!({ "source": RegisterDefaultSeedPackage::STANDARD_DAILY_V1 }),
        ))
        .bind(&draft.created_at)
        .bind(category_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(postgres_auth_error)?;
        summary.categories_skipped += 1;
        return Ok(category_id);
    }

    let row = sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, TRUE, $9, $10::timestamptz, $10::timestamptz)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(name)
    .bind(type_code.to_string())
    .bind(path)
    .bind(icon)
    .bind(color)
    .bind(auth_i64_to_i32(display_order))
    .bind(Json(
        json!({ "source": RegisterDefaultSeedPackage::STANDARD_DAILY_V1 }),
    ))
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    summary.categories_created += 1;
    row.try_get("id").map_err(postgres_auth_error)
}

async fn find_postgres_register_category(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
) -> DbResult<Option<i64>> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM categories
        WHERE user_id = $1
          AND name = $2
          AND (
            ($3::BIGINT IS NULL AND parent_id IS NULL)
            OR parent_id = $3::BIGINT
          )
        ORDER BY id
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(name)
    .bind(parent_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(postgres_auth_error)
}

async fn insert_postgres_standard_account(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    account: &DefaultAccount,
    draft: &RegisterUserDraft,
    summary: &mut RegisterDefaultSeedSummary,
) -> DbResult<i64> {
    let metadata = Json(json!({
        "source": RegisterDefaultSeedPackage::STANDARD_DAILY_V1,
        "category": account.category,
        "icon": account.icon,
        "color": account.color,
        "comment": "",
        "initial_balance_cents": 0,
    }));
    let existing_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM accounts WHERE user_id = $1 AND name = $2 LIMIT 1")
            .bind(user_id)
            .bind(account.name)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(postgres_auth_error)?;

    if let Some(account_id) = existing_id {
        sqlx::query(
            r#"
            UPDATE accounts
            SET account_type = $1,
                currency = $2,
                is_active = TRUE,
                display_order = $3,
                metadata = $4,
                updated_at = $5::timestamptz,
                version = version + 1
            WHERE id = $6 AND user_id = $7
            "#,
        )
        .bind(account.account_type)
        .bind(&draft.default_currency)
        .bind(auth_i64_to_i32(account.display_order))
        .bind(metadata)
        .bind(&draft.created_at)
        .bind(account_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(postgres_auth_error)?;
        summary.accounts_skipped += 1;
        return Ok(account_id);
    }

    let row = sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents,
            is_active, display_order, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, 0, TRUE, $5, $6, $7::timestamptz, $7::timestamptz)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(account.name)
    .bind(account.account_type)
    .bind(&draft.default_currency)
    .bind(auth_i64_to_i32(account.display_order))
    .bind(metadata)
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    summary.accounts_created += 1;
    row.try_get("id").map_err(postgres_auth_error)
}

async fn insert_postgres_standard_category_rule(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    rule: &DefaultCategoryRule,
    summary: &mut RegisterDefaultSeedSummary,
) -> DbResult<()> {
    ensure_register_rule_expression(rule.name, rule.rule_expression)?;
    let Some(category_id) = find_postgres_standard_category_by_path(
        transaction,
        user_id,
        rule.main_category,
        rule.sub_category,
    )
    .await?
    else {
        summary.note_missing_category();
        return Ok(());
    };

    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM category_rules WHERE user_id = $1 AND name = $2 LIMIT 1",
    )
    .bind(user_id)
    .bind(rule.name)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    if exists.is_some() {
        summary.rules_skipped += 1;
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO category_rules (
            user_id, category_id, name, transaction_type_scope, field_scope,
            rule_expression, priority, enabled
        ) VALUES ($1, $2, $3, 'all', $4, $5, $6, TRUE)
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(rule.name)
    .bind(Json(json!([
        "counterparty",
        "payment_method",
        "description"
    ])))
    .bind(register_rule_expression_json(rule.rule_expression, false))
    .bind(auth_i64_to_i32(rule.priority))
    .execute(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    summary.rules_created += 1;
    Ok(())
}

async fn find_postgres_standard_category_by_path(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
) -> DbResult<Option<i64>> {
    if sub_category.trim().is_empty() {
        return find_postgres_register_category(transaction, user_id, None, main_category).await;
    }
    let Some(parent_id) =
        find_postgres_register_category(transaction, user_id, None, main_category).await?
    else {
        return Ok(None);
    };
    find_postgres_register_category(transaction, user_id, Some(parent_id), sub_category).await
}

async fn insert_postgres_standard_account_rule(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    rule: &DefaultAccountRule,
    summary: &mut RegisterDefaultSeedSummary,
) -> DbResult<()> {
    ensure_register_rule_expression(rule.name, rule.rule_expression)?;
    let account_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM accounts WHERE user_id = $1 AND name = $2 LIMIT 1")
            .bind(user_id)
            .bind(rule.account_name)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(postgres_auth_error)?;
    let Some(account_id) = account_id else {
        summary.note_missing_target();
        return Ok(());
    };

    let exists: Option<i64> = sqlx::query_scalar(
        r#"
        SELECT id
        FROM account_rules
        WHERE user_id = $1
          AND account_id = $2
          AND source = $3
          AND source_key = $4
        LIMIT 1
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(RegisterDefaultSeedPackage::STANDARD_DAILY_V1)
    .bind(rule.source_key)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    if exists.is_some() {
        summary.account_rules_skipped += 1;
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO account_rules (
            user_id, account_id, name, rule_expression, regex_enabled,
            priority, enabled, source, source_key
        ) VALUES ($1, $2, $3, $4, FALSE, $5, TRUE, $6, $7)
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .bind(rule.name)
    .bind(register_rule_expression_json(rule.rule_expression, false))
    .bind(auth_i64_to_i32(rule.priority))
    .bind(RegisterDefaultSeedPackage::STANDARD_DAILY_V1)
    .bind(rule.source_key)
    .execute(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    summary.account_rules_created += 1;
    Ok(())
}

fn ensure_register_rule_expression(name: &str, expression: &str) -> DbResult<()> {
    let compiled = compile_rule_expression(expression, false);
    if compiled.is_empty {
        return Err(DbError::InvalidOperation(format!(
            "default rule expression is invalid or empty: {name}"
        )));
    }
    Ok(())
}

fn register_rule_expression_json(expression: &str, regex_enabled: bool) -> Json<Value> {
    Json(json!({
        "expression": expression,
        "regex_enabled": regex_enabled,
    }))
}

fn auth_i64_to_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or_else(|_| {
        if value.is_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

async fn insert_postgres_preset_categories(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    categories: &[RegisterPresetCategory],
    draft: &RegisterUserDraft,
) -> DbResult<()> {
    for item in categories {
        let name = item.name.trim();
        if name.is_empty() {
            continue;
        }
        let parent_id =
            upsert_postgres_register_category(transaction, user_id, None, name, item, draft)
                .await?;
        for sub in &item.sub_categories {
            let sub_name = sub.name.trim();
            if sub_name.is_empty() {
                continue;
            }
            let mut sub_item = item.clone();
            sub_item.name = sub_name.to_string();
            sub_item.icon = if sub.icon.is_empty() {
                item.icon.clone()
            } else {
                sub.icon.clone()
            };
            sub_item.color = if sub.color.is_empty() {
                item.color.clone()
            } else {
                sub.color.clone()
            };
            upsert_postgres_register_category(
                transaction,
                user_id,
                Some(parent_id),
                sub_name,
                &sub_item,
                draft,
            )
            .await?;
        }
    }
    Ok(())
}

async fn upsert_postgres_register_category(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
    item: &RegisterPresetCategory,
    draft: &RegisterUserDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 0, TRUE, $8, $9::timestamptz, $9::timestamptz)
        ON CONFLICT (user_id, parent_id, name) DO UPDATE
            SET icon = EXCLUDED.icon,
                color = EXCLUDED.color,
                updated_at = EXCLUDED.updated_at
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(name)
    .bind(item.type_code.to_string())
    .bind(name)
    .bind(&item.icon)
    .bind(&item.color)
    .bind(Json(json!({ "source": "register_preset" })))
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}
