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

use crate::{create_postgres_bill, BillCreateDraft, DbError, DbResult, PostgresPool};

const LLM_MEMORY_PROMPT_TEXT_MAX_BYTES: usize = 16_384;

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
        for draft in drafts {
            insert_preview_row_async(pool, session_db_id, user_id, draft).await?;
        }
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
        for (index, draft) in drafts.iter().enumerate() {
            insert_standard_row_from_parser_template(
                pool,
                session_db_id,
                source_id,
                user_id_i64,
                i64::try_from(index).unwrap_or(i64::MAX),
                draft,
            )
            .await?;
        }
        Ok(drafts.len())
    })
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
    let mut rows = if request.preview_ids.is_empty() {
        get_preview_by_session(pool, session_id, user_id, false)?
    } else {
        get_preview_by_ids(pool, session_id, &request.preview_ids, user_id)?
    };
    rows = apply_preview_filters(rows, &request.filters);
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
    Ok(ImportPreviewPageResult {
        rows: page_rows,
        total,
        page,
        page_size,
        metadata: build_preview_metadata(total),
    })
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
            preview_amount: row.preview_amount,
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
    selected: bool,
    request: &ImportPreviewPageRequest,
) -> DbResult<usize> {
    let ids = query_preview_page_by_session(pool, session_id, user_id, request)?
        .rows
        .into_iter()
        .map(|row| row.id)
        .collect::<Vec<_>>();
    update_preview_selection(pool, &ids, selected, user_id)
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
            let mut fields = Map::new();
            fields.insert("date".to_string(), json!(preview.preview_date));
            fields.insert("type".to_string(), json!(preview.preview_type));
            fields.insert("amount".to_string(), json!(preview.preview_amount));
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
            if let Some(value) = preview.preview_source_account_id {
                fields.insert("source_account_id".to_string(), json!(value));
            }
            if let Some(value) = preview.preview_destination_account_id {
                fields.insert("destination_account_id".to_string(), json!(value));
            }
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
        for (index, draft) in standard_row_drafts.iter().enumerate() {
            let source_id = source_ids
                .get(&draft.source_index)
                .or_else(|| source_ids.values().next())
                .copied()
                .ok_or_else(|| DbError::InvalidOperation("import source missing".to_string()))?;
            let parser_draft = parser_drafts.get(index);
            insert_standard_row(pool, session_db_id, source_id, user_id, draft, parser_draft)
                .await?;
        }
        return Ok(standard_row_drafts.len());
    }
    let source_id = source_ids
        .values()
        .next()
        .copied()
        .ok_or_else(|| DbError::InvalidOperation("import source missing".to_string()))?;
    for (index, draft) in parser_drafts.iter().enumerate() {
        insert_standard_row_from_parser_template(
            pool,
            session_db_id,
            source_id,
            user_id,
            i64::try_from(index).unwrap_or(i64::MAX),
            draft,
        )
        .await?;
    }
    Ok(parser_drafts.len())
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
    let amount_cents = Money::from_yuan_str(&finite_float_text(draft.preview_amount))
        .unwrap_or(Money::ZERO)
        .to_cents()
        .abs();
    let direction = if draft.preview_type == "收入" || draft.preview_type == "income" {
        "income"
    } else {
        "expense"
    };
    let id = sqlx::query(
        r#"
        INSERT INTO import_preview_rows (
            session_id, user_id, page_sort_key, operation_kind, selected,
            signal_summary, merged_source_ids, occurred_at, amount_cents,
            direction, transaction_type, account_id, transfer_target_account_id,
            merchant, payment_method, description, preview_payload, created_at, updated_at
        ) VALUES (
            $1,$2,$3,'insert',$4,'[]'::jsonb,$5,$6::timestamptz,$7,
            $8,$9,$10,$11,$12,$13,$14,$15::jsonb,now(),now()
        )
        RETURNING id
        "#,
    )
    .bind(session_db_id)
    .bind(user_id)
    .bind(format!(
        "{}:{}",
        normalize_bill_date_text(&draft.preview_date),
        draft.preview_counterparty
    ))
    .bind(draft.preview_selected)
    .bind(&draft.dedup_source_ids)
    .bind(normalize_bill_date_text(&draft.preview_date))
    .bind(amount_cents)
    .bind(direction)
    .bind(&draft.preview_type)
    .bind(draft.preview_source_account_id)
    .bind(draft.preview_destination_account_id)
    .bind(&draft.preview_counterparty)
    .bind(&draft.preview_payment_method)
    .bind(&draft.preview_description)
    .bind(payload.to_string())
    .fetch_one(pool)
    .await?
    .try_get("id")?;
    Ok(id)
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
    let amount_cents = Money::from_yuan_str(&finite_float_text(preview.preview_amount))
        .unwrap_or(Money::ZERO)
        .to_cents()
        .abs();
    let direction = if preview.preview_type == "收入" || preview.preview_type == "income" {
        "income"
    } else {
        "expense"
    };
    let changed = sqlx::query(
        r#"
        UPDATE import_preview_rows
        SET selected = $1,
            occurred_at = $2::timestamptz,
            amount_cents = $3,
            direction = $4,
            transaction_type = $5,
            account_id = $6,
            transfer_target_account_id = $7,
            merchant = $8,
            payment_method = $9,
            description = $10,
            preview_payload = $11::jsonb,
            updated_at = now(),
            version = version + 1
        WHERE id = $12 AND session_id = $13 AND user_id = $14
        "#,
    )
    .bind(preview.preview_selected)
    .bind(normalize_bill_date_text(&preview.preview_date))
    .bind(amount_cents)
    .bind(direction)
    .bind(&preview.preview_type)
    .bind(preview.preview_source_account_id)
    .bind(preview.preview_destination_account_id)
    .bind(&preview.preview_counterparty)
    .bind(&preview.preview_payment_method)
    .bind(&preview.preview_description)
    .bind(payload.to_string())
    .bind(patch.preview_id)
    .bind(session_db_id)
    .bind(user_id)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(changed > 0)
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
    let preview_amount = payload_f64(&payload, "preview_amount").unwrap_or_else(|| {
        row.try_get::<i64, _>("amount_cents").unwrap_or_default() as f64 / 100.0
    });
    Ok(ImportPreviewRow {
        id: row.try_get("id")?,
        session_id,
        user_id: row.try_get("user_id")?,
        preview_date: payload_text(&payload, "preview_date").unwrap_or_else(|| {
            format_pg_time(row.try_get("occurred_at").unwrap_or_else(|_| Utc::now()))
        }),
        preview_type: payload_text(&payload, "preview_type")
            .unwrap_or_else(|| row.try_get("transaction_type").unwrap_or_default()),
        preview_amount,
        preview_destination_amount: payload_f64(&payload, "preview_destination_amount")
            .unwrap_or_default(),
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
        "preview_amount": draft.preview_amount,
        "preview_destination_amount": draft.preview_destination_amount,
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
        (ImportPreviewPatchField::Amount, ImportPreviewPatchValue::Real(value)) => {
            preview.preview_amount = value;
            payload_set(payload, "preview_amount", json!(value));
        }
        (ImportPreviewPatchField::DestinationAmount, ImportPreviewPatchValue::Real(value)) => {
            preview.preview_destination_amount = value;
            payload_set(payload, "preview_destination_amount", json!(value));
        }
        (ImportPreviewPatchField::MainCategory, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_main_category = value.clone();
            payload_set(payload, "preview_main_category", json!(value));
        }
        (ImportPreviewPatchField::SubCategory, ImportPreviewPatchValue::Text(value)) => {
            preview.preview_sub_category = value.clone();
            payload_set(payload, "preview_sub_category", json!(value));
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
            text_filter_matches(filters.transaction_type.as_deref(), &row.preview_type)
                && category_filter_matches(filters.category.as_deref(), row)
                && account_filter_matches(filters.account.as_deref(), row)
                && text_filter_matches(filters.description.as_deref(), &row.preview_description)
        })
        .collect()
}

fn category_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    text_filter_matches(filter, &row.preview_main_category)
        || text_filter_matches(filter, &row.preview_sub_category)
}

fn account_filter_matches(filter: Option<&str>, row: &ImportPreviewRow) -> bool {
    let Some(filter) = filter.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
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

fn sort_preview_rows(rows: &mut [ImportPreviewRow], sort_by: &str, sort_direction: &str) {
    let descending = sort_direction.eq_ignore_ascii_case("desc");
    rows.sort_by(|left, right| {
        let order = match sort_by {
            "amount" | "preview_amount" => left.preview_amount.total_cmp(&right.preview_amount),
            "counterparty" => left.preview_counterparty.cmp(&right.preview_counterparty),
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

fn preview_requires_review(preview: &ImportPreviewRow) -> bool {
    preview.preview_type == "转账"
        && (preview.preview_source_account_id.is_none()
            || preview.preview_destination_account_id.is_none()
            || preview.preview_source_account_id == preview.preview_destination_account_id)
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

fn preview_destination_amount_for_bill(bill: &DedupBill, amount: f64) -> f64 {
    if is_investment_type(&bill.transaction_type) || is_transfer_type(&bill.transaction_type) {
        amount.abs()
    } else {
        0.0
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

fn money_to_yuan_f64(amount: Money) -> f64 {
    amount.to_yuan_string().parse::<f64>().unwrap_or_default()
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
