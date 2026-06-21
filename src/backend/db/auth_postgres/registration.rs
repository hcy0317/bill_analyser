#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：创建注册用户并可选写入 standard_daily_v1 默认包，整个流程在同一事务内完成。
pub async fn create_postgres_registered_user_with_defaults(
    pool: &PostgresPool,
    draft: &RegisterUserDraft,
    preset_categories: &[RegisterPresetCategory],
    default_package: RegisterDefaultSeedPackage,
    auth_log: &AuthLogDraft,
) -> DbResult<RegisterUserResult> {
    let mut metadata = Map::new();
    metadata.insert(
        "nickname".to_string(),
        Value::String(draft.nickname.clone()),
    );
    metadata.insert(
        "language".to_string(),
        Value::String(draft.language.clone()),
    );
    metadata.insert(
        "default_currency".to_string(),
        Value::String(draft.default_currency.clone()),
    );
    metadata.insert(
        "first_day_of_week".to_string(),
        Value::from(draft.first_day_of_week),
    );
    metadata.insert(
        "email_verified".to_string(),
        Value::from(draft.email_verified),
    );
    metadata.insert("is_active".to_string(), Value::Bool(true));
    metadata.insert("import_learning_enabled".to_string(), Value::Bool(true));

    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let row = sqlx::query(
        r#"
        INSERT INTO users (
            username, email, display_name, password_hash, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $6::timestamptz)
        RETURNING id
        "#,
    )
    .bind(&draft.username)
    .bind(&draft.email)
    .bind(&draft.nickname)
    .bind(&draft.password_hash)
    .bind(Json(Value::Object(metadata.clone())))
    .bind(&draft.created_at)
    .fetch_one(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    let user_id: i64 = row.try_get("id").map_err(postgres_auth_error)?;

    let (default_account_id, cash_account_id) =
        insert_postgres_default_accounts(&mut transaction, user_id, draft).await?;
    if let Some(default_account_id) = default_account_id {
        metadata.insert(
            "default_account_id".to_string(),
            Value::from(default_account_id),
        );
    }
    if let Some(cash_account_id) = cash_account_id {
        metadata.insert("cash_account_id".to_string(), Value::from(cash_account_id));
    }
    sqlx::query("UPDATE users SET metadata = $1, updated_at = $2::timestamptz WHERE id = $3")
        .bind(Json(Value::Object(metadata)))
        .bind(&draft.created_at)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?;

    insert_postgres_preset_categories(&mut transaction, user_id, preset_categories, draft).await?;
    let selected_default_seed = if default_package.is_standard_daily_v1() {
        Some(insert_postgres_standard_daily_package(&mut transaction, user_id, draft).await?)
    } else {
        None
    };
    transaction.commit().await.map_err(postgres_auth_error)?;

    let default_seed = match selected_default_seed {
        Some(summary) => summary,
        None => ensure_postgres_category_rule_defaults(pool, user_id).await?,
    };
    create_postgres_auth_log(
        pool,
        &AuthLogDraft {
            user_id: Some(user_id_from_i64(user_id)?),
            username: auth_log.username.clone(),
            event_type: auth_log.event_type.clone(),
            ip_address: auth_log.ip_address.clone(),
            user_agent: auth_log.user_agent.clone(),
            success: auth_log.success,
            error_message: auth_log.error_message.clone(),
            metadata: auth_log.metadata.clone(),
            created_at: auth_log.created_at.clone(),
        },
    )
    .await?;

    Ok(RegisterUserResult {
        user_id,
        preset_categories_saved: !preset_categories.is_empty(),
        preset_accounts_saved: default_account_id.is_some(),
        cash_account_id,
        default_account_id,
        default_seed,
    })
}

async fn insert_postgres_default_accounts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    draft: &RegisterUserDraft,
) -> DbResult<(Option<i64>, Option<i64>)> {
    let zh = draft.language.to_ascii_lowercase().starts_with("zh");
    let accounts = if zh {
        [("现金", "1", 0), ("银行卡", "2", 1)]
    } else {
        [("Cash", "1", 0), ("Bank Card", "2", 1)]
    };
    let mut created = Vec::new();
    for (name, account_type, display_order) in accounts {
        let row = sqlx::query(
            r#"
            INSERT INTO accounts (
                user_id, name, account_type, currency, balance_cents,
                is_active, display_order, metadata, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, 0, TRUE, $5, $6, $7::timestamptz, $7::timestamptz)
            ON CONFLICT (user_id, name) DO UPDATE
                SET updated_at = EXCLUDED.updated_at
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(account_type)
        .bind(&draft.default_currency)
        .bind(display_order)
        .bind(Json(json!({ "source": "register_default" })))
        .bind(&draft.created_at)
        .fetch_one(&mut **transaction)
        .await
        .map_err(postgres_auth_error)?;
        created.push(row.try_get::<i64, _>("id").map_err(postgres_auth_error)?);
    }
    Ok((created.first().copied(), created.first().copied()))
}
