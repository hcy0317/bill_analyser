/// 处理导入 confirm 请求：HTTP 仅构造规范化命令，所有读写由单个 DB 事务入口完成。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_confirm_runtime_handler(
    State(state): State<HttpAppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_confirm_runtime_handler",
        "business operation entered"
    );
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let command = match confirm_command_from_payload(&payload) {
        Ok(command) => command,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };

    match confirm_import_command(runtime.pool(), user_id, &command) {
        Ok(receipt) => route_response(ImportV2RouteResponse {
            status_code: receipt.http_status,
            body: receipt.success_envelope,
        }),
        Err(DbError::InvalidOperation(message)) => {
            route_response(confirm_invalid_operation_response(&message))
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

fn confirm_command_from_payload(
    payload: &Value,
) -> Result<ConfirmCommand, ImportV2RouteResponse> {
    let object = payload_object(payload)?;
    let session_id = required_session_id_from_payload(object)?;
    let expected_session_version = optional_confirm_session_version(object)?;
    let history_acknowledgement = history_rewrite_acknowledgement_from_payload(object)?;
    let declared_confirm_time_effects = declared_confirm_time_effects_from_payload(object)?;
    let preview_patches = preview_update_items_from_payload(payload)?
        .into_iter()
        .map(confirm_preview_patch_from_payload)
        .collect::<Result<Vec<_>, _>>()?;
    let selected_preview_ids = selected_preview_ids_from_payload(object)?;
    let preserve_unpatched_selection = first_value(
        object,
        &["preserve_unpatched_selection", "preserveUnpatchedSelection"],
    )
    .and_then(Value::as_bool)
    .unwrap_or(false);

    Ok(ConfirmCommand {
        session_id,
        expected_session_version,
        preview_patches,
        selected_preview_ids,
        preserve_unpatched_selection,
        history_acknowledgement,
        declared_confirm_time_effects,
    })
}

fn optional_confirm_session_version(
    object: &Map<String, Value>,
) -> Result<Option<i64>, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &[
            "expected_session_version",
            "expectedSessionVersion",
            "session_version",
            "sessionVersion",
        ],
    ) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    value_to_i64(value)
        .filter(|version| *version >= 0)
        .map(Some)
        .ok_or_else(|| import_v2_error_response(400, "Invalid expected session version"))
}

fn selected_preview_ids_from_payload(
    object: &Map<String, Value>,
) -> Result<Option<Vec<i64>>, ImportV2RouteResponse> {
    let Some(value) = first_value(object, &["selected_ids", "selectedIds"]) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let values = value
        .as_array()
        .ok_or_else(|| import_v2_error_response(400, "Invalid selected preview ids"))?;
    for value in values {
        value_to_i64(value)
            .filter(|id| *id > 0)
            .ok_or_else(|| import_v2_error_response(400, "Invalid selected preview ids"))?;
    }
    Ok(id_list_field_from_object(
        object,
        &["selected_ids", "selectedIds"],
    ))
}

fn declared_confirm_time_effects_from_payload(
    object: &Map<String, Value>,
) -> Result<Vec<ConfirmTimeEffect>, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &[
            "declared_confirm_time_effects",
            "declaredConfirmTimeEffects",
            "confirm_time_effects",
            "confirmTimeEffects",
        ],
    ) else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let effects = value
        .as_array()
        .ok_or_else(|| import_v2_error_response(400, "Invalid confirm-time effects"))?;
    if effects.is_empty() {
        Ok(Vec::new())
    } else {
        Err(import_v2_error_response(
            400,
            "Unsupported confirm-time effects",
        ))
    }
}

fn confirm_preview_patch_from_payload(
    object: &Map<String, Value>,
) -> Result<ImportPreviewPatch, ImportV2RouteResponse> {
    let preview_id = preview_id_from_payload(object)?;
    let mut patch = build_preview_patch_from_payload(preview_id, object);
    if let Some(value) = first_value(object, &["categoryId", "category_id"]) {
        patch
            .changes
            .retain(|(field, _)| {
                !matches!(
                    field,
                    ImportPreviewPatchField::CategoryId
                        | ImportPreviewPatchField::MainCategory
                        | ImportPreviewPatchField::SubCategory
                )
            });
        if let Some(category_id) = value_to_i64(value).filter(|value| *value > 0) {
            patch.changes.push((
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(category_id),
            ));
        } else {
            patch
                .changes
                .push((ImportPreviewPatchField::CategoryId, ImportPreviewPatchValue::Null));
            set_preview_patch_text_change(
                &mut patch,
                ImportPreviewPatchField::MainCategory,
                String::new(),
            );
            set_preview_patch_text_change(
                &mut patch,
                ImportPreviewPatchField::SubCategory,
                String::new(),
            );
        }
    }
    Ok(patch)
}

fn confirm_invalid_operation_is_conflict(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    message.contains("fingerprint conflict")
        || message.contains("session version")
        || message.contains("confirm cas mismatch")
        || message.contains("confirmed import session is terminal")
}

fn confirm_invalid_operation_response(message: &str) -> ImportV2RouteResponse {
    if message == "import session not found" {
        return import_session_not_found_response();
    }
    if message.starts_with("preview bill not found") {
        return import_v2_error_response(404, "Preview bill not found");
    }
    if message == "invalid category" {
        return import_v2_error_response(400, "Invalid category");
    }
    if confirm_invalid_operation_is_conflict(message) {
        return import_v2_error_response(409, message);
    }
    import_v2_error_response(400, message)
}

fn history_rewrite_acknowledgement_from_payload(
    object: &Map<String, Value>,
) -> Result<Option<ImportHistoryRewriteAcknowledgement>, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &[
            "history_rewrite_acknowledgement",
            "historyRewriteAcknowledgement",
            "history_rewrite_ack",
            "historyRewriteAck",
        ],
    ) else {
        return Ok(None);
    };
    let mut acknowledgement =
        serde_json::from_value::<ImportHistoryRewriteAcknowledgement>(value.clone()).map_err(
            |_| import_v2_error_response(400, "Invalid history rewrite acknowledgement"),
        )?;
    for operation in &mut acknowledgement.operations {
        let Some(canonical_operation) = normalize_history_operation(&operation.planned_operation)
        else {
            return Err(import_v2_error_response(
                400,
                "Invalid history rewrite acknowledgement operation",
            ));
        };
        operation.planned_operation = canonical_operation.as_str().to_string();
    }
    Ok(Some(acknowledgement))
}

#[cfg(test)]
mod confirm_handler_contract_tests {
    use super::*;

    fn acknowledgement_payload(planned_operation: &str) -> Value {
        json!({
            "acknowledged": true,
            "selectedPreviewIds": [7],
            "operations": [{
                "previewId": 7,
                "operationId": "operation-7",
                "plannedOperation": planned_operation,
                "historyBillId": 42,
                "historyBillVersion": 3,
                "acknowledgementToken": "token-7"
            }],
            "selectionScope": {"mode": "selected"}
        })
    }

    #[test]
    fn confirm_payload_aliases_build_the_same_command_shape() {
        for payload in [
            json!({
                "session_id": "session-7",
                "expected_session_version": 9,
                "preview_updates": [{
                    "id": 7,
                    "category_id": 42,
                    "mainCategory": "client value",
                    "subCategory": "client value",
                    "description": "edited"
                }],
                "selected_ids": [7],
                "preserve_unpatched_selection": true,
                "history_rewrite_acknowledgement": acknowledgement_payload("merge_transfer"),
                "declared_confirm_time_effects": []
            }),
            json!({
                "sessionId": "session-7",
                "expectedSessionVersion": 9,
                "previewUpdates": [{
                    "previewId": 7,
                    "categoryId": 42,
                    "mainCategory": "client value",
                    "subCategory": "client value",
                    "description": "edited"
                }],
                "selectedIds": [7],
                "preserveUnpatchedSelection": true,
                "historyRewriteAcknowledgement": acknowledgement_payload("merge_current_bill_transfer"),
                "declaredConfirmTimeEffects": []
            }),
        ] {
            let command = confirm_command_from_payload(&payload).expect("valid confirm command");
            assert_eq!(command.session_id, "session-7");
            assert_eq!(command.expected_session_version, Some(9));
            assert_eq!(command.selected_preview_ids, Some(vec![7]));
            assert!(command.preserve_unpatched_selection);
            assert_eq!(command.preview_patches.len(), 1);
            assert!(command.preview_patches[0].changes.contains(&(
                ImportPreviewPatchField::CategoryId,
                ImportPreviewPatchValue::Integer(42),
            )));
            assert!(!command.preview_patches[0].changes.iter().any(|(field, _)| {
                matches!(
                    field,
                    ImportPreviewPatchField::MainCategory
                        | ImportPreviewPatchField::SubCategory
                )
            }));
            assert_eq!(
                command
                    .history_acknowledgement
                    .expect("history acknowledgement")
                    .operations[0]
                    .planned_operation,
                "merge_transfer_history"
            );
            assert!(command.declared_confirm_time_effects.is_empty());
        }
    }

    #[test]
    fn confirm_payload_preserves_absent_and_explicit_empty_selection() {
        let absent = confirm_command_from_payload(&json!({"session_id": "session-1"}))
            .expect("selection may be absent");
        let empty = confirm_command_from_payload(&json!({
            "session_id": "session-1",
            "selected_ids": []
        }))
        .expect("selection may be explicitly empty");

        assert!(absent.selected_preview_ids.is_none());
        assert_eq!(empty.selected_preview_ids, Some(Vec::new()));
    }

    #[test]
    fn history_rewrite_acknowledgement_normalizes_supported_aliases() {
        for (input, expected) in [
            ("update_history", "update_history"),
            ("update_current_bill", "update_history"),
            ("merge_transfer_history", "merge_transfer_history"),
            ("merge_current_bill_transfer", "merge_transfer_history"),
            ("merge_transfer", "merge_transfer_history"),
        ] {
            let payload = json!({"historyRewriteAcknowledgement": acknowledgement_payload(input)});
            let acknowledgement = history_rewrite_acknowledgement_from_payload(
                payload.as_object().expect("acknowledgement object"),
            )
            .expect("supported history rewrite operation")
            .expect("history rewrite acknowledgement");

            assert_eq!(acknowledgement.operations[0].planned_operation, expected);
        }
    }

    #[test]
    fn confirm_payload_rejects_unknown_operations_and_undeclared_effect_types() {
        let payload = json!({"historyRewriteAck": acknowledgement_payload("replace_history")});
        let error = history_rewrite_acknowledgement_from_payload(
            payload.as_object().expect("acknowledgement object"),
        )
        .expect_err("unknown operation must be rejected at the HTTP boundary");
        assert_eq!(error.status_code, 400);

        let error = confirm_command_from_payload(&json!({
            "session_id": "session-1",
            "confirmTimeEffects": [{"kind": "learning_accept"}]
        }))
        .expect_err("unsupported effects must not reach the repository");
        assert_eq!(error.status_code, 400);
        assert_eq!(error.body["error"], "Unsupported confirm-time effects");
    }

    #[test]
    fn confirm_repository_errors_keep_existing_not_found_and_category_statuses() {
        for (message, expected_status, expected_error) in [
            (
                "import session not found",
                404,
                "Session not found or expired",
            ),
            ("preview bill not found: 7", 404, "Preview bill not found"),
            (
                "preview bill not found in import session",
                404,
                "Preview bill not found",
            ),
            ("invalid category", 400, "Invalid category"),
            (
                "import session version conflict: expected 2, current 3",
                409,
                "import session version conflict: expected 2, current 3",
            ),
        ] {
            let response = confirm_invalid_operation_response(message);
            assert_eq!(response.status_code, expected_status);
            assert_eq!(response.body["error"], expected_error);
        }
    }

    #[test]
    fn confirm_handler_has_one_transaction_command_call_and_no_direct_mutation_calls() {
        let source = include_str!("confirm_handler.rs");
        let handler = source
            .split("pub async fn import_confirm_runtime_handler")
            .nth(1)
            .expect("confirm handler source")
            .split("fn confirm_command_from_payload")
            .next()
            .expect("confirm handler boundary");

        assert_eq!(handler.matches("confirm_import_command(").count(), 1);
        for forbidden in [
            "get_import_session(",
            "get_preview_bill_by_id(",
            "apply_preview_patches_preserving_selection(",
            "replace_preview_selection_with_patches(",
            "reset_session_preview_selection(",
            "update_preview_selection(",
            "confirm_preview_to_bills_with_ack(",
            "clear_session_data(",
            "init_global_learning_runtime_schema(",
        ] {
            assert!(
                !handler.contains(forbidden),
                "direct DB boundary call remains in confirm handler: {forbidden}"
            );
        }
    }
}
