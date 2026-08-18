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
fn bill_create_fields_from_preview_uses_category_id_as_category_authority() {
    let mut row = preview_row(1);
    row.category_id = Some(42);
    row.preview_main_category = "/".to_string();
    row.preview_sub_category = "民生银行储蓄卡(6332)".to_string();

    let fields = bill_create_fields_from_preview(&row);

    assert_eq!(fields.get("category_id"), Some(&json!(42)));
    assert_eq!(fields.get("main_category"), None);
    assert_eq!(fields.get("sub_category"), None);
}

#[test]
fn confirm_command_fingerprint_canonicalizes_unordered_ack_collections_and_json_keys() {
    let operation_a = ImportHistoryRewriteAcknowledgementOperation {
        preview_id: 7,
        operation_id: "operation-7".to_string(),
        planned_operation: "update_history".to_string(),
        history_bill_id: 70,
        history_bill_version: 3,
        acknowledgement_token: "token-7".to_string(),
    };
    let operation_b = ImportHistoryRewriteAcknowledgementOperation {
        preview_id: 8,
        operation_id: "operation-8".to_string(),
        planned_operation: "merge_transfer_history".to_string(),
        history_bill_id: 80,
        history_bill_version: 4,
        acknowledgement_token: "token-8".to_string(),
    };
    let mut scope_left = Map::new();
    scope_left.insert("z".to_string(), json!({"b": 2, "a": 1}));
    scope_left.insert("a".to_string(), json!(true));
    let mut scope_right = Map::new();
    scope_right.insert("a".to_string(), json!(true));
    scope_right.insert("z".to_string(), json!({"a": 1, "b": 2}));
    let left = ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids: vec![8, 7, 8],
        operations: vec![operation_b.clone(), operation_a.clone()],
        selection_scope: Value::Object(scope_left),
    };
    let right = ImportHistoryRewriteAcknowledgement {
        acknowledged: true,
        selected_preview_ids: vec![7, 8],
        operations: vec![operation_a, operation_b],
        selection_scope: Value::Object(scope_right),
    };

    let command = |session_id: &str, acknowledgement| ConfirmCommand {
        session_id: session_id.to_string(),
        expected_session_version: Some(9),
        preview_patches: Vec::new(),
        selected_preview_ids: None,
        preserve_unpatched_selection: false,
        history_acknowledgement: Some(acknowledgement),
        declared_confirm_time_effects: Vec::new(),
    };
    let left_fingerprint = confirm_command_fingerprint(&command(" session ", left), 9).unwrap();
    let right_command = command("session", right);
    let right_fingerprint = confirm_command_fingerprint(&right_command, 9).unwrap();
    assert_eq!(left_fingerprint, right_fingerprint);
    assert_eq!(left_fingerprint.len(), 64);

    let mut changed = right_command;
    changed
        .history_acknowledgement
        .as_mut()
        .expect("acknowledgement")
        .operations[0]
        .history_bill_version += 1;
    assert_ne!(
        left_fingerprint,
        confirm_command_fingerprint(&changed, 9).unwrap()
    );

    let patch_a = ImportPreviewPatch::new(2).with_change(
        ImportPreviewPatchField::Description,
        ImportPreviewPatchValue::Text("second".to_string()),
    );
    let patch_b = ImportPreviewPatch::new(1).with_change(
        ImportPreviewPatchField::Description,
        ImportPreviewPatchValue::Text("first".to_string()),
    );
    let mut patch_left = ConfirmCommand {
        session_id: "session".to_string(),
        expected_session_version: Some(9),
        preview_patches: vec![patch_a.clone(), patch_b.clone()],
        selected_preview_ids: Some(vec![2, 1]),
        preserve_unpatched_selection: true,
        history_acknowledgement: None,
        declared_confirm_time_effects: Vec::new(),
    };
    let mut patch_right = patch_left.clone();
    patch_right.preview_patches = vec![patch_b, patch_a];
    patch_right.selected_preview_ids = Some(vec![1, 2]);
    assert_eq!(
        confirm_command_fingerprint(&patch_left, 9).unwrap(),
        confirm_command_fingerprint(&patch_right, 9).unwrap()
    );

    let mut category_only = patch_left.clone();
    category_only.preview_patches = vec![ImportPreviewPatch::new(1).with_change(
        ImportPreviewPatchField::CategoryId,
        ImportPreviewPatchValue::Integer(42),
    )];
    let mut category_with_redundant_projection = category_only.clone();
    category_with_redundant_projection.preview_patches[0].changes.extend([
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text("客户端旧类型".to_string()),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text("客户端旧主类".to_string()),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text("客户端旧子类".to_string()),
        ),
    ]);
    assert_eq!(
        confirm_command_fingerprint(&category_only, 9).unwrap(),
        confirm_command_fingerprint(&category_with_redundant_projection, 9).unwrap()
    );

    patch_left.preview_patches.clear();
    patch_left.selected_preview_ids = None;
    let mut no_patch_false = patch_left.clone();
    patch_left.preserve_unpatched_selection = true;
    no_patch_false.preserve_unpatched_selection = false;
    assert_eq!(
        confirm_command_fingerprint(&patch_left, 9).unwrap(),
        confirm_command_fingerprint(&no_patch_false, 9).unwrap()
    );
}

#[test]
fn import_confirm_and_lifecycle_observability_is_complete_and_secret_safe() {
    let confirm_source = concat!(
        include_str!("confirm/orchestration.rs"),
        include_str!("confirm/patches.rs"),
        include_str!("confirm/history.rs"),
        include_str!("confirm/persistence.rs"),
    );
    for required in [
        "operation = \"confirm\"",
        "operation = \"session_locked\"",
        "operation = \"fingerprint_classification\"",
        "operation = \"validation\"",
        "operation = plan.operation.as_str()",
        "operation = \"receipt_persistence\"",
        "operation = \"child_cleanup\"",
        "outcome = \"rolled_back\"",
        "outcome = \"replay\"",
        "outcome = if outcome.replayed { \"replayed\" } else { \"committed\" }",
        "session_key = %command.session_id",
        "request_session_version",
        "session_version",
        "response_schema_version",
    ] {
        assert!(
            confirm_source.contains(required),
            "missing confirm observability contract: {required}"
        );
    }

    let learning_source = concat!(
        include_str!("preview_learning_lifecycle/decisions.rs"),
        include_str!("preview_learning_lifecycle/persistence.rs"),
        include_str!("preview_learning_lifecycle/feedback.rs"),
    );
    let llm_source = concat!(
        include_str!("preview_llm/application.rs"),
        include_str!("preview_llm/review.rs"),
    );
    for (family, source) in [("learning", learning_source), ("llm", llm_source)] {
        for required in [
            "operation = \"lifecycle_transition\"",
            "outcome = \"started\"",
            "outcome = \"idempotent\"",
            "outcome = \"state_conflict\"",
            "outcome = \"expected_state_conflict\"",
            "outcome = \"committed\"",
            "previous_state",
            "next_state",
            "session_key",
            "decision",
        ] {
            assert!(
                source.contains(required),
                "missing {family} lifecycle observability contract: {required}"
            );
        }
        assert!(source.contains(&format!("signal_family = \"{family}\"")));
    }

    for source in [confirm_source, learning_source, llm_source] {
        for invocation in structured_tracing_invocations(source) {
            for forbidden in [
                "command_fingerprint =",
                "acknowledgement_token",
                "fingerprint_source",
                "preview_description",
                "preview_counterparty",
                "preview_payment_method",
                "success_envelope",
                "snapshot_before",
                "snapshot_after",
                "llm_response_raw",
                "payload_json",
                "recommendation_key",
                "credential",
                "password",
                "api_key",
            ] {
                assert!(
                    !invocation.contains(forbidden),
                    "sensitive observability field found: {forbidden} in {invocation}"
                );
            }
        }
    }

    assert_eq!(
        confirm_rollback_reason(&DbError::InvalidOperation(
            "unclassified sensitive detail must not be logged".to_string()
        )),
        "invalid_operation"
    );

    for (error, reason) in [
        (
            DbError::InvalidOperation("fingerprint conflict".to_string()),
            "fingerprint_conflict",
        ),
        (
            DbError::import_session_version_conflict("session-observability", 2, 3),
            "session_version_conflict",
        ),
        (
            DbError::InvalidOperation("identity validation failed".to_string()),
            "preview_validation",
        ),
        (
            DbError::InvalidOperation("requires review".to_string()),
            "preview_validation",
        ),
        (
            DbError::InvalidOperation("history CAS mismatch".to_string()),
            "history_operation",
        ),
        (
            DbError::InvalidOperation("receipt persistence failed".to_string()),
            "receipt_persistence",
        ),
        (DbError::Postgres(sqlx::Error::RowNotFound), "postgres"),
        (
            DbError::Io(std::io::Error::other("safe classifier fixture")),
            "io",
        ),
        (DbError::UnsafePath("fixture".to_string()), "unsafe_path"),
    ] {
        assert_eq!(confirm_rollback_reason(&error), reason);
    }
}

fn structured_tracing_invocations(source: &str) -> Vec<String> {
    let mut invocations = Vec::new();
    let mut current = None::<String>;
    for line in source.lines() {
        let starts_invocation = [
            "tracing::debug!(",
            "tracing::info!(",
            "tracing::warn!(",
            "tracing::error!(",
        ]
        .iter()
        .any(|marker| line.contains(marker));
        if starts_invocation {
            current = Some(String::new());
        }
        if let Some(invocation) = current.as_mut() {
            invocation.push_str(line);
            invocation.push('\n');
            if line.trim() == ");" {
                invocations.push(current.take().expect("tracing invocation"));
            }
        }
    }
    invocations
}
