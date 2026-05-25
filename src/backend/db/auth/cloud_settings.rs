// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_application_cloud_settings(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<ApplicationCloudSettingRow>> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "list_application_cloud_settings", "business operation entered");
    let user_id_sql = user_id_sql(user_id)?;
    let mut statement = connection.prepare(
        r#"
        SELECT setting_key, setting_value
        FROM user_application_cloud_settings
        WHERE user_id = ?1
        ORDER BY created_at ASC, id ASC
        "#,
    )?;
    let rows = statement.query_map([user_id_sql], |row| {
        Ok(ApplicationCloudSettingRow {
            setting_key: row.get::<_, Option<String>>(0)?.unwrap_or_default(),
            setting_value: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn list_user_external_auths(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<ExternalAuthRow>> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "list_user_external_auths", "business operation entered");
    let user_id_sql = user_id_sql(user_id)?;
    let mut statement = connection.prepare(
        r#"
        SELECT external_auth_category, external_auth_type, external_username, created_at
        FROM user_external_auths
        WHERE user_id = ?1
        ORDER BY created_at DESC, id DESC
        "#,
    )?;
    let rows = statement.query_map([user_id_sql], external_auth_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_user_external_auth(
    connection: &Connection,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<Option<ExternalAuthRow>> {
    let user_id_sql = user_id_sql(user_id)?;
    connection
        .query_row(
            r#"
            SELECT external_auth_category, external_auth_type, external_username, created_at
            FROM user_external_auths
            WHERE user_id = ?1 AND external_auth_type = ?2
            LIMIT 1
            "#,
            params![user_id_sql, external_auth_type],
            external_auth_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn delete_user_external_auth(
    connection: &Connection,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "delete_user_external_auth", "business operation entered");
    let user_id_sql = user_id_sql(user_id)?;
    let changed = connection.execute(
        "DELETE FROM user_external_auths WHERE user_id = ?1 AND external_auth_type = ?2",
        params![user_id_sql, external_auth_type],
    )?;
    Ok(changed > 0)
}
