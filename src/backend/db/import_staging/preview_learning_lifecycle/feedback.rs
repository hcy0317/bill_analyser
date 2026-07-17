#[derive(Debug, Clone)]
struct ImportPreviewLlmCategoryMatch {
    id: i64,
    type_code: Option<i64>,
    main_category: String,
    sub_category: String,
}

/// 记录 learning 生命周期反馈，供后续自动应用 eligibility 与召回质量判断。
pub fn record_import_learning_lifecycle_feedback(
    pool: &PostgresPool,
    user_id: UserId,
    input: &ImportLearningLifecycleRecordInput,
) -> DbResult<ImportLearningLifecycleView> {
    let feedback = bill_analyser_core::import_learning_lifecycle::normalize_import_learning_lifecycle_feedback(
        &input.feedback,
    )
    .ok_or_else(|| DbError::InvalidOperation("Invalid learning lifecycle feedback".to_string()))?;
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
        let next = transition_import_learning_lifecycle(&state, feedback);
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
        .bind(&next.event_type)
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

/// 读取 learning 生命周期视图，HTTP 层用它构建可解释的学习建议状态。
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

#[cfg(test)]
mod lifecycle_action_tests {
    use super::*;

    #[test]
    fn learning_review_status_uses_first_nonempty_authority() {
        let feedback = json!({
            "learning": {
                "review_status": "  ",
                "status": "Accepted",
                "lifecycle_status": "rejected",
                "signal_state": "pending"
            }
        });

        assert_eq!(
            preview_learning_review_status(&feedback).as_deref(),
            Some("accepted")
        );
    }

    #[test]
    fn repeated_terminal_learning_actions_are_idempotent() {
        assert!(learning_decision_is_idempotent(
            Some("accepted"),
            true,
            ImportPreviewDecision::Accept
        ));
        assert!(learning_decision_is_idempotent(
            Some("rejected"),
            true,
            ImportPreviewDecision::Reject
        ));
        assert!(learning_decision_is_idempotent(
            None,
            false,
            ImportPreviewDecision::Clear
        ));
        assert!(!learning_decision_is_idempotent(
            Some("pending"),
            true,
            ImportPreviewDecision::Accept
        ));
        assert!(!learning_decision_is_idempotent(
            Some("pending"),
            true,
            ImportPreviewDecision::Reject
        ));
    }

    #[test]
    fn learning_terminal_action_guard_covers_retries_opposites_clear_and_mobile_payloads() {
        let cases = [
            (Some("accepted"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Idempotent),
            (Some("accepted"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (Some("accepted"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Idempotent),
            (Some("rejected"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (Some("rejected"), true, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Conflict),
            (Some("pending"), true, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Apply),
            (None, false, ImportPreviewDecision::Accept, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Reject, ImportPreviewTerminalAction::Conflict),
            (None, false, ImportPreviewDecision::Clear, ImportPreviewTerminalAction::Idempotent),
        ];

        for (status, has_signal, decision, expected) in cases {
            assert_eq!(preview_terminal_action(status, has_signal, decision), expected);
        }
    }

    #[test]
    fn learning_expected_state_rejects_stale_desktop_but_accepts_minimal_mobile_shape() {
        let mut preview = preview_row(1);
        preview.category_id = Some(42);
        preview.preview_matching_feedback = json!({
            "learning": {"review_status": "pending"}
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
                    review_status: Some("accepted".to_string()),
                    preview_category_id: Some(Some(42)),
                    ..ImportPreviewExpectedState::default()
                },
                true,
            ),
            (
                ImportPreviewExpectedState {
                    review_status: Some("pending".to_string()),
                    preview_category_id: Some(Some(43)),
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
    fn serialized_concurrent_learning_opposites_and_identical_retry_emit_one_event() {
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
    fn learning_action_keeps_preview_and_lifecycle_writes_in_one_locked_transaction() {
        let source = concat!(
            include_str!("decisions.rs"),
            include_str!("persistence.rs"),
            include_str!("feedback.rs"),
        );

        assert!(source.contains("let mut transaction = pool.begin().await?;"));
        assert!(source.contains("FOR UPDATE OF p"));
        assert!(source.contains("pg_advisory_xact_lock"));
        assert!(source.contains("update.build().execute(&mut *transaction).await?;"));
        assert!(source.contains("persist_import_learning_lifecycle_transition_in_transaction("));
        assert!(source.contains("transaction.commit().await?;"));
        let lock = source.find("FOR UPDATE OF p").expect("preview row lock");
        let guard = source
            .find("match preview_terminal_action(")
            .expect("terminal action guard");
        let event = source
            .find("persist_import_learning_lifecycle_transition_in_transaction(")
            .expect("learning event persistence");
        let feedback = source
            .find("let lifecycle_input = preview_learning_lifecycle_input")
            .expect("learning feedback transition");
        assert!(lock < guard && guard < feedback && feedback < event);
    }
}
