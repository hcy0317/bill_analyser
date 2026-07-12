/// 处理跨页 selection action，并把 all/valid/needs-review/invert 语义委托给 DB 查询更新。
use bill_analyser_db::update_preview_selection;

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
        for preview_id in selected_ids.iter().chain(deselected_ids.iter()) {
            match get_preview_bill_by_id(runtime.connection(), *preview_id, user_id) {
                Ok(Some(row)) if row.session_id == session_id => {}
                Ok(_) => return route_response(import_v2_error_response(400, "Preview selection is outside this session")),
                Err(error) => return route_response(db_error_response(error)),
            }
        }
        if let Err(error) = update_preview_selection(runtime.connection(), &selected_ids, true, user_id) {
            return route_response(db_error_response(error));
        }
        if let Err(error) = update_preview_selection(runtime.connection(), &deselected_ids, false, user_id) {
            return route_response(db_error_response(error));
        }
        let metadata = match query_preview_page_by_session(
            runtime.connection(), &session_id, user_id,
            &ImportPreviewPageRequest { page: 1, page_size: 1, ..ImportPreviewPageRequest::default() },
        ) {
            Ok(result) => serde_json::to_value(result.metadata).unwrap_or_else(|_| json!({})),
            Err(error) => return route_response(db_error_response(error)),
        };
        return route_response(import_v2_data_response(json!({
            "updated": selected_ids.len() + deselected_ids.len(),
            "selectionAction": "patch",
            "metadata": metadata,
        })));
    }
    let action = match preview_selection_action_from_payload(object) {
        Ok(action) => action,
        Err(response) => return route_response(response),
    };
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
    let updated = match update_session_preview_selection_by_query(
        runtime.connection(),
        &session_id,
        user_id,
        selection_mode,
        selection_target,
        &selection_request,
    ) {
        Ok(updated) => updated,
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

    route_response(import_v2_data_response(json!({
        "updated": updated,
        "selectionAction": action,
        "metadata": metadata,
    })))
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
