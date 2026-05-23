// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端兼容响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn import_preview_request_from_body(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<ImportPreviewRequest, ImportV2RouteResponse> {
    let content_type = content_type_from_headers(headers);
    let content_type_lower = content_type.to_ascii_lowercase();
    if content_type_lower.contains("multipart/form-data") {
        let form = parse_multipart_form_data(&content_type, body)?;
        return Ok(ImportPreviewRequest {
            temp_path: form.text_value(&["temp_path", "tempPath"]),
            uploaded_file: form.file_parts().first().map(|part| part.body.clone()),
            delimiter: form.text_value(&["delimiter"]),
        });
    }
    if content_type_lower.contains("application/json") {
        let payload = serde_json::from_slice::<Value>(body)
            .map_err(|_| import_v2_error_response(400, "Invalid JSON request"))?;
        let object = payload_object(&payload)?;
        return Ok(ImportPreviewRequest {
            temp_path: first_text_from_object(object, &["temp_path", "tempPath"]),
            uploaded_file: None,
            delimiter: first_text_from_object(object, &["delimiter"]),
        });
    }
    let form = parse_urlencoded_form(body);
    Ok(ImportPreviewRequest {
        temp_path: form
            .get("temp_path")
            .or_else(|| form.get("tempPath"))
            .cloned(),
        uploaded_file: None,
        delimiter: form.get("delimiter").cloned(),
    })
}

fn parse_legacy_import_file(
    form: &MultipartForm,
    filename: &str,
    body: &[u8],
    _user_id: UserId,
) -> Result<LegacyImportParseResult, ImportV2RouteResponse> {
    let object = multipart_text_object(form);
    let requested_file_type = form
        .text_value(&[
            "fileType",
            "file_type",
            "parser_type",
            "parserType",
            "parser_id",
            "parserId",
        ])
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let force_generic = matches!(
        requested_file_type.as_str(),
        "generic" | "csv" | "xlsx" | "xls" | "txt"
    );
    let use_column_mapping = first_value(&object, &["columnMapping", "column_mapping"])
        .is_some_and(mapping_value_is_present);
    let dedicated_parsed = if force_generic || use_column_mapping {
        None
    } else {
        parse_dedicated_import_bytes(filename, body, &requested_file_type)
    };
    let text = decode_import_text(body);
    let detected_parser_type = dedicated_parsed
        .as_ref()
        .map(|parsed| parsed.parser_id.clone())
        .unwrap_or_else(|| {
            resolve_import_file_parser_id("auto", filename, &text).replace("rust-import", "")
        });
    let (parser_type, bills) = if force_generic || use_column_mapping {
        let bills = if use_column_mapping {
            standard_bills_from_column_mapped_text(&object, &text)?
        } else {
            parse_standard_bills_from_csv_text(&text, "generic", true).bills
        };
        ("generic".to_string(), bills)
    } else if let Some(parsed) = dedicated_parsed {
        (parsed.parser_id, parsed.bills)
    } else {
        let parser_id = if requested_file_type == "auto" {
            if detected_parser_type.is_empty() {
                "rust-import".to_string()
            } else {
                detected_parser_type.clone()
            }
        } else {
            requested_file_type
        };
        let parsed = parse_standard_bills_from_csv_text(&text, &parser_id, true);
        (parser_id, parsed.bills)
    };
    if bills.is_empty() {
        return Err(import_v2_error_response(400, "No valid bills to parse"));
    }
    let items = bills
        .iter()
        .map(|bill| legacy_import_item_from_standard_bill(bill, &parser_type))
        .collect::<Vec<_>>();
    Ok(LegacyImportParseResult {
        total_count: items.len(),
        items,
        parser_type,
        detected_parser_type,
    })
}

fn multipart_text_object(form: &MultipartForm) -> Map<String, Value> {
    let mut object = Map::new();
    for part in &form.parts {
        if part.filename.is_some() {
            continue;
        }
        let text = decode_import_text(&part.body).trim().to_string();
        if text.is_empty() {
            continue;
        }
        let value = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
        object.insert(part.name.clone(), value);
    }
    object
}

fn mapping_value_is_present(value: &Value) -> bool {
    match value {
        Value::Object(object) => !object.is_empty(),
        Value::String(text) => serde_json::from_str::<Value>(text)
            .ok()
            .and_then(|value| value.as_object().map(|object| !object.is_empty()))
            .unwrap_or_else(|| !text.trim().is_empty()),
        _ => false,
    }
}

fn legacy_import_item_from_standard_bill(bill: &StandardBill, parser_id: &str) -> Value {
    let frontend_type = frontend_type_from_bill_type(&bill.transaction_type);
    let source_amount = bill.amount.to_cents().saturating_abs();
    let amount = bill
        .amount
        .to_yuan_string()
        .parse::<f64>()
        .unwrap_or_default()
        .abs();
    let original_category = original_category_name(&bill.main_category, &bill.sub_category)
        .unwrap_or_else(|| bill.original_category.clone());
    json!({
        "type": frontend_type,
        "categoryId": "",
        "originalCategoryName": original_category,
        "time": legacy_import_time_seconds(&bill.date),
        "utcOffset": 0,
        "sourceAccountId": positive_id_text(&bill.source_account_id),
        "originalSourceAccountName": bill.payment_method,
        "originalSourceAccountCurrency": "CNY",
        "destinationAccountId": "",
        "originalDestinationAccountName": "",
        "originalDestinationAccountCurrency": "CNY",
        "sourceAmount": source_amount,
        "destinationAmount": 0,
        "tagIds": [],
        "originalTagNames": [],
        "comment": legacy_comment_text(&bill.description),
        "counterparty": bill.counterparty,
        "paymentMethod": bill.payment_method,
        "timeText": bill.date,
        "categoryName": bill.main_category,
        "subCategoryName": bill.sub_category,
        "accountName": "",
        "amount": amount,
        "description": legacy_comment_text(&bill.description),
        "parserSource": parser_id,
        "parserTags": bill.parser_tags,
        "isManuallyAnnotated": false,
    })
}

fn legacy_comment_text(description: &str) -> String {
    description
        .split('|')
        .next()
        .unwrap_or(description)
        .trim()
        .to_string()
}

fn legacy_reclassify_passthrough(index: usize, transaction: &Value) -> Value {
    let object = transaction.as_object();
    json!({
        "index": index,
        "mainCategory": object
            .and_then(|item| first_value(item, &["mainCategory", "main_category", "categoryName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "subCategory": object
            .and_then(|item| first_value(item, &["subCategory", "sub_category", "subCategoryName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "categoryId": object
            .and_then(|item| first_value(item, &["categoryId", "category_id"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "categoryName": object
            .and_then(|item| first_value(item, &["categoryName", "originalCategoryName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "sourceAccountId": object
            .and_then(|item| first_value(item, &["sourceAccountId", "source_account_id"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "sourceAccountName": object
            .and_then(|item| first_value(item, &["accountName", "sourceAccountName", "originalSourceAccountName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "destinationAccountId": object
            .and_then(|item| first_value(item, &["destinationAccountId", "destination_account_id"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "destinationAccountName": object
            .and_then(|item| first_value(item, &["destinationAccountName", "originalDestinationAccountName"]))
            .and_then(value_to_text)
            .unwrap_or_default(),
        "provider_bypassed": true,
        "runtime": "rust-import-db-runtime-partial",
    })
}

fn frontend_type_from_bill_type(raw_type: &str) -> i64 {
    match raw_type.trim() {
        "余额调整" => 1,
        "收入" | "退款" => 2,
        "转账" => 4,
        "投资" => 5,
        _ => 3,
    }
}

fn original_category_name(main_category: &str, sub_category: &str) -> Option<String> {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if main_category.is_empty() && sub_category.is_empty() {
        None
    } else if sub_category.is_empty() {
        Some(main_category.to_string())
    } else if main_category.is_empty() {
        Some(sub_category.to_string())
    } else {
        Some(format!("{main_category}-{sub_category}"))
    }
}

fn positive_id_text(raw_value: &str) -> String {
    raw_value
        .trim()
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value.to_string())
        .unwrap_or_default()
}

fn legacy_import_time_seconds(raw_value: &str) -> i64 {
    let text = raw_value.trim();
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y/%m/%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(datetime) = chrono::NaiveDateTime::parse_from_str(text, format) {
            return datetime.and_utc().timestamp();
        }
    }
    for format in ["%Y-%m-%d", "%Y/%m/%d"] {
        if let Ok(date) = chrono::NaiveDate::parse_from_str(text, format) {
            if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                return datetime.and_utc().timestamp();
            }
        }
    }
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}

fn legacy_import_parsers() -> Value {
    json!([
        {
            "id": "auto",
            "name": "自动识别",
            "description": "自动检测文件类型并选择合适的解析器",
            "supported_formats": ["csv"],
        },
        {
            "id": "wechat",
            "name": "微信支付",
            "description": "解析微信支付账单CSV文件",
            "supported_formats": ["csv"],
        },
        {
            "id": "alipay",
            "name": "支付宝",
            "description": "解析支付宝交易明细CSV文件",
            "supported_formats": ["csv"],
        },
        {
            "id": "icbc",
            "name": "工商银行",
            "description": "解析工商银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
        {
            "id": "cmbc",
            "name": "民生银行",
            "description": "解析民生银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
        {
            "id": "abc",
            "name": "农业银行",
            "description": "解析农业银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
        {
            "id": "ccb",
            "name": "建设银行",
            "description": "解析建设银行流水文件",
            "supported_formats": ["csv", "xlsx", "xls"],
        },
    ])
}

fn init_import_config_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    init_app_settings_schema(runtime.connection()).map_err(db_error_response)
}

fn import_config_setting_key(user_id: UserId) -> String {
    format!("import_configs_user_{}", user_id.get())
}

fn load_import_configs(
    connection: &Connection,
    user_id: UserId,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let stored = get_app_setting(connection, &import_config_setting_key(user_id))
        .map_err(db_error_response)?;
    let Some(stored) = stored else {
        return Ok(Vec::new());
    };
    let parsed = serde_json::from_str::<Value>(&stored).unwrap_or_else(|_| Value::Array(vec![]));
    let configs = parsed
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(configs)
}

fn store_import_configs(
    connection: &Connection,
    user_id: UserId,
    configs: &[Value],
) -> Result<(), ImportV2RouteResponse> {
    set_app_setting(
        connection,
        &AppSettingDraft {
            key: import_config_setting_key(user_id),
            value: Value::Array(configs.to_vec()).to_string(),
            value_type: "json".to_string(),
            description: Some("User import column mapping templates".to_string()),
            is_encrypted: false,
        },
    )
    .map(|_| ())
    .map_err(db_error_response)
}

fn build_import_config_from_payload(object: &Map<String, Value>, config_id: i64) -> Value {
    let now = now_text();
    let field_mappings = first_value(object, &["fieldMappings", "field_mappings"])
        .cloned()
        .unwrap_or_else(|| json!({}));
    let file_format = config_text_from_object(object, &["fileFormat", "file_format"])
        .unwrap_or_else(|| "csv".to_string());
    let sample_headers =
        string_array_field_from_object(object, &["sampleHeaders", "sample_headers", "headers"])
            .unwrap_or_default();
    let header_signature =
        config_text_from_object(object, &["headerSignature", "header_signature"])
            .unwrap_or_else(|| header_signature_from_headers(&sample_headers));
    json!({
        "id": config_id,
        "name": config_text_from_object(object, &["name"]).unwrap_or_default(),
        "fileFormat": file_format,
        "description": config_text_from_object(object, &["description"]),
        "descriptionSummary": config_text_from_object(object, &["descriptionSummary", "description_summary"]),
        "fieldMappings": field_mappings,
        "columnMapping": first_value(object, &["columnMapping", "column_mapping"]).cloned().unwrap_or_else(|| first_value(object, &["fieldMappings", "field_mappings"]).cloned().unwrap_or_else(|| json!({}))),
        "transactionTypeMapping": first_value(object, &["transactionTypeMapping", "transaction_type_mapping"]).cloned().unwrap_or_else(|| json!({
            "收入": 2,
            "支出": 3,
            "转账": 4,
            "投资": 5,
        })),
        "dateFormat": config_text_from_object(object, &["dateFormat", "date_format"]).unwrap_or_else(|| "%Y-%m-%d %H:%M:%S".to_string()),
        "encoding": config_text_from_object(object, &["encoding"]).unwrap_or_else(|| "utf-8".to_string()),
        "delimiter": config_text_from_object(object, &["delimiter"]).unwrap_or_else(|| ",".to_string()),
        "skipRows": first_value(object, &["skipRows", "skip_rows"]).and_then(value_to_i64).unwrap_or(0),
        "hasHeader": first_value(object, &["hasHeader", "has_header"]).and_then(value_to_bool).unwrap_or(true),
        "customRules": first_value(object, &["customRules", "custom_rules"]).cloned().unwrap_or_else(|| json!([])),
        "sampleHeaders": sample_headers,
        "headerSignature": header_signature,
        "isDefault": first_value(object, &["isDefault", "is_default"]).and_then(value_to_bool).unwrap_or(false),
        "defaultRecommendation": first_value(object, &["defaultRecommendation", "default_recommendation"]).and_then(value_to_bool).unwrap_or(false),
        "useCount": first_value(object, &["useCount", "use_count"]).and_then(value_to_i64).unwrap_or(0),
        "lastUsedAt": config_text_from_object(object, &["lastUsedAt", "last_used_at"]),
        "createdAt": config_text_from_object(object, &["createdAt", "created_at"]).unwrap_or_else(|| now.clone()),
        "updatedAt": now,
    })
}

fn best_import_config_match(
    configs: &[Value],
    file_format: &str,
    headers: &[String],
) -> Option<Value> {
    let requested_format = normalize_config_text(file_format);
    let normalized_headers = headers
        .iter()
        .map(|header| normalize_config_text(header))
        .filter(|header| !header.is_empty())
        .collect::<Vec<_>>();
    if normalized_headers.is_empty() {
        return None;
    }
    let mut best: Option<(Value, usize, f64)> = None;
    for config in configs {
        let config_format = config_text(config, &["fileFormat", "file_format"])
            .map(|value| normalize_config_text(&value))
            .unwrap_or_default();
        if config_format != requested_format {
            continue;
        }
        let sample_headers = config_string_array(config, &["sampleHeaders", "sample_headers"])
            .or_else(|| {
                config_text(config, &["headerSignature", "header_signature"]).map(|signature| {
                    signature
                        .split('|')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        let sample_headers = sample_headers
            .iter()
            .map(|header| normalize_config_text(header))
            .collect::<Vec<_>>();
        let matched = normalized_headers
            .iter()
            .filter(|header| sample_headers.contains(header))
            .count();
        let score = matched as f64 / normalized_headers.len() as f64;
        if matched == 0 {
            continue;
        }
        if best
            .as_ref()
            .map(|(_, best_matched, best_score)| {
                matched > *best_matched || (matched == *best_matched && score > *best_score)
            })
            .unwrap_or(true)
        {
            best = Some((config.clone(), matched, score));
        }
    }
    best.map(|(mut config, matched, score)| {
        if let Some(object) = config.as_object_mut() {
            object.insert("matchScore".to_string(), json!(score));
            object.insert("matchedHeaderCount".to_string(), json!(matched));
            object.insert(
                "matchReason".to_string(),
                json!(format!("Matched {matched} import headers")),
            );
            object.insert("defaultRecommendation".to_string(), json!(score >= 0.5));
        }
        config
    })
}

fn build_import_config_suggestion(
    file_format: &str,
    headers: &[String],
    matched: Option<&Value>,
) -> Value {
    let mut field_mappings = Map::new();
    for (index, header) in headers.iter().enumerate() {
        if let Some(column_type) = import_config_column_type_for_header(header) {
            field_mappings.insert(column_type.to_string(), json!(index));
        }
    }
    let confidence = if field_mappings.contains_key("1") && field_mappings.contains_key("8") {
        0.8
    } else {
        0.45
    };
    let column_mapping = field_mappings.clone();
    json!({
        "fileFormat": file_format,
        "fieldMappings": field_mappings,
        "columnMapping": column_mapping,
        "transactionTypeMapping": {
            "收入": 2,
            "支出": 3,
            "转账": 4,
            "投资": 5,
        },
        "hasHeader": true,
        "encoding": "utf-8",
        "delimiter": ",",
        "confidence": confidence,
        "matchedConfig": matched.cloned(),
        "defaultRecommendation": matched.is_some(),
    })
}

fn import_config_column_type_for_header(header: &str) -> Option<i64> {
    let header = normalize_config_text(header);
    if header.contains("交易时间")
        || header.contains("交易日期")
        || header.contains("记账日期")
        || header == "date"
        || header == "time"
        || header.contains("trade")
    {
        Some(1)
    } else if header.contains("收支类型") || header.contains("交易类型") || header.contains("type")
    {
        Some(3)
    } else if header.contains("分类") || header.contains("category") {
        Some(4)
    } else if header.contains("账户")
        || header.contains("支付方式")
        || header.contains("付款方式")
        || header.contains("account")
        || header.contains("payment")
    {
        Some(6)
    } else if header.contains("金额") || header.contains("amount") {
        Some(8)
    } else if header.contains("备注")
        || header.contains("说明")
        || header.contains("摘要")
        || header.contains("商品")
        || header.contains("商户")
        || header.contains("对方")
        || header.contains("remark")
        || header.contains("description")
        || header.contains("merchant")
        || header.contains("counterparty")
    {
        Some(14)
    } else {
        None
    }
}
