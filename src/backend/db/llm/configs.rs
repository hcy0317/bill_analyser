use super::*;

/// 查询指定用户的 LLM 配置列表，按启用状态和更新时间返回给配置页展示。
pub async fn list_postgres_llm_configs(pool: &PostgresPool, user_id: i64) -> DbResult<Vec<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "list_postgres_llm_configs",
        "business operation entered"
    );
    let rows = sqlx::query(
        "
        SELECT id, user_id, name, provider, model, api_key, base_url,
               credential_config, advanced_settings, is_active, created_at, updated_at
        FROM llm_configs
        WHERE user_id = $1
        ORDER BY is_active DESC, updated_at DESC
        ",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(llm_config_from_row).collect()
}

/// 读取指定用户当前启用的 LLM 配置，缺失时返回 None 让上层回退默认运行时配置。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_active_llm_config(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Option<Value>> {
    let row = sqlx::query(
        "
        SELECT id, user_id, name, provider, model, api_key, base_url,
               credential_config, advanced_settings, is_active, created_at, updated_at
        FROM llm_configs
        WHERE user_id = $1 AND is_active = true
        ORDER BY updated_at DESC
        LIMIT 1
        ",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(llm_config_from_row).transpose()
}

/// 新增用户 LLM 配置；当草稿要求启用时，会先关闭同用户其他配置以保持单活约束。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_llm_config(
    pool: &PostgresPool,
    user_id: i64,
    draft: &LlmConfigDraft,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "create_postgres_llm_config",
        "business operation entered"
    );
    if draft.is_active {
        deactivate_postgres_llm_configs(pool, user_id, None).await?;
    }
    let row = sqlx::query(
        "
        INSERT INTO llm_configs (
            user_id, name, provider, model, api_key, base_url,
            credential_config, advanced_settings, is_active, created_at, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now(), now())
        RETURNING id
        ",
    )
    .bind(user_id)
    .bind(draft.name.trim())
    .bind(draft.provider.trim())
    .bind(draft.model.trim())
    .bind(draft.api_key.trim())
    .bind(draft.base_url.trim())
    .bind(serialize_credential_config(
        &draft.credential_config,
        &draft.api_key,
    ))
    .bind(serialize_advanced_settings(&draft.advanced_settings))
    .bind(draft.is_active)
    .fetch_one(pool)
    .await?;
    let config_id: i64 = row.try_get("id")?;
    get_postgres_llm_config_by_id(pool, config_id, user_id)
        .await?
        .ok_or_else(|| {
            DbError::InvalidOperation("created llm config could not be reloaded".to_string())
        })
}

/// 按增量字段更新 LLM 配置，并在设为启用时维护同用户配置的单活状态。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_llm_config(
    pool: &PostgresPool,
    config_id: i64,
    user_id: i64,
    update: &LlmConfigUpdate,
) -> DbResult<Option<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "update_postgres_llm_config",
        "business operation entered"
    );
    let Some(existing) = get_postgres_llm_config_by_id(pool, config_id, user_id).await? else {
        return Ok(None);
    };
    if update.is_active == Some(true) {
        deactivate_postgres_llm_configs(pool, user_id, Some(config_id)).await?;
    }

    let name = update
        .name
        .clone()
        .unwrap_or_else(|| text_field(&existing, "name"));
    let provider = update
        .provider
        .clone()
        .unwrap_or_else(|| text_field(&existing, "provider"));
    let model = update
        .model
        .clone()
        .unwrap_or_else(|| text_field(&existing, "model"));
    let api_key = update
        .api_key
        .clone()
        .unwrap_or_else(|| text_field(&existing, "api_key"));
    let base_url = update
        .base_url
        .clone()
        .unwrap_or_else(|| text_field(&existing, "base_url"));
    let credential_config = update
        .credential_config
        .as_ref()
        .map(|value| serialize_credential_config(value, &api_key))
        .unwrap_or_else(|| existing["credential_config"].clone());
    let advanced_settings = update
        .advanced_settings
        .as_ref()
        .map(serialize_advanced_settings)
        .unwrap_or_else(|| existing["advanced_settings"].clone());
    let is_active = update.is_active.unwrap_or_else(|| {
        existing
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    });

    sqlx::query(
        "
        UPDATE llm_configs
        SET name = $1,
            provider = $2,
            model = $3,
            api_key = $4,
            base_url = $5,
            credential_config = $6,
            advanced_settings = $7,
            is_active = $8,
            updated_at = now(),
            version = version + 1
        WHERE id = $9 AND user_id = $10
        ",
    )
    .bind(name.trim())
    .bind(provider.trim())
    .bind(model.trim())
    .bind(api_key.trim())
    .bind(base_url.trim())
    .bind(credential_config)
    .bind(advanced_settings)
    .bind(is_active)
    .bind(config_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    get_postgres_llm_config_by_id(pool, config_id, user_id).await
}

/// 删除指定用户可见的 LLM 配置，返回是否真实删除了数据库记录。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_llm_config(
    pool: &PostgresPool,
    config_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "delete_postgres_llm_config",
        "business operation entered"
    );
    let deleted = sqlx::query("DELETE FROM llm_configs WHERE id = $1 AND user_id = $2")
        .bind(config_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(deleted.rows_affected() > 0)
}

/// 激活指定 LLM 配置，并同步停用同用户的其他配置以避免运行时配置冲突。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn activate_postgres_llm_config(
    pool: &PostgresPool,
    config_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    if get_postgres_llm_config_by_id(pool, config_id, user_id)
        .await?
        .is_none()
    {
        return Ok(false);
    }
    deactivate_postgres_llm_configs(pool, user_id, Some(config_id)).await?;
    sqlx::query(
        "
        UPDATE llm_configs
        SET is_active = true, updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2
        ",
    )
    .bind(config_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(true)
}

/// 生成运行时实际使用的 LLM 配置；优先采用用户启用配置，缺失时使用系统默认值。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn effective_postgres_llm_config_from_saved(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<Value> {
    get_postgres_active_llm_config(pool, user_id)
        .await
        .map(|active| {
            active
                .as_ref()
                .map(build_runtime_llm_config_from_saved_config)
                .unwrap_or_else(default_llm_runtime_config)
        })
}
