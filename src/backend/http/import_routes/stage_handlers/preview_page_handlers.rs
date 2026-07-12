/// 处理 preview 分页查询，把前端 query 归一为 DB server-side page/filter/sort/facet 请求。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_page_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    Query(query): Query<PreviewPageQuery>,
    headers: HeaderMap,
) -> Response {
    let request_started = std::time::Instant::now();
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_preview_page_runtime_handler", "business operation entered");
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
    if let Some(response) = invalid_import_preview_query_response(&query) {
        return route_response(response);
    }
    let requested_preview_ids =
        parse_preview_page_query_ids(query.preview_ids.as_deref().or(query.preview_ids_camel.as_deref()));
    let normalized_query = normalize_import_preview_page_query(
        query.page.map(|value| value as i64),
        query
            .page_size
            .or(query.page_size_camel)
            .map(|value| value as i64),
        query.sort_by.as_deref().or(query.sort_by_camel.as_deref()),
        query
            .sort_direction
            .as_deref()
            .or(query.sort_direction_camel.as_deref()),
        requested_preview_ids.as_slice(),
    );
    let request = ImportPreviewPageRequest {
        page: normalized_query.page,
        page_size: normalized_query.page_size,
        sort_by: normalized_query.sort_by,
        sort_direction: normalized_query.sort_direction.as_str().to_string(),
        preview_ids: normalized_query.preview_ids,
        filters: ImportPreviewQueryFilters {
            min_datetime: query.min_datetime.or(query.min_datetime_camel),
            max_datetime: query.max_datetime.or(query.max_datetime_camel),
            transaction_type: query.transaction_type.or(query.transaction_type_camel),
            category: query.category,
            account: query.account,
            tag: query.tag,
            signal: query.signal,
            annotation: query.annotation,
            description: query.description,
            selected_only: query
                .selected_only
                .or(query.selected_only_camel)
                .unwrap_or(false),
        },
    };
    match query_preview_page_by_session(
        runtime.connection(),
        &session_id,
        user_id,
        &request,
    ) {
        Ok(result) => {
            let preview = result
                .rows
                .into_iter()
                .map(preview_row_to_value)
                .collect::<Vec<_>>();
            let query = serde_json::to_value(&request).ok();
            let metadata = serde_json::to_value(result.metadata).ok();
            let mut response = route_response(import_preview_page_success(ImportPreviewPageData {
                preview,
                total: result.total,
                page: result.page,
                page_size: result.page_size,
                query,
                metadata,
            }));
            attach_preview_server_timing(&mut response, request_started.elapsed());
            response
        }
        Err(error) => route_response(db_error_response(error)),
    }
}

fn attach_preview_server_timing(response: &mut Response, elapsed: std::time::Duration) {
    let timing = format!("preview;dur={:.3}", elapsed.as_secs_f64() * 1000.0);
    if let Ok(value) = axum::http::HeaderValue::from_str(&timing) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static("server-timing"),
            value,
        );
    }
}

#[cfg(test)]
mod preview_server_timing_tests {
    use super::*;

    #[test]
    fn preview_response_exposes_standard_server_timing_metric() {
        let mut response = route_response(import_v2_data_response(serde_json::json!({})));
        attach_preview_server_timing(&mut response, std::time::Duration::from_micros(12_345));
        assert_eq!(
            response.headers().get("server-timing").and_then(|value| value.to_str().ok()),
            Some("preview;dur=12.345")
        );
    }
}

fn invalid_import_preview_query_response(query: &PreviewPageQuery) -> Option<ImportV2RouteResponse> {
    let sort_by = query
        .sort_by
        .as_deref()
        .or(query.sort_by_camel.as_deref())
        .unwrap_or("")
        .trim();
    if !sort_by.is_empty() && !IMPORT_PREVIEW_SORT_KEYS.contains(&sort_by) {
        return Some(import_v2_error_response(400, "Unsupported preview sort key"));
    }
    let sort_direction = query
        .sort_direction
        .as_deref()
        .or(query.sort_direction_camel.as_deref())
        .unwrap_or("")
        .trim();
    if !sort_direction.is_empty()
        && !sort_direction.eq_ignore_ascii_case("asc")
        && !sort_direction.eq_ignore_ascii_case("desc")
    {
        return Some(import_v2_error_response(
            400,
            "Unsupported preview sort direction",
        ));
    }
    None
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_preview_page_query_ids(value: Option<&str>) -> Vec<i64> {
    value
        .unwrap_or("")
        .split(',')
        .filter_map(|item| item.trim().parse::<i64>().ok())
        .collect()
}

/// 返回当前 session 的轻量 preview index，供前端跨页筛选和全局状态展示使用。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_preview_index_runtime_handler(
    State(state): State<HttpAppState>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_preview_index_runtime_handler", "business operation entered");
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
    let rows = match get_preview_filter_index_by_session(runtime.connection(), &session_id, user_id)
    {
        Ok(rows) => rows,
        Err(error) => return route_response(db_error_response(error)),
    };
    let user_id_i64 = match user_id_i64_for_sql(user_id) {
        Ok(user_id_i64) => user_id_i64,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categories = match load_import_intelligence_categories(runtime.connection(), user_id_i64).await
    {
        Ok(categories) => categories,
        Err(error) => return route_response(db_error_response(error)),
    };
    let accounts = match load_import_intelligence_accounts(runtime.connection(), user_id_i64).await {
        Ok(accounts) => accounts,
        Err(error) => return route_response(db_error_response(error)),
    };
    let categories_by_id = preview_index_category_lookup_by_id(categories);
    let accounts_by_id = preview_index_account_lookup_by_id(accounts);
    let items = rows
        .into_iter()
        .filter_map(|row| serde_json::to_value(row).ok()?.as_object().cloned())
        .map(|row| build_import_preview_filter_index_item(&row, &categories_by_id, &accounts_by_id))
        .collect::<Vec<_>>();
    route_response(import_preview_index_success(ImportPreviewIndexData {
        total: items.len(),
        items,
    }))
}

fn preview_index_category_lookup_by_id(
    categories: Vec<ImportIntelligenceCategory>,
) -> BTreeMap<i64, CategoryLookup> {
    categories
        .into_iter()
        .map(|category| {
            (
                category.id,
                CategoryLookup {
                    name: category.sub_category.clone(),
                    sub_category: category.sub_category,
                    main_category: category.main_category,
                },
            )
        })
        .collect()
}

fn preview_index_account_lookup_by_id(
    accounts: Vec<ImportIntelligenceAccount>,
) -> BTreeMap<i64, AccountLookup> {
    accounts
        .into_iter()
        .map(|account| (account.id, AccountLookup { name: account.name }))
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_query_filters_from_payload(object: &Map<String, Value>) -> ImportPreviewQueryFilters {
    let filters = first_value(object, &["filters", "queryFilters", "query_filters"])
        .and_then(Value::as_object);
    ImportPreviewQueryFilters {
        min_datetime: preview_filter_text(filters, &["minDatetime", "min_datetime"]),
        max_datetime: preview_filter_text(filters, &["maxDatetime", "max_datetime"]),
        transaction_type: preview_filter_text(filters, &["transactionType", "transaction_type"]),
        category: preview_filter_text(filters, &["category"]),
        account: preview_filter_text(filters, &["account"]),
        tag: preview_filter_text(filters, &["tag"]),
        signal: preview_filter_text(filters, &["signal"]),
        annotation: preview_filter_text(filters, &["annotation"]),
        description: preview_filter_text(filters, &["description"]),
        selected_only: false,
    }
}

fn preview_filter_text(
    filters: Option<&Map<String, Value>>,
    keys: &[&str],
) -> Option<String> {
    filters.and_then(|object| {
        first_value(object, keys)
            .and_then(value_to_text)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}
