#[derive(Debug, Clone, PartialEq)]
pub struct AccountAuditEventDraft {
    pub operation_type: &'static str,
    pub target_id: i64,
    pub details: Value,
    pub affected_count: i64,
    pub status: &'static str,
    pub error_message: Option<String>,
    pub ip_address: String,
    pub user_agent: String,
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：把一个账户在账单表中的所有账户引用迁移到目标账户，覆盖普通、转账和投资相关字段。
pub async fn move_all_postgres_account_transactions(
    pool: &PostgresPool,
    from_account_id: i64,
    to_account_id: i64,
    user_id: i64,
) -> DbResult<AccountTransactionsMoveResult> {
    if from_account_id == to_account_id {
        return Ok(account_move_failure(
            "Source and target accounts must be different",
        ));
    }
    let mut transaction = pool.begin().await?;
    if !postgres_account_exists(&mut transaction, from_account_id, user_id).await? {
        return Ok(account_move_failure("Source account not found"));
    }
    if !postgres_account_exists(&mut transaction, to_account_id, user_id).await? {
        return Ok(account_move_failure("Target account not found"));
    }
    let moved_count = sqlx::query(
        r#"
        UPDATE bills
        SET account_id = CASE WHEN account_id = $2 THEN $3 ELSE account_id END,
            source_account_id = CASE WHEN source_account_id = $2 THEN $3 ELSE source_account_id END,
            target_account_id = CASE WHEN target_account_id = $2 THEN $3 ELSE target_account_id END,
            transfer_target_account_id = CASE WHEN transfer_target_account_id = $2 THEN $3 ELSE transfer_target_account_id END,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(from_account_id)
    .bind(to_account_id)
    .execute(&mut *transaction)
    .await?
    .rows_affected() as i64;
    transaction.commit().await?;
    Ok(AccountTransactionsMoveResult {
        success: true,
        message: "Transactions moved successfully".to_string(),
        moved_count,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：软删除一个账户关联的正式账单，并先清理 bill_tags 关联以保持数据库一致性。
pub async fn clear_postgres_account_transactions(
    pool: &PostgresPool,
    account_id: i64,
    user_id: i64,
) -> DbResult<AccountTransactionsClearResult> {
    let mut transaction = pool.begin().await?;
    if !postgres_account_exists(&mut transaction, account_id, user_id).await? {
        return Ok(account_clear_failure("Account not found"));
    }
    let affected = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM bills
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .fetch_one(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        DELETE FROM bill_tags
        WHERE user_id = $1
          AND bill_id IN (
              SELECT id FROM bills
              WHERE user_id = $1
                AND is_deleted = false
                AND (
                    account_id = $2 OR source_account_id = $2 OR target_account_id = $2
                    OR transfer_target_account_id = $2
                )
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .execute(&mut *transaction)
    .await?;
    sqlx::query(
        r#"
        UPDATE bills
        SET is_deleted = true,
            updated_at = now(),
            version = version + 1
        WHERE user_id = $1
          AND is_deleted = false
          AND (
              account_id = $2 OR source_account_id = $2 OR target_account_id = $2
              OR transfer_target_account_id = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(account_id)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(AccountTransactionsClearResult {
        success: true,
        message: "Transactions deleted successfully".to_string(),
        deleted_count: affected,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：写入账户迁移或清理审计事件；是否忽略写入失败由调用方决定。
pub async fn create_postgres_account_audit_event(
    pool: &PostgresPool,
    user_id: i64,
    draft: AccountAuditEventDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor,
            before_payload, after_payload, metadata
        )
        VALUES ($1, 'account', $2, $3, 'runtime', '{}'::jsonb, '{}'::jsonb, $4)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(draft.target_id.to_string())
    .bind(draft.operation_type)
    .bind(json!({
        "details": draft.details,
        "affected_count": draft.affected_count,
        "status": draft.status,
        "error_message": draft.error_message,
        "ip_address": draft.ip_address,
        "user_agent": draft.user_agent,
    }))
    .fetch_one(pool)
    .await?;
    row.try_get("id").map_err(Into::into)
}

async fn postgres_account_exists(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: i64,
    user_id: i64,
) -> DbResult<bool> {
    Ok(sqlx::query("SELECT 1 FROM accounts WHERE id = $1 AND user_id = $2")
        .bind(account_id)
        .bind(user_id)
        .fetch_optional(&mut **transaction)
        .await?
        .is_some())
}

fn account_move_failure(message: &str) -> AccountTransactionsMoveResult {
    AccountTransactionsMoveResult {
        success: false,
        message: message.to_string(),
        moved_count: 0,
    }
}

fn account_clear_failure(message: &str) -> AccountTransactionsClearResult {
    AccountTransactionsClearResult {
        success: false,
        message: message.to_string(),
        deleted_count: 0,
    }
}
