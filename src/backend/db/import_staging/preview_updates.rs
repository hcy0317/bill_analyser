pub fn update_preview_bill(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<bool> {
    Ok(execute_preview_patch(connection, session_id, user_id, patch)? > 0)
}

pub fn update_preview_bills_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    if patches.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let mut updated_count = 0;
        for patch in patches {
            if execute_preview_patch(tx, session_id, user_id, patch)? > 0 {
                updated_count += 1;
            }
        }
        Ok(updated_count)
    })
}

pub fn batch_update_preview_classification(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    updates: &[ImportPreviewClassificationUpdate],
) -> DbResult<usize> {
    if updates.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        let user_id = user_id_i64(user_id)?;
        let mut updated_count = 0;
        for update in updates {
            if update.preview_id <= 0 {
                continue;
            }
            let changed = tx.execute(
                "
                UPDATE bills_preview SET
                    preview_type = ?1,
                    preview_main_category = ?2,
                    preview_sub_category = ?3,
                    preview_source_account_id = ?4,
                    preview_destination_account_id = ?5
                WHERE id = ?6 AND session_id = ?7 AND user_id = ?8
                ",
                params![
                    update.preview_type,
                    update.preview_main_category,
                    update.preview_sub_category,
                    update.preview_source_account_id,
                    update.preview_destination_account_id,
                    update.preview_id,
                    session_id,
                    user_id
                ],
            )?;
            if changed > 0 {
                updated_count += 1;
            }
        }
        Ok(updated_count)
    })
}
