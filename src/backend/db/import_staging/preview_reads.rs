pub fn insert_preview_bill(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportPreviewDraft,
) -> DbResult<i64> {
    insert_preview_bill_on_connection(connection, session_id, user_id, draft)?;
    Ok(connection.last_insert_rowid())
}

pub fn insert_preview_bills_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportPreviewDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let created_at = now_text();
        let user_id = user_id_i64(user_id)?;
        let mut statement = tx.prepare(INSERT_PREVIEW_BILL_SQL)?;
        for draft in drafts {
            insert_preview_bill_with_statement(&mut statement, session_id, user_id, draft, &created_at)?;
        }
        Ok(drafts.len())
    })
}

pub fn get_preview_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<Vec<ImportPreviewRow>> {
    let mut query =
        "SELECT * FROM bills_preview WHERE session_id = ?1 AND user_id = ?2".to_string();
    if selected_only {
        query.push_str(" AND preview_selected = 1");
    }
    query.push_str(" ORDER BY preview_date ASC, id ASC");

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(params![session_id, user_id_i64(user_id)?], preview_from_row)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn count_preview_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    selected_only: bool,
) -> DbResult<i64> {
    let selected_clause = if selected_only {
        " AND preview_selected = 1"
    } else {
        ""
    };
    connection
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause}"
            ),
            params![session_id, user_id_i64(user_id)?],
            |row| row.get(0),
        )
        .map_err(DbError::from)
}

pub fn get_preview_page_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    page: i64,
    page_size: i64,
    selected_only: bool,
) -> DbResult<(Vec<ImportPreviewRow>, i64)> {
    let normalized_page = page.max(1);
    let normalized_page_size = page_size.max(1);
    let offset = (normalized_page - 1) * normalized_page_size;
    let selected_clause = if selected_only {
        " AND preview_selected = 1"
    } else {
        ""
    };
    let total: i64 = connection.query_row(
        &format!(
            "SELECT COUNT(*) FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause}"
        ),
        params![session_id, user_id_i64(user_id)?],
        |row| row.get(0),
    )?;
    let mut statement = connection.prepare(&format!(
        "SELECT * FROM bills_preview WHERE session_id = ?1 AND user_id = ?2{selected_clause} \
         ORDER BY preview_date ASC, id ASC LIMIT ?3 OFFSET ?4"
    ))?;
    let rows = statement.query_map(
        params![
            session_id,
            user_id_i64(user_id)?,
            normalized_page_size,
            offset
        ],
        preview_from_row,
    )?;
    Ok((rows.collect::<Result<Vec<_>, _>>()?, total))
}

pub fn get_preview_filter_index_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewFilterIndexRow>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            preview_date,
            preview_type,
            preview_amount,
            preview_main_category,
            preview_sub_category,
            preview_source_account_id,
            preview_destination_account_id,
            preview_counterparty,
            preview_payment_method,
            preview_description,
            preview_parser_id,
            preview_parser_tags_json,
            preview_recurring_id,
            preview_recurring_candidate_count,
            preview_recurring_match_reasons,
            preview_recurring_matched_date,
            preview_selected,
            dedup_type,
            dedup_source_ids,
            preview_matching_feedback_json
        FROM bills_preview
        WHERE session_id = ?1 AND user_id = ?2
        ORDER BY preview_date ASC, id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![session_id, user_id_i64(user_id)?],
        preview_filter_index_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn get_preview_bill_by_id(
    connection: &Connection,
    preview_id: i64,
    user_id: UserId,
) -> DbResult<Option<ImportPreviewRow>> {
    connection
        .query_row(
            "SELECT * FROM bills_preview WHERE id = ?1 AND user_id = ?2",
            params![preview_id, user_id_i64(user_id)?],
            preview_from_row,
        )
        .optional()
        .map_err(DbError::from)
}

pub fn get_preview_by_ids(
    connection: &Connection,
    session_id: &str,
    preview_ids: &[i64],
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewRow>> {
    let normalized_ids: Vec<i64> = preview_ids
        .iter()
        .copied()
        .filter(|preview_id| *preview_id > 0)
        .collect();
    if normalized_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = std::iter::repeat_n("?", normalized_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let mut statement = connection.prepare(&format!(
        "SELECT * FROM bills_preview WHERE session_id = ? AND user_id = ? AND id IN ({placeholders})"
    ))?;
    let user_id = user_id_i64(user_id)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(normalized_ids.len() + 2);
    params.push(&session_id);
    params.push(&user_id);
    for preview_id in &normalized_ids {
        params.push(preview_id);
    }
    let rows = statement.query_map(params.as_slice(), preview_from_row)?;
    let lookup = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|preview| (preview.id, preview))
        .collect::<std::collections::HashMap<_, _>>();
    Ok(normalized_ids
        .iter()
        .filter_map(|preview_id| lookup.get(preview_id).cloned())
        .collect())
}
