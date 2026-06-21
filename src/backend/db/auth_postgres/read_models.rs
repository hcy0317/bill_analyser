#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按用户名或邮箱读取登录所需的认证用户投影，供密码校验和登录审计使用。
pub async fn get_postgres_login_user_by_login_name(
    pool: &PostgresPool,
    login_name: &str,
) -> DbResult<Option<AuthLoginUserRow>> {
    sqlx::query(
        r#"
        SELECT id, username, COALESCE(email, '') AS email,
               COALESCE(display_name, '') AS display_name,
               COALESCE(password_hash, '') AS password_hash,
               metadata
        FROM users
        WHERE username = $1 OR email = $1
        LIMIT 1
        "#,
    )
    .bind(login_name)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| postgres_login_user_from_row(&row))
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按用户 ID 读取登录态用户投影，供 token/session 恢复当前用户身份使用。
pub async fn get_postgres_login_user_by_id(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<AuthLoginUserRow>> {
    sqlx::query(
        r#"
        SELECT id, username, COALESCE(email, '') AS email,
               COALESCE(display_name, '') AS display_name,
               COALESCE(password_hash, '') AS password_hash,
               metadata
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| postgres_login_user_from_row(&row))
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：读取当前用户 profile 详情，包含 metadata 中的偏好、头像和默认账户分类字段。
pub async fn get_postgres_auth_user_profile(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<AuthUserProfileRow>> {
    sqlx::query(
        r#"
        SELECT id, username, COALESCE(email, '') AS email,
               COALESCE(display_name, '') AS display_name,
               COALESCE(password_hash, '') AS password_hash,
               metadata
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| postgres_profile_from_row(&row))
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：读取用户已启用的应用云同步配置，保持 settings key 与 value 的 PostgreSQL 投影。
pub async fn list_postgres_application_cloud_settings(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<ApplicationCloudSettingRow>> {
    let rows = sqlx::query(
        r#"
        SELECT key, value
        FROM settings
        WHERE user_id = $1
          AND key LIKE 'application_cloud_settings.%'
        ORDER BY key
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_all(pool)
    .await
    .map_err(postgres_auth_error)?;

    rows.into_iter()
        .map(|row| {
            let raw_key: String = row.try_get("key").map_err(postgres_auth_error)?;
            let Json(value): Json<Value> = row.try_get("value").map_err(postgres_auth_error)?;
            Ok(ApplicationCloudSettingRow {
                setting_key: raw_key
                    .strip_prefix("application_cloud_settings.")
                    .unwrap_or(&raw_key)
                    .to_string(),
                setting_value: json_value_to_setting_string(&value),
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：列出用户已绑定的外部登录身份，用于安全设置页展示和解绑入口。
pub async fn list_postgres_user_external_auths(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<ExternalAuthRow>> {
    let rows = sqlx::query(
        r#"
        SELECT external_auth_category, external_auth_type,
               COALESCE(external_username, '') AS external_username,
               to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS') AS created_at
        FROM user_external_auths
        WHERE user_id = $1
        ORDER BY created_at DESC, id DESC
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_all(pool)
    .await
    .map_err(postgres_auth_error)?;

    rows.into_iter()
        .map(|row| postgres_external_auth_from_row(&row))
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按 provider 和 provider user id 查询当前用户外部身份绑定，避免跨账号解绑或重复绑定。
pub async fn get_postgres_user_external_auth(
    pool: &PostgresPool,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<Option<ExternalAuthRow>> {
    sqlx::query(
        r#"
        SELECT external_auth_category, external_auth_type,
               COALESCE(external_username, '') AS external_username,
               to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS') AS created_at
        FROM user_external_auths
        WHERE user_id = $1 AND external_auth_type = $2
        LIMIT 1
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(external_auth_type)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| postgres_external_auth_from_row(&row))
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：按用户边界删除外部身份绑定，并返回删除数量供上层判断是否真正解绑。
pub async fn delete_postgres_user_external_auth(
    pool: &PostgresPool,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<bool> {
    sqlx::query("DELETE FROM user_external_auths WHERE user_id = $1 AND external_auth_type = $2")
        .bind(user_id_i64(user_id)?)
        .bind(external_auth_type)
        .execute(pool)
        .await
        .map(|result| result.rows_affected() > 0)
        .map_err(postgres_auth_error)
}
