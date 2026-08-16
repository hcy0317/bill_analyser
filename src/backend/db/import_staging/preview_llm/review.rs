/// 记录用户对 LLM 建议的审核结果，并保持 preview row 与 memory feedback 一致。
pub fn review_preview_llm_recommendation(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    review_preview_llm_recommendation_with_expected_state(pool, request)
}

/// 通过 matching candidate action 审核 LLM 信号，并在同一事务内校验 expected state。
pub fn review_preview_llm_matching_action(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewDecisionResult> {
    review_preview_llm_recommendation_with_expected_state(pool, request).map(
        |result| ImportPreviewDecisionResult {
            preview: result.preview,
            state_conflict: result.state_conflict,
            invalid_recurring_id: false,
        },
    )
}

fn review_preview_llm_recommendation_with_expected_state(
    pool: &PostgresPool,
    request: &ImportPreviewLlmReviewRequest<'_>,
) -> DbResult<ImportPreviewLlmDecisionResult> {
    let decision_name = match request.decision {
        ImportPreviewDecision::Accept => "accept",
        ImportPreviewDecision::Reject => "reject",
        ImportPreviewDecision::Clear => "clear",
    };
    tracing::debug!(
        domain = "import_signal_lifecycle",
        operation = "lifecycle_transition",
        signal_family = "llm",
        outcome = "started",
        session_key = %request.session_id,
        decision = decision_name,
        "import preview signal lifecycle transition started"
    );
    block_on_db(async move {
        let user_id = user_id_i64(request.user_id)?;
        let mut transaction = pool.begin().await?;
        let Some(row) = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            FOR UPDATE OF p
            "#,
        )
        .bind(request.preview_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?
        else {
            transaction.rollback().await?;
            tracing::debug!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "not_found",
                session_key = %request.session_id,
                decision = decision_name,
                "import preview signal lifecycle transition not applied"
            );
            return Ok(empty_llm_decision_result());
        };
        let mut preview = preview_from_pg_row(&row)?;
        if preview.session_id != request.session_id {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "session_conflict",
                session_key = %request.session_id,
                decision = decision_name,
                "import preview signal lifecycle transition rejected"
            );
            return Ok(empty_llm_decision_result());
        }
        let decision = decision_name;
        let current_status = preview_llm_review_status(&preview.preview_matching_feedback);
        match preview_terminal_action(
            current_status.as_deref(),
            preview.preview_matching_feedback.get("llm").is_some(),
            request.decision,
        ) {
            ImportPreviewTerminalAction::Idempotent => {
                transaction.commit().await?;
                tracing::info!(
                    domain = "import_signal_lifecycle",
                    operation = "lifecycle_transition",
                    signal_family = "llm",
                    outcome = "idempotent",
                    session_key = %request.session_id,
                    decision = decision_name,
                    previous_state = current_status.as_deref().unwrap_or("none"),
                    next_state = current_status.as_deref().unwrap_or("none"),
                    "import preview signal lifecycle transition completed"
                );
                return Ok(ImportPreviewLlmDecisionResult {
                    preview: Some(preview),
                    event_id: None,
                    applied_fields: Vec::new(),
                    state_conflict: false,
                });
            }
            ImportPreviewTerminalAction::Conflict => {
                transaction.rollback().await?;
                tracing::warn!(
                    domain = "import_signal_lifecycle",
                    operation = "lifecycle_transition",
                    signal_family = "llm",
                    outcome = "state_conflict",
                    session_key = %request.session_id,
                    decision = decision_name,
                    previous_state = current_status.as_deref().unwrap_or("none"),
                    "import preview signal lifecycle transition rejected"
                );
                return Ok(ImportPreviewLlmDecisionResult {
                    preview: Some(preview),
                    event_id: None,
                    applied_fields: Vec::new(),
                    state_conflict: true,
                });
            }
            ImportPreviewTerminalAction::Apply => {}
        }
        if let Some(expected_row_version) = request
            .expected_state
            .and_then(|expected| expected.expected_row_version)
            .filter(|expected| *expected != preview.version)
        {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "row_version_conflict",
                session_key = %request.session_id,
                decision = decision_name,
                expected_row_version,
                actual_row_version = preview.version,
                "import preview signal lifecycle transition rejected"
            );
            return Err(DbError::preview_version_conflict(
                request.preview_id,
                expected_row_version,
                preview.version,
            ));
        }
        if recommendation_expected_state_conflicts(
            &preview,
            request.expected_state,
            current_status.as_deref(),
        ) {
            transaction.rollback().await?;
            tracing::warn!(
                domain = "import_signal_lifecycle",
                operation = "lifecycle_transition",
                signal_family = "llm",
                outcome = "expected_state_conflict",
                session_key = %request.session_id,
                decision = decision_name,
                previous_state = current_status.as_deref().unwrap_or("none"),
                "import preview signal lifecycle transition rejected"
            );
            return Ok(ImportPreviewLlmDecisionResult {
                preview: Some(preview),
                event_id: None,
                applied_fields: Vec::new(),
                state_conflict: true,
            });
        }

        let snapshot_before = json!(preview);
        match request.decision {
            ImportPreviewDecision::Accept => {
                preview.preview_matching_feedback = set_feedback_review_status(
                    preview.preview_matching_feedback,
                    "llm",
                    "accepted",
                );
            }
            ImportPreviewDecision::Reject => {
                preview.preview_matching_feedback = set_feedback_review_status(
                    preview.preview_matching_feedback,
                    "llm",
                    "rejected",
                );
            }
            ImportPreviewDecision::Clear => {
                clear_feedback_key(&mut preview.preview_matching_feedback, "llm");
            }
        }
        let mut payload = row
            .try_get::<Value, _>("preview_payload")
            .unwrap_or_else(|_| json!({}));
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
        let session_db_id: i64 = row.try_get("session_id")?;
        let mut update = build_preview_row_update_query(
            &preview,
            payload.to_string(),
            amount_cents,
            direction,
            PreviewRowUpdateTarget {
                preview_id: request.preview_id,
                session_db_id,
                user_id,
                expected_row_version: request
                    .expected_state
                    .and_then(|expected| expected.expected_row_version),
            },
        );
        update.build().execute(&mut *transaction).await?;
        let event_id = insert_llm_review_event_in_transaction(
            &mut transaction,
            user_id,
            request,
            decision,
            snapshot_before,
            json!(preview),
        )
        .await?;
        let updated_row = sqlx::query(
            r#"
            SELECT p.*, s.session_key
            FROM import_preview_rows p
            JOIN import_sessions s ON s.id = p.session_id
            WHERE p.id = $1 AND p.user_id = $2
            "#,
        )
        .bind(request.preview_id)
        .bind(user_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let updated = updated_row.as_ref().map(preview_from_pg_row).transpose()?;
        transaction.commit().await?;
        let next_state = match request.decision {
            ImportPreviewDecision::Accept => "accepted",
            ImportPreviewDecision::Reject => "rejected",
            ImportPreviewDecision::Clear => "cleared",
        };
        tracing::info!(
            domain = "import_signal_lifecycle",
            operation = "lifecycle_transition",
            signal_family = "llm",
            outcome = "committed",
            session_key = %request.session_id,
            decision = decision_name,
            previous_state = current_status.as_deref().unwrap_or("none"),
            next_state,
            "import preview signal lifecycle transition completed"
        );
        Ok(ImportPreviewLlmDecisionResult {
            preview: updated,
            event_id: Some(event_id),
            applied_fields: Vec::new(),
            state_conflict: false,
        })
    })
}

fn empty_llm_decision_result() -> ImportPreviewLlmDecisionResult {
    ImportPreviewLlmDecisionResult {
        preview: None,
        event_id: None,
        applied_fields: Vec::new(),
        state_conflict: false,
    }
}

async fn insert_llm_review_event_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
    request: &ImportPreviewLlmReviewRequest<'_>,
    decision: &str,
    snapshot_before: Value,
    snapshot_after: Value,
) -> DbResult<i64> {
    let llm_response_raw = request.suggestion.map(|suggestion| {
        json!({
            "suggested_type": suggestion.suggested_type.clone(),
            "suggested_category_id": suggestion.suggested_category_id,
            "suggested_main_category": suggestion.suggested_main_category.clone(),
            "suggested_sub_category": suggestion.suggested_sub_category.clone(),
            "suggested_source_account": suggestion.suggested_source_account.clone(),
            "suggested_destination_account": suggestion.suggested_destination_account.clone(),
            "confidence": suggestion.confidence,
            "reason": suggestion.reason.clone(),
        })
        .to_string()
    });
    let id = sqlx::query(
        r#"
        INSERT INTO llm_memory_events (
            user_id, session_id, preview_id, event_type, decision, prompt_text,
            llm_response_raw, llm_provider, llm_model, suggested_main_category,
            suggested_sub_category, suggested_source_account, suggested_destination_account,
            confidence, user_correction_category, user_correction_account,
            snapshot_before, snapshot_after, metadata, created_at
        ) VALUES ($1,$2,$3,'preview_review',$4,NULL,$5,NULL,NULL,$6,$7,$8,$9,$10,$11,$12,$13::jsonb,$14::jsonb,'{}'::jsonb,now())
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(request.session_id)
    .bind(request.preview_id)
    .bind(decision)
    .bind(llm_response_raw)
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_main_category.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_sub_category.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_source_account.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.suggested_destination_account.clone()),
    )
    .bind(
        request
            .suggestion
            .map(|value| value.confidence)
            .unwrap_or_default(),
    )
    .bind(request.user_correction_category)
    .bind(request.user_correction_account)
    .bind(snapshot_before.to_string())
    .bind(snapshot_after.to_string())
    .fetch_one(&mut **transaction)
    .await?
    .try_get("id")?;
    Ok(id)
}

fn preview_llm_review_status(feedback: &Value) -> Option<String> {
    feedback
        .get("llm")
        .and_then(Value::as_object)
        .and_then(|llm| {
            [
                "review_status",
                "status",
                "lifecycle_status",
                "signal_state",
            ]
            .iter()
            .find_map(|key| {
                llm.get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_ascii_lowercase())
            })
        })
}

#[cfg(test)]
fn llm_decision_is_idempotent(
    current_status: Option<&str>,
    has_llm: bool,
    decision: ImportPreviewDecision,
) -> bool {
    preview_terminal_action(current_status, has_llm, decision)
        == ImportPreviewTerminalAction::Idempotent
}

#[cfg(test)]
mod llm_lifecycle_action_tests {
    use super::*;

    #[test]
    fn llm_review_status_uses_first_nonempty_authority() {
        let feedback = json!({
            "llm": {
                "review_status": "",
                "status": "Rejected",
                "lifecycle_status": "accepted",
                "signal_state": "pending"
            }
        });

        assert_eq!(
            preview_llm_review_status(&feedback).as_deref(),
            Some("rejected")
        );
    }

    #[test]
    fn repeated_terminal_llm_actions_are_idempotent() {
        assert!(llm_decision_is_idempotent(
            Some("accepted"),
            true,
            ImportPreviewDecision::Accept
        ));
        assert!(llm_decision_is_idempotent(
            Some("rejected"),
            true,
            ImportPreviewDecision::Reject
        ));
        assert!(llm_decision_is_idempotent(
            None,
            false,
            ImportPreviewDecision::Clear
        ));
        assert!(!llm_decision_is_idempotent(
            Some("pending"),
            true,
            ImportPreviewDecision::Accept
        ));
    }

    #[test]
    fn llm_terminal_action_guard_covers_retries_opposites_clear_and_mobile_payloads() {
        let cases = [
            (Some("accepted"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Idempotent),
            (Some("accepted"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (Some("accepted"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Idempotent),
            (Some("rejected"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("pending"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Apply),
            (None, false, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Idempotent),
        ];

        for (status, has_signal, decision, expected) in cases {
            assert_eq!(preview_terminal_action(status, has_signal, decision), expected);
        }
    }

    #[test]
    fn llm_expected_state_rejects_stale_desktop_but_accepts_minimal_mobile_shape() {
        let mut preview = preview_row(1);
        preview.category_id = Some(42);
        preview.preview_matching_feedback = json!({
            "llm": {"review_status": "pending"}
        });
        let cases = [
            (
                ImportPreviewExpectedState {
                    session_id: Some("session".to_string()),
                    ..ImportPreviewExpectedState::default()
                },
                false,
            ),
            (
                ImportPreviewExpectedState {
                    session_id: Some("session".to_string()),
                    review_status: Some("pending".to_string()),
                    preview_category_id: Some(Some(42)),
                    ..ImportPreviewExpectedState::default()
                },
                false,
            ),
            (
                ImportPreviewExpectedState {
                    review_status: Some("rejected".to_string()),
                    preview_category_id: Some(Some(42)),
                    ..ImportPreviewExpectedState::default()
                },
                true,
            ),
            (
                ImportPreviewExpectedState {
                    review_status: Some("pending".to_string()),
                    preview_category_id: Some(None),
                    ..ImportPreviewExpectedState::default()
                },
                true,
            ),
        ];

        for (expected, conflicts) in cases {
            assert_eq!(
                recommendation_expected_state_conflicts(&preview, Some(&expected), Some("pending")),
                conflicts
            );
        }
    }

    #[test]
    fn serialized_concurrent_llm_opposites_and_identical_retry_emit_one_event() {
        for (first, opposite, terminal_status) in [
            (
                ImportPreviewDecision::Accept,
                ImportPreviewDecision::Reject,
                "accepted",
            ),
            (
                ImportPreviewDecision::Reject,
                ImportPreviewDecision::Accept,
                "rejected",
            ),
        ] {
            let mut event_count = 0;
            assert_eq!(
                preview_terminal_action(Some("pending"), true, first),
                ImportPreviewTerminalAction::Apply
            );
            event_count += 1;
            assert_eq!(
                preview_terminal_action(Some(terminal_status), true, first),
                ImportPreviewTerminalAction::Idempotent
            );
            assert_eq!(
                preview_terminal_action(Some(terminal_status), true, opposite),
                ImportPreviewTerminalAction::Conflict
            );
            assert_eq!(
                preview_terminal_action(
                    Some(terminal_status),
                    true,
                    ImportPreviewDecision::Clear,
                ),
                ImportPreviewTerminalAction::Conflict
            );
            assert_eq!(event_count, 1);
        }
    }

    #[test]
    fn llm_review_keeps_preview_and_memory_event_in_one_locked_transaction() {
        let source = concat!(include_str!("application.rs"), include_str!("review.rs"));

        assert!(source.contains("let mut transaction = pool.begin().await?;"));
        assert!(source.contains("FOR UPDATE OF p"));
        assert!(source.contains("update.build().execute(&mut *transaction).await?;"));
        assert!(source.contains("insert_llm_review_event_in_transaction("));
        assert!(source.contains("transaction.commit().await?;"));
        let lock = source.find("FOR UPDATE OF p").expect("preview row lock");
        let guard = source
            .find("match preview_terminal_action(")
            .expect("terminal action guard");
        let event = source
            .find("insert_llm_review_event_in_transaction(")
            .expect("LLM event persistence");
        let feedback = source
            .find("preview.preview_matching_feedback = set_feedback_review_status(")
            .expect("LLM feedback mutation");
        assert!(lock < guard && guard < feedback && feedback < event);
    }
}
