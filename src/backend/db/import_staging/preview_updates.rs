// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_preview_bill(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    patch: &ImportPreviewPatch,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "update_preview_bill", "business operation entered");
    Ok(execute_preview_patch(connection, session_id, user_id, patch)? > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
pub fn update_preview_bills_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    patches: &[ImportPreviewPatch],
) -> DbResult<usize> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "update_preview_bills_batch", "business operation entered");
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

#[tracing::instrument(level = "debug", skip_all)]
pub fn batch_update_preview_classification(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    updates: &[ImportPreviewClassificationUpdate],
) -> DbResult<usize> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "import_parser", operation = "batch_update_preview_classification", "business operation entered");
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
