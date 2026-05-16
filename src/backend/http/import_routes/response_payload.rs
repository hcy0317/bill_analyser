fn route_response(response: ImportV2RouteResponse) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(response.body)).into_response()
}

fn ai_route_response(response: AiRouteResponse) -> Response {
    let status =
        StatusCode::from_u16(response.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(response.body)).into_response()
}

fn payload_object(payload: &Value) -> Result<&Map<String, Value>, ImportV2RouteResponse> {
    payload
        .as_object()
        .ok_or_else(|| import_v2_error_response(400, "Invalid request"))
}

fn import_stage_elapsed_ms(started_at: Instant) -> u128 {
    started_at.elapsed().as_millis()
}

#[derive(Debug)]
struct ImportParseRuntimeInput {
    session_id: String,
    parser_id: String,
    standard_bills: Vec<ImportParsedStandardBill>,
    file_count: i64,
    files: Vec<Value>,
    unmatched_files: Vec<Value>,
    require_existing_session: bool,
}

#[derive(Debug)]
struct ImportParsedStandardBill {
    parser_id: String,
    bill: StandardBill,
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
    elapsed_ms: u128,
}

#[derive(Debug)]
struct ImportMultipartParsedFile {
    parser_id: String,
    parsed_count: usize,
    delimiter: Option<char>,
    bills: Vec<StandardBill>,
}

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
    let standard_bills =
        parsed_standard_bills_from_standard_bills(raw_standard_bills, &parser_id);
    let file_count = first_value(object, &["file_count", "fileCount"])
        .and_then(value_to_i64)
        .filter(|value| *value > 0)
        .unwrap_or_else(|| usize_to_i64(files.len().max(1)));
    persist_import_parse_runtime_response(
        state,
        user_id,
        ImportParseRuntimeInput {
            session_id,
            parser_id,
            standard_bills,
            file_count,
            files,
            unmatched_files: Vec::new(),
            require_existing_session,
        },
        Instant::now(),
    )
}

async fn import_parse_multipart_runtime_response(
    state: &HttpAppState,
    headers: &HeaderMap,
    content_type: &str,
    body: &[u8],
) -> Response {
    let request_started_at = Instant::now();
    let user_id = match user_id_from_headers(headers, &state.config) {
        Ok(user_id) => user_id,
        Err(response) => return route_response(response),
    };
    let multipart_started_at = Instant::now();
    let form = match parse_multipart_form_data(content_type, body) {
        Ok(form) => form,
        Err(response) => return route_response(response),
    };
    eprintln!(
        "[bill analyser import] stage1 multipart parsed user_id={} body_bytes={} elapsed_ms={}",
        user_id.get(),
        body.len(),
        import_stage_elapsed_ms(multipart_started_at)
    );
    let session_id = form
        .text_value(&["session_id", "sessionId"])
        .unwrap_or_else(generate_import_session_id);
    let requested_parser = form
        .text_value(&["parser_id", "parserId", "parser_type", "parserType"])
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let file_parts = form.file_parts();
    if file_parts.is_empty() {
        return route_response(import_v2_error_response(400, "Missing import files"));
    }
    let file_count = file_parts.len();

    let mut standard_bills = Vec::new();
    let mut files = Vec::new();
    let mut unmatched_files = Vec::new();
    let mut first_detected_parser_id: Option<String> = None;

    let parse_results =
        match parse_multipart_import_files_parallel(file_parts, &requested_parser).await {
            Ok(parse_results) => parse_results,
            Err(response) => return route_response(response),
        };

    for result in parse_results {
        let original_name = result.original_name;
        if let Some(parsed) = result.parsed {
            first_detected_parser_id.get_or_insert_with(|| parsed.parser_id.clone());
            eprintln!(
                "[bill analyser import] stage1 dedicated parser matched user_id={} session_id={session_id} filename={original_name} parser_id={} parsed_count={parsed_count} elapsed_ms={}",
                user_id.get(),
                parsed.parser_id,
                result.elapsed_ms,
                parsed_count = parsed.parsed_count
            );
            files.push(json!({
                "filename": original_name,
                "parser_id": parsed.parser_id,
                "parsed_count": parsed.parsed_count,
                "delimiter": delimiter_to_response(parsed.delimiter),
            }));
            standard_bills.extend(
                parsed
                    .bills
                    .into_iter()
                    .map(|bill| ImportParsedStandardBill {
                        parser_id: parsed.parser_id.clone(),
                        bill,
                    }),
            );
        } else {
            let unmatched_started_at = Instant::now();
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
            eprintln!(
                "[bill analyser import] stage1 unmatched file persisted user_id={} session_id={session_id} filename={original_name} temp_path={temp_path} elapsed_ms={}",
                user_id.get(),
                import_stage_elapsed_ms(unmatched_started_at)
            );
            unmatched_files.push(json!({
                "original_name": original_name,
                "originalName": original_name,
                "filename": original_name,
                "temp_path": temp_path,
                "tempPath": temp_path,
                "parser_id": unmatched_parser_id,
                "reason": "No dedicated Rust parser matched the uploaded file",
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
    persist_import_parse_runtime_response(
        state,
        user_id,
        ImportParseRuntimeInput {
            session_id,
            parser_id,
            standard_bills,
            file_count: usize_to_i64(file_count),
            files,
            unmatched_files,
            require_existing_session: false,
        },
        request_started_at,
    )
}

async fn parse_multipart_import_files_parallel(
    file_parts: Vec<&MultipartPart>,
    requested_parser: &str,
) -> Result<Vec<ImportMultipartFileParseResult>, ImportV2RouteResponse> {
    let mut handles = Vec::with_capacity(file_parts.len());
    for (index, part) in file_parts.into_iter().enumerate() {
        let input = ImportMultipartFileParseInput {
            index,
            original_name: part
                .filename
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "import-file.csv".to_string()),
            body: part.body.clone(),
            requested_parser: requested_parser.to_string(),
        };
        handles.push(tokio::task::spawn_blocking(move || {
            parse_multipart_import_file(input)
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let result = handle.await.map_err(|error| {
            import_v2_error_response(500, &format!("Import parser worker failed: {error}"))
        })?;
        results.push(result);
    }
    results.sort_by_key(|result| result.index);
    Ok(results)
}

fn parse_multipart_import_file(
    input: ImportMultipartFileParseInput,
) -> ImportMultipartFileParseResult {
    let started_at = Instant::now();
    let parsed =
        parse_dedicated_import_bytes(&input.original_name, &input.body, &input.requested_parser)
            .filter(|parsed| !parsed.bills.is_empty())
            .map(|parsed| ImportMultipartParsedFile {
                parser_id: parsed.parser_id,
                parsed_count: parsed.bills.len(),
                delimiter: parsed.delimiter,
                bills: parsed.bills,
            });
    ImportMultipartFileParseResult {
        index: input.index,
        original_name: input.original_name,
        body: input.body,
        parsed,
        elapsed_ms: import_stage_elapsed_ms(started_at),
    }
}

fn persist_import_parse_runtime_response(
    state: &HttpAppState,
    user_id: UserId,
    input: ImportParseRuntimeInput,
    request_started_at: Instant,
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
    let staging_started_at = Instant::now();
    let staging_result = match stage_import_parser_templates(
        runtime.connection_mut(),
        &ImportSessionDraft {
            session_id: input.session_id.clone(),
            user_id,
            file_count: input.file_count.max(1),
        },
        &drafts,
        input.require_existing_session,
    ) {
        Ok(result) => result,
        Err(error) => return route_response(db_error_response(error)),
    };
    eprintln!(
        "[bill analyser import] stage1 staging inserted user_id={} session_id={} parser_id={} drafts={} inserted_count={} staging_elapsed_ms={} total_elapsed_ms={}",
        user_id.get(),
        input.session_id,
        input.parser_id,
        drafts.len(),
        staging_result.inserted_count,
        import_stage_elapsed_ms(staging_started_at),
        import_stage_elapsed_ms(request_started_at)
    );
    if !staging_result.session_found {
        return route_response(import_session_not_found_response());
    }
    route_response(import_stage_parse_success(ImportStageParseData {
        session_id: input.session_id,
        parsed_count: staging_result.inserted_count,
        files: input.files,
        unmatched_files: input.unmatched_files,
        errors: Vec::new(),
    }))
}

fn parsed_standard_bills_from_standard_bills(
    standard_bills: Vec<StandardBill>,
    parser_id: &str,
) -> Vec<ImportParsedStandardBill> {
    standard_bills
        .into_iter()
        .map(|bill| ImportParsedStandardBill {
            parser_id: parser_id.to_string(),
            bill,
        })
        .collect()
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
            &[
                "source_amount",
                "sourceAmount",
                "金额",
                "金额(元)",
                "交易金额",
            ],
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

