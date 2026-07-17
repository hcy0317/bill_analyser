//! PostgreSQL repository for user-scoped import mapping templates.

use bill_analyser_core::{
    normalize_import_config_format, normalize_import_config_headers, ImportConfigDraft,
    ImportConfigDto, ImportConfigMatchDto, ImportConfigValidationError, UserId,
};
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, Row};
use thiserror::Error;

use crate::{DbError, PostgresPool, UserScope};

#[derive(Debug, Error)]
pub enum ImportConfigRepositoryError {
    #[error("invalid import config: {0}")]
    Validation(#[from] ImportConfigValidationError),
    #[error("import config not found")]
    NotFound,
    #[error("import config already exists")]
    Conflict,
    #[error(transparent)]
    Database(#[from] DbError),
}

impl From<sqlx::Error> for ImportConfigRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(DbError::Postgres(error))
    }
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn list_postgres_import_configs(
    pool: &PostgresPool,
    user_id: UserId,
    file_format: &str,
) -> Result<Vec<ImportConfigDto>, ImportConfigRepositoryError> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let file_format = normalize_import_config_format(file_format)?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, file_format, description, field_mappings, sample_headers,
               date_format, delimiter, encoding, skip_rows, has_header, custom_rules,
               is_default, created_at, updated_at
        FROM import_configs
        WHERE user_id = $1 AND file_format = $2
        ORDER BY is_default DESC, updated_at DESC, id ASC
        "#,
    )
    .bind(user_id)
    .bind(file_format)
    .fetch_all(pool)
    .await?;
    rows.iter().map(import_config_from_row).collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn save_postgres_import_config(
    pool: &PostgresPool,
    user_id: UserId,
    draft: &ImportConfigDraft,
) -> Result<i64, ImportConfigRepositoryError> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let draft = draft.validate_and_normalize()?;
    let mut transaction = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!("import-config:{user_id}:{}", draft.file_format))
        .execute(&mut *transaction)
        .await?;

    if let Some(id) = draft.id {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM import_configs WHERE id = $1 AND user_id = $2 FOR UPDATE",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        if exists.is_none() {
            return Err(ImportConfigRepositoryError::NotFound);
        }
    }

    if draft.is_default {
        sqlx::query(
            "UPDATE import_configs SET is_default = FALSE, updated_at = now() WHERE user_id = $1 AND file_format = $2 AND is_default",
        )
        .bind(user_id)
        .bind(&draft.file_format)
        .execute(&mut *transaction)
        .await?;
    }

    let save_result = match draft.id {
        Some(id) => {
            sqlx::query_scalar::<_, i64>(
                r#"
                UPDATE import_configs
                SET name = $3, file_format = $4, description = $5, field_mappings = $6,
                    sample_headers = $7, date_format = $8, delimiter = $9, encoding = $10,
                    skip_rows = $11, has_header = $12, custom_rules = $13,
                    is_default = $14, updated_at = now()
                WHERE id = $1 AND user_id = $2
                RETURNING id
                "#,
            )
            .bind(id)
            .bind(user_id)
            .bind(&draft.name)
            .bind(&draft.file_format)
            .bind(&draft.description)
            .bind(&draft.field_mappings)
            .bind(Value::Array(
                draft
                    .sample_headers
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ))
            .bind(&draft.date_format)
            .bind(&draft.delimiter)
            .bind(&draft.encoding)
            .bind(draft.skip_rows)
            .bind(draft.has_header)
            .bind(&draft.custom_rules)
            .bind(draft.is_default)
            .fetch_one(&mut *transaction)
            .await
        }
        None => {
            sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO import_configs (
                    user_id, name, file_format, description, field_mappings, sample_headers,
                    date_format, delimiter, encoding, skip_rows, has_header, custom_rules,
                    is_default
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                RETURNING id
                "#,
            )
            .bind(user_id)
            .bind(&draft.name)
            .bind(&draft.file_format)
            .bind(&draft.description)
            .bind(&draft.field_mappings)
            .bind(Value::Array(
                draft
                    .sample_headers
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ))
            .bind(&draft.date_format)
            .bind(&draft.delimiter)
            .bind(&draft.encoding)
            .bind(draft.skip_rows)
            .bind(draft.has_header)
            .bind(&draft.custom_rules)
            .bind(draft.is_default)
            .fetch_one(&mut *transaction)
            .await
        }
    };
    let id = save_result.map_err(map_save_error)?;
    transaction.commit().await?;
    Ok(id)
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn match_postgres_import_config(
    pool: &PostgresPool,
    user_id: UserId,
    file_format: &str,
    headers: &[String],
) -> Result<Option<ImportConfigMatchDto>, ImportConfigRepositoryError> {
    let normalized_headers = normalize_import_config_headers(headers)?;
    let configs = list_postgres_import_configs(pool, user_id, file_format).await?;
    if let Some(exact) = configs.iter().find(|config| {
        !config.sample_headers.is_empty()
            && normalize_import_config_headers(&config.sample_headers)
                .is_ok_and(|stored| stored == normalized_headers)
    }) {
        return Ok(Some(ImportConfigMatchDto::exact(exact.clone())));
    }
    Ok(configs
        .into_iter()
        .find(|config| config.is_default)
        .map(ImportConfigMatchDto::default_fallback))
}

#[tracing::instrument(level = "debug", skip_all)]
pub async fn delete_postgres_import_config(
    pool: &PostgresPool,
    user_id: UserId,
    id: i64,
) -> Result<i64, ImportConfigRepositoryError> {
    if id <= 0 {
        return Err(ImportConfigValidationError::InvalidId.into());
    }
    let user_id = UserScope::new(user_id).bind_value()?;
    sqlx::query_scalar::<_, i64>(
        "DELETE FROM import_configs WHERE id = $1 AND user_id = $2 RETURNING id",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(ImportConfigRepositoryError::NotFound)
}

fn import_config_from_row(row: &PgRow) -> Result<ImportConfigDto, ImportConfigRepositoryError> {
    let sample_headers: Value = row.try_get("sample_headers")?;
    let sample_headers = serde_json::from_value(sample_headers).map_err(|error| {
        DbError::InvalidOperation(format!("invalid import config sample_headers: {error}"))
    })?;
    Ok(ImportConfigDto {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        file_format: row.try_get("file_format")?,
        description: row.try_get("description")?,
        field_mappings: row.try_get("field_mappings")?,
        sample_headers,
        date_format: row.try_get("date_format")?,
        delimiter: row.try_get("delimiter")?,
        encoding: row.try_get("encoding")?,
        skip_rows: row.try_get("skip_rows")?,
        has_header: row.try_get("has_header")?,
        custom_rules: row.try_get("custom_rules")?,
        is_default: row.try_get("is_default")?,
        created_at: timestamp_text(row, "created_at")?,
        updated_at: timestamp_text(row, "updated_at")?,
    })
}

fn timestamp_text(row: &PgRow, column: &str) -> Result<String, ImportConfigRepositoryError> {
    Ok(row
        .try_get::<DateTime<Utc>, _>(column)?
        .to_rfc3339_opts(SecondsFormat::Micros, true))
}

fn map_save_error(error: sqlx::Error) -> ImportConfigRepositoryError {
    match &error {
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("23505") =>
        {
            ImportConfigRepositoryError::Conflict
        }
        _ => error.into(),
    }
}
