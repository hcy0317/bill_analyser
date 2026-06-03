// 中文导读：PostgreSQL auth repository helpers for authoritative HTTP paths.
// 维护重点：Postgres 模式只读取 authoritative 表与 JSONB metadata，不回落 non-Postgres。
// 不变式：用户身份、登录锁定和 profile 投影必须保持 user-scope 与前端字段。

use std::collections::HashSet;

use bill_analyser_core::{auth::recovery_code_hash_input, UserId};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgRow, types::Json, Postgres, Row};

use crate::auth::{
    ApplicationCloudSettingDraft, ApplicationCloudSettingRow, AuthLogDraft, AuthLoginUserRow,
    AuthRefreshSessionRow, AuthTokenUserRow, AuthUserProfileRow, AuthUserProfileUpdate,
    CreateTokenSessionDraft, ExternalAuthRow, LoginFailureUpdate, TokenSessionRow,
};
use crate::auth_registration::{
    RegisterDefaultSeedSummary, RegisterPresetCategory, RegisterUserDraft, RegisterUserResult,
};
use crate::taxonomy::postgres_reads::ensure_postgres_category_rule_defaults;
use crate::{DbError, DbResult, PostgresPool};

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_username_exists(pool: &PostgresPool, username: &str) -> DbResult<bool> {
    postgres_exists_by_text(pool, "username", username).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_email_exists(pool: &PostgresPool, email: &str) -> DbResult<bool> {
    postgres_exists_by_text(pool, "email", email).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_email_exists_for_other_user(
    pool: &PostgresPool,
    user_id: UserId,
    email: &str,
) -> DbResult<bool> {
    sqlx::query("SELECT 1 FROM users WHERE email = $1 AND id != $2 LIMIT 1")
        .bind(email)
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map(|row| row.is_some())
        .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_account_belongs_to_user(
    pool: &PostgresPool,
    user_id: UserId,
    account_id: i64,
) -> DbResult<bool> {
    postgres_scoped_id_exists(pool, "accounts", user_id, account_id).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_auth_category_belongs_to_user(
    pool: &PostgresPool,
    user_id: UserId,
    category_id: i64,
) -> DbResult<bool> {
    postgres_scoped_id_exists(pool, "categories", user_id, category_id).await
}

async fn postgres_exists_by_text(
    pool: &PostgresPool,
    column: &'static str,
    value: &str,
) -> DbResult<bool> {
    let sql = format!("SELECT 1 FROM users WHERE {column} = $1 LIMIT 1");
    sqlx::query(&sql)
        .bind(value)
        .fetch_optional(pool)
        .await
        .map(|row| row.is_some())
        .map_err(postgres_auth_error)
}

async fn postgres_scoped_id_exists(
    pool: &PostgresPool,
    table_name: &'static str,
    user_id: UserId,
    id: i64,
) -> DbResult<bool> {
    let sql = format!("SELECT 1 FROM {table_name} WHERE id = $1 AND user_id = $2 LIMIT 1");
    sqlx::query(&sql)
        .bind(id)
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map(|row| row.is_some())
        .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_auth_log(pool: &PostgresPool, draft: &AuthLogDraft) -> DbResult<i64> {
    let metadata = auth_log_metadata(draft);
    let row = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor, metadata, created_at
        ) VALUES ($1, 'auth', $2, $3, $4, $5, $6::timestamptz)
        RETURNING id
        "#,
    )
    .bind(draft.user_id.map(user_id_i64).transpose()?)
    .bind(
        draft
            .user_id
            .map(|value| value.get().to_string())
            .unwrap_or_else(|| draft.username.clone()),
    )
    .bind(&draft.event_type)
    .bind(&draft.username)
    .bind(Json(metadata))
    .bind(&draft.created_at)
    .fetch_one(pool)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_auth_log_under_event_limit(
    pool: &PostgresPool,
    user_id: UserId,
    event_type: &str,
    since: &str,
    limit: i64,
    draft: &AuthLogDraft,
) -> DbResult<bool> {
    if draft.user_id != Some(user_id) {
        return Err(DbError::InvalidOperation(
            "auth log user does not match event limit user".to_string(),
        ));
    }
    if draft.event_type != event_type {
        return Err(DbError::InvalidOperation(
            "auth log event type does not match event limit type".to_string(),
        ));
    }

    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'auth'
          AND action = $2
          AND created_at >= $3::timestamptz
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(event_type)
    .bind(since)
    .fetch_one(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    if count >= limit {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Ok(false);
    }

    insert_postgres_auth_log_in_transaction(&mut transaction, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn count_postgres_auth_events_since(
    pool: &PostgresPool,
    user_id: UserId,
    event_type: &str,
    window_start: &str,
) -> DbResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'auth'
          AND action = $2
          AND metadata->>'success' = 'false'
          AND created_at >= $3::timestamptz
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(event_type)
    .bind(window_start)
    .fetch_one(pool)
    .await
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_operation_password(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<String>> {
    let value: Option<Value> =
        sqlx::query_scalar("SELECT value FROM settings WHERE user_id = $1 AND key = $2")
            .bind(user_id_i64(user_id)?)
            .bind("operation_password")
            .fetch_optional(pool)
            .await
            .map_err(postgres_auth_error)?;
    Ok(value.as_ref().and_then(json_setting_string))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn set_postgres_user_email_verified(
    pool: &PostgresPool,
    user_id: UserId,
    verified: bool,
    updated_at: &str,
) -> DbResult<bool> {
    let row = sqlx::query("SELECT metadata FROM users WHERE id = $1")
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map_err(postgres_auth_error)?;
    let Some(row) = row else {
        return Ok(false);
    };
    let Json(mut metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    metadata_set_bool(&mut metadata, "email_verified", verified);
    let result = sqlx::query(
        r#"
        UPDATE users
        SET metadata = $1,
            updated_at = COALESCE(NULLIF($2, '')::timestamptz, now()),
            version = version + 1
        WHERE id = $3
        "#,
    )
    .bind(Json(metadata))
    .bind(updated_at)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map_err(postgres_auth_error)?;
    Ok(result.rows_affected() > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_user_password_hash(
    pool: &PostgresPool,
    user_id: UserId,
    password_hash: &str,
    updated_at: &str,
) -> DbResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET password_hash = $1,
            updated_at = COALESCE(NULLIF($2, '')::timestamptz, now()),
            version = version + 1
        WHERE id = $3
        "#,
    )
    .bind(password_hash)
    .bind(updated_at)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map_err(postgres_auth_error)?;
    Ok(result.rows_affected() > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn cleanup_postgres_expired_sessions(pool: &PostgresPool, now: &str) -> DbResult<u64> {
    sqlx::query(
        r#"
        DELETE FROM token_sessions
        WHERE (refresh_expires_at IS NULL AND expires_at < $1::timestamptz)
           OR (refresh_expires_at IS NOT NULL AND refresh_expires_at < $1::timestamptz)
        "#,
    )
    .bind(now)
    .execute(pool)
    .await
    .map(|result| result.rows_affected())
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_auth_token_user(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Option<AuthTokenUserRow>> {
    sqlx::query(
        r#"
        SELECT id, username, COALESCE(password_hash, '') AS password_hash
        FROM users
        WHERE id = $1
        LIMIT 1
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| {
        let raw_id: i64 = row.try_get("id").map_err(postgres_auth_error)?;
        Ok(AuthTokenUserRow {
            id: user_id_from_i64(raw_id)?,
            username: row.try_get("username").map_err(postgres_auth_error)?,
            password_hash: row.try_get("password_hash").map_err(postgres_auth_error)?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_user_sessions(
    pool: &PostgresPool,
    user_id: UserId,
) -> DbResult<Vec<TokenSessionRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, COALESCE(user_agent, '') AS user_agent,
               COALESCE(ip_address, '') AS ip_address,
               to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS') AS expires_at,
               COALESCE(to_char(last_activity_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS'), '') AS last_activity_at,
               to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS') AS created_at
        FROM token_sessions
        WHERE user_id = $1 AND is_active = TRUE
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .fetch_all(pool)
    .await
    .map_err(postgres_auth_error)?;

    rows.into_iter()
        .map(|row| {
            Ok(TokenSessionRow {
                id: row.try_get("id").map_err(postgres_auth_error)?,
                user_agent: row.try_get("user_agent").map_err(postgres_auth_error)?,
                ip_address: row.try_get("ip_address").map_err(postgres_auth_error)?,
                expires_at: row.try_get("expires_at").map_err(postgres_auth_error)?,
                last_activity_at: row
                    .try_get("last_activity_at")
                    .map_err(postgres_auth_error)?,
                created_at: row.try_get("created_at").map_err(postgres_auth_error)?,
            })
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_active_session_id_by_token_hash(
    pool: &PostgresPool,
    token_hash: &str,
) -> DbResult<Option<i64>> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM token_sessions
        WHERE token_hash = $1 AND is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_active_refresh_session(
    pool: &PostgresPool,
    refresh_token_hash: &str,
) -> DbResult<Option<AuthRefreshSessionRow>> {
    sqlx::query(
        r#"
        SELECT s.id, s.user_id, u.username,
               COALESCE(to_char(s.refresh_expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS.MS'), '') AS refresh_expires_at,
               COALESCE((u.metadata->>'is_active')::boolean, TRUE) AS user_is_active
        FROM token_sessions s
        JOIN users u ON s.user_id = u.id
        WHERE s.refresh_token_hash = $1 AND s.is_active = TRUE
        LIMIT 1
        "#,
    )
    .bind(refresh_token_hash)
    .fetch_optional(pool)
    .await
    .map_err(postgres_auth_error)?
    .map(|row| {
        let raw_user_id: i64 = row.try_get("user_id").map_err(postgres_auth_error)?;
        Ok(AuthRefreshSessionRow {
            id: row.try_get("id").map_err(postgres_auth_error)?,
            user_id: user_id_from_i64(raw_user_id)?,
            username: row.try_get("username").map_err(postgres_auth_error)?,
            refresh_expires_at: row
                .try_get("refresh_expires_at")
                .map_err(postgres_auth_error)?,
            user_is_active: row.try_get("user_is_active").map_err(postgres_auth_error)?,
        })
    })
    .transpose()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_token_session(
    pool: &PostgresPool,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let session_id = create_postgres_token_session_in_tx(&mut transaction, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(session_id)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn rotate_postgres_refresh_token_session(
    pool: &PostgresPool,
    consumed_session_id: i64,
    consumed_refresh_token_hash: &str,
    draft: &CreateTokenSessionDraft,
) -> DbResult<Option<i64>> {
    let user_id = user_id_i64(draft.user_id)?;
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let consumed = sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE,
            refresh_token_hash = NULL,
            updated_at = now(),
            version = version + 1
        WHERE id = $1
          AND user_id = $2
          AND refresh_token_hash = $3
          AND is_active = TRUE
        "#,
    )
    .bind(consumed_session_id)
    .bind(user_id)
    .bind(consumed_refresh_token_hash)
    .execute(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    if consumed.rows_affected() == 0 {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Ok(None);
    }
    let session_id = create_postgres_token_session_in_tx(&mut transaction, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(Some(session_id))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn invalidate_postgres_session_by_id(
    pool: &PostgresPool,
    session_id: i64,
    user_id: UserId,
) -> DbResult<bool> {
    sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE, updated_at = now(), version = version + 1
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(session_id)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map(|result| result.rows_affected() > 0)
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn invalidate_postgres_session_by_token_hash(
    pool: &PostgresPool,
    token_hash: &str,
) -> DbResult<bool> {
    sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE, updated_at = now(), version = version + 1
        WHERE token_hash = $1 AND is_active = TRUE
        "#,
    )
    .bind(token_hash)
    .execute(pool)
    .await
    .map(|result| result.rows_affected() > 0)
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn invalidate_other_postgres_user_sessions(
    pool: &PostgresPool,
    user_id: UserId,
    current_session_id: i64,
) -> DbResult<u64> {
    sqlx::query(
        r#"
        UPDATE token_sessions
        SET is_active = FALSE, updated_at = now(), version = version + 1
        WHERE user_id = $1 AND id != $2 AND is_active = TRUE
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(current_session_id)
    .execute(pool)
    .await
    .map(|result| result.rows_affected())
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn count_postgres_recent_token_password_failures(
    pool: &PostgresPool,
    user_id: UserId,
    since: &str,
) -> DbResult<i64> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM business_audit_events
        WHERE user_id = $1
          AND entity_type = 'auth'
          AND metadata->>'success' = 'false'
          AND action IN ('api_token_generate_failed', 'mcp_token_generate_failed')
          AND created_at >= $2::timestamptz
        "#,
    )
    .bind(user_id_i64(user_id)?)
    .bind(since)
    .fetch_one(pool)
    .await
    .map_err(postgres_auth_error)
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_auth_user_profile(
    pool: &PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
) -> DbResult<bool> {
    update_postgres_auth_user_profile_internal(pool, user_id, updates, updated_at, None).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_auth_user_profile_with_auth_log(
    pool: &PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
    auth_log: &AuthLogDraft,
) -> DbResult<bool> {
    update_postgres_auth_user_profile_internal(pool, user_id, updates, updated_at, Some(auth_log))
        .await
}

async fn update_postgres_auth_user_profile_internal(
    pool: &PostgresPool,
    user_id: UserId,
    updates: &[AuthUserProfileUpdate],
    updated_at: &str,
    auth_log: Option<&AuthLogDraft>,
) -> DbResult<bool> {
    if updates.is_empty() {
        return Ok(false);
    }
    let user_id_sql = user_id_i64(user_id)?;
    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let row =
        sqlx::query("SELECT email, display_name, metadata FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id_sql)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(postgres_auth_error)?;
    let Some(row) = row else {
        transaction.rollback().await.map_err(postgres_auth_error)?;
        return Ok(false);
    };
    let current_email: String = row.try_get("email").map_err(postgres_auth_error)?;
    let mut display_name: String = row.try_get("display_name").map_err(postgres_auth_error)?;
    let Json(mut metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    let mut email = current_email.clone();

    for update in updates {
        apply_postgres_profile_update(
            update,
            &mut email,
            &current_email,
            &mut display_name,
            &mut metadata,
        );
    }

    let changed = sqlx::query(
        r#"
        UPDATE users
        SET email = $1,
            display_name = $2,
            metadata = $3,
            updated_at = $4::timestamptz
        WHERE id = $5
        "#,
    )
    .bind(email)
    .bind(display_name)
    .bind(Json(metadata))
    .bind(updated_at)
    .bind(user_id_sql)
    .execute(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?
    .rows_affected()
        > 0;

    if changed {
        if let Some(auth_log) = auth_log {
            insert_postgres_auth_log_in_transaction(&mut transaction, auth_log).await?;
        }
    }
    transaction.commit().await.map_err(postgres_auth_error)?;
    Ok(changed)
}

fn apply_postgres_profile_update(
    update: &AuthUserProfileUpdate,
    email: &mut String,
    current_email: &str,
    display_name: &mut String,
    metadata: &mut Value,
) {
    match update {
        AuthUserProfileUpdate::Nickname(value) => {
            *display_name = value.clone();
            metadata_set_string(metadata, "nickname", value);
        }
        AuthUserProfileUpdate::Email(value) => {
            if value != current_email {
                metadata_set_bool(metadata, "email_verified", false);
            }
            *email = value.clone();
        }
        AuthUserProfileUpdate::Avatar(value) => metadata_set_string(metadata, "avatar", value),
        AuthUserProfileUpdate::Language(value) => metadata_set_string(metadata, "language", value),
        AuthUserProfileUpdate::DefaultCurrency(value) => {
            metadata_set_string(metadata, "default_currency", value);
        }
        AuthUserProfileUpdate::FirstDayOfWeek(value) => {
            metadata_set_i64(metadata, "first_day_of_week", *value);
        }
        AuthUserProfileUpdate::DefaultAccountId(value) => {
            metadata_set_optional_i64(metadata, "default_account_id", *value);
        }
        AuthUserProfileUpdate::TransactionEditScope(value) => {
            metadata_set_i64(metadata, "transaction_edit_scope", *value);
        }
        AuthUserProfileUpdate::FiscalYearStart(value) => {
            metadata_set_i64(metadata, "fiscal_year_start", *value);
        }
        AuthUserProfileUpdate::CalendarDisplayType(value) => {
            metadata_set_i64(metadata, "calendar_display_type", *value);
        }
        AuthUserProfileUpdate::DateDisplayType(value) => {
            metadata_set_i64(metadata, "date_display_type", *value);
        }
        AuthUserProfileUpdate::LongDateFormat(value) => {
            metadata_set_i64(metadata, "long_date_format", *value);
        }
        AuthUserProfileUpdate::ShortDateFormat(value) => {
            metadata_set_i64(metadata, "short_date_format", *value);
        }
        AuthUserProfileUpdate::LongTimeFormat(value) => {
            metadata_set_i64(metadata, "long_time_format", *value);
        }
        AuthUserProfileUpdate::ShortTimeFormat(value) => {
            metadata_set_i64(metadata, "short_time_format", *value);
        }
        AuthUserProfileUpdate::FiscalYearFormat(value) => {
            metadata_set_i64(metadata, "fiscal_year_format", *value);
        }
        AuthUserProfileUpdate::CurrencyDisplayType(value) => {
            metadata_set_i64(metadata, "currency_display_type", *value);
        }
        AuthUserProfileUpdate::NumeralSystem(value) => {
            metadata_set_i64(metadata, "numeral_system", *value);
        }
        AuthUserProfileUpdate::DecimalSeparator(value) => {
            metadata_set_i64(metadata, "decimal_separator", *value);
        }
        AuthUserProfileUpdate::DigitGroupingSymbol(value) => {
            metadata_set_i64(metadata, "digit_grouping_symbol", *value);
        }
        AuthUserProfileUpdate::DigitGrouping(value) => {
            metadata_set_i64(metadata, "digit_grouping", *value);
        }
        AuthUserProfileUpdate::CoordinateDisplayType(value) => {
            metadata_set_i64(metadata, "coordinate_display_type", *value);
        }
        AuthUserProfileUpdate::ExpenseAmountColor(value) => {
            metadata_set_i64(metadata, "expense_amount_color", *value);
        }
        AuthUserProfileUpdate::IncomeAmountColor(value) => {
            metadata_set_i64(metadata, "income_amount_color", *value);
        }
        AuthUserProfileUpdate::CashAccountId(value) => {
            metadata_set_optional_i64(metadata, "cash_account_id", *value);
        }
        AuthUserProfileUpdate::CashTransferCategoryId(value) => {
            metadata_set_optional_i64(metadata, "cash_transfer_category_id", *value);
        }
        AuthUserProfileUpdate::ImportLearningEnabled(value) => {
            metadata_set_bool(metadata, "import_learning_enabled", *value);
        }
        AuthUserProfileUpdate::InvestmentPlatformKeywords(value) => {
            metadata_set_string(metadata, "investment_platform_keywords", value);
        }
        AuthUserProfileUpdate::InvestmentProductKeywords(value) => {
            metadata_set_string(metadata, "investment_product_keywords", value);
        }
        AuthUserProfileUpdate::InvestmentExcludeKeywords(value) => {
            metadata_set_string(metadata, "investment_exclude_keywords", value);
        }
    }
}

async fn insert_postgres_auth_log_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    draft: &AuthLogDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO business_audit_events (
            user_id, entity_type, entity_id, action, actor, metadata, created_at
        ) VALUES ($1, 'auth', $2, $3, $4, $5, $6::timestamptz)
        RETURNING id
        "#,
    )
    .bind(draft.user_id.map(user_id_i64).transpose()?)
    .bind(
        draft
            .user_id
            .map(|value| value.get().to_string())
            .unwrap_or_else(|| draft.username.clone()),
    )
    .bind(&draft.event_type)
    .bind(&draft.username)
    .bind(Json(auth_log_metadata(draft)))
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

async fn create_postgres_token_session_in_tx(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    draft: &CreateTokenSessionDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO token_sessions (
            user_id, token_hash, refresh_token_hash, expires_at, refresh_expires_at,
            user_agent, ip_address, is_active, last_activity_at, created_at, updated_at
        ) VALUES (
            $1, $2, $3, $4::timestamptz, NULLIF($5, '')::timestamptz,
            $6, $7, TRUE, NULL, $8::timestamptz, $8::timestamptz
        )
        RETURNING id
        "#,
    )
    .bind(user_id_i64(draft.user_id)?)
    .bind(&draft.token_hash)
    .bind(&draft.refresh_token_hash)
    .bind(&draft.expires_at)
    .bind(draft.refresh_expires_at.as_deref().unwrap_or(""))
    .bind(&draft.user_agent)
    .bind(&draft.ip_address)
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

fn hash_two_factor_recovery_code(recovery_code: &str) -> Option<String> {
    recovery_code_hash_input(recovery_code)
        .map(|value| format!("{:x}", Sha256::digest(value.as_bytes())))
}

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

fn auth_log_metadata(draft: &AuthLogDraft) -> Value {
    json!({
        "username": draft.username,
        "ip_address": draft.ip_address,
        "user_agent": draft.user_agent,
        "success": draft.success,
        "error_message": draft.error_message,
        "metadata": parse_auth_metadata(draft.metadata.as_deref()),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
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

#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_user_last_login(
    pool: &PostgresPool,
    user_id: UserId,
    now: &str,
    ip_address: &str,
) -> DbResult<bool> {
    let mut metadata = load_user_metadata(pool, user_id).await?;
    metadata_set_string(&mut metadata, "last_login_at", now);
    metadata_set_string(&mut metadata, "last_login_ip", ip_address);
    metadata_set_i64(&mut metadata, "failed_login_attempts", 0);
    metadata_set_string(&mut metadata, "locked_until", "");
    update_user_metadata(pool, user_id, metadata, now).await
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn increment_postgres_failed_login(
    pool: &PostgresPool,
    user_id: UserId,
    max_attempts: i64,
    lockout_until: &str,
    expired_locked_until: Option<&str>,
) -> DbResult<LoginFailureUpdate> {
    let mut metadata = load_user_metadata(pool, user_id).await?;
    if expired_locked_until.is_some() {
        metadata_set_i64(&mut metadata, "failed_login_attempts", 0);
        metadata_set_string(&mut metadata, "locked_until", "");
    }
    let failed_attempts = metadata_i64(&metadata, &["failed_login_attempts"], 0) + 1;
    let locked = failed_attempts >= max_attempts;
    metadata_set_i64(&mut metadata, "failed_login_attempts", failed_attempts);
    if locked {
        metadata_set_string(&mut metadata, "locked_until", lockout_until);
    }
    update_user_metadata(pool, user_id, metadata, "").await?;
    Ok(LoginFailureUpdate {
        failed_attempts,
        locked,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_registered_user_with_defaults(
    pool: &PostgresPool,
    draft: &RegisterUserDraft,
    preset_categories: &[RegisterPresetCategory],
    auth_log: &AuthLogDraft,
) -> DbResult<RegisterUserResult> {
    let mut metadata = Map::new();
    metadata.insert(
        "nickname".to_string(),
        Value::String(draft.nickname.clone()),
    );
    metadata.insert(
        "language".to_string(),
        Value::String(draft.language.clone()),
    );
    metadata.insert(
        "default_currency".to_string(),
        Value::String(draft.default_currency.clone()),
    );
    metadata.insert(
        "first_day_of_week".to_string(),
        Value::from(draft.first_day_of_week),
    );
    metadata.insert(
        "email_verified".to_string(),
        Value::from(draft.email_verified),
    );
    metadata.insert("is_active".to_string(), Value::Bool(true));
    metadata.insert("import_learning_enabled".to_string(), Value::Bool(true));

    let mut transaction = pool.begin().await.map_err(postgres_auth_error)?;
    let row = sqlx::query(
        r#"
        INSERT INTO users (
            username, email, display_name, password_hash, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $6::timestamptz)
        RETURNING id
        "#,
    )
    .bind(&draft.username)
    .bind(&draft.email)
    .bind(&draft.nickname)
    .bind(&draft.password_hash)
    .bind(Json(Value::Object(metadata.clone())))
    .bind(&draft.created_at)
    .fetch_one(&mut *transaction)
    .await
    .map_err(postgres_auth_error)?;
    let user_id: i64 = row.try_get("id").map_err(postgres_auth_error)?;

    let (default_account_id, cash_account_id) =
        insert_postgres_default_accounts(&mut transaction, user_id, draft).await?;
    if let Some(default_account_id) = default_account_id {
        metadata.insert(
            "default_account_id".to_string(),
            Value::from(default_account_id),
        );
    }
    if let Some(cash_account_id) = cash_account_id {
        metadata.insert("cash_account_id".to_string(), Value::from(cash_account_id));
    }
    sqlx::query("UPDATE users SET metadata = $1, updated_at = $2::timestamptz WHERE id = $3")
        .bind(Json(Value::Object(metadata)))
        .bind(&draft.created_at)
        .bind(user_id)
        .execute(&mut *transaction)
        .await
        .map_err(postgres_auth_error)?;

    insert_postgres_preset_categories(&mut transaction, user_id, preset_categories, draft).await?;
    transaction.commit().await.map_err(postgres_auth_error)?;

    let default_seed = ensure_postgres_category_rule_defaults(pool, user_id).await?;
    create_postgres_auth_log(
        pool,
        &AuthLogDraft {
            user_id: Some(user_id_from_i64(user_id)?),
            username: auth_log.username.clone(),
            event_type: auth_log.event_type.clone(),
            ip_address: auth_log.ip_address.clone(),
            user_agent: auth_log.user_agent.clone(),
            success: auth_log.success,
            error_message: auth_log.error_message.clone(),
            metadata: auth_log.metadata.clone(),
            created_at: auth_log.created_at.clone(),
        },
    )
    .await?;

    Ok(RegisterUserResult {
        user_id,
        preset_categories_saved: !preset_categories.is_empty(),
        preset_accounts_saved: default_account_id.is_some(),
        cash_account_id,
        default_account_id,
        default_seed: RegisterDefaultSeedSummary {
            categories_created: default_seed.categories_created,
            categories_skipped: default_seed.categories_skipped,
            rules_created: default_seed.rules_created,
            rules_skipped: default_seed.rules_skipped,
            rules_missing_categories: default_seed.rules_missing_categories,
        },
    })
}

async fn insert_postgres_default_accounts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    draft: &RegisterUserDraft,
) -> DbResult<(Option<i64>, Option<i64>)> {
    let zh = draft.language.to_ascii_lowercase().starts_with("zh");
    let accounts = if zh {
        [("现金", "1", 0), ("银行卡", "2", 1)]
    } else {
        [("Cash", "1", 0), ("Bank Card", "2", 1)]
    };
    let mut created = Vec::new();
    for (name, account_type, display_order) in accounts {
        let row = sqlx::query(
            r#"
            INSERT INTO accounts (
                user_id, name, account_type, currency, balance_cents,
                is_active, display_order, metadata, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, 0, TRUE, $5, $6, $7::timestamptz, $7::timestamptz)
            ON CONFLICT (user_id, name) DO UPDATE
                SET updated_at = EXCLUDED.updated_at
            RETURNING id
            "#,
        )
        .bind(user_id)
        .bind(name)
        .bind(account_type)
        .bind(&draft.default_currency)
        .bind(display_order)
        .bind(Json(json!({ "source": "register_default" })))
        .bind(&draft.created_at)
        .fetch_one(&mut **transaction)
        .await
        .map_err(postgres_auth_error)?;
        created.push(row.try_get::<i64, _>("id").map_err(postgres_auth_error)?);
    }
    Ok((created.first().copied(), created.first().copied()))
}

async fn insert_postgres_preset_categories(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    categories: &[RegisterPresetCategory],
    draft: &RegisterUserDraft,
) -> DbResult<()> {
    for item in categories {
        let name = item.name.trim();
        if name.is_empty() {
            continue;
        }
        let parent_id =
            upsert_postgres_register_category(transaction, user_id, None, name, item, draft)
                .await?;
        for sub in &item.sub_categories {
            let sub_name = sub.name.trim();
            if sub_name.is_empty() {
                continue;
            }
            let mut sub_item = item.clone();
            sub_item.name = sub_name.to_string();
            sub_item.icon = if sub.icon.is_empty() {
                item.icon.clone()
            } else {
                sub.icon.clone()
            };
            sub_item.color = if sub.color.is_empty() {
                item.color.clone()
            } else {
                sub.color.clone()
            };
            upsert_postgres_register_category(
                transaction,
                user_id,
                Some(parent_id),
                sub_name,
                &sub_item,
                draft,
            )
            .await?;
        }
    }
    Ok(())
}

async fn upsert_postgres_register_category(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    parent_id: Option<i64>,
    name: &str,
    item: &RegisterPresetCategory,
    draft: &RegisterUserDraft,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, 0, TRUE, $8, $9::timestamptz, $9::timestamptz)
        ON CONFLICT (user_id, parent_id, name) DO UPDATE
            SET icon = EXCLUDED.icon,
                color = EXCLUDED.color,
                updated_at = EXCLUDED.updated_at
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(name)
    .bind(item.type_code.to_string())
    .bind(name)
    .bind(&item.icon)
    .bind(&item.color)
    .bind(Json(json!({ "source": "register_preset" })))
    .bind(&draft.created_at)
    .fetch_one(&mut **transaction)
    .await
    .map_err(postgres_auth_error)?;
    row.try_get("id").map_err(postgres_auth_error)
}

async fn load_user_metadata(pool: &PostgresPool, user_id: UserId) -> DbResult<Value> {
    let row = sqlx::query("SELECT metadata FROM users WHERE id = $1")
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await
        .map_err(postgres_auth_error)?
        .ok_or_else(|| DbError::InvalidOperation("Postgres auth user not found".to_string()))?;
    let Json(metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    Ok(metadata)
}

async fn update_user_metadata(
    pool: &PostgresPool,
    user_id: UserId,
    metadata: Value,
    updated_at: &str,
) -> DbResult<bool> {
    let result = sqlx::query(
        r#"
        UPDATE users
        SET metadata = $1,
            updated_at = COALESCE(NULLIF($2, '')::timestamptz, now())
        WHERE id = $3
        "#,
    )
    .bind(Json(metadata))
    .bind(updated_at)
    .bind(user_id_i64(user_id)?)
    .execute(pool)
    .await
    .map_err(postgres_auth_error)?;
    Ok(result.rows_affected() > 0)
}

fn postgres_login_user_from_row(row: &PgRow) -> DbResult<AuthLoginUserRow> {
    let profile = postgres_profile_from_row(row)?;
    let Json(metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    Ok(AuthLoginUserRow {
        profile,
        password_hash: row.try_get("password_hash").map_err(postgres_auth_error)?,
        is_active: metadata_bool(&metadata, &["is_active"], true),
        two_factor_enabled: metadata_bool(&metadata, &["two_factor_enabled"], false),
        two_factor_secret: metadata_string(&metadata, &["two_factor_secret"], ""),
        failed_login_attempts: metadata_i64(&metadata, &["failed_login_attempts"], 0),
        locked_until: metadata_string(&metadata, &["locked_until"], ""),
    })
}

fn postgres_profile_from_row(row: &PgRow) -> DbResult<AuthUserProfileRow> {
    let raw_id: i64 = row.try_get("id").map_err(postgres_auth_error)?;
    let user_id = user_id_from_i64(raw_id)?;
    let username: String = row.try_get("username").map_err(postgres_auth_error)?;
    let email: String = row.try_get("email").map_err(postgres_auth_error)?;
    let display_name: String = row.try_get("display_name").map_err(postgres_auth_error)?;
    let Json(metadata): Json<Value> = row.try_get("metadata").map_err(postgres_auth_error)?;
    let nickname = if display_name.trim().is_empty() {
        metadata_string(&metadata, &["nickname"], &username)
    } else {
        display_name
    };
    Ok(AuthUserProfileRow {
        id: user_id,
        username,
        email,
        nickname,
        avatar: metadata_string(&metadata, &["avatar"], ""),
        default_account_id: metadata_optional_i64(&metadata, &["default_account_id"]),
        transaction_edit_scope: metadata_i64(&metadata, &["transaction_edit_scope"], 0),
        language: metadata_string(&metadata, &["language"], "zh_Hans"),
        default_currency: metadata_string(&metadata, &["default_currency"], "CNY"),
        first_day_of_week: metadata_i64(&metadata, &["first_day_of_week"], 1),
        fiscal_year_start: metadata_i64(&metadata, &["fiscal_year_start"], 1),
        calendar_display_type: metadata_i64(&metadata, &["calendar_display_type"], 0),
        date_display_type: metadata_i64(&metadata, &["date_display_type"], 0),
        long_date_format: metadata_i64(&metadata, &["long_date_format"], 0),
        short_date_format: metadata_i64(&metadata, &["short_date_format"], 0),
        long_time_format: metadata_i64(&metadata, &["long_time_format"], 0),
        short_time_format: metadata_i64(&metadata, &["short_time_format"], 0),
        fiscal_year_format: metadata_i64(&metadata, &["fiscal_year_format"], 0),
        currency_display_type: metadata_i64(&metadata, &["currency_display_type"], 0),
        numeral_system: metadata_i64(&metadata, &["numeral_system"], 0),
        decimal_separator: metadata_i64(&metadata, &["decimal_separator"], 0),
        digit_grouping_symbol: metadata_i64(&metadata, &["digit_grouping_symbol"], 0),
        digit_grouping: metadata_i64(&metadata, &["digit_grouping"], 0),
        coordinate_display_type: metadata_i64(&metadata, &["coordinate_display_type"], 0),
        expense_amount_color: metadata_i64(&metadata, &["expense_amount_color"], 0),
        income_amount_color: metadata_i64(&metadata, &["income_amount_color"], 0),
        cash_account_id: metadata_optional_i64(&metadata, &["cash_account_id"]),
        cash_transfer_category_id: metadata_optional_i64(&metadata, &["cash_transfer_category_id"]),
        import_learning_enabled: metadata_bool(&metadata, &["import_learning_enabled"], true),
        investment_platform_keywords: metadata_optional_string(
            &metadata,
            &["investment_platform_keywords"],
        ),
        investment_product_keywords: metadata_optional_string(
            &metadata,
            &["investment_product_keywords"],
        ),
        investment_exclude_keywords: metadata_optional_string(
            &metadata,
            &["investment_exclude_keywords"],
        ),
        email_verified: metadata_bool(&metadata, &["email_verified"], false),
    })
}

fn postgres_external_auth_from_row(row: &PgRow) -> DbResult<ExternalAuthRow> {
    Ok(ExternalAuthRow {
        external_auth_category: row
            .try_get("external_auth_category")
            .map_err(postgres_auth_error)?,
        external_auth_type: row
            .try_get("external_auth_type")
            .map_err(postgres_auth_error)?,
        external_username: row
            .try_get("external_username")
            .map_err(postgres_auth_error)?,
        created_at: row.try_get("created_at").map_err(postgres_auth_error)?,
    })
}

fn metadata_string(metadata: &Value, keys: &[&str], default: &str) -> String {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::String(text) => Some(text.trim().to_string()),
            Value::Number(number) => Some(number.to_string()),
            Value::Bool(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn metadata_optional_string(metadata: &Value, keys: &[&str]) -> Option<String> {
    let value = metadata_string(metadata, keys, "");
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn metadata_i64(metadata: &Value, keys: &[&str], default: i64) -> i64 {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.trim().parse().ok(),
            Value::Bool(value) => Some(i64::from(*value)),
            _ => None,
        })
        .unwrap_or(default)
}

fn metadata_optional_i64(metadata: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => {
                let text = text.trim();
                if text.is_empty() {
                    None
                } else {
                    text.parse().ok()
                }
            }
            _ => None,
        })
}

fn metadata_bool(metadata: &Value, keys: &[&str], default: bool) -> bool {
    keys.iter()
        .find_map(|key| metadata.get(*key))
        .and_then(|value| match value {
            Value::Bool(value) => Some(*value),
            Value::Number(number) => number.as_i64().map(|value| value != 0),
            Value::String(text) => match text.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" => Some(true),
                "0" | "false" | "no" => Some(false),
                _ => None,
            },
            _ => None,
        })
        .unwrap_or(default)
}

fn metadata_set_string(metadata: &mut Value, key: &str, value: &str) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::String(value.to_string()));
}

fn metadata_set_i64(metadata: &mut Value, key: &str, value: i64) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::from(value));
}

fn metadata_set_bool(metadata: &mut Value, key: &str, value: bool) {
    ensure_metadata_object(metadata).insert(key.to_string(), Value::from(value));
}

fn metadata_set_optional_i64(metadata: &mut Value, key: &str, value: Option<i64>) {
    let object = ensure_metadata_object(metadata);
    if let Some(value) = value {
        object.insert(key.to_string(), Value::from(value));
    } else {
        object.remove(key);
    }
}

fn ensure_metadata_object(metadata: &mut Value) -> &mut Map<String, Value> {
    if !metadata.is_object() {
        *metadata = Value::Object(Map::new());
    }
    metadata.as_object_mut().expect("metadata object")
}

fn parse_auth_metadata(raw: Option<&str>) -> Value {
    raw.and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or(Value::Null)
}

fn json_value_to_setting_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn json_setting_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get()).map_err(|_| {
        DbError::InvalidOperation("user id is outside PostgreSQL BIGINT range".to_string())
    })
}

fn user_id_from_i64(raw_id: i64) -> DbResult<UserId> {
    let raw_id = u64::try_from(raw_id)
        .map_err(|_| DbError::InvalidOperation("Postgres user id must be positive".to_string()))?;
    UserId::new(raw_id)
        .map_err(|_| DbError::InvalidOperation("Postgres user id must be non-zero".to_string()))
}

fn postgres_auth_error(error: sqlx::Error) -> DbError {
    DbError::InvalidOperation(format!("postgres auth error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_helpers_preserve_postgres_json_edges() {
        let metadata = json!({
            "name_string": " Alice ",
            "name_number": 42,
            "name_bool": true,
            "empty": " ",
            "int_number": 7,
            "int_string": "8",
            "int_bool": true,
            "optional_empty": "",
            "bool_true": "yes",
            "bool_false": 0,
            "bool_invalid": "maybe"
        });

        assert_eq!(
            metadata_string(&metadata, &["missing", "name_string"], "fallback"),
            "Alice"
        );
        assert_eq!(
            metadata_string(&metadata, &["name_number"], "fallback"),
            "42"
        );
        assert_eq!(
            metadata_string(&metadata, &["name_bool"], "fallback"),
            "true"
        );
        assert_eq!(
            metadata_string(&metadata, &["empty"], "fallback"),
            "fallback"
        );
        assert_eq!(
            metadata_optional_string(&metadata, &["name_string"]),
            Some("Alice".to_string())
        );
        assert_eq!(metadata_optional_string(&metadata, &["missing"]), None);
        assert_eq!(metadata_i64(&metadata, &["int_number"], 1), 7);
        assert_eq!(metadata_i64(&metadata, &["int_string"], 1), 8);
        assert_eq!(metadata_i64(&metadata, &["int_bool"], 1), 1);
        assert_eq!(metadata_i64(&metadata, &["missing"], 9), 9);
        assert_eq!(metadata_optional_i64(&metadata, &["int_string"]), Some(8));
        assert_eq!(metadata_optional_i64(&metadata, &["optional_empty"]), None);
        assert!(metadata_bool(&metadata, &["bool_true"], false));
        assert!(!metadata_bool(&metadata, &["bool_false"], true));
        assert!(metadata_bool(&metadata, &["bool_invalid"], true));
    }

    #[test]
    fn metadata_mutation_and_json_helpers_cover_non_object_edges() {
        let mut metadata = Value::Null;
        metadata_set_string(&mut metadata, "last_login_ip", "127.0.0.1");
        metadata_set_i64(&mut metadata, "failed_login_attempts", 3);
        metadata_set_bool(&mut metadata, "email_verified", false);
        metadata_set_optional_i64(&mut metadata, "default_account_id", Some(42));
        metadata_set_optional_i64(&mut metadata, "cash_account_id", None);

        assert_eq!(metadata["last_login_ip"], "127.0.0.1");
        assert_eq!(metadata["failed_login_attempts"], 3);
        assert_eq!(metadata["email_verified"], false);
        assert_eq!(metadata["default_account_id"], 42);
        assert!(metadata.get("cash_account_id").is_none());
        assert_eq!(
            parse_auth_metadata(Some(r#"{"reason":"failed"}"#))["reason"],
            "failed"
        );
        assert_eq!(parse_auth_metadata(Some("not-json")), Value::Null);
        assert_eq!(parse_auth_metadata(None), Value::Null);
        assert_eq!(
            json_value_to_setting_string(&Value::String("enabled".to_string())),
            "enabled"
        );
        assert_eq!(json_value_to_setting_string(&Value::Null), "");
        assert_eq!(
            json_value_to_setting_string(&json!({"enabled": true})),
            "{\"enabled\":true}"
        );
    }

    #[test]
    fn profile_update_application_covers_all_postgres_metadata_variants() {
        let mut email = "old@example.test".to_string();
        let current_email = email.clone();
        let mut display_name = "Old".to_string();
        let mut metadata = json!({ "email_verified": true });
        let updates = [
            AuthUserProfileUpdate::Nickname("New".to_string()),
            AuthUserProfileUpdate::Email("new@example.test".to_string()),
            AuthUserProfileUpdate::Avatar("data:image/png;base64,avatar".to_string()),
            AuthUserProfileUpdate::Language("en".to_string()),
            AuthUserProfileUpdate::DefaultCurrency("USD".to_string()),
            AuthUserProfileUpdate::FirstDayOfWeek(0),
            AuthUserProfileUpdate::DefaultAccountId(Some(10)),
            AuthUserProfileUpdate::TransactionEditScope(2),
            AuthUserProfileUpdate::FiscalYearStart(4),
            AuthUserProfileUpdate::CalendarDisplayType(1),
            AuthUserProfileUpdate::DateDisplayType(2),
            AuthUserProfileUpdate::LongDateFormat(3),
            AuthUserProfileUpdate::ShortDateFormat(4),
            AuthUserProfileUpdate::LongTimeFormat(5),
            AuthUserProfileUpdate::ShortTimeFormat(6),
            AuthUserProfileUpdate::FiscalYearFormat(7),
            AuthUserProfileUpdate::CurrencyDisplayType(8),
            AuthUserProfileUpdate::NumeralSystem(9),
            AuthUserProfileUpdate::DecimalSeparator(10),
            AuthUserProfileUpdate::DigitGroupingSymbol(11),
            AuthUserProfileUpdate::DigitGrouping(12),
            AuthUserProfileUpdate::CoordinateDisplayType(13),
            AuthUserProfileUpdate::ExpenseAmountColor(14),
            AuthUserProfileUpdate::IncomeAmountColor(15),
            AuthUserProfileUpdate::CashAccountId(Some(16)),
            AuthUserProfileUpdate::CashTransferCategoryId(Some(17)),
            AuthUserProfileUpdate::ImportLearningEnabled(false),
            AuthUserProfileUpdate::InvestmentPlatformKeywords("[\"ETF\"]".to_string()),
            AuthUserProfileUpdate::InvestmentProductKeywords("[\"Fund\"]".to_string()),
            AuthUserProfileUpdate::InvestmentExcludeKeywords("[\"Exclude\"]".to_string()),
        ];

        for update in &updates {
            apply_postgres_profile_update(
                update,
                &mut email,
                &current_email,
                &mut display_name,
                &mut metadata,
            );
        }

        assert_eq!(email, "new@example.test");
        assert_eq!(display_name, "New");
        assert_eq!(metadata["nickname"], "New");
        assert_eq!(metadata["email_verified"], false);
        assert_eq!(metadata["avatar"], "data:image/png;base64,avatar");
        assert_eq!(metadata["language"], "en");
        assert_eq!(metadata["default_currency"], "USD");
        assert_eq!(metadata["first_day_of_week"], 0);
        assert_eq!(metadata["default_account_id"], 10);
        assert_eq!(metadata["transaction_edit_scope"], 2);
        assert_eq!(metadata["fiscal_year_start"], 4);
        assert_eq!(metadata["calendar_display_type"], 1);
        assert_eq!(metadata["date_display_type"], 2);
        assert_eq!(metadata["long_date_format"], 3);
        assert_eq!(metadata["short_date_format"], 4);
        assert_eq!(metadata["long_time_format"], 5);
        assert_eq!(metadata["short_time_format"], 6);
        assert_eq!(metadata["fiscal_year_format"], 7);
        assert_eq!(metadata["currency_display_type"], 8);
        assert_eq!(metadata["numeral_system"], 9);
        assert_eq!(metadata["decimal_separator"], 10);
        assert_eq!(metadata["digit_grouping_symbol"], 11);
        assert_eq!(metadata["digit_grouping"], 12);
        assert_eq!(metadata["coordinate_display_type"], 13);
        assert_eq!(metadata["expense_amount_color"], 14);
        assert_eq!(metadata["income_amount_color"], 15);
        assert_eq!(metadata["cash_account_id"], 16);
        assert_eq!(metadata["cash_transfer_category_id"], 17);
        assert_eq!(metadata["import_learning_enabled"], false);
        assert_eq!(metadata["investment_platform_keywords"], "[\"ETF\"]");
        assert_eq!(metadata["investment_product_keywords"], "[\"Fund\"]");
        assert_eq!(metadata["investment_exclude_keywords"], "[\"Exclude\"]");

        for update in [
            AuthUserProfileUpdate::DefaultAccountId(None),
            AuthUserProfileUpdate::CashAccountId(None),
            AuthUserProfileUpdate::CashTransferCategoryId(None),
        ] {
            apply_postgres_profile_update(
                &update,
                &mut email,
                &current_email,
                &mut display_name,
                &mut metadata,
            );
        }
        assert!(metadata.get("default_account_id").is_none());
        assert!(metadata.get("cash_account_id").is_none());
        assert!(metadata.get("cash_transfer_category_id").is_none());

        assert!(matches!(
            postgres_auth_error(sqlx::Error::RowNotFound),
            DbError::InvalidOperation(message) if message.contains("postgres auth error")
        ));
    }

    #[tokio::test]
    async fn empty_profile_update_short_circuits_before_postgres_io() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
            .expect("lazy postgres pool");

        let changed = update_postgres_auth_user_profile(
            &pool,
            UserId::new(1).expect("user id"),
            &[],
            "2026-01-01T00:00:00Z",
        )
        .await
        .expect("empty updates do not touch postgres");

        assert!(!changed);
    }

    #[test]
    fn postgres_user_id_conversion_rejects_invalid_bigint_boundaries() {
        assert!(user_id_from_i64(-1).is_err());
        assert!(user_id_from_i64(0).is_err());
        assert_eq!(user_id_from_i64(42).unwrap().get(), 42);
        assert!(user_id_i64(UserId::new(u64::MAX).unwrap()).is_err());
    }
}
