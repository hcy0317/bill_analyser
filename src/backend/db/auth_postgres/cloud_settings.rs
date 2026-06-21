#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：全量或增量写入用户应用云同步设置，保持配置项按用户边界幂等 upsert。
pub async fn update_postgres_application_cloud_settings(
    pool: &PostgresPool,
    user_id: UserId,
    settings: &[ApplicationCloudSettingDraft],
    full_update: bool,
    updated_at: &str,
) -> DbResult<bool> {
    let user_id_sql = user_id_i64(user_id)?;
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let normalized_settings = settings
        .iter()
        .filter(|setting| !setting.setting_key.trim().is_empty())
        .collect::<Vec<_>>();
    if full_update {
        sqlx::query(
            "DELETE FROM settings WHERE user_id = $1 AND key LIKE 'application_cloud_settings.%'",
        )
        .bind(user_id_sql)
        .execute(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?;
    }
    for setting in normalized_settings {
        let key = format!("application_cloud_settings.{}", setting.setting_key.trim());
        sqlx::query(
            r#"
            INSERT INTO settings (user_id, key, value, updated_at)
            VALUES ($1, $2, $3, $4::timestamptz)
            ON CONFLICT (user_id, key) DO UPDATE SET
                value = EXCLUDED.value,
                updated_at = EXCLUDED.updated_at,
                version = settings.version + 1
            "#,
        )
        .bind(user_id_sql)
        .bind(key)
        .bind(Json(Value::String(setting.setting_value.clone())))
        .bind(updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?;
    }
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：删除当前用户全部应用云同步设置，用于关闭云同步时清理服务端配置。
pub async fn delete_postgres_application_cloud_settings(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<bool> {
    sqlx::query(
        "DELETE FROM settings WHERE user_id = $1 AND key LIKE 'application_cloud_settings.%'",
    )
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map(|_| true)
    .map_err(postgres_auth_error)
}
