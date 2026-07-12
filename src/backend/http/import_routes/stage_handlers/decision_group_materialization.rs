struct ImportDecisionGroupMaterializationInput {
    pool: PostgresPool,
    session_id: String,
    user_id: UserId,
    duplicate_groups: Vec<DuplicateGroup>,
    transfer_pairs: Vec<TransferPair>,
    templates: Vec<bill_analyser_db::ImportParserTemplateRow>,
    standard_rows: Vec<bill_analyser_db::ImportStandardRow>,
    history_duplicate_plan: Vec<HistoryDuplicatePreviewPlan>,
    history_transfer_plan: Vec<HistoryTransferPreviewPlan>,
}

fn spawn_import_decision_group_materialization(input: ImportDecisionGroupMaterializationInput) {
    tokio::task::spawn_blocking(move || {
        let started_at = Instant::now();
        let preview_rows_for_groups =
            match get_preview_by_session(&input.pool, &input.session_id, input.user_id, false) {
                Ok(rows) => rows,
                Err(error) => {
                    let _ = set_import_decision_materialization_failed(
                        &input.pool,
                        &input.session_id,
                        input.user_id,
                        &error.to_string(),
                    );
                    tracing::error!(
                        domain = "import_parser",
                        operation = "import_decision_group_materialization",
                        user_id = input.user_id.get(),
                        session_id = %input.session_id,
                        error = %error,
                        "import decision group materialization failed to load preview rows"
                    );
                    return;
                }
            };
        let decision_groups = build_import_match_decision_groups(ImportMatchDecisionGroupInput {
            session_id: &input.session_id,
            duplicate_groups: &input.duplicate_groups,
            transfer_pairs: &input.transfer_pairs,
            templates: &input.templates,
            standard_rows: &input.standard_rows,
            preview_rows: &preview_rows_for_groups,
            history_duplicate_plan: &input.history_duplicate_plan,
            history_transfer_plan: &input.history_transfer_plan,
        });
        match insert_import_decision_groups_batch(
            &input.pool,
            &input.session_id,
            input.user_id,
            &decision_groups,
        ) {
            Ok(inserted) => {
                if let Err(error) = set_import_decision_materialization_status(
                    &input.pool,
                    &input.session_id,
                    input.user_id,
                    "completed",
                ) {
                    tracing::error!(operation = "import_decision_group_materialization", error = %error, "failed to mark decision group materialization completed");
                    return;
                }
                tracing::debug!(
                    domain = "import_parser",
                    operation = "import_decision_group_materialization",
                    user_id = input.user_id.get(),
                    session_id = %input.session_id,
                    decision_groups = inserted,
                    elapsed_ms = import_stage_elapsed_ms(started_at),
                    "import decision group materialization completed"
                );
            }
            Err(error) => {
                let _ = set_import_decision_materialization_failed(
                    &input.pool,
                    &input.session_id,
                    input.user_id,
                    &error.to_string(),
                );
                tracing::error!(
                    domain = "import_parser",
                    operation = "import_decision_group_materialization",
                    user_id = input.user_id.get(),
                    session_id = %input.session_id,
                    error = %error,
                    elapsed_ms = import_stage_elapsed_ms(started_at),
                    "import decision group materialization failed"
                );
            }
        }
    });
}
