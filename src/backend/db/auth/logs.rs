// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

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
