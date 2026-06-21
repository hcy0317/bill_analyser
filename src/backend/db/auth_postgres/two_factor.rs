#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：读取用户 2FA 启用状态，供登录分支和安全设置页判断是否需要验证码。
pub async fn get_postgres_auth_user_two_factor_enabled(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<bool>> {
    let value: Option<Value> = sqlx::query_scalar("SELECT metadata FROM users WHERE id = $1")
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map_err(postgres_auth_error)?;
    Ok(value.map(|metadata| metadata_bool(&metadata, &["two_factor_enabled"], false)))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：启用 2FA 并写入 recovery code hash，同时刷新当前 session 的 2FA 状态。
pub async fn enable_postgres_two_factor_with_recovery_codes_and_session(
    pool: &PostgresPool,
    user_id: UserId,
    secret: &str,
    recovery_codes: &[&str],
    session_draft: &CreateTokenSessionDraft,
    now: &str,
) -> DbResult<(usize, i64)> {
    if session_draft.user_id != user_id {
        return Err(DbError::InvalidOperation(
            "session user mismatch".to_string(),
        ));
    }
    let user_id_sql = user_id_i64(user_id)?;
    let code_hashes = normalized_two_factor_recovery_code_hashes(recovery_codes);
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let row = sqlx::query("SELECT metadata FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id_sql)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?;
    let Some(row) = row else {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Err(DbError::InvalidOperation("user not found".to_string()));
    };
    let Json(mut metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    if metadata_bool(&metadata, &["two_factor_enabled"], false) {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Err(DbError::InvalidOperation(
            "two-factor authentication is already enabled".to_string(),
        ));
    }
    metadata_set_bool(&mut metadata, "two_factor_enabled", true);
    metadata_set_string(&mut metadata, "two_factor_secret", secret);
    sqlx::query(
        r#"
        UPDATE users
        SET metadata = $1, updated_at = $2::timestamptz, version = version + 1
        WHERE id = $3
        "#,
    )
    .bind(Json(metadata))
    .bind(now)
    .bind(user_id_sql)
    .execute(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    let stored_count = replace_postgres_two_factor_recovery_code_hashes_in_tx(
        &mut transaction,
        user_id_sql,
        &code_hashes,
        now,
    )
    .await?;
    let session_id = create_postgres_token_session_in_tx(&mut transaction, session_draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok((stored_count, session_id))
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：禁用 2FA 并清理 recovery code，同时更新当前 session 状态避免旧状态残留。
pub async fn disable_postgres_two_factor_and_clear_recovery_codes(
    pool: &PostgresPool,
    user_id: UserId,
    now: &str,
) -> DbResult<u64> {
    let user_id_sql = user_id_i64(user_id)?;
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let row = sqlx::query("SELECT metadata FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id_sql)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?;
    let Some(row) = row else {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Err(DbError::InvalidOperation("user not found".to_string()));
    };
    let Json(mut metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    metadata_set_bool(&mut metadata, "two_factor_enabled", false);
    metadata_set_string(&mut metadata, "two_factor_secret", "");
    sqlx::query(
        r#"
        UPDATE users
        SET metadata = $1, updated_at = $2::timestamptz, version = version + 1
        WHERE id = $3
        "#,
    )
    .bind(Json(metadata))
    .bind(now)
    .bind(user_id_sql)
    .execute(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    let cleared = sqlx::query("DELETE FROM user_two_factor_recovery_codes WHERE user_id = $1")
        .bind(user_id_sql)
        .execute(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?
        .rows_affected();
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(cleared)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：重新生成 recovery code hash 列表，替换旧备份码并保留用户 2FA 启用状态。
pub async fn replace_postgres_two_factor_recovery_codes(
    pool: &PostgresPool,
    user_id: UserId,
    recovery_codes: &[&str],
    now: &str,
) -> DbResult<usize> {
    let user_id_sql = user_id_i64(user_id)?;
    let code_hashes = normalized_two_factor_recovery_code_hashes(recovery_codes);
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let count = replace_postgres_two_factor_recovery_code_hashes_in_tx(
        &mut transaction,
        user_id_sql,
        &code_hashes,
        now,
    )
    .await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(count)
}

#[tracing::instrument(level = "debug", skip_all)]
/// 中文说明：消费单个 recovery code hash，命中后删除该码以保证备份码只能使用一次。
pub async fn consume_postgres_two_factor_recovery_code(
    pool: &PostgresPool,
    user_id: UserId,
    recovery_code: &str,
    now: &str,
) -> DbResult<bool> {
    let Some(code_hash) = hash_two_factor_recovery_code(recovery_code) else {
        return Ok(false);
    };
    sqlx::query(
        r#"
        UPDATE user_two_factor_recovery_codes
        SET used_at = $1::timestamptz, updated_at = $1::timestamptz, version = version + 1
        WHERE user_id = $2 AND code_hash = $3 AND used_at IS NULL
        "#,
    )
    .bind(now)
    .bind(user_id_i64(user_id)?)
    .bind(code_hash)
    .execute(pool)
    .await
    .map(|result| result.rows_affected() > 0)
    .map_err(postgres_auth_error)
}

// 中文说明：归一化单个 recovery code 后计算 hash，空值直接忽略避免写入无效备份码。
fn hash_two_factor_recovery_code(recovery_code: &str) -> Option<String> {
    recovery_code_hash_input(recovery_code)
        .map(|value| format!("{:x}", Sha256::digest(value.as_bytes())))
}

// 中文说明：把 recovery code 明文列表转换为 hash 列表，供启用和重新生成 2FA 时持久化。
fn normalized_two_factor_recovery_code_hashes(recovery_codes: &[&str]) -> Vec<String> {
    let mut seen_hashes = HashSet::new();
    let mut code_hashes = Vec::new();
    for recovery_code in recovery_codes {
        let Some(code_hash) = hash_two_factor_recovery_code(recovery_code) else {
            continue;
        };
        if seen_hashes.insert(code_hash.clone()) {
            code_hashes.push(code_hash);
        }
    }
    code_hashes
}

// 中文说明：在事务内替换 recovery code hash 数组，确保启用/重生成流程可原子提交。
async fn replace_postgres_two_factor_recovery_code_hashes_in_tx(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    code_hashes: &[String],
    now: &str,
) -> DbResult<usize> {
    sqlx::query("DELETE FROM user_two_factor_recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **transaction)
        .await
        .map_err(postgres_auth_error)?;
    for code_hash in code_hashes {
        sqlx::query(
            r#"
            INSERT INTO user_two_factor_recovery_codes (
                user_id, code_hash, created_at, updated_at
            ) VALUES ($1, $2, $3::timestamptz, $3::timestamptz)
            "#,
        )
        .bind(user_id)
        .bind(code_hash)
        .bind(now)
        .execute(&mut **transaction)
        .await
        .map_err(postgres_auth_error)?;
    }
    Ok(code_hashes.len())
}
