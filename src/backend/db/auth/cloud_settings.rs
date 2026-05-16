pub fn list_application_cloud_settings(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<ApplicationCloudSettingRow>> {
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

pub fn list_user_external_auths(
    connection: &Connection,
    user_id: UserId,
) -> DbResult<Vec<ExternalAuthRow>> {
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

pub fn delete_user_external_auth(
    connection: &Connection,
    user_id: UserId,
    external_auth_type: &str,
) -> DbResult<bool> {
    let user_id_sql = user_id_sql(user_id)?;
    let changed = connection.execute(
        "DELETE FROM user_external_auths WHERE user_id = ?1 AND external_auth_type = ?2",
        params![user_id_sql, external_auth_type],
    )?;
    Ok(changed > 0)
}
