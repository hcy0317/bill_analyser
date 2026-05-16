fn create_two_factor_recovery_audit_log_best_effort(
    connection: &rusqlite::Connection,
    user_id: UserId,
    ip_address: &str,
    user_agent: &str,
    now: &str,
) {
    create_user_audit_log_best_effort(
        connection,
        UserAuditLogDraft {
            operation_type: "2fa_recovery_code_used",
            user_id,
            details: json!({ "verification": "recovery_code" }),
            affected_count: 1_i64,
            ip_address,
            user_agent,
            now,
        },
    );
}

struct UserAuditLogDraft<'a> {
    operation_type: &'a str,
    user_id: UserId,
    details: Value,
    affected_count: i64,
    ip_address: &'a str,
    user_agent: &'a str,
    now: &'a str,
}

struct UserDataAuditLogDraft<'a> {
    operation_type: &'a str,
    user_id: UserId,
    details: Value,
    affected_count: i64,
    ip_address: &'a str,
    user_agent: &'a str,
    now: &'a str,
}

fn create_user_audit_log_best_effort(
    connection: &rusqlite::Connection,
    draft: UserAuditLogDraft<'_>,
) {
    let Ok(target_id) = i64::try_from(draft.user_id.get()) else {
        return;
    };
    let details = draft.details.to_string();
    let _ = connection.execute(
        r#"
        INSERT INTO audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        (
            draft.operation_type,
            "user",
            target_id,
            details,
            draft.affected_count,
            draft.ip_address,
            draft.user_agent,
            Option::<String>::None,
            "success",
            Option::<String>::None,
            draft.now,
        ),
    );
}

fn create_user_data_audit_log_best_effort(
    connection: &rusqlite::Connection,
    draft: UserDataAuditLogDraft<'_>,
) {
    let Ok(target_id) = i64::try_from(draft.user_id.get()) else {
        return;
    };
    let _ = connection.execute(
        r#"
        INSERT INTO audit_logs (
            operation_type, operation_target, target_id, details,
            affected_count, ip_address, user_agent, session_id,
            status, error_message, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        (
            draft.operation_type,
            "user_data",
            target_id,
            draft.details.to_string(),
            draft.affected_count,
            draft.ip_address,
            draft.user_agent,
            Option::<String>::None,
            "success",
            Option::<String>::None,
            draft.now,
        ),
    );
}
