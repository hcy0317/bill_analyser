// 中文导读：Postgres import staging adapter，负责当前 import v2 session、source、standard row、preview、confirm、LLM memory 与学习反馈写读。
// 维护重点：只实现 Postgres 当前运行态；不读取历史 non-Postgres 数据，不提供历史 staging schema 路径。
// 不变式：外部 session key 映射到 Postgres import_sessions.id，所有查询必须 user scoped。

use std::future::Future;

use bill_analyser_core::{
    build_transfer_source_snapshot, learning_lifecycle_is_auto_eligible,
    learning_lifecycle_signal_state, normalize_bill_date_text,
    transition_import_learning_lifecycle, DedupBill, DeduplicationType,
    ImportLearningLifecycleState, Money, TransferSourceSnapshot, UserId,
};
use bill_analyser_parsers::{parser_source_label, serialize_parser_tags, StandardBill};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{postgres::PgRow, Postgres, QueryBuilder, Row};

use crate::{create_postgres_bill, BillCreateDraft, BillRecord, DbError, DbResult, PostgresPool};

const LLM_MEMORY_PROMPT_TEXT_MAX_BYTES: usize = 16_384;
const IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE: usize = 500;

include!("import_staging/types.rs");
include!("import_staging/ledger_types.rs");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearSessionDataResult {
    pub parser_count: usize,
    pub preview_count: usize,
    pub annotation_count: usize,
    pub session_count: usize,
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn init_import_staging_schema(_pool: &PostgresPool) -> DbResult<()> {
    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn create_import_session(pool: &PostgresPool, draft: &ImportSessionDraft) -> DbResult<i64> {
    block_on_db(async move {
        let user_id = user_id_i64(draft.user_id)?;
        clear_user_import_staging_data_async(pool, user_id).await?;
        create_import_session_async(pool, draft).await
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn clear_user_import_staging_data(pool: &PostgresPool, user_id: i64) -> DbResult<usize> {
    block_on_db(clear_user_import_staging_data_async(pool, user_id))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_import_session_status(
    pool: &PostgresPool,
    update: &ImportSessionStatusUpdate,
) -> DbResult<bool> {
    block_on_db(async move {
        let user_id = user_id_i64(update.user_id)?;
        let changed = sqlx::query(
            r#"
            UPDATE import_sessions
            SET status = $1,
                updated_at = now(),
                total_parsed = COALESCE($2, total_parsed),
                total_preview = COALESCE($3, total_preview),
                total_confirmed = COALESCE($4, total_confirmed),
                row_count = COALESCE($2, row_count),
                version = version + 1
            WHERE session_key = $5 AND user_id = $6
            "#,
        )
        .bind(&update.status)
        .bind(update.total_parsed)
        .bind(update.total_preview)
        .bind(update.total_confirmed)
        .bind(&update.session_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(changed > 0)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Option<ImportSessionRow>> {
    block_on_db(get_import_session_async(pool, session_id, user_id))
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn stage_import_parser_templates_with_sources(
    pool: &PostgresPool,
    draft: &ImportSessionDraft,
    parser_drafts: &[ImportParserTemplateDraft],
    source_drafts: &[ImportSourceDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    require_existing_session: bool,
) -> DbResult<ImportParseStagingResult> {
    block_on_db(async move {
        let user_id = user_id_i64(draft.user_id)?;
        let session = if require_existing_session {
            get_import_session_async(pool, &draft.session_id, draft.user_id)
                .await?
                .map(|session| session.id)
        } else {
            clear_user_import_staging_data_async(pool, user_id).await?;
            Some(create_import_session_async(pool, draft).await?)
        };
        let Some(session_db_id) = session else {
            return Ok(ImportParseStagingResult {
                inserted_count: 0,
                total_parsed: 0,
                session_found: false,
            });
        };
        let sources = if source_drafts.is_empty() {
            vec![ImportSourceDraft {
                source_index: 0,
                original_file_name: "inline-standard-bills".to_string(),
                parser_id: parser_drafts
                    .first()
                    .map(|draft| draft.parser_id.clone())
                    .unwrap_or_else(|| "auto".to_string()),
                parser_name: "auto".to_string(),
                parser_signal: "provided".to_string(),
                parser_confidence: 1.0,
                feature_signature: format!("{}:inline", draft.session_id),
                metadata: json!({}),
            }]
        } else {
            source_drafts.to_vec()
        };
        let source_ids = upsert_import_sources(pool, session_db_id, user_id, &sources).await?;
        let inserted = insert_standard_rows_and_parser_payloads(
            pool,
            session_db_id,
            user_id,
            parser_drafts,
            standard_row_drafts,
            &source_ids,
        )
        .await?;
        sqlx::query(
            r#"
            UPDATE import_sessions
            SET row_count = row_count + $1,
                total_parsed = total_parsed + $1,
                total_preview = total_preview,
                updated_at = now(),
                version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(i64::try_from(inserted).unwrap_or(i64::MAX))
        .bind(session_db_id)
        .bind(user_id)
        .execute(pool)
        .await?;
        let total_parsed = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*)::BIGINT FROM import_standard_rows WHERE session_id = $1 AND user_id = $2",
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_one(pool)
        .await?;
        Ok(ImportParseStagingResult {
            inserted_count: inserted,
            total_parsed,
            session_found: true,
        })
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn stage_import_parser_templates(
    pool: &PostgresPool,
    draft: &ImportSessionDraft,
    parser_drafts: &[ImportParserTemplateDraft],
    require_existing_session: bool,
) -> DbResult<ImportParseStagingResult> {
    stage_import_parser_templates_with_sources(
        pool,
        draft,
        parser_drafts,
        &[],
        &[],
        require_existing_session,
    )
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_preview_bill(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        insert_preview_row_async(pool, session_db_id, user_id, draft).await
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_preview_bills_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        if drafts.is_empty() {
            return Ok(0);
        }
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        insert_preview_rows_batch_async(pool, session_db_id, user_id, drafts).await?;
        update_session_preview_count(pool, session_db_id, user_id).await?;
        Ok(drafts.len())
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_unprocessed_templates_for_dedup(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT r.*, s.session_key, src.parser_id AS source_parser_id
            FROM import_standard_rows r
            JOIN import_sessions s ON s.id = r.session_id
            LEFT JOIN import_sources src ON src.id = r.source_id
            WHERE r.session_id = $1
              AND r.user_id = $2
              AND COALESCE((r.parser_payload->>'parser_is_processed')::boolean, false) = false
            ORDER BY r.occurred_at ASC, r.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter().map(parser_template_from_pg_row).collect()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn mark_unprocessed_parser_templates_processed_for_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let changed = sqlx::query(
            r#"
            UPDATE import_standard_rows
            SET parser_payload = jsonb_set(parser_payload, '{parser_is_processed}', 'true'::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE session_id = $1 AND user_id = $2
              AND COALESCE((parser_payload->>'parser_is_processed')::boolean, false) = false
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_parser_template_status(
    pool: &PostgresPool,
    template_id: i64,
    user_id: UserId,
    processed: bool,
) -> DbResult<bool> {
    block_on_db(async move {
        let changed = sqlx::query(
            r#"
            UPDATE import_standard_rows
            SET parser_payload = jsonb_set(parser_payload, '{parser_is_processed}', $1::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE id = $2 AND user_id = $3
            "#,
        )
        .bind(Value::Bool(processed).to_string())
        .bind(template_id)
        .bind(user_id_i64(user_id)?)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(changed > 0)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_parser_templates_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT r.*, s.session_key, src.parser_id AS source_parser_id
            FROM import_standard_rows r
            JOIN import_sessions s ON s.id = r.session_id
            LEFT JOIN import_sources src ON src.id = r.source_id
            WHERE r.session_id = $1 AND r.user_id = $2
            ORDER BY r.occurred_at ASC, r.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        rows.iter().map(parser_template_from_pg_row).collect()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_parser_template(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    draft: &ImportParserTemplateDraft,
) -> DbResult<i64> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let source_id =
            ensure_default_source(pool, session_db_id, user_id_i64, &draft.parser_id).await?;
        insert_standard_row_from_parser_template(
            pool,
            session_db_id,
            source_id,
            user_id_i64,
            0,
            draft,
        )
        .await
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn insert_parser_templates_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportParserTemplateDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        if drafts.is_empty() {
            return Ok(0);
        }
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let source_id = ensure_default_source(
            pool,
            session_db_id,
            user_id_i64,
            drafts
                .first()
                .map(|draft| draft.parser_id.as_str())
                .unwrap_or("auto"),
        )
        .await?;
        let rows = standard_row_batch_values_from_parser_templates(source_id, drafts);
        insert_standard_rows_batch_async(pool, session_db_id, user_id_i64, &rows).await?;
        Ok(drafts.len())
    })
}

fn standard_row_batch_values_from_parser_templates(
    source_id: i64,
    drafts: &[ImportParserTemplateDraft],
) -> Vec<StandardRowBatchValue> {
    drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            standard_row_batch_value_from_parser_template(
                source_id,
                i64::try_from(index).unwrap_or(i64::MAX),
                draft,
            )
        })
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_sources_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportSourceRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT src.*, s.session_key
            FROM import_sources src
            JOIN import_sessions s ON s.id = src.session_id
            WHERE src.session_id = $1 AND src.user_id = $2
            ORDER BY src.source_index ASC, src.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64(user_id)?)
        .fetch_all(pool)
        .await?;
        rows.iter().map(import_source_from_pg_row).collect()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_import_standard_rows_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportStandardRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let rows = sqlx::query(
            r#"
            SELECT r.*, s.session_key, src.source_index, src.parser_id
            FROM import_standard_rows r
            JOIN import_sessions s ON s.id = r.session_id
            JOIN import_sources src ON src.id = r.source_id
            WHERE r.session_id = $1 AND r.user_id = $2
            ORDER BY src.source_index ASC, r.source_row_index ASC, r.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64(user_id)?)
        .fetch_all(pool)
        .await?;
        rows.iter().map(import_standard_row_from_pg_row).collect()
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_preview_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        load_preview_rows(pool, session_db_id, user_id_i64(user_id)?, selected_only).await
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn count_preview_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<i64> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE session_id = ",
        );
        query.push_bind(session_db_id);
        query.push(" AND user_id = ");
        query.push_bind(user_id_i64(user_id)?);
        if selected_only {
            query.push(" AND selected = true");
        }
        query
            .build()
            .fetch_one(pool)
            .await?
            .try_get(0)
            .map_err(DbError::from)
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn get_preview_page_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    page: i64,
    page_size: i64,
    selected_only: bool,
) -> DbResult<(Vec<ImportPreviewRow>, i64)> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let selected_clause = if selected_only {
            " AND selected = true"
        } else {
            ""
        };
        let total = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COUNT(*)::BIGINT FROM import_preview_rows WHERE session_id = $1 AND user_id = $2{selected_clause}"
        ))
        .bind(session_db_id)
        .bind(user_id_i64)
        .fetch_one(pool)
        .await?;
        let rows = sqlx::query(&format!(
            "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = $1 AND p.user_id = $2{selected_clause} ORDER BY p.occurred_at ASC, p.id ASC LIMIT $3 OFFSET $4"
        ))
        .bind(session_db_id)
        .bind(user_id_i64)
        .bind(page_size.max(1))
        .bind((page.max(1) - 1) * page_size.max(1))
        .fetch_all(pool)
        .await?;
        let rows = rows
            .iter()
            .map(preview_from_pg_row)
            .collect::<DbResult<Vec<_>>>()?;
        Ok((rows, total))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn query_preview_page_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    request: &ImportPreviewPageRequest,
) -> DbResult<ImportPreviewPageResult> {
    if !request.preview_ids.is_empty() {
        let rows = get_preview_by_ids(pool, session_id, &request.preview_ids, user_id)?;
        return Ok(build_preview_page_result_from_rows(rows, request));
    }

    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let total =
            count_preview_rows_by_query(pool, session_db_id, user_id_i64, &request.filters).await?;
        let page = request.page.max(1);
        let page_size = request.page_size.max(1);
        let offset = page.saturating_sub(1).saturating_mul(page_size);
        let mut query = build_preview_page_query(
            session_db_id,
            user_id_i64,
            &request.filters,
            &request.sort_by,
            &request.sort_direction,
            page_size,
            offset,
        );
        let rows = query.build().fetch_all(pool).await?;
        let page_rows = rows
            .iter()
            .map(preview_from_pg_row)
            .collect::<DbResult<Vec<_>>>()?;
        Ok(ImportPreviewPageResult {
            rows: page_rows,
            total: usize::try_from(total).unwrap_or(usize::MAX),
            page,
            page_size,
            metadata: build_preview_metadata(usize::try_from(total).unwrap_or(usize::MAX)),
        })
    })
}

async fn count_preview_rows_by_query(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> DbResult<i64> {
    let mut query = build_preview_count_query(session_db_id, user_id, filters);
    query
        .build_query_scalar()
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

fn build_preview_page_result_from_rows(
    rows: Vec<ImportPreviewRow>,
    request: &ImportPreviewPageRequest,
) -> ImportPreviewPageResult {
    let mut rows = apply_preview_filters(rows, &request.filters);
    sort_preview_rows(&mut rows, &request.sort_by, &request.sort_direction);
    let total = rows.len();
    let page = request.page.max(1);
    let page_size = request.page_size.max(1);
    let start = page.saturating_sub(1).saturating_mul(page_size);
    let page_rows = rows
        .into_iter()
        .skip(start)
        .take(page_size)
        .collect::<Vec<_>>();
    ImportPreviewPageResult {
        rows: page_rows,
        total,
        page,
        page_size,
        metadata: build_preview_metadata(total),
    }
}

fn build_preview_count_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT FROM import_preview_rows p WHERE p.session_id = ",
    );
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates(&mut query, filters, "p");
    query
}

fn build_preview_page_query(
    session_db_id: i64,
    user_id: i64,
    filters: &ImportPreviewQueryFilters,
    sort_by: &str,
    sort_direction: &str,
    page_size: usize,
    offset: usize,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = ",
    );
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates(&mut query, filters, "p");
    push_preview_order_by(&mut query, sort_by, sort_direction);
    query.push(" LIMIT ");
    query.push_bind(i64::try_from(page_size).unwrap_or(i64::MAX).max(1));
    query.push(" OFFSET ");
    query.push_bind(i64::try_from(offset).unwrap_or(i64::MAX));
    query
}

fn push_preview_query_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    filters: &ImportPreviewQueryFilters,
    alias: &str,
) {
    let column = |name: &str| format!("{alias}.{name}");
    if filters.selected_only {
        query.push(" AND ");
        query.push(column("selected"));
        query.push(" = true");
    }
    if let Some(value) = normalized_filter(filters.min_datetime.as_deref()) {
        query.push(" AND ");
        query.push(column("occurred_at"));
        query.push(" >= ");
        query.push_bind(normalize_bill_date_text(&value));
        query.push("::timestamptz");
    }
    if let Some(value) = normalized_filter(filters.max_datetime.as_deref()) {
        query.push(" AND ");
        query.push(column("occurred_at"));
        query.push(" <= ");
        query.push_bind(normalize_bill_date_text(&value));
        query.push("::timestamptz");
    }
    if let Some(value) = normalized_filter(filters.transaction_type.as_deref()) {
        push_ilike_predicate(query, &column("transaction_type"), &value);
    }
    if let Some(value) = normalized_filter(filters.category.as_deref()) {
        push_category_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.account.as_deref()) {
        push_account_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.tag.as_deref()) {
        push_tag_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.signal.as_deref()) {
        query.push(" AND (");
        query.push(alias);
        query.push(".preview_payload->'preview_matching_feedback'");
        let pattern = match value.split_once(':') {
            Some((family, status)) => {
                query.push("->");
                query.push_bind(family.trim().to_string());
                like_pattern(status.trim())
            }
            None => like_pattern(&value),
        };
        query.push(")::text ILIKE ");
        query.push_bind(pattern);
    }
    if let Some(value) = normalized_filter(filters.annotation.as_deref()) {
        push_annotation_predicate(query, alias, &value);
    }
    if let Some(value) = normalized_filter(filters.description.as_deref()) {
        query.push(" AND (");
        query.push(column("description"));
        query.push(" ILIKE ");
        query.push_bind(like_pattern(&value));
        query.push(" OR ");
        query.push(alias);
        query.push(".preview_payload->>'preview_description' ILIKE ");
        query.push_bind(like_pattern(&value));
        query.push(")");
    }
}

fn push_preview_order_by(
    query: &mut QueryBuilder<'_, Postgres>,
    sort_by: &str,
    sort_direction: &str,
) {
    let direction = if sort_direction.eq_ignore_ascii_case("desc") {
        "DESC"
    } else {
        "ASC"
    };
    query.push(" ORDER BY ");
    match sort_by {
        "amount_cents" | "preview_amount_cents" | "previewAmountCents" | "sourceAmountCents" => {
            query.push("p.amount_cents")
        }
        "counterparty" => query.push("COALESCE(p.merchant, '')"),
        "type" => query.push(
            "CASE lower(p.transaction_type) WHEN '收入' THEN 0 WHEN 'income' THEN 0 WHEN '2' THEN 0 WHEN '支出' THEN 1 WHEN 'expense' THEN 1 WHEN '3' THEN 1 WHEN '转账' THEN 2 WHEN 'transfer' THEN 2 WHEN '4' THEN 2 WHEN '投资' THEN 3 WHEN 'investment' THEN 3 WHEN '5' THEN 3 ELSE 4 END",
        ),
        "paymentMethod" => query.push("COALESCE(p.payment_method, '')"),
        "comment" => query.push("COALESCE(p.description, '')"),
        "time" => query.push("p.occurred_at"),
        _ => query.push("p.occurred_at"),
    };
    query.push(" ");
    query.push(direction);
    query.push(", p.id ");
    query.push(direction);
}

fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn like_pattern(value: &str) -> String {
    format!("%{value}%")
}

fn push_ilike_predicate(query: &mut QueryBuilder<'_, Postgres>, column: &str, value: &str) {
    query.push(" AND ");
    query.push(column);
    query.push(" ILIKE ");
    query.push_bind(like_pattern(value));
}

fn push_category_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    if value == "__none__" {
        query.push(" AND ");
        query.push(alias);
        query.push(".category_id IS NULL AND COALESCE(");
        query.push(alias);
        query.push(".preview_payload->>'preview_main_category', '') = '' AND COALESCE(");
        query.push(alias);
        query.push(".preview_payload->>'preview_sub_category', '') = ''");
        return;
    }
    if value == "__invalid__" {
        query.push(" AND ");
        push_preview_missing_category_condition(query, alias);
        return;
    }
    query.push(" AND (");
    query.push(alias);
    query.push(".category_id::text = ");
    query.push_bind(value.to_string());
    query.push(" OR ");
    query.push(alias);
    query.push(".preview_payload->>'preview_main_category' ILIKE ");
    query.push_bind(like_pattern(value));
    query.push(" OR ");
    query.push(alias);
    query.push(".preview_payload->>'preview_sub_category' ILIKE ");
    query.push_bind(like_pattern(value));
    query.push(")");
}

fn push_account_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    if value == "__none__" {
        query.push(" AND ");
        query.push(alias);
        query.push(".account_id IS NULL AND ");
        query.push(alias);
        query.push(".transfer_target_account_id IS NULL AND COALESCE(");
        query.push(alias);
        query.push(".payment_method, '') = ''");
        return;
    }
    if value == "__invalid__" {
        query.push(" AND ");
        push_preview_account_filter_invalid_condition(query, alias);
        return;
    }
    query.push(" AND (");
    query.push(alias);
    query.push(".account_id::text = ");
    query.push_bind(value.to_string());
    query.push(" OR ");
    query.push(alias);
    query.push(".transfer_target_account_id::text = ");
    query.push_bind(value.to_string());
    query.push(")");
}

fn push_tag_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    if matches!(value, "__none__" | "__invalid__") {
        query.push(" AND ");
        push_preview_empty_parser_tags_condition(query, alias);
        return;
    }
    query.push(" AND ");
    query.push(alias);
    query.push(".preview_payload->>'preview_parser_tags' ILIKE ");
    query.push_bind(like_pattern(value));
}

fn push_annotation_predicate(query: &mut QueryBuilder<'_, Postgres>, alias: &str, value: &str) {
    match value {
        "needs-review" => {
            query.push(" AND ");
            push_preview_current_review_condition(query, alias);
        }
        "no-issues" => {
            query.push(" AND NOT ");
            push_preview_current_review_condition(query, alias);
        }
        _ => {
            query.push(" AND ");
            query.push(alias);
            query.push(".preview_payload#>>'{preview_matching_feedback,annotation}' ILIKE ");
            query.push_bind(like_pattern(value));
        }
    }
}

fn push_preview_selection_target_predicates(
    query: &mut QueryBuilder<'_, Postgres>,
    target: ImportPreviewSelectionTarget,
    alias: &str,
) {
    match target {
        ImportPreviewSelectionTarget::All => {}
        ImportPreviewSelectionTarget::Valid => {
            query.push(" AND NOT ");
            push_preview_current_review_condition(query, alias);
        }
        ImportPreviewSelectionTarget::NeedsReview => {
            query.push(" AND ");
            push_preview_current_review_condition(query, alias);
        }
    }
}

fn push_preview_current_review_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(");
    push_preview_missing_category_condition(query, alias);
    query.push(" OR ");
    push_preview_missing_source_account_condition(query, alias);
    query.push(" OR ");
    push_preview_missing_destination_account_condition(query, alias);
    query.push(" OR ");
    push_preview_same_transfer_accounts_condition(query, alias);
    query.push(")");
}

fn push_preview_missing_category_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(");
    push_preview_category_required_type_condition(query, alias);
    query.push(" AND ");
    query.push(alias);
    query.push(".category_id IS NULL)");
}

fn push_preview_missing_source_account_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push(alias);
    query.push(".account_id IS NULL");
}

fn push_preview_missing_destination_account_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    push_preview_destination_account_type_condition(query, alias);
    query.push(" AND ");
    query.push(alias);
    query.push(".transfer_target_account_id IS NULL)");
}

fn push_preview_same_transfer_accounts_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("(");
    push_preview_destination_account_type_condition(query, alias);
    query.push(" AND ");
    query.push(alias);
    query.push(".account_id IS NOT NULL AND ");
    query.push(alias);
    query.push(".transfer_target_account_id IS NOT NULL AND ");
    query.push(alias);
    query.push(".account_id = ");
    query.push(alias);
    query.push(".transfer_target_account_id)");
}

fn push_preview_category_required_type_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("lower(");
    query.push(alias);
    query.push(".transaction_type) IN ('收入', 'income', '2', '支出', 'expense', '3', '转账', 'transfer', '4', '投资', 'investment', '5')");
}

fn push_preview_destination_account_type_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("lower(");
    query.push(alias);
    query.push(".transaction_type) IN ('转账', 'transfer', '4', '投资', 'investment', '5')");
}

fn push_preview_transfer_filter_type_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("lower(");
    query.push(alias);
    query.push(".transaction_type) IN ('转账', 'transfer', '4')");
}

fn push_preview_account_filter_invalid_condition(
    query: &mut QueryBuilder<'_, Postgres>,
    alias: &str,
) {
    query.push("((");
    query.push("NOT (");
    push_preview_transfer_filter_type_condition(query, alias);
    query.push(") AND ");
    query.push(alias);
    query.push(".account_id IS NULL) OR (");
    push_preview_transfer_filter_type_condition(query, alias);
    query.push(" AND (");
    query.push(alias);
    query.push(".account_id IS NULL OR ");
    query.push(alias);
    query.push(".transfer_target_account_id IS NULL)))");
}

fn push_preview_empty_parser_tags_condition(query: &mut QueryBuilder<'_, Postgres>, alias: &str) {
    query.push("(COALESCE(jsonb_array_length(CASE WHEN jsonb_typeof(");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags') = 'array' THEN ");
    query.push(alias);
    query.push(".preview_payload->'preview_parser_tags' ELSE '[]'::jsonb END), 0) = 0)");
}

pub fn get_preview_by_ids(
    pool: &PostgresPool,
    session_id: &str,
    preview_ids: &[i64],
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewRow>> {
    block_on_db(async move {
        if preview_ids.is_empty() {
            return Ok(Vec::new());
        }
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = ",
        );
        query.push_bind(session_db_id);
        query.push(" AND p.user_id = ");
        query.push_bind(user_id_i64(user_id)?);
        query.push(" AND p.id IN (");
        let mut separated = query.separated(", ");
        for id in preview_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(") ORDER BY p.occurred_at ASC, p.id ASC");
        let rows = query.build().fetch_all(pool).await?;
        rows.iter().map(preview_from_pg_row).collect()
    })
}

pub fn get_preview_bill_by_id(
    pool: &PostgresPool,
    preview_id: i64,
    user_id: UserId,
) -> DbResult<Option<ImportPreviewRow>> {
    block_on_db(async move {
        let row = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            "#,
        )
        .bind(preview_id)
        .bind(user_id_i64(user_id)?)
        .fetch_optional(pool)
        .await?;
        row.as_ref().map(preview_from_pg_row).transpose()
    })
}

pub fn get_preview_filter_index_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewFilterIndexRow>> {
    Ok(get_preview_by_session(pool, session_id, user_id, false)?
        .into_iter()
        .map(|row| ImportPreviewFilterIndexRow {
            id: row.id,
            preview_date: row.preview_date,
            preview_type: row.preview_type,
            preview_amount_cents: row.preview_amount_cents,
            category_id: row.category_id,
            preview_main_category: row.preview_main_category,
            preview_sub_category: row.preview_sub_category,
            preview_source_account_id: row.preview_source_account_id,
            preview_destination_account_id: row.preview_destination_account_id,
            preview_counterparty: row.preview_counterparty,
            preview_payment_method: row.preview_payment_method,
            preview_description: row.preview_description,
            preview_parser_id: row.preview_parser_id,
            preview_parser_tags: row.preview_parser_tags,
            preview_recurring_id: row.preview_recurring_id,
            preview_recurring_candidate_count: row.preview_recurring_candidate_count,
            preview_recurring_match_reasons: row.preview_recurring_match_reasons,
            preview_recurring_matched_date: row.preview_recurring_matched_date,
            preview_selected: row.preview_selected,
            dedup_type: row.dedup_type,
            dedup_source_ids: row.dedup_source_ids,
            preview_matching_feedback: row.preview_matching_feedback,
        })
        .collect())
}

pub fn update_preview_bill(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<bool> {
    replace_preview_selection_with_patches(pool, session_id, user_id, std::slice::from_ref(patch))
        .map(|count| count > 0)
}

pub fn update_preview_bills_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    replace_preview_selection_with_patches(pool, session_id, user_id, patches)
}

pub fn replace_preview_selection_with_patches(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let mut changed = 0usize;
        for patch in patches {
            if apply_preview_patch_async(pool, session_db_id, user_id_i64, patch).await? {
                changed += 1;
            }
        }
        Ok(changed)
    })
}

pub fn apply_preview_patches_preserving_selection(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    replace_preview_selection_with_patches(pool, session_id, user_id, patches)
}

pub fn update_preview_selection(
    pool: &PostgresPool,
    preview_ids: &[i64],
    selected: bool,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        if preview_ids.is_empty() {
            return Ok(0);
        }
        let mut query = QueryBuilder::<Postgres>::new("UPDATE import_preview_rows SET selected = ");
        query.push_bind(selected);
        query.push(", preview_payload = jsonb_set(preview_payload, '{preview_selected}', ");
        query.push_bind(Value::Bool(selected));
        query.push("::jsonb, true), updated_at = now(), version = version + 1 WHERE user_id = ");
        query.push_bind(user_id_i64(user_id)?);
        query.push(" AND id IN (");
        let mut separated = query.separated(", ");
        for id in preview_ids {
            separated.push_bind(id);
        }
        separated.push_unseparated(")");
        let changed = query.build().execute(pool).await?.rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

pub fn reset_session_preview_selection(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let changed = sqlx::query(
            r#"
            UPDATE import_preview_rows
            SET selected = false,
                preview_payload = jsonb_set(preview_payload, '{preview_selected}', 'false'::jsonb, true),
                updated_at = now(),
                version = version + 1
            WHERE session_id = $1 AND user_id = $2
            "#,
        )
        .bind(session_db_id)
        .bind(user_id_i64(user_id)?)
        .execute(pool)
        .await?
        .rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

pub fn update_session_preview_selection_by_query(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    mode: ImportPreviewSelectionMode,
    target: ImportPreviewSelectionTarget,
    request: &ImportPreviewPageRequest,
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id_i64 = user_id_i64(user_id)?;
        let mut query =
            build_preview_selection_update_query(session_db_id, user_id_i64, mode, target, request);
        let changed = query.build().execute(pool).await?.rows_affected();
        Ok(usize::try_from(changed).unwrap_or(usize::MAX))
    })
}

fn build_preview_selection_update_query(
    session_db_id: i64,
    user_id: i64,
    mode: ImportPreviewSelectionMode,
    target: ImportPreviewSelectionTarget,
    request: &ImportPreviewPageRequest,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new("UPDATE import_preview_rows p SET selected = ");
    match mode {
        ImportPreviewSelectionMode::Select => {
            query.push_bind(true);
            query.push(
                ", preview_payload = jsonb_set(p.preview_payload, '{preview_selected}', to_jsonb(",
            );
            query.push_bind(true);
            query.push("::boolean), true)");
        }
        ImportPreviewSelectionMode::Deselect => {
            query.push_bind(false);
            query.push(
                ", preview_payload = jsonb_set(p.preview_payload, '{preview_selected}', to_jsonb(",
            );
            query.push_bind(false);
            query.push("::boolean), true)");
        }
        ImportPreviewSelectionMode::Invert => {
            query.push("NOT p.selected, preview_payload = jsonb_set(p.preview_payload, '{preview_selected}', to_jsonb(NOT p.selected), true)");
        }
    }
    query.push(", updated_at = now(), version = version + 1 WHERE p.session_id = ");
    query.push_bind(session_db_id);
    query.push(" AND p.user_id = ");
    query.push_bind(user_id);
    push_preview_query_predicates(&mut query, &request.filters, "p");
    push_preview_selection_target_predicates(&mut query, target, "p");
    if !request.preview_ids.is_empty() {
        query.push(" AND p.id IN (");
        let mut separated = query.separated(", ");
        for id in &request.preview_ids {
            separated.push_bind(*id);
        }
        separated.push_unseparated(")");
    }
    query
}

pub fn batch_update_preview_classification(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    updates: &[ImportPreviewClassificationUpdate],
) -> DbResult<usize> {
    let patches = updates
        .iter()
        .map(|update| {
            ImportPreviewPatch::new(update.preview_id)
                .with_change(
                    ImportPreviewPatchField::Type,
                    ImportPreviewPatchValue::Text(update.preview_type.clone()),
                )
                .with_change(
                    ImportPreviewPatchField::MainCategory,
                    ImportPreviewPatchValue::Text(update.preview_main_category.clone()),
                )
                .with_change(
                    ImportPreviewPatchField::SubCategory,
                    ImportPreviewPatchValue::Text(update.preview_sub_category.clone()),
                )
                .with_change(
                    ImportPreviewPatchField::SourceAccountId,
                    update
                        .preview_source_account_id
                        .map(ImportPreviewPatchValue::Integer)
                        .unwrap_or(ImportPreviewPatchValue::Null),
                )
                .with_change(
                    ImportPreviewPatchField::DestinationAccountId,
                    update
                        .preview_destination_account_id
                        .map(ImportPreviewPatchValue::Integer)
                        .unwrap_or(ImportPreviewPatchValue::Null),
                )
        })
        .collect::<Vec<_>>();
    replace_preview_selection_with_patches(pool, session_id, user_id, &patches)
}

pub fn apply_preview_transfer_decision(
    pool: &PostgresPool,
    session_id: &str,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    let Some(preview) = get_preview_bill_by_id(pool, preview_id, user_id)? else {
        return Ok(preview_decision_not_found());
    };
    if expected_state_conflicts(&preview, expected_state) {
        return Ok(preview_decision_state_conflict());
    }
    let patch = match decision {
        ImportPreviewDecision::Accept => {
            let feedback = set_feedback_review_status(
                preview.preview_matching_feedback,
                "transfer",
                "accepted",
            );
            ImportPreviewPatch::new(preview_id).with_change(
                ImportPreviewPatchField::MatchingFeedback,
                ImportPreviewPatchValue::Json(feedback),
            )
        }
        ImportPreviewDecision::Reject => {
            ImportPreviewPatch::new(preview_id).with_transfer_decision_cleared()
        }
        ImportPreviewDecision::Clear => {
            ImportPreviewPatch::new(preview_id).with_transfer_decision_cleared()
        }
    };
    replace_preview_selection_with_patches(pool, session_id, user_id, &[patch])?;
    Ok(ImportPreviewDecisionResult {
        preview: get_preview_bill_by_id(pool, preview_id, user_id)?,
        state_conflict: false,
        invalid_recurring_id: false,
    })
}

pub fn update_preview_recurring_match_decision(
    pool: &PostgresPool,
    session_id: &str,
    preview_id: i64,
    user_id: UserId,
    update: &ImportPreviewRecurringMatchUpdate,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    let Some(preview) = get_preview_bill_by_id(pool, preview_id, user_id)? else {
        return Ok(preview_decision_not_found());
    };
    if expected_state_conflicts(&preview, expected_state) {
        return Ok(preview_decision_state_conflict());
    }
    let candidate = update.target_candidate.as_ref();
    let patch = ImportPreviewPatch::new(preview_id)
        .with_change(
            ImportPreviewPatchField::RecurringId,
            update
                .recurring_id
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        )
        .with_change(
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text(
                candidate.map(|item| item.name.clone()).unwrap_or_default(),
            ),
        )
        .with_change(
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(update.candidate_count),
        )
        .with_change(
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(
                candidate.map(|item| item.match_score).unwrap_or_default(),
            ),
        )
        .with_change(
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text(
                candidate
                    .map(|item| item.match_reasons.join("；"))
                    .unwrap_or_default(),
            ),
        )
        .with_change(
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text(
                candidate
                    .map(|item| item.matched_occurrence_date.clone())
                    .unwrap_or_default(),
            ),
        );
    replace_preview_selection_with_patches(pool, session_id, user_id, &[patch])?;
    Ok(ImportPreviewDecisionResult {
        preview: get_preview_bill_by_id(pool, preview_id, user_id)?,
        state_conflict: false,
        invalid_recurring_id: false,
    })
}

pub fn apply_preview_learning_decision(
    pool: &PostgresPool,
    session_id: &str,
    preview_id: i64,
    user_id: UserId,
    decision: ImportPreviewDecision,
    applied_result: Option<&ImportPreviewLearningApply>,
    expected_state: Option<&ImportPreviewExpectedState>,
) -> DbResult<ImportPreviewDecisionResult> {
    let Some(preview) = get_preview_bill_by_id(pool, preview_id, user_id)? else {
        return Ok(preview_decision_not_found());
    };
    if expected_state_conflicts(&preview, expected_state) {
        return Ok(preview_decision_state_conflict());
    }
    let mut patch = ImportPreviewPatch::new(preview_id);
    match decision {
        ImportPreviewDecision::Accept => {
            if let Some(applied) = applied_result {
                if let Some(value) = &applied.preview_type {
                    patch = patch.with_change(
                        ImportPreviewPatchField::Type,
                        ImportPreviewPatchValue::Text(value.clone()),
                    );
                }
                if let Some(value) = &applied.preview_main_category {
                    patch = patch.with_change(
                        ImportPreviewPatchField::MainCategory,
                        ImportPreviewPatchValue::Text(value.clone()),
                    );
                }
                if let Some(value) = &applied.preview_sub_category {
                    patch = patch.with_change(
                        ImportPreviewPatchField::SubCategory,
                        ImportPreviewPatchValue::Text(value.clone()),
                    );
                }
                if let Some(value) = applied.preview_source_account_id {
                    patch = patch.with_change(
                        ImportPreviewPatchField::SourceAccountId,
                        value
                            .map(ImportPreviewPatchValue::Integer)
                            .unwrap_or(ImportPreviewPatchValue::Null),
                    );
                }
                if let Some(value) = applied.preview_destination_account_id {
                    patch = patch.with_change(
                        ImportPreviewPatchField::DestinationAccountId,
                        value
                            .map(ImportPreviewPatchValue::Integer)
                            .unwrap_or(ImportPreviewPatchValue::Null),
                    );
                }
            }
            let feedback = set_feedback_review_status(
                preview.preview_matching_feedback,
                "learning",
                "accepted",
            );
            patch = patch.with_change(
                ImportPreviewPatchField::MatchingFeedback,
                ImportPreviewPatchValue::Json(feedback),
            );
        }
        ImportPreviewDecision::Reject | ImportPreviewDecision::Clear => {
            patch = patch.with_learning_decision_cleared();
        }
    }
    replace_preview_selection_with_patches(pool, session_id, user_id, &[patch])?;
    Ok(ImportPreviewDecisionResult {
        preview: get_preview_bill_by_id(pool, preview_id, user_id)?,
        state_conflict: false,
        invalid_recurring_id: false,
    })
}

pub fn apply_preview_llm_recommendation(
    pool: &PostgresPool,
    request: &ImportPreviewLlmApplyRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let Some(preview) = get_preview_bill_by_id(pool, request.preview_id, request.user_id)? else {
        return Ok(ImportPreviewLlmDecisionResult {
            preview: None,
            event_id: None,
            applied_fields: Vec::new(),
        });
    };
    let mut applied_fields = Vec::new();
    let mut patch = ImportPreviewPatch::new(request.preview_id);
    if !request.suggestion.suggested_main_category.trim().is_empty() {
        applied_fields.push("main_category".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(request.suggestion.suggested_main_category.clone()),
        );
    }
    if !request.suggestion.suggested_sub_category.trim().is_empty() {
        applied_fields.push("sub_category".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(request.suggestion.suggested_sub_category.clone()),
        );
    }
    if let Some(value) = request.suggestion.resolved_source_account_id {
        applied_fields.push("source_account_id".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::SourceAccountId,
            ImportPreviewPatchValue::Integer(value),
        );
    }
    if let Some(value) = request.suggestion.resolved_destination_account_id {
        applied_fields.push("destination_account_id".to_string());
        patch = patch.with_change(
            ImportPreviewPatchField::DestinationAccountId,
            ImportPreviewPatchValue::Integer(value),
        );
    }
    let event_id = create_llm_memory_event(
        pool,
        &LlmMemoryEventDraft {
            user_id: request.user_id,
            session_id: Some(request.session_id.to_string()),
            preview_id: Some(request.preview_id),
            event_type: "preview_apply".to_string(),
            decision: Some("accept".to_string()),
            prompt_text: request.prompt_text.map(str::to_string),
            llm_response_raw: None,
            llm_provider: request.llm_provider.map(str::to_string),
            llm_model: request.llm_model.map(str::to_string),
            suggested_main_category: Some(request.suggestion.suggested_main_category.clone()),
            suggested_sub_category: Some(request.suggestion.suggested_sub_category.clone()),
            suggested_source_account: Some(request.suggestion.suggested_source_account.clone()),
            suggested_destination_account: Some(
                request.suggestion.suggested_destination_account.clone(),
            ),
            confidence: request.suggestion.confidence,
            user_correction_category: None,
            user_correction_account: None,
            snapshot_before: Some(json!(preview)),
            snapshot_after: None,
            metadata: Some(json!({ "reason": request.suggestion.reason })),
        },
    )?;
    replace_preview_selection_with_patches(pool, request.session_id, request.user_id, &[patch])?;
    Ok(ImportPreviewLlmDecisionResult {
        preview: get_preview_bill_by_id(pool, request.preview_id, request.user_id)?,
        event_id: Some(event_id),
        applied_fields,
    })
}

pub fn review_preview_llm_recommendation(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let Some(preview) = get_preview_bill_by_id(pool, request.preview_id, request.user_id)? else {
        return Ok(ImportPreviewLlmDecisionResult {
            preview: None,
            event_id: None,
            applied_fields: Vec::new(),
        });
    };
    let decision = match request.decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    };
    let mut patch = ImportPreviewPatch::new(request.preview_id);
    if !matches!(request.decision, ImportPreviewDecision::Accept) {
        patch = patch.with_llm_decision_cleared();
        replace_preview_selection_with_patches(
            pool,
            request.session_id,
            request.user_id,
            &[patch],
        )?;
    }
    let event_id = create_llm_memory_event(
        pool,
        &LlmMemoryEventDraft {
            user_id: request.user_id,
            session_id: Some(request.session_id.to_string()),
            preview_id: Some(request.preview_id),
            event_type: "preview_review".to_string(),
            decision: Some(decision.to_string()),
            prompt_text: None,
            llm_response_raw: None,
            llm_provider: None,
            llm_model: None,
            suggested_main_category: request
                .suggestion
                .map(|value| value.suggested_main_category.clone()),
            suggested_sub_category: request
                .suggestion
                .map(|value| value.suggested_sub_category.clone()),
            suggested_source_account: request
                .suggestion
                .map(|value| value.suggested_source_account.clone()),
            suggested_destination_account: request
                .suggestion
                .map(|value| value.suggested_destination_account.clone()),
            confidence: request
                .suggestion
                .map(|value| value.confidence)
                .unwrap_or_default(),
            user_correction_category: request.user_correction_category.map(str::to_string),
            user_correction_account: request.user_correction_account.map(str::to_string),
            snapshot_before: Some(json!(preview)),
            snapshot_after: get_preview_bill_by_id(pool, request.preview_id, request.user_id)?
                .map(|row| json!(row)),
            metadata: None,
        },
    )?;
    Ok(ImportPreviewLlmDecisionResult {
        preview: get_preview_bill_by_id(pool, request.preview_id, request.user_id)?,
        event_id: Some(event_id),
        applied_fields: Vec::new(),
    })
}

pub fn record_import_learning_lifecycle_feedback(
    pool: &PostgresPool,
    user_id: UserId,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<ImportLearningLifecycleView> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let existing =
            get_import_learning_lifecycle_view_async(pool, user_id, &input.recommendation_key)
                .await?;
        let state = ImportLearningLifecycleState {
            status: existing
                .as_ref()
                .map(|item| item.status.clone())
                .unwrap_or_else(|| "yellow".to_string()),
            accepted_count: existing
                .as_ref()
                .map(|item| item.accepted_count)
                .unwrap_or(0),
            rejected_count: existing
                .as_ref()
                .map(|item| item.rejected_count)
                .unwrap_or(0),
            auto_applied_count: existing
                .as_ref()
                .map(|item| item.auto_applied_count)
                .unwrap_or(0),
        };
        let next = transition_import_learning_lifecycle(&state, &input.feedback);
        let signal_state = learning_lifecycle_signal_state(&next.next_status);
        sqlx::query(
            r#"
            INSERT INTO import_learning_lifecycle (
                user_id, recommendation_key, recommendation_type, status,
                accepted_count, rejected_count, auto_applied_count, auto_apply_enabled,
                metadata, last_feedback_at, created_at, updated_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, '{}'::jsonb, now(), now(), now())
            ON CONFLICT (user_id, recommendation_key) DO UPDATE SET
                recommendation_type = excluded.recommendation_type,
                status = excluded.status,
                accepted_count = excluded.accepted_count,
                rejected_count = excluded.rejected_count,
                auto_applied_count = excluded.auto_applied_count,
                auto_apply_enabled = excluded.auto_apply_enabled,
                last_feedback_at = now(),
                updated_at = now(),
                version = import_learning_lifecycle.version + 1
            "#,
        )
        .bind(user_id)
        .bind(&input.recommendation_key)
        .bind(&input.recommendation_type)
        .bind(&next.next_status)
        .bind(next.accepted_count)
        .bind(next.rejected_count)
        .bind(next.auto_applied_count)
        .bind(learning_lifecycle_is_auto_eligible(&next.next_status))
        .execute(pool)
        .await?;
        let lifecycle_id = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT id FROM import_learning_lifecycle WHERE user_id = $1 AND recommendation_key = $2",
        )
        .bind(user_id)
        .bind(&input.recommendation_key)
        .fetch_one(pool)
        .await?
        .unwrap_or_default();
        sqlx::query(
            r#"
            INSERT INTO import_learning_feedback_events (
                user_id, rule_id, suggestion_id, lifecycle_id, recommendation_key,
                event_type, session_id, preview_id, bill_id, candidate_id,
                previous_signal_state, next_signal_state, payload, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::jsonb,now())
            "#,
        )
        .bind(user_id)
        .bind(input.rule_id)
        .bind(input.suggestion_id)
        .bind(lifecycle_id)
        .bind(&input.recommendation_key)
        .bind(&input.feedback)
        .bind(&input.session_id)
        .bind(input.preview_id)
        .bind(input.bill_id)
        .bind(&input.candidate_id)
        .bind(state.status)
        .bind(signal_state)
        .bind(input.payload_json.as_deref().unwrap_or("{}"))
        .execute(pool)
        .await?;
        get_import_learning_lifecycle_view_async(pool, user_id, &input.recommendation_key)
            .await?
            .ok_or_else(|| {
                DbError::InvalidOperation("learning lifecycle was not persisted".to_string())
            })
    })
}

pub fn get_import_learning_lifecycle_view(
    pool: &PostgresPool,
    user_id: UserId,
    recommendation_key: &str,
) -> DbResult<Option<ImportLearningLifecycleView>> {
    block_on_db(async move {
        get_import_learning_lifecycle_view_async(pool, user_id_i64(user_id)?, recommendation_key)
            .await
    })
}

pub fn create_llm_memory_event(pool: &PostgresPool, draft: &LlmMemoryEventDraft) -> DbResult<i64> {
    block_on_db(async move {
        let prompt_text = draft
            .prompt_text
            .as_deref()
            .map(|value| truncate_text_bytes(value, LLM_MEMORY_PROMPT_TEXT_MAX_BYTES));
        let id = sqlx::query(
            r#"
            INSERT INTO llm_memory_events (
                user_id, session_id, preview_id, event_type, decision, prompt_text,
                llm_response_raw, llm_provider, llm_model, suggested_main_category,
                suggested_sub_category, suggested_source_account, suggested_destination_account,
                confidence, user_correction_category, user_correction_account,
                snapshot_before, snapshot_after, metadata, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17::jsonb,$18::jsonb,$19::jsonb,now())
            RETURNING id
            "#,
        )
        .bind(user_id_i64(draft.user_id)?)
        .bind(&draft.session_id)
        .bind(draft.preview_id)
        .bind(&draft.event_type)
        .bind(&draft.decision)
        .bind(&prompt_text)
        .bind(&draft.llm_response_raw)
        .bind(&draft.llm_provider)
        .bind(&draft.llm_model)
        .bind(&draft.suggested_main_category)
        .bind(&draft.suggested_sub_category)
        .bind(&draft.suggested_source_account)
        .bind(&draft.suggested_destination_account)
        .bind(draft.confidence)
        .bind(&draft.user_correction_category)
        .bind(&draft.user_correction_account)
        .bind(draft.snapshot_before.as_ref().unwrap_or(&Value::Null).to_string())
        .bind(draft.snapshot_after.as_ref().unwrap_or(&Value::Null).to_string())
        .bind(draft.metadata.as_ref().unwrap_or(&json!({})).to_string())
        .fetch_one(pool)
        .await?
        .try_get("id")?;
        Ok(id)
    })
}

pub fn get_llm_memory_events(
    pool: &PostgresPool,
    user_id: UserId,
    session_id: Option<&str>,
    limit: usize,
) -> DbResult<Vec<LlmMemoryEventRow>> {
    block_on_db(async move {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT * FROM llm_memory_events WHERE user_id = ");
        builder.push_bind(user_id_i64(user_id)?);
        if let Some(session_id) = session_id {
            builder.push(" AND session_id = ");
            builder.push_bind(session_id);
        }
        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(i64::try_from(limit.clamp(1, 500)).unwrap_or(500));
        let rows = builder.build().fetch_all(pool).await?;
        rows.iter().map(llm_memory_event_from_pg_row).collect()
    })
}

pub fn save_import_annotation_samples(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportAnnotationSampleDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let user_id = user_id_i64(user_id)?;
        let mut changed = 0usize;
        for draft in drafts {
            let payload = json!({
                "annotated_type": draft.annotated_type,
                "annotated_category_id": draft.annotated_category_id,
                "annotated_source_account_id": draft.annotated_source_account_id,
                "annotated_destination_account_id": draft.annotated_destination_account_id,
            });
            let annotation_type = draft.annotated_type.as_deref().unwrap_or("classification");
            let key = format!("{}:{}", draft.preview_id, annotation_type);
            sqlx::query(
                r#"
                INSERT INTO import_annotation_samples (
                    user_id, session_id, preview_id, annotation_type, annotation_key,
                    sample_payload, created_at
                ) VALUES ($1,$2,$3,$4,$5,$6::jsonb,now())
                ON CONFLICT (user_id, session_id, annotation_type, annotation_key)
                DO UPDATE SET sample_payload = excluded.sample_payload,
                              preview_id = excluded.preview_id,
                              created_at = now(),
                              version = import_annotation_samples.version + 1
                "#,
            )
            .bind(user_id)
            .bind(session_id)
            .bind(draft.preview_id)
            .bind(annotation_type)
            .bind(key)
            .bind(payload.to_string())
            .execute(pool)
            .await?;
            changed += 1;
        }
        Ok(changed)
    })
}

pub fn get_import_annotation_samples(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportAnnotationSampleRow>> {
    block_on_db(async move {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, session_id, preview_id, annotation_type, sample_payload, created_at
            FROM import_annotation_samples
            WHERE user_id = $1 AND session_id = $2
            ORDER BY id ASC
            "#,
        )
        .bind(user_id_i64(user_id)?)
        .bind(session_id)
        .fetch_all(pool)
        .await?;
        rows.iter()
            .map(|row| {
                let payload: Value = row.try_get("sample_payload")?;
                let created_at = format_pg_time(row.try_get("created_at")?);
                Ok(ImportAnnotationSampleRow {
                    id: row.try_get("id")?,
                    session_id: row.try_get("session_id")?,
                    user_id: row.try_get("user_id")?,
                    preview_id: row.try_get("preview_id")?,
                    annotated_type: payload_text(&payload, "annotated_type"),
                    annotated_category_id: payload_i64(&payload, "annotated_category_id"),
                    annotated_source_account_id: payload_i64(
                        &payload,
                        "annotated_source_account_id",
                    ),
                    annotated_destination_account_id: payload_i64(
                        &payload,
                        "annotated_destination_account_id",
                    ),
                    created_at: created_at.clone(),
                    updated_at: created_at,
                })
            })
            .collect()
    })
}

pub fn confirm_preview_to_bills(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ConfirmPreviewResult> {
    confirm_preview_to_bills_with_ack(pool, session_id, user_id, None)
}

pub fn confirm_preview_to_bills_with_ack(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    _history_acknowledgement: Option<&ImportHistoryRewriteAcknowledgement>,
) -> DbResult<ConfirmPreviewResult> {
    block_on_db(async move {
        let user_id_i64 = user_id_i64(user_id)?;
        let previews = load_preview_rows(
            pool,
            session_db_id(pool, session_id, user_id).await?,
            user_id_i64,
            true,
        )
        .await?;
        let mut result = ConfirmPreviewResult {
            confirmed_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            errors: Vec::new(),
        };
        for preview in previews {
            if preview_requires_review(&preview) {
                result.skipped_count += 1;
                result.errors.push(format!(
                    "preview {} requires review before confirm",
                    preview.id
                ));
                continue;
            }
            let fields = bill_create_fields_from_preview(&preview);
            match create_postgres_bill(
                pool,
                user_id_i64,
                &BillCreateDraft {
                    fields,
                    tag_ids: Vec::new(),
                },
            )
            .await
            {
                Ok(_) => result.confirmed_count += 1,
                Err(error) => result.errors.push(error.to_string()),
            }
        }
        update_import_session_status(
            pool,
            &ImportSessionStatusUpdate {
                session_id: session_id.to_string(),
                user_id,
                status: "confirmed".to_string(),
                total_parsed: None,
                total_preview: None,
                total_confirmed: Some(i64::try_from(result.confirmed_count).unwrap_or(i64::MAX)),
            },
        )?;
        Ok(result)
    })
}

fn bill_create_fields_from_preview(preview: &ImportPreviewRow) -> BillRecord {
    let mut fields = Map::new();
    fields.insert("date".to_string(), json!(preview.preview_date));
    fields.insert("type".to_string(), json!(preview.preview_type));
    fields.insert(
        "amount_cents".to_string(),
        json!(preview.preview_amount_cents),
    );
    fields.insert(
        "destination_amount_cents".to_string(),
        json!(preview.preview_destination_amount_cents),
    );
    fields.insert(
        "counterparty".to_string(),
        json!(preview.preview_counterparty),
    );
    fields.insert(
        "description".to_string(),
        json!(preview.preview_description),
    );
    fields.insert(
        "payment_method".to_string(),
        json!(preview.preview_payment_method),
    );
    fields.insert(
        "main_category".to_string(),
        json!(preview.preview_main_category),
    );
    fields.insert(
        "sub_category".to_string(),
        json!(preview.preview_sub_category),
    );
    if let Some(value) = preview.category_id {
        fields.insert("category_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_source_account_id {
        fields.insert("source_account_id".to_string(), json!(value));
    }
    if let Some(value) = preview.preview_destination_account_id {
        fields.insert("destination_account_id".to_string(), json!(value));
    }
    fields
}

pub fn clear_session_data(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ClearSessionDataResult> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let annotation_count = sqlx::query(
            "DELETE FROM import_annotation_samples WHERE user_id = $1 AND session_id = $2",
        )
        .bind(user_id)
        .bind(session_id)
        .execute(pool)
        .await?
        .rows_affected();
        let preview_count =
            sqlx::query("DELETE FROM import_preview_rows WHERE user_id = $1 AND session_id = $2")
                .bind(user_id)
                .bind(session_db_id)
                .execute(pool)
                .await?
                .rows_affected();
        let parser_count =
            sqlx::query("DELETE FROM import_standard_rows WHERE user_id = $1 AND session_id = $2")
                .bind(user_id)
                .bind(session_db_id)
                .execute(pool)
                .await?
                .rows_affected();
        sqlx::query("DELETE FROM import_sources WHERE user_id = $1 AND session_id = $2")
            .bind(user_id)
            .bind(session_db_id)
            .execute(pool)
            .await?;
        let session_count =
            sqlx::query("DELETE FROM import_sessions WHERE user_id = $1 AND id = $2")
                .bind(user_id)
                .bind(session_db_id)
                .execute(pool)
                .await?
                .rows_affected();
        Ok(ClearSessionDataResult {
            parser_count: usize::try_from(parser_count).unwrap_or(usize::MAX),
            preview_count: usize::try_from(preview_count).unwrap_or(usize::MAX),
            annotation_count: usize::try_from(annotation_count).unwrap_or(usize::MAX),
            session_count: usize::try_from(session_count).unwrap_or(usize::MAX),
        })
    })
}

pub fn clear_import_preview_materialization_state(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
) -> DbResult<usize> {
    Ok(0)
}

pub fn insert_import_history_materializations_batch(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
    _drafts: &[ImportHistoryMaterializationDraft],
) -> DbResult<usize> {
    Ok(0)
}

pub fn get_import_history_materializations_by_session(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
) -> DbResult<Vec<ImportHistoryMaterializationRow>> {
    Ok(Vec::new())
}

pub fn get_import_history_candidate_bills_for_session(
    _pool: &PostgresPool,
    _session_id: &str,
    _user_id: UserId,
) -> DbResult<Vec<ImportHistoryBillRow>> {
    Ok(Vec::new())
}

pub fn insert_import_decision_groups_batch(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportDecisionGroupDraft],
) -> DbResult<usize> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let mut inserted = 0usize;
        for draft in drafts {
            let group_id: i64 = sqlx::query(
                r#"
                INSERT INTO import_decision_groups (
                    session_id, user_id, group_type, group_key, decision_status,
                    base_preview_row_id, signal_payload, created_at, updated_at
                ) VALUES ($1,$2,$3,$4,$5,$6,$7::jsonb,now(),now())
                ON CONFLICT (session_id, group_type, group_key) DO UPDATE SET
                    decision_status = excluded.decision_status,
                    base_preview_row_id = excluded.base_preview_row_id,
                    signal_payload = excluded.signal_payload,
                    updated_at = now(),
                    version = import_decision_groups.version + 1
                RETURNING id
                "#,
            )
            .bind(session_db_id)
            .bind(user_id)
            .bind(&draft.group_type)
            .bind(&draft.group_key)
            .bind(&draft.decision_status)
            .bind(draft.base_preview_row_id)
            .bind(draft.signal_payload.to_string())
            .fetch_one(pool)
            .await?
            .try_get("id")?;
            for member in &draft.members {
                sqlx::query(
                    r#"
                    INSERT INTO import_decision_group_members (
                        group_id, preview_row_id, standard_row_id, history_bill_id,
                        member_role, parser_name, metadata, created_at
                    ) VALUES ($1,$2,$3,$4,$5,$6,$7::jsonb,now())
                    ON CONFLICT DO NOTHING
                    "#,
                )
                .bind(group_id)
                .bind(member.preview_row_id)
                .bind(member.standard_row_id)
                .bind(member.history_bill_id)
                .bind(&member.member_role)
                .bind(&member.parser_name)
                .bind(member.metadata.to_string())
                .execute(pool)
                .await?;
            }
            inserted += 1;
        }
        Ok(inserted)
    })
}

pub fn get_import_decision_groups_by_session(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportDecisionGroupRow>> {
    block_on_db(async move {
        let session_db_id = session_db_id(pool, session_id, user_id).await?;
        let user_id = user_id_i64(user_id)?;
        let rows = sqlx::query(
            r#"
            SELECT g.*, s.session_key
            FROM import_decision_groups g
            JOIN import_sessions s ON s.id = g.session_id
            WHERE g.session_id = $1 AND g.user_id = $2
            ORDER BY g.id ASC
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .fetch_all(pool)
        .await?;
        let mut groups = Vec::new();
        for row in rows {
            let group_id: i64 = row.try_get("id")?;
            let member_rows = sqlx::query(
                "SELECT * FROM import_decision_group_members WHERE group_id = $1 ORDER BY id ASC",
            )
            .bind(group_id)
            .fetch_all(pool)
            .await?;
            groups.push(ImportDecisionGroupRow {
                id: group_id,
                session_id: row.try_get("session_key")?,
                user_id: row.try_get("user_id")?,
                group_type: row.try_get("group_type")?,
                group_key: row.try_get("group_key")?,
                decision_status: row.try_get("decision_status")?,
                base_preview_row_id: row.try_get("base_preview_row_id")?,
                signal_payload: row.try_get("signal_payload")?,
                members: member_rows
                    .iter()
                    .map(import_decision_member_from_pg_row)
                    .collect::<DbResult<Vec<_>>>()?,
                created_at: format_pg_time(row.try_get("created_at")?),
                updated_at: format_pg_time(row.try_get("updated_at")?),
            });
        }
        Ok(groups)
    })
}

pub fn calculate_import_bill_hash(
    date: &str,
    bill_type: &str,
    amount: f64,
    counterparty: &str,
    description: &str,
) -> String {
    let source = format!(
        "{date}|{bill_type}|{}|{counterparty}|{description}",
        finite_float_text(amount)
    );
    format!("{:x}", md5::compute(source.as_bytes()))
}

fn block_on_db<T, F>(future: F) -> DbResult<T>
where
    F: Future<Output = DbResult<T>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        tokio::task::block_in_place(|| handle.block_on(future))
    } else {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(DbError::from)?
            .block_on(future)
    }
}

async fn create_import_session_async(
    pool: &PostgresPool,
    draft: &ImportSessionDraft,
) -> DbResult<i64> {
    let user_id = user_id_i64(draft.user_id)?;
    let row = sqlx::query(
        r#"
        INSERT INTO import_sessions (
            user_id, session_key, status, import_mode, source_count, row_count,
            file_count, total_parsed, total_preview, total_confirmed, metadata,
            created_at, updated_at
        ) VALUES ($1,$2,'parsing','preview',$3,0,$3,0,0,0,'{}'::jsonb,now(),now())
        ON CONFLICT (user_id, session_key) DO UPDATE SET
            status = 'parsing',
            source_count = excluded.source_count,
            file_count = excluded.file_count,
            updated_at = now(),
            version = import_sessions.version + 1
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(&draft.session_id)
    .bind(draft.file_count)
    .fetch_one(pool)
    .await?;
    row.try_get("id").map_err(DbError::from)
}

async fn clear_user_import_staging_data_async(
    pool: &PostgresPool,
    user_id: i64,
) -> DbResult<usize> {
    let annotation_count = sqlx::query("DELETE FROM import_annotation_samples WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    let session_count = sqlx::query("DELETE FROM import_sessions WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(usize::try_from(annotation_count + session_count).unwrap_or(usize::MAX))
}

async fn get_import_session_async(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Option<ImportSessionRow>> {
    let row = sqlx::query(
        r#"
        SELECT *
        FROM import_sessions
        WHERE session_key = $1 AND user_id = $2
        "#,
    )
    .bind(session_id)
    .bind(user_id_i64(user_id)?)
    .fetch_optional(pool)
    .await?;
    row.as_ref().map(import_session_from_pg_row).transpose()
}

async fn session_db_id(pool: &PostgresPool, session_id: &str, user_id: UserId) -> DbResult<i64> {
    sqlx::query_scalar::<_, Option<i64>>(
        "SELECT id FROM import_sessions WHERE session_key = $1 AND user_id = $2",
    )
    .bind(session_id)
    .bind(user_id_i64(user_id)?)
    .fetch_one(pool)
    .await?
    .ok_or_else(|| DbError::InvalidOperation("import session not found".to_string()))
}

async fn upsert_import_sources(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    drafts: &[ImportSourceDraft],
) -> DbResult<std::collections::BTreeMap<i64, i64>> {
    let mut ids = std::collections::BTreeMap::new();
    for draft in drafts {
        let id: i64 = sqlx::query(
            r#"
            INSERT INTO import_sources (
                session_id, user_id, source_index, original_file_name,
                parser_id, parser_name, parser_signal, parser_confidence,
                feature_signature, metadata, created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::jsonb,now(),now())
            ON CONFLICT (session_id, source_index) DO UPDATE SET
                original_file_name = excluded.original_file_name,
                parser_id = excluded.parser_id,
                parser_name = excluded.parser_name,
                parser_signal = excluded.parser_signal,
                parser_confidence = excluded.parser_confidence,
                feature_signature = excluded.feature_signature,
                metadata = excluded.metadata,
                updated_at = now(),
                version = import_sources.version + 1
            RETURNING id
            "#,
        )
        .bind(session_db_id)
        .bind(user_id)
        .bind(draft.source_index)
        .bind(&draft.original_file_name)
        .bind(&draft.parser_id)
        .bind(&draft.parser_name)
        .bind(&draft.parser_signal)
        .bind(draft.parser_confidence)
        .bind(&draft.feature_signature)
        .bind(draft.metadata.to_string())
        .fetch_one(pool)
        .await?
        .try_get("id")?;
        ids.insert(draft.source_index, id);
    }
    Ok(ids)
}

async fn ensure_default_source(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    parser_id: &str,
) -> DbResult<i64> {
    let source_ids = upsert_import_sources(
        pool,
        session_db_id,
        user_id,
        &[ImportSourceDraft {
            source_index: 0,
            original_file_name: "inline-standard-bills".to_string(),
            parser_id: parser_id.to_string(),
            parser_name: parser_source_label(parser_id).to_string(),
            parser_signal: "provided".to_string(),
            parser_confidence: 1.0,
            feature_signature: format!("{session_db_id}:default"),
            metadata: json!({}),
        }],
    )
    .await?;
    source_ids.get(&0).copied().ok_or_else(|| {
        DbError::InvalidOperation("default import source was not created".to_string())
    })
}

async fn insert_standard_rows_and_parser_payloads(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    parser_drafts: &[ImportParserTemplateDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    source_ids: &std::collections::BTreeMap<i64, i64>,
) -> DbResult<usize> {
    if !standard_row_drafts.is_empty() {
        let rows = standard_row_batch_values_from_standard_row_drafts(
            parser_drafts,
            standard_row_drafts,
            source_ids,
        )?;
        insert_standard_rows_batch_async(pool, session_db_id, user_id, &rows).await?;
        return Ok(standard_row_drafts.len());
    }
    let source_id = source_ids
        .values()
        .next()
        .copied()
        .ok_or_else(|| DbError::InvalidOperation("import source missing".to_string()))?;
    let rows = standard_row_batch_values_from_parser_templates(source_id, parser_drafts);
    insert_standard_rows_batch_async(pool, session_db_id, user_id, &rows).await?;
    Ok(parser_drafts.len())
}

fn standard_row_batch_values_from_standard_row_drafts(
    parser_drafts: &[ImportParserTemplateDraft],
    standard_row_drafts: &[ImportStandardRowDraft],
    source_ids: &std::collections::BTreeMap<i64, i64>,
) -> DbResult<Vec<StandardRowBatchValue>> {
    standard_row_drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            let source_id = source_ids
                .get(&draft.source_index)
                .or_else(|| source_ids.values().next())
                .copied()
                .ok_or_else(|| DbError::InvalidOperation("import source missing".to_string()))?;
            let parser_draft = parser_drafts.get(index);
            Ok(standard_row_batch_value_from_standard_row(
                source_id,
                draft,
                parser_draft,
            ))
        })
        .collect()
}

struct StandardRowBatchValue {
    source_id: i64,
    source_row_index: i64,
    occurred_at: String,
    amount_cents: i64,
    direction: String,
    transaction_type: String,
    merchant: String,
    payment_method: String,
    description: String,
    parser_payload: String,
    standard_payload: String,
}

fn standard_row_batch_value_from_standard_row(
    source_id: i64,
    draft: &ImportStandardRowDraft,
    parser_draft: Option<&ImportParserTemplateDraft>,
) -> StandardRowBatchValue {
    StandardRowBatchValue {
        source_id,
        source_row_index: draft.source_row_index,
        occurred_at: normalize_bill_date_text(&draft.occurred_at),
        amount_cents: draft.amount_cents,
        direction: draft.direction.clone(),
        transaction_type: draft.transaction_type.clone(),
        merchant: draft.merchant.clone(),
        payment_method: draft.payment_method.clone(),
        description: draft.description.clone(),
        parser_payload: parser_payload_from_standard_row(draft, parser_draft).to_string(),
        standard_payload: draft.standard_payload.to_string(),
    }
}

fn standard_row_batch_value_from_parser_template(
    source_id: i64,
    source_row_index: i64,
    draft: &ImportParserTemplateDraft,
) -> StandardRowBatchValue {
    let amount = Money::from_yuan_str(&finite_float_text(draft.parser_amount))
        .unwrap_or(Money::ZERO)
        .to_cents()
        .abs();
    let row = ImportStandardRowDraft {
        source_index: 0,
        source_row_index,
        occurred_at: draft.parser_date.clone(),
        amount_cents: amount,
        direction: if draft.parser_type == "收入" || draft.parser_type == "income" {
            "income".to_string()
        } else {
            "expense".to_string()
        },
        transaction_type: draft.parser_type.clone(),
        merchant: draft.parser_counterparty.clone(),
        payment_method: draft.parser_payment_method.clone(),
        description: draft.parser_description.clone(),
        parser_payload: parser_payload_from_parser_template(draft),
        standard_payload: json!({
            "parser_original_type": draft.parser_original_type,
            "parser_original_category": draft.parser_original_category,
            "parser_account_id": draft.parser_account_id,
        }),
    };
    standard_row_batch_value_from_standard_row(source_id, &row, Some(draft))
}

async fn insert_standard_rows_batch_async(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    rows: &[StandardRowBatchValue],
) -> DbResult<usize> {
    for chunk in rows.chunks(IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE) {
        let mut query = build_standard_rows_insert_query(session_db_id, user_id, chunk);
        query.build().execute(pool).await?;
    }
    Ok(rows.len())
}

fn build_standard_rows_insert_query<'a>(
    session_db_id: i64,
    user_id: i64,
    rows: &'a [StandardRowBatchValue],
) -> QueryBuilder<'a, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_standard_rows (
            session_id, source_id, user_id, source_row_index, occurred_at,
            amount_cents, direction, transaction_type, merchant, payment_method,
            description, parser_payload, standard_payload, created_at, updated_at
        )
        "#,
    );
    query.push_values(rows, |mut row, value| {
        row.push_bind(session_db_id)
            .push_bind(value.source_id)
            .push_bind(user_id)
            .push_bind(value.source_row_index)
            .push_bind(&value.occurred_at)
            .push_unseparated("::timestamptz")
            .push_bind(value.amount_cents)
            .push_bind(&value.direction)
            .push_bind(&value.transaction_type)
            .push_bind(&value.merchant)
            .push_bind(&value.payment_method)
            .push_bind(&value.description)
            .push_bind(&value.parser_payload)
            .push_unseparated("::jsonb")
            .push_bind(&value.standard_payload)
            .push_unseparated("::jsonb")
            .push("now()")
            .push("now()");
    });
    query.push(
        r#"
        ON CONFLICT (source_id, source_row_index) DO UPDATE SET
            occurred_at = excluded.occurred_at,
            amount_cents = excluded.amount_cents,
            direction = excluded.direction,
            transaction_type = excluded.transaction_type,
            merchant = excluded.merchant,
            payment_method = excluded.payment_method,
            description = excluded.description,
            parser_payload = excluded.parser_payload,
            standard_payload = excluded.standard_payload,
            updated_at = now(),
            version = import_standard_rows.version + 1
        "#,
    );
    query
}

async fn insert_standard_row(
    pool: &PostgresPool,
    session_db_id: i64,
    source_id: i64,
    user_id: i64,
    draft: &ImportStandardRowDraft,
    parser_draft: Option<&ImportParserTemplateDraft>,
) -> DbResult<i64> {
    let parser_payload = parser_payload_from_standard_row(draft, parser_draft);
    let id = sqlx::query(
        r#"
        INSERT INTO import_standard_rows (
            session_id, source_id, user_id, source_row_index, occurred_at,
            amount_cents, direction, transaction_type, merchant, payment_method,
            description, parser_payload, standard_payload, created_at, updated_at
        ) VALUES ($1,$2,$3,$4,$5::timestamptz,$6,$7,$8,$9,$10,$11,$12::jsonb,$13::jsonb,now(),now())
        ON CONFLICT (source_id, source_row_index) DO UPDATE SET
            occurred_at = excluded.occurred_at,
            amount_cents = excluded.amount_cents,
            direction = excluded.direction,
            transaction_type = excluded.transaction_type,
            merchant = excluded.merchant,
            payment_method = excluded.payment_method,
            description = excluded.description,
            parser_payload = excluded.parser_payload,
            standard_payload = excluded.standard_payload,
            updated_at = now(),
            version = import_standard_rows.version + 1
        RETURNING id
        "#,
    )
    .bind(session_db_id)
    .bind(source_id)
    .bind(user_id)
    .bind(draft.source_row_index)
    .bind(normalize_bill_date_text(&draft.occurred_at))
    .bind(draft.amount_cents)
    .bind(&draft.direction)
    .bind(&draft.transaction_type)
    .bind(&draft.merchant)
    .bind(&draft.payment_method)
    .bind(&draft.description)
    .bind(parser_payload.to_string())
    .bind(draft.standard_payload.to_string())
    .fetch_one(pool)
    .await?
    .try_get("id")?;
    Ok(id)
}

async fn insert_standard_row_from_parser_template(
    pool: &PostgresPool,
    session_db_id: i64,
    source_id: i64,
    user_id: i64,
    source_row_index: i64,
    draft: &ImportParserTemplateDraft,
) -> DbResult<i64> {
    let amount = Money::from_yuan_str(&finite_float_text(draft.parser_amount))
        .unwrap_or(Money::ZERO)
        .to_cents()
        .abs();
    let row = ImportStandardRowDraft {
        source_index: 0,
        source_row_index,
        occurred_at: draft.parser_date.clone(),
        amount_cents: amount,
        direction: if draft.parser_type == "收入" || draft.parser_type == "income" {
            "income".to_string()
        } else {
            "expense".to_string()
        },
        transaction_type: draft.parser_type.clone(),
        merchant: draft.parser_counterparty.clone(),
        payment_method: draft.parser_payment_method.clone(),
        description: draft.parser_description.clone(),
        parser_payload: parser_payload_from_parser_template(draft),
        standard_payload: json!({
            "parser_original_type": draft.parser_original_type,
            "parser_original_category": draft.parser_original_category,
            "parser_account_id": draft.parser_account_id,
        }),
    };
    insert_standard_row(pool, session_db_id, source_id, user_id, &row, Some(draft)).await
}

async fn insert_preview_row_async(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    let payload = preview_payload_from_draft(draft);
    let amount_cents = draft.preview_amount_cents.abs();
    let direction = if draft.preview_type == "收入" || draft.preview_type == "income" {
        "income"
    } else {
        "expense"
    };
    let mut query = build_preview_row_insert_returning_query(
        session_db_id,
        user_id,
        draft,
        payload.to_string(),
        amount_cents,
        direction,
    );
    let id = query.build().fetch_one(pool).await?.try_get("id")?;
    Ok(id)
}

fn build_preview_row_insert_returning_query(
    session_db_id: i64,
    user_id: i64,
    draft: &ImportPreviewDraft,
    preview_payload: String,
    amount_cents: i64,
    direction: &str,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id, category_id,
            merchant, payment_method, description, preview_payload, created_at, updated_at
        ) VALUES (
        "#,
    );
    query.push_bind(session_db_id);
    query.push(", ");
    query.push_bind(user_id);
    query.push(", ");
    query.push_bind(format!(
        "{}:{}",
        normalize_bill_date_text(&draft.preview_date),
        draft.preview_counterparty
    ));
    query.push(", 'insert', ");
    query.push_bind(draft.preview_selected);
    query.push(", '[]'::jsonb, ");
    query.push_bind(draft.dedup_source_ids.clone());
    query.push(", ");
    query.push_bind(normalize_bill_date_text(&draft.preview_date));
    query.push("::timestamptz, ");
    query.push_bind(amount_cents);
    query.push(", ");
    query.push_bind(direction.to_string());
    query.push(", ");
    query.push_bind(draft.preview_type.clone());
    query.push(", ");
    query.push_bind(draft.preview_source_account_id);
    query.push(", ");
    query.push_bind(draft.preview_destination_account_id);
    query.push(", ");
    query.push_bind(draft.category_id);
    query.push(", ");
    query.push_bind(draft.preview_counterparty.clone());
    query.push(", ");
    query.push_bind(draft.preview_payment_method.clone());
    query.push(", ");
    query.push_bind(draft.preview_description.clone());
    query.push(", ");
    query.push_bind(preview_payload);
    query.push("::jsonb, now(), now()) RETURNING id");
    query
}

struct PreviewRowBatchValue {
    page_sort_key: String,
    selected: bool,
    merged_source_ids: Vec<i64>,
    occurred_at: String,
    amount_cents: i64,
    direction: String,
    transaction_type: String,
    account_id: Option<i64>,
    transfer_target_account_id: Option<i64>,
    category_id: Option<i64>,
    merchant: String,
    payment_method: String,
    description: String,
    preview_payload: String,
}

fn preview_row_batch_value_from_draft(draft: &ImportPreviewDraft) -> PreviewRowBatchValue {
    let payload = preview_payload_from_draft(draft);
    let amount_cents = draft.preview_amount_cents.abs();
    let occurred_at = normalize_bill_date_text(&draft.preview_date);
    PreviewRowBatchValue {
        page_sort_key: format!("{}:{}", occurred_at, draft.preview_counterparty),
        selected: draft.preview_selected,
        merged_source_ids: draft.dedup_source_ids.clone(),
        occurred_at,
        amount_cents,
        direction: if draft.preview_type == "收入" || draft.preview_type == "income" {
            "income".to_string()
        } else {
            "expense".to_string()
        },
        transaction_type: draft.preview_type.clone(),
        account_id: draft.preview_source_account_id,
        transfer_target_account_id: draft.preview_destination_account_id,
        category_id: draft.category_id,
        merchant: draft.preview_counterparty.clone(),
        payment_method: draft.preview_payment_method.clone(),
        description: draft.preview_description.clone(),
        preview_payload: payload.to_string(),
    }
}

async fn insert_preview_rows_batch_async(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    let rows = drafts
        .iter()
        .map(preview_row_batch_value_from_draft)
        .collect::<Vec<_>>();
    for chunk in rows.chunks(IMPORT_STAGING_BULK_INSERT_CHUNK_SIZE) {
        let mut query = build_preview_rows_insert_query(session_db_id, user_id, chunk);
        query.build().execute(pool).await?;
    }
    Ok(rows.len())
}

fn build_preview_rows_insert_query<'a>(
    session_db_id: i64,
    user_id: i64,
    rows: &'a [PreviewRowBatchValue],
) -> QueryBuilder<'a, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id, category_id,
            merchant, payment_method, description, preview_payload, created_at, updated_at
        )
        "#,
    );
    query.push_values(rows, |mut row, value| {
        row.push_bind(session_db_id)
            .push_bind(user_id)
            .push_bind(&value.page_sort_key)
            .push_bind("insert")
            .push_bind(value.selected)
            .push("'[]'::jsonb")
            .push_bind(&value.merged_source_ids)
            .push_bind(&value.occurred_at)
            .push_unseparated("::timestamptz")
            .push_bind(value.amount_cents)
            .push_bind(&value.direction)
            .push_bind(&value.transaction_type)
            .push_bind(value.account_id)
            .push_bind(value.transfer_target_account_id)
            .push_bind(value.category_id)
            .push_bind(&value.merchant)
            .push_bind(&value.payment_method)
            .push_bind(&value.description)
            .push_bind(&value.preview_payload)
            .push_unseparated("::jsonb")
            .push("now()")
            .push("now()");
    });
    query
}

async fn load_preview_rows(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    let selected_clause = if selected_only {
        " AND p.selected = true"
    } else {
        ""
    };
    let rows = sqlx::query(&format!(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.session_id = $1 AND p.user_id = $2{selected_clause} ORDER BY p.occurred_at ASC, p.id ASC"
    ))
    .bind(session_db_id)
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    rows.iter().map(preview_from_pg_row).collect()
}

async fn update_session_preview_count(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE import_sessions
        SET total_preview = (
                SELECT COUNT(*)::BIGINT FROM import_preview_rows
                WHERE session_id = $1 AND user_id = $2
            ),
            updated_at = now(),
            version = version + 1
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn apply_preview_patch_async(
    pool: &PostgresPool,
    session_db_id: i64,
    user_id: i64,
    patch: &ImportPreviewPatch,
) -> DbResult<bool> {
    let Some(row) = sqlx::query(
        "SELECT p.*, s.session_key FROM import_preview_rows p JOIN import_sessions s ON s.id = p.session_id WHERE p.id = $1 AND p.session_id = $2 AND p.user_id = $3",
    )
    .bind(patch.preview_id)
    .bind(session_db_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    else {
        return Ok(false);
    };
    let mut preview = preview_from_pg_row(&row)?;
    let mut payload = row
        .try_get::<Value, _>("preview_payload")
        .unwrap_or_else(|_| json!({}));
    for (field, value) in &patch.changes {
        apply_patch_value_to_preview(&mut preview, &mut payload, *field, value.clone());
    }
    if patch.clear_transfer_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "transfer");
    }
    if patch.clear_learning_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "learning");
    }
    if patch.clear_llm_decision {
        clear_feedback_key(&mut preview.preview_matching_feedback, "llm");
    }
    payload_set(
        &mut payload,
        "preview_matching_feedback",
        preview.preview_matching_feedback.clone(),
    );
    let amount_cents = preview.preview_amount_cents.abs();
    let direction = if preview.preview_type == "收入" || preview.preview_type == "income" {
        "income"
    } else {
        "expense"
    };
    let mut query = build_preview_row_update_query(
        &preview,
        payload.to_string(),
        amount_cents,
        direction,
        patch.preview_id,
        session_db_id,
        user_id,
    );
    let changed = query.build().execute(pool).await?.rows_affected();
    Ok(changed > 0)
}

fn build_preview_row_update_query(
    preview: &ImportPreviewRow,
    preview_payload: String,
    amount_cents: i64,
    direction: &str,
    preview_id: i64,
    session_db_id: i64,
    user_id: i64,
) -> QueryBuilder<'static, Postgres> {
    let mut query = QueryBuilder::<Postgres>::new(
        r#"
        UPDATE import_preview_rows
        SET selected =
        "#,
    );
    query.push_bind(preview.preview_selected);
    query.push(", occurred_at = ");
    query.push_bind(normalize_bill_date_text(&preview.preview_date));
    query.push("::timestamptz, amount_cents = ");
    query.push_bind(amount_cents);
    query.push(", direction = ");
    query.push_bind(direction.to_string());
    query.push(", transaction_type = ");
    query.push_bind(preview.preview_type.clone());
    query.push(", account_id = ");
    query.push_bind(preview.preview_source_account_id);
    query.push(", transfer_target_account_id = ");
    query.push_bind(preview.preview_destination_account_id);
    query.push(", category_id = ");
    query.push_bind(preview.category_id);
    query.push(", merchant = ");
    query.push_bind(preview.preview_counterparty.clone());
    query.push(", payment_method = ");
    query.push_bind(preview.preview_payment_method.clone());
    query.push(", description = ");
    query.push_bind(preview.preview_description.clone());
    query.push(", preview_payload = ");
    query.push_bind(preview_payload);
    query.push(
        r#"::jsonb,
            updated_at = now(),
            version = version + 1
        "#,
    );
    query.push(" WHERE id = ");
    query.push_bind(preview_id);
    query.push(" AND session_id = ");
    query.push_bind(session_db_id);
    query.push(" AND user_id = ");
    query.push_bind(user_id);
    query
}

async fn get_import_learning_lifecycle_view_async(
    pool: &PostgresPool,
    user_id: i64,
    recommendation_key: &str,
) -> DbResult<Option<ImportLearningLifecycleView>> {
    let row = sqlx::query(
        r#"
        SELECT recommendation_key, recommendation_type, status, accepted_count,
               rejected_count, auto_applied_count, auto_apply_enabled, suppressed_until
        FROM import_learning_lifecycle
        WHERE user_id = $1 AND recommendation_key = $2
        "#,
    )
    .bind(user_id)
    .bind(recommendation_key)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        let status: String = row.try_get("status")?;
        let state = ImportLearningLifecycleState {
            status: status.clone(),
            accepted_count: row.try_get::<i32, _>("accepted_count")? as i64,
            rejected_count: row.try_get::<i32, _>("rejected_count")? as i64,
            auto_applied_count: row.try_get::<i32, _>("auto_applied_count")? as i64,
        };
        let auto_apply_enabled = row.try_get("auto_apply_enabled")?;
        let suppressed = row
            .try_get::<Option<DateTime<Utc>>, _>("suppressed_until")?
            .is_some();
        Ok(ImportLearningLifecycleView {
            recommendation_key: row.try_get("recommendation_key")?,
            recommendation_type: row.try_get("recommendation_type")?,
            status,
            signal_state: learning_lifecycle_signal_state(&state.status).to_string(),
            accepted_count: state.accepted_count,
            rejected_count: state.rejected_count,
            auto_applied_count: state.auto_applied_count,
            auto_apply_enabled,
            suppressed,
        })
    })
    .transpose()
}

fn import_session_from_pg_row(row: &PgRow) -> DbResult<ImportSessionRow> {
    let created_at = format_pg_time(row.try_get("created_at")?);
    let updated_at = format_pg_time(row.try_get("updated_at")?);
    Ok(ImportSessionRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        user_id: row.try_get("user_id")?,
        status: row.try_get("status")?,
        file_count: row.try_get::<i64, _>("file_count")?,
        total_parsed: row.try_get::<i64, _>("total_parsed")?,
        total_preview: row.try_get::<i64, _>("total_preview")?,
        total_confirmed: row.try_get::<i64, _>("total_confirmed")?,
        created_at,
        updated_at,
    })
}

fn import_source_from_pg_row(row: &PgRow) -> DbResult<ImportSourceRow> {
    Ok(ImportSourceRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        user_id: row.try_get("user_id")?,
        source_index: row.try_get::<i32, _>("source_index")? as i64,
        original_file_name: row
            .try_get::<Option<String>, _>("original_file_name")?
            .unwrap_or_default(),
        parser_id: row.try_get("parser_id")?,
        parser_name: row.try_get("parser_name")?,
        parser_signal: row
            .try_get::<Option<String>, _>("parser_signal")?
            .unwrap_or_default(),
        parser_confidence: row
            .try_get::<Option<f64>, _>("parser_confidence")?
            .unwrap_or_default(),
        feature_signature: row.try_get("feature_signature")?,
        metadata: row.try_get("metadata")?,
        created_at: format_pg_time(row.try_get("created_at")?),
        updated_at: format_pg_time(row.try_get("updated_at")?),
    })
}

fn import_standard_row_from_pg_row(row: &PgRow) -> DbResult<ImportStandardRow> {
    Ok(ImportStandardRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        source_id: row.try_get("source_id")?,
        user_id: row.try_get("user_id")?,
        source_index: row.try_get::<i32, _>("source_index")? as i64,
        source_row_index: row.try_get::<i32, _>("source_row_index")? as i64,
        parser_id: row.try_get("parser_id")?,
        occurred_at: format_pg_time(row.try_get("occurred_at")?),
        amount_cents: row.try_get("amount_cents")?,
        direction: row.try_get("direction")?,
        transaction_type: row.try_get("transaction_type")?,
        merchant: row
            .try_get::<Option<String>, _>("merchant")?
            .unwrap_or_default(),
        payment_method: row
            .try_get::<Option<String>, _>("payment_method")?
            .unwrap_or_default(),
        description: row
            .try_get::<Option<String>, _>("description")?
            .unwrap_or_default(),
        parser_payload: row.try_get("parser_payload")?,
        standard_payload: row.try_get("standard_payload")?,
        created_at: format_pg_time(row.try_get("created_at")?),
        updated_at: format_pg_time(row.try_get("updated_at")?),
    })
}

fn parser_template_from_pg_row(row: &PgRow) -> DbResult<ImportParserTemplateRow> {
    let payload: Value = row.try_get("parser_payload")?;
    Ok(ImportParserTemplateRow {
        id: row.try_get("id")?,
        session_id: row.try_get("session_key")?,
        user_id: row.try_get("user_id")?,
        parser_date: format_pg_time(row.try_get("occurred_at")?),
        parser_amount: (row.try_get::<i64, _>("amount_cents")? as f64) / 100.0,
        parser_type: payload_text(&payload, "parser_type")
            .unwrap_or_else(|| row.try_get("transaction_type").unwrap_or_default()),
        parser_description: payload_text(&payload, "parser_description").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("description")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        parser_id: payload_text(&payload, "parser_id")
            .or_else(|| {
                row.try_get::<Option<String>, _>("source_parser_id")
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| "auto".to_string()),
        parser_tags: payload
            .get("parser_tags")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(value_string).collect())
            .unwrap_or_default(),
        parser_counterparty: payload_text(&payload, "parser_counterparty").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("merchant")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        parser_payment_method: payload_text(&payload, "parser_payment_method").unwrap_or_else(
            || {
                row.try_get::<Option<String>, _>("payment_method")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            },
        ),
        parser_original_type: payload_text(&payload, "parser_original_type").unwrap_or_default(),
        parser_original_category: payload_text(&payload, "parser_original_category")
            .unwrap_or_default(),
        parser_account_id: payload_text(&payload, "parser_account_id").unwrap_or_default(),
        parser_is_processed: payload_bool(&payload, "parser_is_processed"),
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn preview_from_pg_row(row: &PgRow) -> DbResult<ImportPreviewRow> {
    let payload: Value = row.try_get("preview_payload")?;
    let session_id: String = row.try_get("session_key")?;
    let preview_amount_cents = payload_i64(&payload, "preview_amount_cents")
        .unwrap_or_else(|| row.try_get::<i64, _>("amount_cents").unwrap_or_default());
    Ok(ImportPreviewRow {
        id: row.try_get("id")?,
        session_id,
        user_id: row.try_get("user_id")?,
        preview_date: payload_text(&payload, "preview_date").unwrap_or_else(|| {
            format_pg_time(row.try_get("occurred_at").unwrap_or_else(|_| Utc::now()))
        }),
        preview_type: payload_text(&payload, "preview_type")
            .unwrap_or_else(|| row.try_get("transaction_type").unwrap_or_default()),
        preview_amount_cents,
        preview_destination_amount_cents: payload_i64(&payload, "preview_destination_amount_cents")
            .unwrap_or_default(),
        category_id: payload_i64(&payload, "category_id")
            .or_else(|| payload_i64(&payload, "categoryId"))
            .or_else(|| row.try_get::<Option<i64>, _>("category_id").ok().flatten()),
        preview_main_category: payload_text(&payload, "preview_main_category").unwrap_or_default(),
        preview_sub_category: payload_text(&payload, "preview_sub_category").unwrap_or_default(),
        preview_source_account_id: payload_i64(&payload, "preview_source_account_id")
            .or_else(|| row.try_get::<Option<i64>, _>("account_id").ok().flatten()),
        preview_destination_account_id: payload_i64(&payload, "preview_destination_account_id")
            .or_else(|| {
                row.try_get::<Option<i64>, _>("transfer_target_account_id")
                    .ok()
                    .flatten()
            }),
        preview_counterparty: payload_text(&payload, "preview_counterparty").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("merchant")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        preview_payment_method: payload_text(&payload, "preview_payment_method").unwrap_or_else(
            || {
                row.try_get::<Option<String>, _>("payment_method")
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            },
        ),
        preview_description: payload_text(&payload, "preview_description").unwrap_or_else(|| {
            row.try_get::<Option<String>, _>("description")
                .ok()
                .flatten()
                .unwrap_or_default()
        }),
        preview_parser_id: payload_text(&payload, "preview_parser_id").unwrap_or_default(),
        preview_parser_tags: payload_array_strings(&payload, "preview_parser_tags"),
        preview_recurring_id: payload_i64(&payload, "preview_recurring_id"),
        preview_recurring_name: payload_text(&payload, "preview_recurring_name")
            .unwrap_or_default(),
        preview_recurring_candidate_count: payload_i64(
            &payload,
            "preview_recurring_candidate_count",
        )
        .unwrap_or_default(),
        preview_recurring_match_score: payload_f64(&payload, "preview_recurring_match_score")
            .unwrap_or_default(),
        preview_recurring_match_reasons: payload_text(&payload, "preview_recurring_match_reasons")
            .unwrap_or_default(),
        preview_recurring_matched_date: payload_text(&payload, "preview_recurring_matched_date")
            .unwrap_or_default(),
        preview_selected: row
            .try_get("selected")
            .unwrap_or_else(|_| payload_bool(&payload, "preview_selected")),
        dedup_type: payload_text(&payload, "dedup_type").unwrap_or_default(),
        dedup_source_ids: row
            .try_get::<Vec<i64>, _>("merged_source_ids")
            .unwrap_or_default(),
        preview_matching_feedback: payload
            .get("preview_matching_feedback")
            .cloned()
            .unwrap_or_else(|| json!({})),
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn import_decision_member_from_pg_row(row: &PgRow) -> DbResult<ImportDecisionGroupMemberRow> {
    Ok(ImportDecisionGroupMemberRow {
        id: row.try_get("id")?,
        group_id: row.try_get("group_id")?,
        preview_row_id: row.try_get("preview_row_id")?,
        standard_row_id: row.try_get("standard_row_id")?,
        history_bill_id: row.try_get("history_bill_id")?,
        member_role: row.try_get("member_role")?,
        parser_name: row
            .try_get::<Option<String>, _>("parser_name")?
            .unwrap_or_default(),
        metadata: row.try_get("metadata")?,
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn llm_memory_event_from_pg_row(row: &PgRow) -> DbResult<LlmMemoryEventRow> {
    Ok(LlmMemoryEventRow {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        session_id: row.try_get("session_id")?,
        preview_id: row.try_get("preview_id")?,
        event_type: row.try_get("event_type")?,
        decision: row.try_get("decision")?,
        prompt_text: row.try_get("prompt_text")?,
        llm_response_raw: row.try_get("llm_response_raw")?,
        llm_provider: row.try_get("llm_provider")?,
        llm_model: row.try_get("llm_model")?,
        suggested_main_category: row.try_get("suggested_main_category")?,
        suggested_sub_category: row.try_get("suggested_sub_category")?,
        suggested_source_account: row.try_get("suggested_source_account")?,
        suggested_destination_account: row.try_get("suggested_destination_account")?,
        confidence: row.try_get("confidence")?,
        user_correction_category: row.try_get("user_correction_category")?,
        user_correction_account: row.try_get("user_correction_account")?,
        snapshot_before: row.try_get("snapshot_before")?,
        snapshot_after: row.try_get("snapshot_after")?,
        metadata: row.try_get("metadata")?,
        created_at: format_pg_time(row.try_get("created_at")?),
    })
}

fn parser_payload_from_standard_row(
    draft: &ImportStandardRowDraft,
    parser_draft: Option<&ImportParserTemplateDraft>,
) -> Value {
    if let Some(parser_draft) = parser_draft {
        return parser_payload_from_parser_template(parser_draft);
    }
    json!({
        "parser_date": draft.occurred_at,
        "parser_amount": draft.amount_cents as f64 / 100.0,
        "parser_type": draft.transaction_type,
        "parser_description": draft.description,
        "parser_id": draft.parser_payload.get("parser_id").and_then(Value::as_str).unwrap_or("auto"),
        "parser_tags": draft.parser_payload.get("parser_tags").cloned().unwrap_or_else(|| json!([])),
        "parser_counterparty": draft.merchant,
        "parser_payment_method": draft.payment_method,
        "parser_original_type": draft.parser_payload.get("original_type").and_then(Value::as_str).unwrap_or(""),
        "parser_original_category": draft.parser_payload.get("original_category").and_then(Value::as_str).unwrap_or(""),
        "parser_account_id": draft.parser_payload.get("source_account_id").and_then(Value::as_str).unwrap_or(""),
        "parser_is_processed": false
    })
}

fn parser_payload_from_parser_template(draft: &ImportParserTemplateDraft) -> Value {
    let parser_tags = serialize_parser_tags(
        draft.parser_tags.as_ref(),
        &draft.parser_id,
        &draft.parser_payment_method,
        "",
    );
    json!({
        "parser_date": draft.parser_date,
        "parser_amount": draft.parser_amount,
        "parser_type": draft.parser_type,
        "parser_description": draft.parser_description,
        "parser_id": draft.parser_id,
        "parser_tags": parse_json_array_strings(&parser_tags),
        "parser_counterparty": draft.parser_counterparty,
        "parser_payment_method": draft.parser_payment_method,
        "parser_original_type": draft.parser_original_type,
        "parser_original_category": draft.parser_original_category,
        "parser_account_id": draft.parser_account_id,
        "parser_is_processed": false
    })
}

fn preview_payload_from_draft(draft: &ImportPreviewDraft) -> Value {
    json!({
        "preview_date": normalize_bill_date_text(&draft.preview_date),
        "preview_type": draft.preview_type,
        "preview_amount_cents": draft.preview_amount_cents,
        "preview_destination_amount_cents": draft.preview_destination_amount_cents,
        "category_id": draft.category_id,
        "categoryId": draft.category_id,
        "preview_main_category": draft.preview_main_category,
        "preview_sub_category": draft.preview_sub_category,
        "preview_source_account_id": draft.preview_source_account_id,
        "preview_destination_account_id": draft.preview_destination_account_id,
        "preview_counterparty": draft.preview_counterparty,
        "preview_payment_method": draft.preview_payment_method,
        "preview_description": draft.preview_description,
        "preview_parser_id": draft.preview_parser_id,
        "preview_parser_tags": draft.preview_parser_tags.clone().unwrap_or_else(|| json!([])),
        "preview_recurring_id": draft.preview_recurring_id,
        "preview_recurring_name": draft.preview_recurring_name,
        "preview_recurring_candidate_count": draft.preview_recurring_candidate_count,
        "preview_recurring_match_score": draft.preview_recurring_match_score,
        "preview_recurring_match_reasons": draft.preview_recurring_match_reasons,
        "preview_recurring_matched_date": draft.preview_recurring_matched_date,
        "preview_selected": draft.preview_selected,
        "dedup_type": draft.dedup_type,
        "dedup_source_ids": draft.dedup_source_ids,
        "preview_matching_feedback": draft.preview_matching_feedback,
    })
}

fn apply_patch_value_to_preview(
    preview: &mut ImportPreviewRow,
    payload: &mut Value,
    field: ImportPreviewPatchField,
    value: ImportPreviewPatchValue,
) {
    match (field, value) {
        (ImportPreviewPatchField::Date, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_date = value.clone();
            payload_set(payload, "preview_date", json!(value));
        }
        (ImportPreviewPatchField::Type, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_type = value.clone();
            payload_set(payload, "preview_type", json!(value));
        }
        (ImportPreviewPatchField::Amount, ImportPreviewPatchValue::Integer(value)) => {
            preview.preview_amount_cents = value.abs();
            payload_set(payload, "preview_amount_cents", json!(value.abs()));
        }
        (ImportPreviewPatchField::DestinationAmount, ImportPreviewPatchValue::Integer(value)) => {
            preview.preview_destination_amount_cents = value.abs();
            payload_set(
                payload,
                "preview_destination_amount_cents",
                json!(value.abs()),
            );
        }
        (ImportPreviewPatchField::MainCategory, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_main_category = value.clone();
            payload_set(payload, "preview_main_category", json!(value));
        }
        (ImportPreviewPatchField::SubCategory, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_sub_category = value.clone();
            payload_set(payload, "preview_sub_category", json!(value));
        }
        (ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Integer(value)) => {
            preview.category_id = Some(value);
            payload_set(payload, "category_id", json!(value));
            payload_set(payload, "categoryId", json!(value));
        }
        (ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Null) => {
            preview.category_id = None;
            payload_set(payload, "category_id", Value::Null);
            payload_set(payload, "categoryId", Value::Null);
        }
        (ImportPreviewPatchField::SourceAccountId, ImportPreviewPatchValue::Integer(value)) => {
            preview.preview_source_account_id = Some(value);
            payload_set(payload, "preview_source_account_id", json!(value));
        }
        (ImportPreviewPatchField::SourceAccountId, ImportPreviewPatchValue::Null) => {
            preview.preview_source_account_id = None;
            payload_set(payload, "preview_source_account_id", Value::Null);
        }
        (
            ImportPreviewPatchField::DestinationAccountId,
            ImportPreviewPatchValue::Integer(value),
        ) => {
            preview.preview_destination_account_id = Some(value);
            payload_set(payload, "preview_destination_account_id", json!(value));
        }
        (ImportPreviewPatchField::DestinationAccountId, ImportPreviewPatchValue::Null) => {
            preview.preview_destination_account_id = None;
            payload_set(payload, "preview_destination_account_id", Value::Null);
        }
        (ImportPreviewPatchField::Counterparty, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_counterparty = value.clone();
            payload_set(payload, "preview_counterparty", json!(value));
        }
        (ImportPreviewPatchField::PaymentMethod, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_payment_method = value.clone();
            payload_set(payload, "preview_payment_method", json!(value));
        }
        (ImportPreviewPatchField::Description, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_description = value.clone();
            payload_set(payload, "preview_description", json!(value));
        }
        (ImportPreviewPatchField::RecurringId, ImportPreviewPatchValue::Integer(value)) => {
            preview.preview_recurring_id = Some(value);
            payload_set(payload, "preview_recurring_id", json!(value));
        }
        (ImportPreviewPatchField::RecurringId, ImportPreviewPatchValue::Null) => {
            preview.preview_recurring_id = None;
            payload_set(payload, "preview_recurring_id", Value::Null);
        }
        (ImportPreviewPatchField::RecurringName, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_recurring_name = value.clone();
            payload_set(payload, "preview_recurring_name", json!(value));
        }
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(value),
        ) => {
            preview.preview_recurring_candidate_count = value;
            payload_set(payload, "preview_recurring_candidate_count", json!(value));
        }
        (ImportPreviewPatchField::RecurringMatchScore, ImportPreviewPatchValue::Real(value)) => {
            preview.preview_recurring_match_score = value;
            payload_set(payload, "preview_recurring_match_score", json!(value));
        }
        (ImportPreviewPatchField::RecurringMatchReasons, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_recurring_match_reasons = value.clone();
            payload_set(payload, "preview_recurring_match_reasons", json!(value));
        }
        (ImportPreviewPatchField::RecurringMatchedDate, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_recurring_matched_date = value.clone();
            payload_set(payload, "preview_recurring_matched_date", json!(value));
        }
        (ImportPreviewPatchField::Selected, ImportPreviewPatchValue::Bool(value)) => {
            preview.preview_selected = value;
            payload_set(payload, "preview_selected", json!(value));
        }
        (ImportPreviewPatchField::MatchingFeedback, ImportPreviewPatchValue::Json(value)) => {
            preview.preview_matching_feedback = value.clone();
            payload_set(payload, "preview_matching_feedback", value);
        }
        _ => {}
    }
}

fn expected_state_conflicts(
    preview: &ImportPreviewRow,
    expected: Option<&ImportPreviewExpectedState>,
) -> bool {
    let Some(expected) = expected else {
        return false;
    };
    if let Some(session_id) = &expected.session_id {
        if session_id != &preview.session_id {
            return true;
        }
    }
    if let Some(value) = &expected.preview_type {
        if value != &preview.preview_type {
            return true;
        }
    }
    if let Some(value) = &expected.preview_main_category {
        if value != &preview.preview_main_category {
            return true;
        }
    }
    if let Some(value) = &expected.preview_sub_category {
        if value != &preview.preview_sub_category {
            return true;
        }
    }
    false
}

fn preview_decision_not_found() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: false,
        invalid_recurring_id: false,
    }
}

fn preview_decision_state_conflict() -> ImportPreviewDecisionResult {
    ImportPreviewDecisionResult {
        preview: None,
        state_conflict: true,
        invalid_recurring_id: false,
    }
}

fn build_preview_metadata(total: usize) -> ImportPreviewMetadata {
    let counts = ImportPreviewCounts {
        total,
        ..ImportPreviewCounts::default()
    };
    ImportPreviewMetadata {
        counts,
        facets: ImportPreviewFacets::default(),
    }
}

fn apply_preview_filters(
    rows: Vec<ImportPreviewRow>,
    filters: &ImportPreviewQueryFilters,
) -> Vec<ImportPreviewRow> {
    rows.into_iter()
        .filter(|row| {
            datetime_filter_matches(
                filters.min_datetime.as_deref(),
                filters.max_datetime.as_deref(),
                row,
            ) && selected_filter_matches(filters.selected_only, row)
                && text_filter_matches(filters.transaction_type.as_deref(), &row.preview_type)
                && category_filter_matches(filters.category.as_deref(), row)
                && account_filter_matches(filters.account.as_deref(), row)
                && tag_filter_matches(filters.tag.as_deref(), row)
                && signal_filter_matches(filters.signal.as_deref(), row)
                && annotation_filter_matches(filters.annotation.as_deref(), row)
                && text_filter_matches(filters.description.as_deref(), &row.preview_description)
        })
        .collect()
}

pub fn get_import_preview_category_by_id(
    pool: &PostgresPool,
    user_id: UserId,
    category_id: i64,
) -> DbResult<Option<ImportPreviewCategoryLookup>> {
    block_on_db(async move {
        let row = sqlx::query(import_preview_category_lookup_sql())
            .bind(category_id)
            .bind(user_id_i64(user_id)?)
            .fetch_optional(pool)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let name = row.try_get::<String, _>("name")?;
        let path = row
            .try_get::<Option<String>, _>("path")?
            .unwrap_or_default();
        let category_type = row.try_get::<Option<String>, _>("category_type")?;
        Ok(Some(import_preview_category_lookup_from_values(
            &name,
            &path,
            category_type.as_deref(),
        )))
    })
}

fn import_preview_category_lookup_sql() -> &'static str {
    r#"
        SELECT name, category_type, path
        FROM categories
        WHERE id = $1 AND user_id = $2 AND is_active = true
    "#
    .trim()
}

fn import_preview_category_lookup_from_values(
    name: &str,
    path: &str,
    category_type: Option<&str>,
) -> ImportPreviewCategoryLookup {
    let (main_category, sub_category) = preview_category_names_from_path(path, name);
    let type_code = category_type.and_then(preview_category_type_code);
    ImportPreviewCategoryLookup {
        type_code,
        main_category,
        sub_category,
    }
}

fn preview_category_names_from_path(path: &str, name: &str) -> (String, String) {
    let parts = path
        .split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    match parts.as_slice() {
        [] => (name.to_string(), String::new()),
        [main] => ((*main).to_string(), String::new()),
        [main, rest @ ..] => ((*main).to_string(), rest.join("/")),
    }
}

fn preview_category_type_code(value: &str) -> Option<i64> {
    match value.trim().to_ascii_lowercase().as_str() {
        "2" | "income" | "收入" => Some(2),
        "3" | "expense" | "支出" => Some(3),
        "4" | "transfer" | "转账" => Some(4),
        "5" | "investment" | "投资" => Some(5),
        _ => None,
    }
}

fn datetime_filter_matches(
    min_datetime: Option<&str>,
    max_datetime: Option<&str>,
    row: &ImportPreviewRow,
) -> bool {
    let preview_date = normalize_bill_date_text(&row.preview_date);
    min_datetime
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none_or(|value| preview_date.as_str() >= value)
        && max_datetime
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none_or(|value| preview_date.as_str() <= value)
}

fn selected_filter_matches(selected_only: bool, row: &ImportPreviewRow) -> bool {
    !selected_only || row.preview_selected
}

fn category_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == "__none__" {
        return row.category_id.is_none()
            && row.preview_main_category.trim().is_empty()
            && row.preview_sub_category.trim().is_empty();
    }
    if filter == "__invalid__" {
        return preview_has_missing_category_issue(row);
    }
    row.category_id
        .is_some_and(|category_id| category_id.to_string() == filter)
        || row.preview_main_category == filter
        || row.preview_sub_category == filter
}

fn account_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == "__none__" {
        return row.preview_source_account_id.is_none()
            && row.preview_destination_account_id.is_none()
            && row.preview_payment_method.trim().is_empty();
    }
    if filter == "__invalid__" {
        return preview_account_filter_invalid_matches(row);
    }
    let source = row.preview_source_account_id.map(|value| value.to_string());
    let destination = row
        .preview_destination_account_id
        .map(|value| value.to_string());
    source.as_deref().is_some_and(|value| value == filter)
        || destination.as_deref().is_some_and(|value| value == filter)
}

fn text_filter_matches(filter: Option<&str>, value: &str) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    value.to_lowercase().contains(&filter.to_lowercase())
}

fn tag_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if matches!(filter, "__none__" | "__invalid__") {
        return row.preview_parser_tags.is_empty();
    }
    row.preview_parser_tags
        .iter()
        .any(|tag| tag.eq_ignore_ascii_case(filter) || text_filter_matches(Some(filter), tag))
}

fn signal_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let filter = filter.to_ascii_lowercase();
    preview_feedback_contains_signal(&row.preview_matching_feedback, &filter)
}

fn annotation_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    if filter == "needs-review" {
        return preview_requires_review(row);
    }
    if filter == "no-issues" {
        return !preview_requires_review(row);
    }
    let filter = filter.to_ascii_lowercase();
    row.preview_matching_feedback
        .get("annotation")
        .is_some_and(|value| json_value_contains_text(value, &filter))
}

fn preview_feedback_contains_signal(feedback: &Value, filter: &str) -> bool {
    match filter.split_once(':') {
        Some((family, status)) => feedback
            .get(family)
            .is_some_and(|value| json_value_contains_text(value, status)),
        None => json_value_contains_text(feedback, filter),
    }
}

fn json_value_contains_text(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(text) => text.to_ascii_lowercase().contains(needle),
        Value::Array(items) => items
            .iter()
            .any(|item| json_value_contains_text(item, needle)),
        Value::Object(object) => object
            .values()
            .any(|item| json_value_contains_text(item, needle)),
        Value::Number(number) => number.to_string().contains(needle),
        Value::Bool(value) => value.to_string().contains(needle),
        Value::Null => false,
    }
}

fn sort_preview_rows(rows: &mut [ImportPreviewRow], sort_by: &str, sort_direction: &str) {
    let descending = sort_direction.eq_ignore_ascii_case("desc");
    rows.sort_by(|left, right| {
        let order = match sort_by {
            "amount_cents"
            | "preview_amount_cents"
            | "previewAmountCents"
            | "sourceAmountCents" => left.preview_amount_cents.cmp(&right.preview_amount_cents),
            "counterparty" => left.preview_counterparty.cmp(&right.preview_counterparty),
            "type" => preview_type_sort_rank(&left.preview_type)
                .cmp(&preview_type_sort_rank(&right.preview_type))
                .then(left.preview_type.cmp(&right.preview_type)),
            "paymentMethod" => left
                .preview_payment_method
                .cmp(&right.preview_payment_method),
            "comment" => left.preview_description.cmp(&right.preview_description),
            "time" => left.preview_date.cmp(&right.preview_date),
            _ => left
                .preview_date
                .cmp(&right.preview_date)
                .then(left.id.cmp(&right.id)),
        };
        if descending {
            order.reverse()
        } else {
            order
        }
    });
}

fn preview_type_sort_rank(value: &str) -> u8 {
    match value.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => 0,
        "支出" | "expense" | "3" => 1,
        "转账" | "transfer" | "4" => 2,
        "投资" | "investment" | "5" => 3,
        _ => 4,
    }
}

fn preview_requires_review(preview: &ImportPreviewRow) -> bool {
    preview_has_missing_category_issue(preview)
        || preview_has_missing_source_account_issue(preview)
        || preview_has_missing_destination_account_issue(preview)
        || preview_has_same_transfer_accounts_issue(preview)
}

fn preview_has_missing_category_issue(preview: &ImportPreviewRow) -> bool {
    preview_category_required(&preview.preview_type) && preview.category_id.is_none()
}

fn preview_has_missing_source_account_issue(preview: &ImportPreviewRow) -> bool {
    preview.preview_source_account_id.is_none()
}

fn preview_has_missing_destination_account_issue(preview: &ImportPreviewRow) -> bool {
    preview_destination_account_required(&preview.preview_type)
        && preview.preview_destination_account_id.is_none()
}

fn preview_has_same_transfer_accounts_issue(preview: &ImportPreviewRow) -> bool {
    preview_destination_account_required(&preview.preview_type)
        && preview.preview_source_account_id.is_some()
        && preview.preview_destination_account_id.is_some()
        && preview.preview_source_account_id == preview.preview_destination_account_id
}

fn preview_account_filter_invalid_matches(preview: &ImportPreviewRow) -> bool {
    if preview_transfer_filter_type(&preview.preview_type) {
        return preview.preview_source_account_id.is_none()
            || preview.preview_destination_account_id.is_none();
    }
    preview.preview_source_account_id.is_none()
}

fn preview_category_required(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "收入"
            | "income"
            | "2"
            | "支出"
            | "expense"
            | "3"
            | "转账"
            | "transfer"
            | "4"
            | "投资"
            | "investment"
            | "5"
    )
}

fn preview_destination_account_required(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "转账" | "transfer" | "4" | "投资" | "investment" | "5"
    )
}

fn preview_transfer_filter_type(preview_type: &str) -> bool {
    matches!(
        preview_type.trim().to_ascii_lowercase().as_str(),
        "转账" | "transfer" | "4"
    )
}

fn set_feedback_review_status(mut feedback: Value, key: &str, status: &str) -> Value {
    if !feedback.is_object() {
        feedback = json!({});
    }
    let object = feedback.as_object_mut().expect("feedback object");
    let mut child = object.get(key).cloned().unwrap_or_else(|| json!({}));
    if !child.is_object() {
        child = json!({});
    }
    child
        .as_object_mut()
        .expect("child object")
        .insert("review_status".to_string(), json!(status));
    object.insert(key.to_string(), child);
    feedback
}

fn clear_feedback_key(feedback: &mut Value, key: &str) {
    if let Some(object) = feedback.as_object_mut() {
        object.remove(key);
    }
}

fn payload_set(payload: &mut Value, key: &str, value: Value) {
    if !payload.is_object() {
        *payload = json!({});
    }
    payload
        .as_object_mut()
        .expect("payload object")
        .insert(key.to_string(), value);
}

fn payload_text(payload: &Value, key: &str) -> Option<String> {
    payload.get(key).and_then(value_string)
}

fn payload_i64(payload: &Value, key: &str) -> Option<i64> {
    payload.get(key).and_then(Value::as_i64)
}

fn payload_f64(payload: &Value, key: &str) -> Option<f64> {
    payload.get(key).and_then(Value::as_f64)
}

fn payload_bool(payload: &Value, key: &str) -> bool {
    payload.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn payload_array_strings(payload: &Value, key: &str) -> Vec<String> {
    payload
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(value_string).collect())
        .unwrap_or_default()
}

fn value_string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_string)
}

fn parse_json_array_strings(raw: &str) -> Vec<String> {
    serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
}

fn format_pg_time(value: DateTime<Utc>) -> String {
    value.naive_utc().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn truncate_text_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    value[..end].to_string()
}

fn user_id_i64(user_id: UserId) -> DbResult<i64> {
    i64::try_from(user_id.get())
        .map_err(|_| DbError::InvalidOperation("invalid user id".to_string()))
}

fn finite_float_text(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        value.to_string()
    }
}

fn standard_bill_parser_tags_value(bill: &StandardBill) -> Option<Value> {
    if bill.parser_tags.is_empty() {
        None
    } else {
        serde_json::to_value(&bill.parser_tags).ok()
    }
}

fn dedup_bill_parser_tags_value(bill: &DedupBill) -> Option<Value> {
    let mut tags = bill.parser_tags.clone();
    if !bill.destination_parser_id.trim().is_empty() {
        tags.push(format!("parser:{}", bill.destination_parser_id.trim()));
    }
    if tags.is_empty() {
        None
    } else {
        serde_json::to_value(tags).ok()
    }
}

fn parser_template_type(transaction_type: &str, amount: Money) -> String {
    let transaction_type = transaction_type.trim();
    if !transaction_type.is_empty() {
        return transaction_type.to_string();
    }

    if amount.is_positive() {
        "收入".to_string()
    } else if amount.is_negative() {
        "支出".to_string()
    } else {
        "其他".to_string()
    }
}

fn preview_destination_amount_cents_for_bill(bill: &DedupBill, amount_cents: i64) -> i64 {
    if is_investment_type(&bill.transaction_type) || is_transfer_type(&bill.transaction_type) {
        amount_cents.abs()
    } else {
        0
    }
}

fn is_transfer_type(bill_type: &str) -> bool {
    matches!(
        bill_type.trim().to_ascii_lowercase().as_str(),
        "转账" | "transfer" | "4"
    )
}

fn is_investment_type(bill_type: &str) -> bool {
    matches!(
        bill_type.trim().to_ascii_lowercase().as_str(),
        "投资" | "investment" | "5"
    )
}

fn parse_positive_i64(value: &str) -> Option<i64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<i64>().ok().filter(|value| *value > 0)
}

fn first_non_empty(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    values
        .into_iter()
        .map(|value| value.as_ref().trim().to_string())
        .find(|value| !value.is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod import_preview_query_tests {
    use super::*;
    use sqlx::Execute;

    fn preview_row(id: i64) -> ImportPreviewRow {
        ImportPreviewRow {
            id,
            session_id: "session".to_string(),
            user_id: 1,
            preview_date: "2026-01-01 09:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 1000,
            preview_destination_amount_cents: 0,
            category_id: None,
            preview_main_category: "餐饮".to_string(),
            preview_sub_category: "午餐".to_string(),
            preview_source_account_id: Some(11),
            preview_destination_account_id: None,
            preview_counterparty: "商户".to_string(),
            preview_payment_method: "付款卡".to_string(),
            preview_description: "默认备注".to_string(),
            preview_parser_id: "fixture".to_string(),
            preview_parser_tags: vec!["parser:fixture".to_string()],
            preview_recurring_id: None,
            preview_recurring_name: String::new(),
            preview_recurring_candidate_count: 0,
            preview_recurring_match_score: 0.0,
            preview_recurring_match_reasons: String::new(),
            preview_recurring_matched_date: String::new(),
            preview_selected: true,
            dedup_type: String::new(),
            dedup_source_ids: Vec::new(),
            preview_matching_feedback: json!({}),
            created_at: "2026-01-01 09:00:00".to_string(),
        }
    }

    #[test]
    fn preview_filters_cover_server_paged_contract_fields() {
        let mut first = preview_row(1);
        first.preview_date = "2026-01-10 10:00:00".to_string();
        first.preview_description = "含 手续费".to_string();
        first.preview_parser_tags = vec!["银行".to_string(), "工资".to_string()];
        first.preview_matching_feedback = json!({
            "annotation": {"status": "missing_category"},
            "learning": {"review_status": "needs_review"}
        });

        let mut second = preview_row(2);
        second.preview_date = "2026-02-01 10:00:00".to_string();
        second.preview_selected = false;
        second.preview_parser_tags = vec!["微信".to_string()];
        second.preview_matching_feedback = json!({
            "annotation": {"status": "ok"},
            "learning": {"review_status": "none"}
        });

        let filters = ImportPreviewQueryFilters {
            min_datetime: Some("2026-01-01 00:00:00".to_string()),
            max_datetime: Some("2026-01-31 23:59:59".to_string()),
            tag: Some("工资".to_string()),
            signal: Some("learning:needs_review".to_string()),
            annotation: Some("missing_category".to_string()),
            description: Some("手续费".to_string()),
            selected_only: true,
            ..ImportPreviewQueryFilters::default()
        };

        let rows = apply_preview_filters(vec![first, second], &filters);
        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn preview_category_filter_uses_persisted_category_identity() {
        let mut matched = preview_row(1);
        matched.category_id = Some(42);
        matched.preview_main_category = "理财".to_string();
        matched.preview_sub_category = "理财收益".to_string();

        let mut same_name_wrong_identity = preview_row(2);
        same_name_wrong_identity.category_id = Some(99);
        same_name_wrong_identity.preview_main_category = "理财".to_string();
        same_name_wrong_identity.preview_sub_category = "理财收益".to_string();

        let mut missing = preview_row(3);
        missing.category_id = None;
        missing.preview_main_category.clear();
        missing.preview_sub_category.clear();

        let id_filters = ImportPreviewQueryFilters {
            category: Some("42".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let rows = apply_preview_filters(
            vec![
                matched.clone(),
                same_name_wrong_identity.clone(),
                missing.clone(),
            ],
            &id_filters,
        );
        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1]
        );

        let missing_filters = ImportPreviewQueryFilters {
            category: Some("__none__".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let rows = apply_preview_filters(
            vec![matched, same_name_wrong_identity, missing],
            &missing_filters,
        );
        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![3]
        );

        let mut named_without_identity = preview_row(4);
        named_without_identity.category_id = None;
        named_without_identity.preview_main_category = "理财".to_string();
        named_without_identity.preview_sub_category = "理财收益".to_string();
        let invalid_filters = ImportPreviewQueryFilters {
            category: Some("__invalid__".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let rows = apply_preview_filters(vec![named_without_identity], &invalid_filters);
        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![4]
        );
    }

    #[test]
    fn preview_filters_preserve_account_tag_and_annotation_sentinels() {
        let mut valid = preview_row(1);
        valid.category_id = Some(42);
        valid.preview_source_account_id = Some(11);
        valid.preview_parser_tags = vec!["工资".to_string()];

        let mut invalid = preview_row(2);
        invalid.preview_source_account_id = None;
        invalid.preview_parser_tags.clear();

        let invalid_filters = ImportPreviewQueryFilters {
            account: Some("__invalid__".to_string()),
            tag: Some("__invalid__".to_string()),
            annotation: Some("needs-review".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let rows = apply_preview_filters(vec![valid.clone(), invalid.clone()], &invalid_filters);
        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2]
        );

        let no_issue_filters = ImportPreviewQueryFilters {
            annotation: Some("no-issues".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let rows = apply_preview_filters(vec![valid, invalid], &no_issue_filters);
        assert_eq!(
            rows.into_iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn preview_bulk_insert_value_preserves_amount_and_category_identity() {
        let draft = ImportPreviewDraft {
            preview_date: "2026-01-01 09:00:00".to_string(),
            preview_type: "收入".to_string(),
            preview_amount_cents: 1234,
            category_id: Some(42),
            preview_main_category: "理财".to_string(),
            preview_sub_category: "理财收益".to_string(),
            preview_counterparty: "基金平台".to_string(),
            preview_payment_method: "招商卡".to_string(),
            preview_description: "收益".to_string(),
            preview_selected: true,
            dedup_source_ids: vec![7, 8],
            ..ImportPreviewDraft::default()
        };

        let value = preview_row_batch_value_from_draft(&draft);
        let payload: Value = serde_json::from_str(&value.preview_payload).expect("valid payload");

        assert_eq!(value.amount_cents, 1234);
        assert_eq!(value.direction, "income");
        assert_eq!(value.category_id, Some(42));
        assert_eq!(value.merged_source_ids, vec![7, 8]);
        assert_eq!(payload["category_id"], json!(42));
        assert_eq!(payload["categoryId"], json!(42));
    }

    #[test]
    fn preview_sql_query_builder_covers_server_filter_and_sort_contract() {
        let filters = ImportPreviewQueryFilters {
            min_datetime: Some("2026-01-01".to_string()),
            max_datetime: Some("2026-01-31".to_string()),
            transaction_type: Some("支出".to_string()),
            category: Some("42".to_string()),
            account: Some("__none__".to_string()),
            tag: Some("工资".to_string()),
            signal: Some("learning:needs_review".to_string()),
            annotation: Some("missing_category".to_string()),
            description: Some("手续费".to_string()),
            selected_only: true,
        };
        let mut query =
            build_preview_page_query(1, 2, &filters, "sourceAmountCents", "desc", 50, 100);

        let built = query.build();
        let sql = built.sql();

        assert!(sql.contains("JOIN import_sessions"));
        assert!(sql.contains("p.selected = true"));
        assert!(sql.contains("p.category_id::text"));
        assert!(sql.contains("p.account_id IS NULL"));
        assert!(sql.contains("preview_matching_feedback'->"));
        assert!(sql.contains("ORDER BY p.amount_cents DESC"));
        assert!(sql.contains("LIMIT"));
        assert!(sql.contains("OFFSET"));

        let mut count_query = build_preview_count_query(1, 2, &filters);
        let count_sql = count_query.build().sql().to_string();
        assert!(count_sql.contains("SELECT COUNT(*)::BIGINT"));
        assert!(count_sql.contains("p.selected = true"));
    }

    #[test]
    fn preview_sql_query_builder_covers_none_category_and_account_id_filters() {
        let filters = ImportPreviewQueryFilters {
            category: Some("__none__".to_string()),
            account: Some("11".to_string()),
            signal: Some("learning".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
        );
        query.push_bind(1_i64);
        push_preview_query_predicates(&mut query, &filters, "p");
        push_preview_order_by(&mut query, "type", "asc");

        let built = query.build();
        let sql = built.sql();

        assert!(sql.contains("p.category_id IS NULL"));
        assert!(sql.contains("p.account_id::text"));
        assert!(sql.contains("preview_matching_feedback')::text ILIKE"));
        assert!(sql.contains("CASE lower(p.transaction_type)"));
    }

    #[test]
    fn preview_sql_query_builder_preserves_invalid_sentinel_semantics() {
        let filters = ImportPreviewQueryFilters {
            category: Some("__invalid__".to_string()),
            account: Some("__invalid__".to_string()),
            tag: Some("__invalid__".to_string()),
            annotation: Some("needs-review".to_string()),
            ..ImportPreviewQueryFilters::default()
        };
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT p.* FROM import_preview_rows p WHERE p.session_id = ",
        );
        query.push_bind(1_i64);
        push_preview_query_predicates(&mut query, &filters, "p");

        let sql = query.build().sql().to_string();

        assert!(sql.contains("lower(p.transaction_type) IN ('收入'"));
        assert!(sql.contains("p.category_id IS NULL"));
        assert!(sql.contains("NOT (lower(p.transaction_type) IN ('转账'"));
        assert!(sql.contains("jsonb_array_length"));
        assert!(sql.contains("p.transfer_target_account_id IS NULL"));
    }

    #[test]
    fn preview_page_result_builder_filters_sorts_and_pages_rows() {
        let mut first = preview_row(1);
        first.preview_selected = true;
        first.preview_amount_cents = 3000;
        first.preview_description = "保留 3".to_string();

        let mut second = preview_row(2);
        second.preview_selected = false;
        second.preview_amount_cents = 4000;
        second.preview_description = "过滤".to_string();

        let mut third = preview_row(3);
        third.preview_selected = true;
        third.preview_amount_cents = 1000;
        third.preview_description = "保留 1".to_string();

        let request = ImportPreviewPageRequest {
            page: 2,
            page_size: 1,
            sort_by: "sourceAmountCents".to_string(),
            sort_direction: "desc".to_string(),
            filters: ImportPreviewQueryFilters {
                selected_only: true,
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        };

        let result = build_preview_page_result_from_rows(vec![third, second, first], &request);

        assert_eq!(result.total, 2);
        assert_eq!(result.page, 2);
        assert_eq!(result.page_size, 1);
        assert_eq!(
            result
                .rows
                .into_iter()
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(result.metadata.counts.total, 2);
    }

    #[test]
    fn bill_create_fields_from_preview_preserves_category_identity() {
        let mut row = preview_row(1);
        row.category_id = Some(42);
        row.preview_main_category = "理财".to_string();
        row.preview_sub_category = "理财收益".to_string();

        let fields = bill_create_fields_from_preview(&row);

        assert_eq!(fields.get("category_id"), Some(&json!(42)));
        assert_eq!(fields.get("main_category"), Some(&json!("理财")));
        assert_eq!(fields.get("sub_category"), Some(&json!("理财收益")));
    }

    #[test]
    fn preview_selection_update_query_covers_modes_filters_and_ids() {
        let request = ImportPreviewPageRequest {
            preview_ids: vec![11, 12],
            filters: ImportPreviewQueryFilters {
                category: Some("42".to_string()),
                selected_only: true,
                ..ImportPreviewQueryFilters::default()
            },
            ..ImportPreviewPageRequest::default()
        };

        let mut select_query = build_preview_selection_update_query(
            1,
            2,
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::All,
            &request,
        );
        let select_sql = select_query.build().sql().to_string();
        assert!(select_sql.contains("UPDATE import_preview_rows p SET selected ="));
        assert!(select_sql.contains("jsonb_set"));
        assert!(select_sql.contains("p.selected = true"));
        assert!(select_sql.contains("p.category_id::text"));
        assert!(select_sql.contains("p.id IN"));

        let mut deselect_query = build_preview_selection_update_query(
            1,
            2,
            ImportPreviewSelectionMode::Deselect,
            ImportPreviewSelectionTarget::All,
            &ImportPreviewPageRequest::default(),
        );
        let deselect_sql = deselect_query.build().sql().to_string();
        assert!(deselect_sql.contains("to_jsonb($"));

        let mut invert_query = build_preview_selection_update_query(
            1,
            2,
            ImportPreviewSelectionMode::Invert,
            ImportPreviewSelectionTarget::All,
            &ImportPreviewPageRequest::default(),
        );
        let invert_sql = invert_query.build().sql().to_string();
        assert!(invert_sql.contains("NOT p.selected"));

        let mut needs_review_query = build_preview_selection_update_query(
            1,
            2,
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::NeedsReview,
            &ImportPreviewPageRequest::default(),
        );
        let needs_review_sql = needs_review_query.build().sql().to_string();
        assert!(needs_review_sql.contains("p.category_id IS NULL"));
        assert!(needs_review_sql.contains("p.account_id IS NULL"));
        assert!(needs_review_sql.contains("p.transfer_target_account_id IS NULL"));

        let mut valid_query = build_preview_selection_update_query(
            1,
            2,
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::Valid,
            &ImportPreviewPageRequest::default(),
        );
        let valid_sql = valid_query.build().sql().to_string();
        assert!(valid_sql.contains("AND NOT ("));
    }

    #[test]
    fn standard_row_bulk_insert_value_preserves_parser_payload_and_amount() {
        let draft = ImportParserTemplateDraft {
            parser_date: "2026-01-01 09:00:00".to_string(),
            parser_amount: -19.88,
            parser_type: "支出".to_string(),
            parser_description: "午餐".to_string(),
            parser_id: "fixture".to_string(),
            parser_counterparty: "餐厅".to_string(),
            parser_payment_method: "招商卡".to_string(),
            parser_original_type: "消费".to_string(),
            parser_original_category: "餐饮".to_string(),
            parser_account_id: "11".to_string(),
            ..ImportParserTemplateDraft::default()
        };

        let value = standard_row_batch_value_from_parser_template(5, 3, &draft);
        let parser_payload: Value =
            serde_json::from_str(&value.parser_payload).expect("valid parser payload");

        assert_eq!(value.source_id, 5);
        assert_eq!(value.source_row_index, 3);
        assert_eq!(value.amount_cents, 1988);
        assert_eq!(value.direction, "expense");
        assert_eq!(parser_payload["parser_id"], json!("fixture"));

        let values = standard_row_batch_values_from_parser_templates(5, &[draft]);
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].source_row_index, 0);
        assert_eq!(values[0].parser_payload, value.parser_payload);
    }

    #[test]
    fn standard_row_batch_values_from_drafts_use_source_fallback_and_parser_payload() {
        let parser_draft = ImportParserTemplateDraft {
            parser_id: "wechat_pay".to_string(),
            parser_original_type: "交易".to_string(),
            parser_original_category: "餐饮".to_string(),
            parser_account_id: "card-1".to_string(),
            ..ImportParserTemplateDraft::default()
        };
        let standard_row = ImportStandardRowDraft {
            source_index: 99,
            source_row_index: 3,
            occurred_at: "2026-01-01 09:00:00".to_string(),
            amount_cents: 1234,
            direction: "expense".to_string(),
            transaction_type: "支出".to_string(),
            merchant: "餐厅".to_string(),
            payment_method: "招商卡".to_string(),
            description: "午餐".to_string(),
            parser_payload: json!({"raw": true}),
            standard_payload: json!({"normalized": true}),
        };
        let mut source_ids = std::collections::BTreeMap::new();
        source_ids.insert(0, 100);

        let values = standard_row_batch_values_from_standard_row_drafts(
            &[parser_draft],
            &[standard_row],
            &source_ids,
        )
        .expect("source fallback value");

        assert_eq!(values.len(), 1);
        assert_eq!(values[0].source_id, 100);
        assert_eq!(values[0].source_row_index, 3);
        let parser_payload: Value =
            serde_json::from_str(&values[0].parser_payload).expect("parser payload json");
        let standard_payload: Value =
            serde_json::from_str(&values[0].standard_payload).expect("standard payload json");
        assert_eq!(parser_payload["parser_id"], json!("wechat_pay"));
        assert_eq!(parser_payload["parser_original_category"], json!("餐饮"));
        assert_eq!(standard_payload["normalized"], json!(true));

        let missing = standard_row_batch_values_from_standard_row_drafts(&[], &[], &source_ids);
        assert_eq!(missing.expect("empty standard rows").len(), 0);
        let missing_source = standard_row_batch_values_from_standard_row_drafts(
            &[],
            &[ImportStandardRowDraft {
                source_index: 1,
                source_row_index: 0,
                occurred_at: String::new(),
                amount_cents: 0,
                direction: String::new(),
                transaction_type: String::new(),
                merchant: String::new(),
                payment_method: String::new(),
                description: String::new(),
                parser_payload: json!({}),
                standard_payload: json!({}),
            }],
            &std::collections::BTreeMap::new(),
        );
        assert!(missing_source.is_err());
    }

    #[test]
    fn preview_patch_value_updates_category_identity_in_row_and_payload() {
        let mut row = preview_row(1);
        let mut payload = json!({});

        apply_patch_value_to_preview(
            &mut row,
            &mut payload,
            ImportPreviewPatchField::CategoryId,
            ImportPreviewPatchValue::Integer(42),
        );

        assert_eq!(row.category_id, Some(42));
        assert_eq!(payload["category_id"], json!(42));
        assert_eq!(payload["categoryId"], json!(42));

        apply_patch_value_to_preview(
            &mut row,
            &mut payload,
            ImportPreviewPatchField::CategoryId,
            ImportPreviewPatchValue::Null,
        );

        assert_eq!(row.category_id, None);
        assert_eq!(payload["category_id"], Value::Null);
        assert_eq!(payload["categoryId"], Value::Null);

        apply_patch_value_to_preview(
            &mut row,
            &mut payload,
            ImportPreviewPatchField::Amount,
            ImportPreviewPatchValue::Integer(-12345),
        );
        assert_eq!(row.preview_amount_cents, 12345);
        assert_eq!(payload["preview_amount_cents"], json!(12345));

        apply_patch_value_to_preview(
            &mut row,
            &mut payload,
            ImportPreviewPatchField::DestinationAmount,
            ImportPreviewPatchValue::Integer(-54321),
        );
        assert_eq!(row.preview_destination_amount_cents, 54321);
        assert_eq!(payload["preview_destination_amount_cents"], json!(54321));
    }

    #[test]
    fn preview_category_lookup_helpers_preserve_type_path_and_sql_contract() {
        let lookup =
            import_preview_category_lookup_from_values("理财收益", "理财/理财收益", Some("income"));

        assert_eq!(lookup.type_code, Some(2));
        assert_eq!(lookup.main_category, "理财");
        assert_eq!(lookup.sub_category, "理财收益");
        assert!(import_preview_category_lookup_sql().contains("FROM categories"));
        assert!(import_preview_category_lookup_sql().contains("is_active = true"));

        let fallback = import_preview_category_lookup_from_values("未分类", "", Some("unknown"));
        assert_eq!(fallback.type_code, None);
        assert_eq!(fallback.main_category, "未分类");
        assert!(fallback.sub_category.is_empty());

        let single = preview_category_names_from_path("理财", "理财");
        assert_eq!(single, ("理财".to_string(), String::new()));
    }

    #[test]
    fn preview_filter_helpers_cover_none_account_and_nested_feedback_edges() {
        let mut row = preview_row(1);
        row.preview_source_account_id = None;
        row.preview_destination_account_id = None;
        row.preview_payment_method.clear();
        row.preview_matching_feedback = json!([
            {"learning": ["needs_review", {"reason": "manual"}]},
            12,
            true,
            null
        ]);

        assert!(account_filter_matches(Some("__none__"), &row));
        assert!(signal_filter_matches(Some("manual"), &row));
        assert!(signal_filter_matches(Some("12"), &row));
        assert!(signal_filter_matches(Some("true"), &row));
        assert!(!signal_filter_matches(Some("missing"), &row));
    }

    #[tokio::test]
    async fn preview_pg_row_projection_reads_explicit_cents_payload_when_database_available(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return Ok(());
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await?;
        let row = sqlx::query(
            r#"
            SELECT
                7::BIGINT AS id,
                'session-1'::TEXT AS session_key,
                2::BIGINT AS user_id,
                now() AS occurred_at,
                'expense'::TEXT AS transaction_type,
                111::BIGINT AS amount_cents,
                9::BIGINT AS category_id,
                11::BIGINT AS account_id,
                12::BIGINT AS transfer_target_account_id,
                '商户'::TEXT AS merchant,
                '招商卡'::TEXT AS payment_method,
                '午餐'::TEXT AS description,
                true AS selected,
                ARRAY[1, 2]::BIGINT[] AS merged_source_ids,
                '{
                    "preview_date": "2026-06-01 09:00:00",
                    "preview_type": "支出",
                    "preview_amount_cents": 12345,
                    "preview_destination_amount_cents": 54321,
                    "preview_main_category": "餐饮",
                    "preview_sub_category": "午餐",
                    "preview_matching_feedback": {"learning": "accepted"}
                }'::jsonb AS preview_payload,
                now() AS created_at
            "#,
        )
        .fetch_one(&pool)
        .await?;

        let projected = preview_from_pg_row(&row).expect("preview row");

        assert_eq!(projected.preview_amount_cents, 12345);
        assert_eq!(projected.preview_destination_amount_cents, 54321);
        assert_eq!(projected.preview_main_category, "餐饮");
        assert_eq!(projected.dedup_source_ids, vec![1, 2]);
        Ok(())
    }

    #[test]
    fn bulk_insert_query_builders_preserve_insert_shapes() {
        let preview_draft = ImportPreviewDraft {
            preview_date: "2026-01-01 09:00:00".to_string(),
            preview_type: "支出".to_string(),
            preview_amount_cents: 1000,
            category_id: Some(42),
            preview_counterparty: "商户".to_string(),
            preview_payment_method: "招商卡".to_string(),
            preview_description: "备注".to_string(),
            dedup_source_ids: vec![1, 2],
            ..ImportPreviewDraft::default()
        };
        let preview_values = vec![preview_row_batch_value_from_draft(&preview_draft)];
        let mut preview_builder = build_preview_rows_insert_query(1, 2, &preview_values);
        let preview_query = preview_builder.build();
        let preview_sql = preview_query.sql();
        assert!(preview_sql.contains("INSERT INTO import_preview_rows"));
        assert!(preview_sql.contains("category_id"));
        assert!(preview_sql.contains("preview_payload"));

        let parser_draft = ImportParserTemplateDraft {
            parser_date: "2026-01-01 09:00:00".to_string(),
            parser_amount: 10.0,
            parser_type: "收入".to_string(),
            parser_id: "fixture".to_string(),
            ..ImportParserTemplateDraft::default()
        };
        let standard_values = vec![standard_row_batch_value_from_parser_template(
            3,
            4,
            &parser_draft,
        )];
        let mut standard_builder = build_standard_rows_insert_query(1, 2, &standard_values);
        let standard_query = standard_builder.build();
        let standard_sql = standard_query.sql();
        assert!(standard_sql.contains("INSERT INTO import_standard_rows"));
        assert!(standard_sql.contains("ON CONFLICT (source_id, source_row_index)"));
        assert!(standard_sql.contains("standard_payload"));
        let mut single_insert_builder = build_preview_row_insert_returning_query(
            1,
            2,
            &preview_draft,
            json!({"category_id": 42}).to_string(),
            1000,
            "expense",
        );
        let single_insert_sql = single_insert_builder.build().sql().to_string();
        assert!(single_insert_sql.contains("INSERT INTO import_preview_rows"));
        assert!(single_insert_sql.contains("category_id"));
        assert!(single_insert_sql.contains("RETURNING id"));

        let mut preview = preview_row(7);
        preview.category_id = Some(42);
        let mut update_builder = build_preview_row_update_query(
            &preview,
            json!({"category_id": 42}).to_string(),
            1000,
            "expense",
            7,
            1,
            2,
        );
        let update_sql = update_builder.build().sql().to_string();
        assert!(update_sql.contains("UPDATE import_preview_rows"));
        assert!(update_sql.contains("category_id ="));
        assert!(update_sql.contains("WHERE id ="));
    }

    #[test]
    fn preview_sort_accepts_frontend_server_paged_keys() {
        let mut earlier = preview_row(1);
        earlier.preview_date = "2026-01-01 09:00:00".to_string();
        earlier.preview_type = "支出".to_string();
        earlier.preview_amount_cents = 3000;
        earlier.preview_payment_method = "B卡".to_string();
        earlier.preview_description = "bbb".to_string();

        let mut later = preview_row(2);
        later.preview_date = "2026-01-02 09:00:00".to_string();
        later.preview_type = "收入".to_string();
        later.preview_amount_cents = 1000;
        later.preview_payment_method = "A卡".to_string();
        later.preview_description = "aaa".to_string();

        let mut rows = vec![earlier.clone(), later.clone()];
        sort_preview_rows(&mut rows, "time", "desc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 1]
        );

        let mut rows = vec![earlier.clone(), later.clone()];
        sort_preview_rows(&mut rows, "sourceAmountCents", "asc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 1]
        );

        let mut rows = vec![earlier.clone(), later.clone()];
        sort_preview_rows(&mut rows, "type", "asc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 1]
        );

        let mut rows = vec![earlier.clone(), later.clone()];
        sort_preview_rows(&mut rows, "paymentMethod", "asc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 1]
        );

        let mut rows = vec![earlier, later];
        sort_preview_rows(&mut rows, "comment", "asc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![2, 1]
        );

        let mut fallback_earlier = preview_row(1);
        fallback_earlier.preview_date = "2026-01-01 09:00:00".to_string();
        let mut fallback_later = preview_row(2);
        fallback_later.preview_date = "2026-01-02 09:00:00".to_string();
        let mut rows = vec![fallback_later, fallback_earlier];
        sort_preview_rows(&mut rows, "unknown", "asc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2]
        );

        let mut transfer = preview_row(1);
        transfer.preview_type = "转账".to_string();
        let mut investment = preview_row(2);
        investment.preview_type = "投资".to_string();
        let mut rows = vec![investment, transfer];
        sort_preview_rows(&mut rows, "type", "asc");
        assert_eq!(
            rows.iter().map(|row| row.id).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }
}
