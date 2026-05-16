pub fn update_preview_selection(
    connection: &mut Connection,
    preview_ids: &[i64],
    selected: bool,
    user_id: UserId,
) -> DbResult<usize> {
    if preview_ids.is_empty() {
        return Ok(0);
    }
    let selected_value = i64::from(selected);
    run_transaction(connection, |tx| {
        for preview_id in preview_ids {
            tx.execute(
                "UPDATE bills_preview SET preview_selected = ?1 WHERE user_id = ?2 AND id = ?3",
                params![selected_value, user_id_i64(user_id)?, preview_id],
            )?;
        }
        Ok(preview_ids.len())
    })
}

pub fn reset_session_preview_selection(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    let changed = connection.execute(
        "UPDATE bills_preview SET preview_selected = 0 WHERE session_id = ?1 AND user_id = ?2",
        params![session_id, user_id_i64(user_id)?],
    )?;
    Ok(changed)
}

pub fn replace_preview_selection_with_patches(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    run_transaction(connection, |tx| {
        tx.execute(
            "UPDATE bills_preview SET preview_selected = 0 WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id_i64(user_id)?],
        )?;
        let mut updated_count = 0;
        for patch in patches {
            if execute_preview_patch(tx, session_id, user_id, patch)? > 0 {
                updated_count += 1;
            }
        }
        Ok(updated_count)
    })
}

pub fn save_import_annotation_samples(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    samples: &[ImportAnnotationSampleDraft],
) -> DbResult<usize> {
    if samples.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let now = now_text();
        let mut saved_count = 0;
        for sample in samples {
            if sample.preview_id <= 0 {
                continue;
            }
            let changed = tx.execute(
                "
                INSERT INTO import_annotation_samples (
                    session_id, user_id, preview_id,
                    annotated_type, annotated_category_id,
                    annotated_source_account_id, annotated_destination_account_id,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                ON CONFLICT(session_id, preview_id) DO UPDATE SET
                    annotated_type = excluded.annotated_type,
                    annotated_category_id = excluded.annotated_category_id,
                    annotated_source_account_id = excluded.annotated_source_account_id,
                    annotated_destination_account_id = excluded.annotated_destination_account_id,
                    updated_at = excluded.updated_at
                WHERE import_annotation_samples.user_id = excluded.user_id
                ",
                params![
                    session_id,
                    user_id,
                    sample.preview_id,
                    sample.annotated_type,
                    sample.annotated_category_id,
                    sample.annotated_source_account_id,
                    sample.annotated_destination_account_id,
                    now
                ],
            )?;
            if changed > 0 {
                saved_count += 1;
            }
        }
        Ok(saved_count)
    })
}

pub fn get_import_annotation_samples(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportAnnotationSampleRow>> {
    let mut statement = connection.prepare(
        "
        SELECT * FROM import_annotation_samples
        WHERE session_id = ?1 AND user_id = ?2
        ORDER BY updated_at ASC, id ASC
        ",
    )?;
    let rows = statement.query_map(
        params![session_id, user_id_i64(user_id)?],
        annotation_sample_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn clear_session_data(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ClearSessionDataResult> {
    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let parser_count = tx.execute(
            "DELETE FROM bills_parser_template WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        let preview_count = tx.execute(
            "DELETE FROM bills_preview WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        let annotation_count = tx.execute(
            "DELETE FROM import_annotation_samples WHERE session_id = ?1 AND user_id = ?2",
            params![session_id, user_id],
        )?;
        Ok(ClearSessionDataResult {
            parser_count,
            preview_count,
            annotation_count,
        })
    })
}

pub fn confirm_preview_to_bills(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<ConfirmPreviewResult> {
    let user_scope = user_id;
    let user_id = user_id_i64(user_scope)?;
    let now = now_text();
    let batch_id = Utc::now().format("%Y%m%d%H%M%S").to_string();

    run_transaction(connection, |tx| {
        if let Some(session) = get_import_session(tx, session_id, user_scope)? {
            if session.status == "completed" {
                return Ok(ConfirmPreviewResult {
                    confirmed_count: i64_to_usize_saturating(session.total_confirmed),
                    skipped_count: 0,
                    duplicate_count: 0,
                    errors: Vec::new(),
                });
            }
        }
        let previews = get_preview_by_session(tx, session_id, user_scope, true)?;
        let mut result = ConfirmPreviewResult {
            confirmed_count: 0,
            skipped_count: 0,
            duplicate_count: 0,
            errors: Vec::new(),
        };

        for preview in &previews {
            let bill_type = normalize_confirm_bill_type(&preview.preview_type);
            let preview_date_text = normalize_bill_date_text(&preview.preview_date);
            let amount = confirm_amount_for_type(&bill_type, preview.preview_amount);
            let bill_hash = calculate_import_bill_hash(
                &preview_date_text,
                &bill_type,
                amount,
                &preview.preview_counterparty,
                &preview.preview_description,
            );
            let insert_result = tx.execute(
                "
                INSERT INTO bills (
                    user_id, date, type, amount, counterparty, description,
                    payment_method, main_category, sub_category,
                    source_account_id, destination_account_id, destination_amount,
                    batch_id, hash, created_from_recurring, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                ",
                params![
                    user_id,
                    preview_date_text,
                    bill_type,
                    amount,
                    preview.preview_counterparty,
                    preview.preview_description,
                    preview.preview_payment_method,
                    preview.preview_main_category,
                    preview.preview_sub_category,
                    preview.preview_source_account_id,
                    preview.preview_destination_account_id,
                    preview.preview_destination_amount,
                    batch_id,
                    bill_hash,
                    preview.preview_recurring_id,
                    now,
                    now,
                ],
            );

            match insert_result {
                Ok(_) => result.confirmed_count += 1,
                Err(error) if is_sqlite_constraint_error(&error) => result.duplicate_count += 1,
                Err(error) => return Err(DbError::from(error)),
            }
        }

        tx.execute(
            "
            UPDATE import_sessions
            SET status = 'completed',
                updated_at = ?1,
                total_confirmed = MAX(total_confirmed, ?2)
            WHERE session_id = ?3 AND user_id = ?4
            ",
            params![now, result.confirmed_count as i64, session_id, user_id],
        )?;
        Ok(result)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClearSessionDataResult {
    pub parser_count: usize,
    pub preview_count: usize,
    pub annotation_count: usize,
}
