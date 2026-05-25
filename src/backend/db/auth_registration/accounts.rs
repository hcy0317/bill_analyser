// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

struct RegisterAccountsResult {
    success: bool,
    cash_account_id: Option<i64>,
    default_account_id: Option<i64>,
}

#[tracing::instrument(level = "debug", skip_all)]
fn create_register_default_accounts(
    connection: &Connection,
    user_id: i64,
    language: &str,
    created_at: &str,
) -> DbResult<RegisterAccountsResult> {
    let templates = if language.to_ascii_lowercase().starts_with("zh") {
        ZH_DEFAULT_ACCOUNTS
    } else {
        DEFAULT_ACCOUNTS
    };
    let mut created_account_ids = Vec::new();
    let mut cash_account_id = None;
    for account in templates {
        let aliases = serde_json::to_string(account.aliases)
            .map_err(|error| DbError::InvalidOperation(error.to_string()))?;
        connection.execute(
            r#"
            INSERT INTO accounts (
                user_id, name, type, category, currency, icon, color,
                balance, initial_balance, hidden, display_order, comment,
                aliases, parent_id, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 0, 0, ?8, NULL, ?9, 0, ?10, ?10)
            "#,
            params![
                user_id,
                account.name,
                account.type_code,
                account.category,
                account.currency,
                account.icon,
                account.color,
                account.display_order,
                aliases,
                created_at,
            ],
        )?;
        let account_id = connection.last_insert_rowid();
        created_account_ids.push(account_id);
        if account.category == 1 && cash_account_id.is_none() {
            cash_account_id = Some(account_id);
        }
    }
    let default_account_id = created_account_ids.first().copied();
    if let Some(default_account_id) = default_account_id {
        connection.execute(
            "UPDATE users SET default_account_id = ?1, cash_account_id = ?2, updated_at = ?3 WHERE id = ?4",
            params![default_account_id, cash_account_id, created_at, user_id],
        )?;
    }
    Ok(RegisterAccountsResult {
        success: true,
        cash_account_id,
        default_account_id,
    })
}
