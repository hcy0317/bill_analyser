pub fn create_auth_log(connection: &Connection, draft: &AuthLogDraft) -> DbResult<i64> {
    let user_id = draft.user_id.map(user_id_sql).transpose()?;
    connection.execute(
        r#"
        INSERT INTO auth_logs (
            user_id, username, event_type, ip_address, user_agent,
            success, error_message, metadata, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        "#,
        params![
            user_id,
            draft.username,
            draft.event_type,
            draft.ip_address,
            draft.user_agent,
            if draft.success { 1 } else { 0 },
            draft.error_message,
            draft.metadata,
            draft.created_at,
        ],
    )?;
    Ok(connection.last_insert_rowid())
}
