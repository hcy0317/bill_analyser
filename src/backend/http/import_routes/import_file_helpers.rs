// 中文导读：import v2 文件预览、CSV 行解析与临时文件边界 helper。
// 维护重点：只保存当前 session 临时上传文件；不承担历史响应转换。
// 不变式：临时文件路径必须 user/session scoped，禁止绝对路径和目录逃逸。

const GENERIC_TEXT_IMPORT_MAX_TOTAL_ROWS: usize = 50_000;
const GENERIC_TEXT_IMPORT_MAX_COLUMNS: usize = 128;
const GENERIC_TEXT_IMPORT_MAX_CELL_BYTES: usize = 16 * 1024;
const GENERIC_TEXT_IMPORT_MAX_TOTAL_CELLS: usize = 1_000_000;

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
    let csv_text = table_text_from_line(text, start_line);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter as u8)
        .from_reader(csv_text.as_bytes());
    let mut rows = Vec::new();
    let mut total_cells = 0usize;
    for record in reader.records() {
        let record = record.map_err(|error| {
            import_v2_error_response(400, &format!("Invalid CSV file: {error}"))
        })?;
        if rows.len() >= GENERIC_TEXT_IMPORT_MAX_TOTAL_ROWS {
            return Err(import_v2_error_response(
                413,
                "Generic import table exceeds the row limit",
            ));
        }
        if record.len() > GENERIC_TEXT_IMPORT_MAX_COLUMNS {
            return Err(import_v2_error_response(
                413,
                "Generic import table exceeds the column limit",
            ));
        }
        total_cells = total_cells.checked_add(record.len()).ok_or_else(|| {
            import_v2_error_response(413, "Generic import table exceeds the cell limit")
        })?;
        if total_cells > GENERIC_TEXT_IMPORT_MAX_TOTAL_CELLS {
            return Err(import_v2_error_response(
                413,
                "Generic import table exceeds the cell limit",
            ));
        }
        if record
            .iter()
            .any(|value| value.len() > GENERIC_TEXT_IMPORT_MAX_CELL_BYTES)
        {
            return Err(import_v2_error_response(
                413,
                "Generic import table contains an oversized cell",
            ));
        }
        rows.push(
            record
                .iter()
                .map(|value| value.trim().to_string())
                .collect(),
        );
    }
    Ok(rows)
}

fn table_text_from_line(text: &str, start_line: usize) -> &str {
    if start_line == 0 {
        return text;
    }
    let mut remaining = start_line;
    for (index, byte) in text.bytes().enumerate() {
        if byte == b'\n' {
            remaining -= 1;
            if remaining == 0 {
                return &text[index + 1..];
            }
        }
    }
    ""
}

fn split_preview_line(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|value| value.trim().trim_matches('"').to_string())
        .collect()
}

#[tracing::instrument(level = "debug", skip_all)]
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

fn save_unmatched_import_file(
    user_id: UserId,
    session_id: &str,
    original_name: &str,
    body: &[u8],
) -> Result<String, ImportV2RouteResponse> {
    let user_component = import_temp_user_component(user_id);
    let session_component = import_temp_session_component(session_id)?;
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

#[tracing::instrument(level = "debug", skip_all)]
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
        base = base.join(import_temp_session_component(session_id)?);
    }
    let path = root.join(raw_path);
    if expected_session_id.is_some() && !path.starts_with(&base) {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    let base = base
        .canonicalize()
        .map_err(|_| import_v2_error_response(404, "Temp import file not found"))?;
    if !base.starts_with(&root) {
        return Err(import_v2_error_response(
            400,
            "Invalid temp import file path",
        ));
    }
    let path = path
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

fn temp_import_root() -> PathBuf {
    std::env::temp_dir().join("bill-analyser-rust-import")
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_temp_user_component(user_id: UserId) -> String {
    format!("user-{}", user_id.get())
}

fn import_temp_session_component(session_id: &str) -> Result<String, ImportV2RouteResponse> {
    let session_id = session_id.trim();
    if session_id.is_empty()
        || session_id.len() > 128
        || !session_id
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
    {
        return Err(import_v2_error_response(
            400,
            "Invalid import session id for temp file",
        ));
    }
    Ok(session_id.to_string())
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
