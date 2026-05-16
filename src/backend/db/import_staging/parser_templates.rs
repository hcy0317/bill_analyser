pub fn insert_parser_template(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    draft: &ImportParserTemplateDraft,
) -> DbResult<i64> {
    insert_parser_template_on_connection(connection, session_id, user_id, draft)?;
    Ok(connection.last_insert_rowid())
}

pub fn insert_parser_templates_batch(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
    drafts: &[ImportParserTemplateDraft],
) -> DbResult<usize> {
    if drafts.is_empty() {
        return Ok(0);
    }

    run_transaction(connection, |tx| {
        for draft in drafts {
            insert_parser_template_on_connection(tx, session_id, user_id, draft)?;
        }
        Ok(drafts.len())
    })
}

pub fn stage_import_parser_templates(
    connection: &mut Connection,
    session: &ImportSessionDraft,
    drafts: &[ImportParserTemplateDraft],
    require_existing_session: bool,
) -> DbResult<ImportParseStagingResult> {
    run_transaction(connection, |tx| {
        let existing_session = get_import_session(tx, &session.session_id, session.user_id)?;
        if require_existing_session && existing_session.is_none() {
            return Ok(ImportParseStagingResult {
                inserted_count: 0,
                total_parsed: 0,
                session_found: false,
            });
        }
        let previous_parsed = existing_session
            .as_ref()
            .map(|row| row.total_parsed)
            .unwrap_or_default();
        if existing_session.is_none() {
            create_import_session(tx, session)?;
        }
        let mut inserted_count = 0;
        for draft in drafts {
            insert_parser_template_on_connection(tx, &session.session_id, session.user_id, draft)?;
            inserted_count += 1;
        }
        let total_parsed = previous_parsed.saturating_add(usize_to_i64_saturating(inserted_count));
        update_import_session_status(
            tx,
            &ImportSessionStatusUpdate {
                session_id: session.session_id.clone(),
                user_id: session.user_id,
                status: "parsed".to_string(),
                total_parsed: Some(total_parsed),
                total_preview: None,
                total_confirmed: None,
            },
        )?;
        Ok(ImportParseStagingResult {
            inserted_count,
            total_parsed,
            session_found: true,
        })
    })
}

pub fn get_parser_templates_by_session(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
    processed_only: Option<bool>,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    let mut query =
        "SELECT * FROM bills_parser_template WHERE session_id = ?1 AND user_id = ?2".to_string();
    match processed_only {
        Some(true) => query.push_str(" AND parser_is_processed = '1'"),
        Some(false) => query.push_str(" AND parser_is_processed = '0'"),
        None => {}
    }
    query.push_str(" ORDER BY parser_date ASC, id ASC");

    let mut statement = connection.prepare(&query)?;
    let rows = statement.query_map(
        params![session_id, user_id_i64(user_id)?],
        parser_template_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

pub fn get_unprocessed_templates_for_dedup(
    connection: &Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<Vec<ImportParserTemplateRow>> {
    get_parser_templates_by_session(connection, session_id, user_id, Some(false))
}

pub fn update_parser_template_status(
    connection: &mut Connection,
    template_ids: &[i64],
    processed: bool,
    account_id: Option<&str>,
    user_id: UserId,
) -> DbResult<usize> {
    let normalized_ids: Vec<i64> = template_ids
        .iter()
        .copied()
        .filter(|template_id| *template_id > 0)
        .collect();
    if normalized_ids.is_empty() {
        return Ok(0);
    }

    let processed_value = if processed { "1" } else { "0" };
    let user_id = user_id_i64(user_id)?;
    let placeholders = std::iter::repeat_n("?", normalized_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let account_update = if account_id.is_some() {
        ", parser_account_id = ?"
    } else {
        ""
    };
    let query = format!(
        "UPDATE bills_parser_template SET parser_is_processed = ?{account_update} \
         WHERE id IN ({placeholders}) AND user_id = ?"
    );

    run_transaction(connection, |tx| {
        let mut params = Vec::with_capacity(normalized_ids.len() + 3);
        params.push(SqlValue::Text(processed_value.to_string()));
        if let Some(account_id) = account_id {
            params.push(SqlValue::Text(account_id.to_string()));
        }
        for template_id in &normalized_ids {
            params.push(SqlValue::Integer(*template_id));
        }
        params.push(SqlValue::Integer(user_id));
        tx.execute(&query, rusqlite::params_from_iter(params))
            .map_err(DbError::from)
    })
}

pub fn mark_unprocessed_parser_templates_processed_for_session(
    connection: &mut Connection,
    session_id: &str,
    user_id: UserId,
) -> DbResult<usize> {
    let user_id = user_id_i64(user_id)?;
    run_transaction(connection, |tx| {
        tx.execute(
            "
            UPDATE bills_parser_template
            SET parser_is_processed = '1'
            WHERE session_id = ?1 AND user_id = ?2 AND parser_is_processed = '0'
            ",
            params![session_id, user_id],
        )
        .map_err(DbError::from)
    })
}

#[cfg(test)]
mod parser_template_tests {
    use super::*;
    use std::error::Error;

    fn test_user_id() -> UserId {
        UserId::new(42).expect("positive user id")
    }

    fn seed_user(connection: &Connection) -> DbResult<()> {
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY, username TEXT NOT NULL);",
        )?;
        connection.execute(
            "INSERT INTO users(id, username) VALUES (?1, ?2)",
            params![42_i64, "user-42"],
        )?;
        Ok(())
    }

    #[test]
    fn parser_template_empty_single_and_missing_session_edges_are_pinned(
    ) -> Result<(), Box<dyn Error>> {
        let mut connection = Connection::open_in_memory()?;
        seed_user(&connection)?;
        init_import_staging_schema(&connection)?;

        assert_eq!(
            insert_parser_templates_batch(&mut connection, "session-a", test_user_id(), &[])?,
            0
        );
        assert_eq!(
            update_parser_template_status(&mut connection, &[], true, None, test_user_id())?,
            0
        );

        let draft = ImportParserTemplateDraft {
            parser_date: "2026-05-01".to_string(),
            parser_amount: 12.5,
            parser_type: "支出".to_string(),
            parser_description: "coffee".to_string(),
            parser_id: "wechat".to_string(),
            parser_tags: None,
            parser_counterparty: "cafe".to_string(),
            parser_payment_method: "card".to_string(),
            parser_original_type: String::new(),
            parser_original_category: String::new(),
            parser_account_id: String::new(),
        };

        let missing = stage_import_parser_templates(
            &mut connection,
            &ImportSessionDraft {
                session_id: "missing-session".to_string(),
                user_id: test_user_id(),
                file_count: 1,
            },
            std::slice::from_ref(&draft),
            true,
        )?;
        assert_eq!(
            missing,
            ImportParseStagingResult {
                inserted_count: 0,
                total_parsed: 0,
                session_found: false,
            }
        );

        create_import_session(
            &connection,
            &ImportSessionDraft {
                session_id: "session-a".to_string(),
                user_id: test_user_id(),
                file_count: 1,
            },
        )?;
        let inserted_id = insert_parser_template(&connection, "session-a", test_user_id(), &draft)?;
        assert!(inserted_id > 0);

        let rows = get_parser_templates_by_session(&connection, "session-a", test_user_id(), None)?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].parser_description, "coffee");
        Ok(())
    }
}
