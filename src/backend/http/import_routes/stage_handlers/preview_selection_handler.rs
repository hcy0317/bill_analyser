/// 处理跨页 selection action，并把 all/valid/needs-review/invert 语义委托给 DB 查询更新。

#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_selection_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_preview_selection_runtime_handler", "business operation entered");
    let user_id = match user_id_from_headers(&headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let runtime = match open_runtime(&state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    match get_import_session(runtime.connection(), &session_id, user_id) {
        Ok(Some(_)) => {}
        Ok(None) => return route_response(import_session_not_found_response()),
        Err(error) => return route_response(db_error_response(error)),
    }
    let object = match payload_object(&payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    if first_text_from_object(object, &["selectionAction", "selection_action", "action"])
        .is_some_and(|action| action.eq_ignore_ascii_case("patch"))
    {
        let selected_ids = limited_id_list_field_from_object(object, &["selected_ids", "selectedIds"], 5000)
            .and_then(|ids| ids.ok_or_else(|| import_v2_error_response(400, "selected_ids is required")));
        let selected_ids = match selected_ids { Ok(ids) => ids, Err(response) => return route_response(response) };
        let deselected_ids = limited_id_list_field_from_object(object, &["deselected_ids", "deselectedIds"], 5000)
            .and_then(|ids| ids.ok_or_else(|| import_v2_error_response(400, "deselected_ids is required")));
        let deselected_ids = match deselected_ids { Ok(ids) => ids, Err(response) => return route_response(response) };
        let selected_set = selected_ids.iter().copied().collect::<BTreeSet<_>>();
        if deselected_ids.iter().any(|preview_id| selected_set.contains(preview_id)) {
            return route_response(import_v2_error_response(
                400,
                "Preview selection patch contains overlapping ids",
            ));
        }
        let expected_selection_hash = match expected_selection_hash_from_payload(object) {
            Ok(hash) => hash,
            Err(response) => return route_response(response),
        };
        let updated = match patch_preview_selection(
            runtime.connection(),
            &session_id,
            &selected_ids,
            &deselected_ids,
            expected_selection_hash.as_deref(),
            user_id,
        ) {
            Ok(updated) => updated,
            Err(DbError::PreviewSelectionConflict { expected, actual }) => {
                let response = preview_selection_conflict_response(
                    runtime.connection(),
                    &session_id,
                    user_id,
                    &selected_ids,
                    &deselected_ids,
                    &expected,
                    &actual,
                );
                return route_response(match response {
                    Ok(response) => response,
                    Err(error) => db_error_response(error),
                });
            }
            Err(DbError::PreviewSelectionTargetMismatch { .. }) => {
                return route_response(import_v2_error_response(
                    400,
                    "Preview selection is outside this session",
                ));
            }
            Err(error) => return route_response(db_error_response(error)),
        };
        let metadata = match query_preview_page_by_session(
            runtime.connection(), &session_id, user_id,
            &ImportPreviewPageRequest { page: 1, page_size: 1, ..ImportPreviewPageRequest::default() },
        ) {
            Ok(result) => serde_json::to_value(result.metadata).unwrap_or_else(|_| json!({})),
            Err(error) => return route_response(db_error_response(error)),
        };
        return route_response(import_v2_data_response(json!({
            "updated": updated,
            "selectionAction": "patch",
            "metadata": metadata,
        })));
    }
    let action = match preview_selection_action_from_payload(object) {
        Ok(action) => action,
        Err(response) => return route_response(response),
    };
    let expected_selection_hash = match expected_selection_hash_from_payload(object) {
        Ok(hash) => hash,
        Err(response) => return route_response(response),
    };
    let update_items = match preview_update_items_from_payload(&payload) {
        Ok(update_items) => update_items,
        Err(response) => return route_response(response),
    };
    let mut update_by_preview_id = BTreeMap::new();
    for item in update_items {
        let preview_id = match preview_id_from_payload(item) {
            Ok(preview_id) => preview_id,
            Err(response) => return route_response(response),
        };
        update_by_preview_id.insert(preview_id, item);
    }
    if update_by_preview_id.len() > 500 {
        return route_response(preview_selection_too_large_response());
    }
    let required_row_versions = update_by_preview_id.len();
    let preview_ids = update_by_preview_id.keys().copied().collect::<Vec<_>>();
    let scoped_previews = match get_preview_by_ids(
        runtime.connection(),
        &session_id,
        &preview_ids,
        user_id,
    ) {
        Ok(previews) => previews,
        Err(error) => return route_response(db_error_response(error)),
    };
    if scoped_previews.len() != preview_ids.len() {
        return route_response(import_v2_error_response(
            400,
            "Preview update is outside this session",
        ));
    }
    let category_ids = update_by_preview_id
        .values()
        .filter_map(|item| first_value(item, &["categoryId", "category_id"]))
        .filter_map(value_to_i64)
        .filter(|category_id| *category_id > 0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let categories = match get_import_preview_categories_by_ids(
        runtime.connection(),
        user_id,
        &category_ids,
    ) {
        Ok(categories) => categories,
        Err(error) => return route_response(db_error_response(error)),
    };
    let mut patches = Vec::with_capacity(update_by_preview_id.len());
    let mut present_row_versions = 0usize;
    for (preview_id, item) in update_by_preview_id {
        let expected_row_version = match expected_row_version_from_payload(item) {
            Ok(expected_row_version) => expected_row_version,
            Err(response) => return route_response(response),
        };
        present_row_versions += 1;
        let patch = match build_preview_patch_from_payload_with_loaded_categories(
            preview_id,
            item,
            &categories,
        ) {
            Ok(patch) => patch,
            Err(response) => return route_response(response),
        };
        patches.push(attach_expected_row_version(
            patch,
            expected_row_version,
        ));
    }
    observe_import_version_contract(
        "preview_selection_patch",
        "row_version",
        required_row_versions,
        present_row_versions,
    );
    let filters = import_preview_query_filters_from_payload(object);
    let selection_request = ImportPreviewPageRequest {
        page: 1,
        page_size: usize::MAX / 2,
        sort_by: String::new(),
        sort_direction: "asc".to_string(),
        preview_ids: Vec::new(),
        filters: filters.clone(),
    };
    let (selection_mode, selection_target) = match action.as_str() {
        "select_valid" => (
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::Valid,
        ),
        "select_invalid" | "select_needs_annotation" => (
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::NeedsReview,
        ),
        "select_none" => (
            ImportPreviewSelectionMode::Deselect,
            ImportPreviewSelectionTarget::All,
        ),
        "invert" => (
            ImportPreviewSelectionMode::Invert,
            ImportPreviewSelectionTarget::All,
        ),
        _ => (
            ImportPreviewSelectionMode::Select,
            ImportPreviewSelectionTarget::All,
        ),
    };
    let mutation = match apply_preview_patches_and_update_selection_by_query(
        runtime.connection(),
        &session_id,
        user_id,
        &patches,
        ImportPreviewConditionalSelectionCommand {
            mode: selection_mode,
            target: selection_target,
            request: &selection_request,
            expected_selection_hash: expected_selection_hash.as_deref(),
        },
    ) {
        Ok(result) => result,
        Err(DbError::PreviewSelectionConflict { expected, actual }) => {
            let response = preview_selection_conflict_response(
                runtime.connection(),
                &session_id,
                user_id,
                &preview_ids,
                &[],
                &expected,
                &actual,
            );
            return route_response(match response {
                Ok(response) => response,
                Err(error) => db_error_response(error),
            });
        }
        Err(DbError::PreviewVersionConflict {
            preview_id,
            expected,
            ..
        }) => {
            let latest_row = match get_preview_bill_by_id(
                runtime.connection(),
                preview_id,
                user_id,
            ) {
                Ok(Some(row)) if row.session_id == session_id => row,
                Ok(Some(_)) | Ok(None) => {
                    return route_response(import_v2_error_response(
                        404,
                        "Preview bill not found",
                    ));
                }
                Err(error) => return route_response(db_error_response(error)),
            };
            return route_response(preview_row_version_conflict_response(expected, latest_row));
        }
        Err(error) => return route_response(db_error_response(error)),
    };
    let metadata = match query_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 1,
            sort_by: String::new(),
            sort_direction: "asc".to_string(),
            preview_ids: Vec::new(),
            filters,
        },
    ) {
        Ok(result) => serde_json::to_value(result.metadata).unwrap_or_else(|_| json!({})),
        Err(error) => return route_response(db_error_response(error)),
    };
    let preview_items = match get_preview_by_ids(
        runtime.connection(),
        &session_id,
        &preview_ids,
        user_id,
    ) {
        Ok(rows) => rows.into_iter().map(preview_row_to_value).collect::<Vec<_>>(),
        Err(error) => return route_response(db_error_response(error)),
    };

    route_response(import_v2_data_response(json!({
        "updated": mutation.updated_selection,
        "applied_preview_updates": mutation.applied_preview_updates,
        "selectionAction": action,
        "metadata": metadata,
        "previewItems": preview_items,
    })))
}

fn expected_selection_hash_from_payload(
    object: &Map<String, Value>,
) -> Result<Option<String>, ImportV2RouteResponse> {
    let Some(value) = first_value(
        object,
        &["expected_selection_hash", "expectedSelectionHash"],
    ) else {
        return Ok(None);
    };
    let Some(hash) = value.as_str().map(str::trim) else {
        return Err(import_v2_error_response(400, "Invalid expected_selection_hash"));
    };
    let suffix = hash.strip_prefix("fnv1a32:");
    if suffix.is_none_or(|suffix| {
        suffix.len() != 8 || !suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(import_v2_error_response(400, "Invalid expected_selection_hash"));
    }
    Ok(Some(hash.to_ascii_lowercase()))
}

fn preview_selection_conflict_response(
    pool: &PostgresPool,
    session_id: &str,
    user_id: UserId,
    selected_ids: &[i64],
    deselected_ids: &[i64],
    expected: &str,
    actual: &str,
) -> Result<ImportV2RouteResponse, DbError> {
    let target_ids = selected_ids
        .iter()
        .chain(deselected_ids.iter())
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let preview_items = get_preview_by_ids(pool, session_id, &target_ids, user_id)?
        .into_iter()
        .map(preview_row_to_value)
        .collect::<Vec<_>>();
    let metadata = query_preview_page_by_session(
        pool,
        session_id,
        user_id,
        &ImportPreviewPageRequest {
            page: 1,
            page_size: 1,
            ..ImportPreviewPageRequest::default()
        },
    )?;
    let metadata = serde_json::to_value(metadata.metadata).unwrap_or_else(|_| json!({}));
    Ok(ImportV2RouteResponse {
        status_code: 409,
        body: json!({
            "success": false,
            "error": "Preview selection changed, please refresh",
            "code": "PREVIEW_SELECTION_CONFLICT",
            "data": {
                "expected_selection_hash": expected,
                "actual_selection_hash": actual,
                "metadata": metadata,
                "previewItems": preview_items,
            },
        }),
    })
}

fn preview_selection_action_from_payload(
    object: &Map<String, Value>,
) -> Result<String, ImportV2RouteResponse> {
    let action = first_value(
        object,
        &["selectionAction", "selection_action", "action"],
    )
    .and_then(value_to_text)
    .unwrap_or_default();
    match action
        .trim()
        .replace('-', "_")
        .to_ascii_lowercase()
        .as_str()
    {
        "selectall" | "select_all" | "all" => Ok("select_all".to_string()),
        "selectvalid" | "select_valid" | "valid" => Ok("select_valid".to_string()),
        "selectinvalid" | "select_invalid" | "invalid" => Ok("select_invalid".to_string()),
        "selectneedsannotation" | "select_needs_annotation" | "needs_annotation" => {
            Ok("select_needs_annotation".to_string())
        }
        "selectnone" | "select_none" | "none" => Ok("select_none".to_string()),
        "selectinvert" | "select_invert" | "invert" => Ok("invert".to_string()),
        _ => Err(import_v2_error_response(400, "Invalid selection action")),
    }
}
