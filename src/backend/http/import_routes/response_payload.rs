// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn route_response(response: ImportV2RouteResponse) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(response.body)).into_response()
}

/// 将 provider 内部失败一次性投影为既有 OCR HTTP 错误合同。
fn ocr_provider_failure_response(failure: OcrProviderFailure) -> ImportV2RouteResponse {
    let (code, message) = match failure {
        OcrProviderFailure::Unavailable { message } => ("provider_unconfigured", message),
        OcrProviderFailure::TimedOut { message } => ("timeout", message),
        OcrProviderFailure::InvalidOutput { message } => ("parse_error", message),
        OcrProviderFailure::ReauthenticationRequired { message } => {
            ("provider_relogin_required", message)
        }
    };
    build_ocr_error_response(code, Some(&message))
}

fn payload_object(payload: &Value) -> Result<&Map<String, Value>, ImportV2RouteResponse> {
    payload
        .as_object()
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_stage_elapsed_ms(started_at: Instant) -> u128 {
    started_at.elapsed().as_millis()
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ImportParseServerTiming {
    multipart_ms: u128,
    parser_ms: u128,
    staging_ms: u128,
    total_ms: u128,
}

fn attach_import_parse_server_timing(
    response: &mut Response,
    timing: ImportParseServerTiming,
) {
    let value = format!(
        "multipart;dur={}, parser;dur={}, staging;dur={}, total;dur={}",
        timing.multipart_ms, timing.parser_ms, timing.staging_ms, timing.total_ms
    );
    if let Ok(value) = axum::http::HeaderValue::from_str(&value) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static("server-timing"),
            value,
        );
    }
}

#[derive(Debug)]
struct ImportParseRuntimeInput {
    session_id: String,
    _parser_id: String,
    standard_bills: Vec<ImportParsedStandardBill>,
    source_drafts: Vec<ImportSourceDraft>,
    standard_row_drafts: Vec<ImportStandardRowDraft>,
    file_count: i64,
    files: Vec<Value>,
    unmatched_files: Vec<Value>,
    require_existing_session: bool,
}

#[derive(Debug)]
struct ImportParsedStandardBill {
    source_index: i64,
    source_row_index: i64,
    parser_id: String,
    bill: StandardBill,
    parser_decision: Value,
}

#[derive(Debug)]
struct ImportMultipartFileParseInput {
    index: usize,
    original_name: String,
    body: Vec<u8>,
    requested_parser: String,
}

#[derive(Debug)]
struct ImportMultipartFileParseResult {
    index: usize,
    original_name: String,
    body: Vec<u8>,
    parsed: Option<ImportMultipartParsedFile>,
    decision: DedicatedParserDecision,
    _elapsed_ms: u128,
}

#[derive(Debug)]
struct ImportMultipartParsedFile {
    source_index: usize,
    parser_id: String,
    parsed_count: usize,
    delimiter: Option<char>,
    bills: Vec<StandardBill>,
    decision: DedicatedParserDecision,
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_parse_json_runtime_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    payload: &Value,
    require_existing_session: bool,
) -> Response {
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let object = match payload_object(payload) {
        Ok(object) => object,
        Err(response) => return route_response(response),
    };
    let session_id = if require_existing_session {
        match required_session_id_from_payload(object) {
            Ok(session_id) => session_id,
            Err(response) => return route_response(response),
        }
    } else {
        optional_session_id_from_payload(object).unwrap_or_else(generate_import_session_id)
    };
    let parser_id = parser_id_from_payload(object);
    let mut files = Vec::new();
    let raw_standard_bills = if let Some(temp_path) =
        first_text_from_object(object, &["temp_path", "tempPath"])
    {
        let standard_bills =
            match standard_bills_from_temp_path_payload(object, &temp_path, user_id, &session_id) {
                Ok(standard_bills) => standard_bills,
                Err(response) => return route_response(response),
            };
        files.push(json!({
            "filename": FsPath::new(&temp_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("import-file"),
            "temp_path": temp_path,
            "parser_id": parser_id,
            "parsed_count": standard_bills.len(),
            "source": "temp_path",
        }));
        standard_bills
    } else {
        match standard_bills_from_payload(object) {
            Ok(standard_bills) => standard_bills,
            Err(response) => return route_response(response),
        }
    };
    if raw_standard_bills.is_empty() {
        return route_response(import_v2_error_response(400, "No valid bills to parse"));
    }
    let parser_decision = provided_parser_decision(&parser_id);
    let standard_bills = parsed_standard_bills_from_standard_bills(
        raw_standard_bills,
        &parser_id,
        0,
        parser_decision.clone(),
    );
    let file_count = first_value(object, &["file_count", "fileCount"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
        .unwrap_or_else(|| usize_to_i64(files.len().max(1)));
    let source_name = files
        .first()
        .and_then(|file| file.as_object())
        .and_then(|file| first_value(file, &["filename", "original_name", "originalName"]))
        .and_then(value_to_text)
        .unwrap_or_else(|| "inline-standard-bills".to_string());
    let source_drafts = vec![import_source_draft(ImportSourceDraftInput {
        session_id: &session_id,
        source_index: 0,
        original_file_name: &source_name,
        parser_id: &parser_id,
        parser_signal: "provided",
        parser_confidence: 1.0,
        parser_decision,
        body: &[],
    })];
    let standard_row_drafts = import_standard_row_drafts_from_parsed_bills(&standard_bills);
    persist_import_parse_runtime_response(
        state,
        user_id,
        ImportParseRuntimeInput {
            session_id,
            _parser_id: parser_id,
            standard_bills,
            source_drafts,
            standard_row_drafts,
            file_count,
            files,
            unmatched_files: Vec::new(),
            require_existing_session,
        },
        Instant::now(),
        ImportParseServerTiming::default(),
    )
}

#[tracing::instrument(level = "debug", skip_all)]
async fn import_parse_multipart_runtime_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    content_type: &str,
    body: &[u8],
) -> Response {
    #[cfg(not(coverage))]
    tracing::debug!(domain = "import_parser", operation = "import_parse_multipart_runtime_response", "business operation entered");
    let request_started_at = Instant::now();
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let _multipart_started_at = Instant::now();
    let form = match parse_multipart_form_data(content_type, body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    let multipart_elapsed_ms = import_stage_elapsed_ms(_multipart_started_at);
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "import_parse_multipart_runtime_response",
        user_id = user_id.get(),
        body_bytes = body.len(),
        elapsed_ms = multipart_elapsed_ms,
        "stage1 multipart parsed"
    );
    let session_id = form
        .text_value(&["session_id", "sessionId"])
        .unwrap_or_else(generate_import_session_id);
    let requested_parser = form
        .text_value(&["parser_id", "parserId", "parser_type", "parserType"])
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let file_parts = form.into_file_parts();
    if file_parts.is_empty() {
        return route_response(import_v2_error_response(400, "Missing import files"));
    }
    let file_count = file_parts.len();

    let mut standard_bills = Vec::new();
    let mut source_drafts = Vec::new();
    let mut files = Vec::new();
    let mut unmatched_files = Vec::new();
    let mut first_detected_parser_id: Option<String> = None;

    let parser_started_at = Instant::now();
    let parse_results =
        match parse_multipart_import_files_bounded(file_parts, &requested_parser).await {
            Ok(parse_results) => parse_results,
            Err(response) => return route_response(response),
        };
    let parser_elapsed_ms = import_stage_elapsed_ms(parser_started_at);

    for result in parse_results {
        let _file_index = result.index;
        let original_name = result.original_name;
        let parser_decision = parser_decision_json(&result.decision);
        if let Some(parsed) = result.parsed {
            first_detected_parser_id.get_or_insert_with(|| parsed.parser_id.clone());
            #[cfg(not(coverage))]
            tracing::debug!(
                domain = "import_parser",
                operation = "import_parse_multipart_runtime_response",
                user_id = user_id.get(),
                session_id = %session_id,
                file_index = _file_index,
                parser_id = %parsed.parser_id,
                parsed_count = parsed.parsed_count,
                elapsed_ms = result._elapsed_ms,
                "stage1 dedicated parser matched"
            );
            files.push(json!({
                "filename": original_name.clone(),
                "parser_id": parsed.parser_id.clone(),
                "parsed_count": parsed.parsed_count,
                "delimiter": delimiter_to_response(parsed.delimiter),
                "parser_decision": parser_decision.clone(),
            }));
            source_drafts.push(import_source_draft(ImportSourceDraftInput {
                session_id: &session_id,
                source_index: i64::try_from(parsed.source_index).unwrap_or(i64::MAX),
                original_file_name: &original_name,
                parser_id: &parsed.parser_id,
                parser_signal: &parsed.decision.status,
                parser_confidence: selected_parser_confidence(&parsed.decision),
                parser_decision: parser_decision.clone(),
                body: &result.body,
            }));
            standard_bills.extend(
                parsed
                    .bills
                    .into_iter()
                    .enumerate()
                    .map(|(source_row_index, bill)| ImportParsedStandardBill {
                        source_index: i64::try_from(parsed.source_index).unwrap_or(i64::MAX),
                        source_row_index: i64::try_from(source_row_index).unwrap_or(i64::MAX),
                        parser_id: parsed.parser_id.clone(),
                        bill,
                        parser_decision: parser_decision.clone(),
                    }),
            );
        } else {
            let _unmatched_started_at = Instant::now();
            let unmatched_parser_id = if requested_parser == "auto" {
                "rust-import"
            } else {
                requested_parser.as_str()
            };
            let temp_path = match save_unmatched_import_file(
                user_id,
                &session_id,
                &original_name,
                &result.body,
            ) {
                Ok(path) => path,
                Err(response) => return route_response(response),
            };
            #[cfg(not(coverage))]
            tracing::debug!(
                domain = "import_parser",
                operation = "import_parse_multipart_runtime_response",
                user_id = user_id.get(),
                session_id = %session_id,
                file_index = _file_index,
                elapsed_ms = import_stage_elapsed_ms(_unmatched_started_at),
                "stage1 unmatched file persisted"
            );
            unmatched_files.push(unmatched_import_file_payload(
                &original_name,
                &temp_path,
                unmatched_parser_id,
                &result.decision,
            ));
            source_drafts.push(import_source_draft(ImportSourceDraftInput {
                session_id: &session_id,
                source_index: i64::try_from(_file_index).unwrap_or(i64::MAX),
                original_file_name: &original_name,
                parser_id: unmatched_parser_id,
                parser_signal: &result.decision.status,
                parser_confidence: 0.0,
                parser_decision: parser_decision_json(&result.decision),
                body: &result.body,
            }));
        }
    }

    if standard_bills.is_empty() && unmatched_files.is_empty() {
        return route_response(import_v2_error_response(400, "No import files parsed"));
    }

    let parser_id = first_detected_parser_id.unwrap_or_else(|| {
        if requested_parser == "auto" {
            "rust-import".to_string()
        } else {
            requested_parser
        }
    });
    let standard_row_drafts = import_standard_row_drafts_from_parsed_bills(&standard_bills);
    persist_import_parse_runtime_response(
        state,
        user_id,
        ImportParseRuntimeInput {
            session_id,
            _parser_id: parser_id,
            standard_bills,
            source_drafts,
            standard_row_drafts,
            file_count: usize_to_i64(file_count),
            files,
            unmatched_files,
            require_existing_session: false,
        },
        request_started_at,
        ImportParseServerTiming {
            multipart_ms: multipart_elapsed_ms,
            parser_ms: parser_elapsed_ms,
            ..ImportParseServerTiming::default()
        },
    )
}

#[tracing::instrument(level = "debug", skip_all)]
fn persist_import_parse_runtime_response(
    state: &HttpAppState,
    user_id: UserId,
    input: ImportParseRuntimeInput,
    _request_started_at: Instant,
    timing: ImportParseServerTiming,
) -> Response {
    let mut runtime = match open_runtime(state) {
        Ok(runtime) => runtime,
        Err(response) => return route_response(response),
    };
    if let Err(response) = init_import_runtime_schema(&runtime) {
        return route_response(response);
    }
    let drafts = input
        .standard_bills
        .iter()
        .map(|parsed_bill| {
            parser_template_draft_from_standard_bill(&parsed_bill.bill, &parsed_bill.parser_id)
        })
        .collect::<Vec<_>>();
    let _staging_started_at = Instant::now();
    let staging_result = match stage_import_parser_templates_with_sources(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: input.session_id.clone(),
            user_id,
            file_count: input.file_count.max(1),
        },
        &drafts,
        &input.source_drafts,
        &input.standard_row_drafts,
        input.require_existing_session,
    ) {
        Ok(result) => result,
        Err(error) => return route_response(db_error_response(error)),
    };
    let _staging_elapsed_ms = import_stage_elapsed_ms(_staging_started_at);
    let _total_elapsed_ms = import_stage_elapsed_ms(_request_started_at);
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "import_parser",
        operation = "persist_import_parse_runtime_response",
        user_id = user_id.get(),
        session_id = %input.session_id,
        file_count = input.file_count,
        matched_file_count = input.files.len(),
        unmatched_file_count = input.unmatched_files.len(),
        raw_transaction_count = input.standard_bills.len(),
        standardized_row_count = staging_result.inserted_count,
        elapsed_staging_ms = _staging_elapsed_ms,
        elapsed_total_ms = _total_elapsed_ms,
        "import stage1 summary"
    );
    if !staging_result.session_found {
        return route_response(import_session_not_found_response());
    }
    let mut response = route_response(import_stage_parse_success(ImportStageParseData {
        session_id: input.session_id,
        parsed_count: staging_result.inserted_count,
        files: input.files,
        unmatched_files: input.unmatched_files,
        errors: Vec::new(),
    }));
    attach_import_parse_server_timing(
        &mut response,
        ImportParseServerTiming {
            staging_ms: _staging_elapsed_ms,
            total_ms: _total_elapsed_ms,
            ..timing
        },
    );
    response
}

fn required_session_id_from_payload(
    object: &Map<String, Value>,
) -> Result<String, ImportV2RouteResponse> {
    optional_session_id_from_payload(object)
        .ok_or_else(|| import_v2_error_response(400, "Missing session_id"))
}

fn optional_session_id_from_payload(object: &Map<String, Value>) -> Option<String> {
    first_value(object, &["session_id", "sessionId"])
        .and_then(value_to_text)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parser_id_from_payload(object: &Map<String, Value>) -> String {
    first_value(
        object,
        &[
            "parser_id",
            "parserId",
            "parser_type",
            "parserType",
            "source",
        ],
    )
    .and_then(value_to_text)
    .map(|value| value.trim().to_ascii_lowercase())
    .filter(|value| !value.is_empty() && value != "auto")
    .unwrap_or_else(|| "rust-import".to_string())
}

fn standard_bills_from_payload(
    object: &Map<String, Value>,
) -> Result<Vec<StandardBill>, ImportV2RouteResponse> {
    let bills = first_value(
        object,
        &[
            "bills",
            "standard_bills",
            "standardBills",
            "raw_bills",
            "rawBills",
            "rows",
            "transactions",
            "items",
        ],
    )
    .and_then(Value::as_array)
    .ok_or_else(|| {
        import_v2_error_response(
            400,
            "Rust import parse currently requires inline normalized bills",
        )
    })?;
    let parsed = bills
        .iter()
        .map(standard_bill_from_payload_value)
        .filter(|bill| !bill.date.trim().is_empty())
        .collect::<Vec<_>>();
    if parsed.is_empty() && !bills.is_empty() {
        return Err(import_v2_error_response(400, "No valid bills to parse"));
    }
    Ok(parsed)
}

fn standard_bill_from_payload_value(value: &Value) -> StandardBill {
    let mut normalized = value.clone();
    if let Some(object) = normalized.as_object_mut() {
        copy_alias_if_missing(
            object,
            "date",
            &[
                "trade_time",
                "time",
                "交易时间",
                "交易日期",
                "记账日期",
                "日期",
            ],
        );
        copy_alias_if_missing(
            object,
            "type",
            &[
                "transaction_type",
                "trade_type",
                "收支类型",
                "收/支",
                "类型",
            ],
        );
        copy_alias_if_missing(
            object,
            "amount",
            &["金额", "金额(元)", "交易金额"],
        );
        copy_alias_if_missing(
            object,
            "description",
            &[
                "remark",
                "memo",
                "summary",
                "商品",
                "备注",
                "说明",
                "交易说明",
            ],
        );
        copy_alias_if_missing(
            object,
            "counterparty",
            &["opponent", "merchant", "shop", "对方", "交易对方", "商户"],
        );
        copy_alias_if_missing(
            object,
            "payment_method",
            &[
                "paymentMethod",
                "channel",
                "account",
                "账户",
                "支付方式",
                "收/付款方式",
            ],
        );
        copy_alias_if_missing(object, "original_category", &["交易分类", "分类", "类别"]);
        copy_alias_if_missing(object, "transaction_id", &["交易单号", "订单号"]);
        copy_alias_if_missing(object, "merchant_id", &["商家订单号", "商户单号"]);
        copy_alias_if_missing(object, "status", &["交易状态", "当前状态", "状态"]);
    }
    StandardBill::from_json_value(&normalized)
}

fn copy_alias_if_missing(object: &mut Map<String, Value>, target: &str, aliases: &[&str]) {
    if object
        .get(target)
        .and_then(value_to_text)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return;
    }
    if let Some(value) = aliases.iter().find_map(|alias| object.get(*alias).cloned()) {
        object.insert(target.to_string(), value);
    }
}
