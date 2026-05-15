fn init_legacy_bills_runtime_schema(runtime: &SqliteRuntime) -> Result<(), ImportV2RouteResponse> {
    runtime
        .connection()
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bills (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                user_id INTEGER NOT NULL DEFAULT 1,
                date TEXT NOT NULL,
                type TEXT NOT NULL,
                amount REAL NOT NULL,
                counterparty TEXT NOT NULL,
                description TEXT NOT NULL,
                payment_method TEXT DEFAULT '',
                main_category TEXT,
                sub_category TEXT,
                batch_id TEXT,
                hash TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                source_account_id INTEGER DEFAULT 0,
                destination_account_id INTEGER DEFAULT 0,
                destination_amount REAL DEFAULT 0,
                created_from_template INTEGER,
                created_from_recurring INTEGER,
                import_history_id INTEGER,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_bills_user_hash_unique ON bills(user_id, hash);
            ",
        )
        .map_err(db_error_response)
}

fn insert_legacy_confirmed_bills(
    connection: &mut Connection,
    user_id: UserId,
    bills: &[Value],
) -> Result<Value, ImportV2RouteResponse> {
    let user_id_i64 = user_id_i64_value(user_id)?;
    let batch_id = format!("legacy-import-{}", legacy_import_time_seconds(""));
    let now = now_text();
    let tx = connection.transaction().map_err(db_error_response)?;
    let mut inserted = 0_i64;
    let mut duplicates = 0_i64;
    let mut skipped = 0_i64;
    let mut errors = Vec::new();
    for (index, bill) in bills.iter().enumerate() {
        let Some(object) = bill.as_object() else {
            skipped += 1;
            errors.push(json!({"index": index, "error": "Invalid bill"}));
            continue;
        };
        let Some(amount) = legacy_bill_amount(object) else {
            skipped += 1;
            errors.push(json!({"index": index, "error": "Invalid amount"}));
            continue;
        };
        let bill_type = legacy_bill_type(object);
        let amount = legacy_confirm_amount_for_type(&bill_type, amount);
        let date = legacy_bill_date(object);
        let counterparty = first_text_from_object(object, &["counterparty"]).unwrap_or_else(|| {
            first_text_from_object(object, &["originalSourceAccountName", "accountName"])
                .unwrap_or_default()
        });
        let description = first_text_from_object(object, &["description", "comment", "remark"])
            .unwrap_or_else(|| counterparty.clone());
        let payment_method = first_text_from_object(
            object,
            &[
                "paymentMethod",
                "payment_method",
                "originalSourceAccountName",
                "accountName",
            ],
        )
        .unwrap_or_default();
        let main_category = first_text_from_object(
            object,
            &[
                "mainCategory",
                "main_category",
                "categoryName",
                "originalCategoryName",
            ],
        );
        let sub_category =
            first_text_from_object(object, &["subCategory", "sub_category", "subCategoryName"]);
        let source_account_id = first_value(object, &["sourceAccountId", "source_account_id"])
            .and_then(value_to_i64)
            .unwrap_or_default();
        let destination_account_id =
            first_value(object, &["destinationAccountId", "destination_account_id"])
                .and_then(value_to_i64)
                .unwrap_or_default();
        let destination_amount = first_value(object, &["destinationAmount", "destination_amount"])
            .and_then(value_to_f64)
            .map(|value| value / 100.0)
            .unwrap_or_default();
        let hash =
            calculate_import_bill_hash(&date, &bill_type, amount, &counterparty, &description);
        let changed = tx
            .execute(
                "
                INSERT OR IGNORE INTO bills (
                    user_id, date, type, amount, counterparty, description,
                    payment_method, main_category, sub_category,
                    source_account_id, destination_account_id, destination_amount,
                    batch_id, hash, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)
                ",
                params![
                    user_id_i64,
                    date,
                    bill_type,
                    amount,
                    counterparty,
                    description,
                    payment_method,
                    main_category,
                    sub_category,
                    source_account_id,
                    destination_account_id,
                    destination_amount,
                    batch_id,
                    hash,
                    now,
                ],
            )
            .map_err(db_error_response)?;
        if changed > 0 {
            inserted += 1;
        } else {
            duplicates += 1;
        }
    }
    tx.commit().map_err(db_error_response)?;
    Ok(json!({
        "total": bills.len(),
        "inserted": inserted,
        "duplicates": duplicates,
        "skipped": skipped,
        "errors": errors,
        "batch_id": batch_id,
        "runtime": "rust-import-db-runtime-partial",
    }))
}

fn preview_rows_from_temp_path(
    temp_path: &FsPath,
    delimiter: Option<&str>,
) -> Result<Value, ImportV2RouteResponse> {
    let text = read_import_temp_text(temp_path)?;
    preview_rows_from_text(&text, delimiter)
}

fn preview_rows_from_file_bytes(
    body: &[u8],
    delimiter: Option<&str>,
) -> Result<Value, ImportV2RouteResponse> {
    let text = decode_import_text(body);
    preview_rows_from_text(&text, delimiter)
}

fn preview_rows_from_text(
    text: &str,
    delimiter: Option<&str>,
) -> Result<Value, ImportV2RouteResponse> {
    let detected_delimiter = delimiter_from_hint(delimiter)
        .or_else(|| detect_csv_table_start(text, false).map(|(_, delimiter)| delimiter))
        .unwrap_or(',');
    let rows = csv_rows_from_text(text, delimiter)?;
    let headers = rows.first().cloned().unwrap_or_default();
    let total_rows = rows.len();
    Ok(json!({
        "headers": headers,
        "sampleData": rows.iter().take(300).cloned().collect::<Vec<_>>(),
        "previewRows": rows.iter().skip(1).take(50).cloned().collect::<Vec<_>>(),
        "totalRows": total_rows,
        "rowCount": total_rows,
        "encoding": "utf-8-or-gbk",
        "delimiter": delimiter_to_response(Some(detected_delimiter)),
    }))
}

fn csv_rows_from_text(
    text: &str,
    delimiter_hint: Option<&str>,
) -> Result<Vec<Vec<String>>, ImportV2RouteResponse> {
    let delimiter = delimiter_from_hint(delimiter_hint)
        .or_else(|| detect_csv_table_start(text, false).map(|(_, delimiter)| delimiter))
        .unwrap_or(',');
    let start_line = detect_csv_table_start(text, false)
        .map(|(line, _)| line)
        .unwrap_or(0);
    let csv_text = text.lines().skip(start_line).collect::<Vec<_>>().join("\n");
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(csv_text.as_bytes());
    let rows = reader
        .records()
        .map(|record| {
            record
                .map(|record| {
                    record
                        .iter()
                        .map(|value| value.trim().to_string())
                        .collect()
                })
                .map_err(|error| {
                    import_v2_error_response(400, &format!("Invalid CSV file: {error}"))
                })
        })
        .collect::<Result<Vec<Vec<String>>, ImportV2RouteResponse>>()?;
    Ok(rows)
}

fn split_preview_line(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|value| value.trim().trim_matches('"').to_string())
        .collect()
}

fn detect_delimiter_for_line(line: &str, hint: Option<&str>) -> Option<char> {
    if let Some(delimiter) = delimiter_from_hint(hint) {
        return Some(delimiter);
    }
    [',', '\t', ';']
        .into_iter()
        .map(|delimiter| (delimiter, line.matches(delimiter).count()))
        .max_by_key(|(_, count)| *count)
        .and_then(|(delimiter, count)| if count > 0 { Some(delimiter) } else { None })
}

fn delimiter_from_hint(delimiter: Option<&str>) -> Option<char> {
    let delimiter = delimiter?.trim();
    if delimiter.is_empty() {
        return None;
    }
    match delimiter {
        "\\t" | "tab" | "TAB" => Some('\t'),
        _ => delimiter.chars().next(),
    }
}

fn delimiter_to_response(delimiter: Option<char>) -> String {
    match delimiter.unwrap_or(',') {
        '\t' => "\t".to_string(),
        value => value.to_string(),
    }
}

fn parse_urlencoded_form(body: &[u8]) -> HashMap<String, String> {
    let text = String::from_utf8_lossy(body);
    text.split('&')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (
                percent_decode_form_component(key),
                percent_decode_form_component(value),
            )
        })
        .collect()
}

fn percent_decode_form_component(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16) {
                    output.push(value);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            value => {
                output.push(value);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).to_string()
}

fn save_unmatched_import_file(
    user_id: UserId,
    session_id: &str,
    original_name: &str,
    body: &[u8],
) -> Result<String, ImportV2RouteResponse> {
    let user_component = import_temp_user_component(user_id);
    let session_component = sanitize_path_component(session_id);
    let dir = temp_import_root()
        .join(&user_component)
        .join(&session_component);
    fs::create_dir_all(&dir).map_err(|error| {
        import_v2_error_response(
            500,
            &format!("Unable to prepare temp import directory: {error}"),
        )
    })?;
    let file_name = format!(
        "{}-{}",
        generate_import_session_id(),
        sanitize_path_component(original_name)
    );
    let path = dir.join(file_name);
    fs::write(&path, body).map_err(|error| {
        import_v2_error_response(500, &format!("Unable to persist temp import file: {error}"))
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| import_v2_error_response(500, "Unable to resolve temp import filename"))?;
    Ok(format!("{user_component}/{session_component}/{file_name}"))
}

fn validate_import_temp_path(
    raw_path: &str,
    user_id: UserId,
    expected_session_id: Option<&str>,
) -> Result<PathBuf, ImportV2RouteResponse> {
    let raw_path = raw_path.trim();
    if raw_path.is_empty() || FsPath::new(raw_path).is_absolute() {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    if FsPath::new(raw_path).components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    let root = temp_import_root();
    fs::create_dir_all(&root).map_err(|error| {
        import_v2_error_response(
            500,
            &format!("Unable to prepare temp import directory: {error}"),
        )
    })?;
    let root = root.canonicalize().map_err(|error| {
        import_v2_error_response(500, &format!("Unable to resolve temp import root: {error}"))
    })?;
    let mut base = root.join(import_temp_user_component(user_id));
    if let Some(session_id) = expected_session_id {
        base = base.join(sanitize_path_component(session_id));
    }
    let base = base
        .canonicalize()
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    let path = root
        .join(raw_path)
        .canonicalize()
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    if !path.starts_with(&base) {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    Ok(path)
}

fn read_import_temp_text(path: &FsPath) -> Result<String, ImportV2RouteResponse> {
    let bytes = fs::read(path).map_err(|error| {
        import_v2_error_response(500, &format!("Unable to read temp import file: {error}"))
    })?;
    Ok(decode_import_text(&bytes))
}

fn temp_import_root() -> PathBuf {
    std::env::temp_dir().join("bill-analyser-rust-import")
}

fn import_temp_user_component(user_id: UserId) -> String {
    format!("user-{}", user_id.get())
}

fn sanitize_path_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let sanitized = sanitized.trim_matches('.').trim_matches('_');
    if sanitized.is_empty() {
        "import-file".to_string()
    } else {
        sanitized.to_string()
    }
}

