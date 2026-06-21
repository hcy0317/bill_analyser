pub fn transaction_picture_extension(filename: &str) -> Option<String> {
    let (_, extension) = filename.rsplit_once('.')?;
    if extension.is_empty() {
        return None;
    }
    let extension = extension.to_ascii_lowercase();
    ALLOWED_TRANSACTION_PICTURE_EXTENSIONS
        .contains(&extension.as_str())
        .then_some(extension)
}

pub fn is_allowed_transaction_picture_filename(filename: &str) -> bool {
    transaction_picture_extension(filename).is_some()
}

pub fn unsupported_transaction_picture_type_message() -> String {
    format!(
        "Picture type not allowed. Supported: {}",
        ALLOWED_TRANSACTION_PICTURE_EXTENSIONS.join(", ")
    )
}

pub fn transaction_picture_upload_id(
    uuid_hex: &str,
    original_filename: &str,
) -> Result<String, RuntimeError> {
    if uuid_hex.len() != 32 || !uuid_hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            "invalid picture uuid",
        ));
    }
    if !is_allowed_transaction_picture_filename(original_filename) {
        return Err(RuntimeError::new(
            ErrorCode::InvalidInput,
            unsupported_transaction_picture_type_message(),
        ));
    }
    let secured_filename = secure_picture_file_name(original_filename);
    let suffix = path_suffix_lower(&secured_filename).unwrap_or_default();
    Ok(format!("{}{suffix}", uuid_hex.to_ascii_lowercase()))
}

pub fn secure_picture_file_name(raw_value: &str) -> String {
    let ascii_filename: String = raw_value.nfkd().filter(char::is_ascii).collect();
    let normalized_separators = ascii_filename
        .chars()
        .map(|character| {
            if matches!(character, '/' | '\\') {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let joined_parts = normalized_separators
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");
    let mut secured = joined_parts
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
        .collect::<String>()
        .trim_matches(|character| matches!(character, '.' | '_'))
        .to_string();

    if !secured.is_empty() && windows_device_file_name(&secured) {
        secured.insert(0, '_');
    }
    secured
}

pub fn transaction_picture_delete_path(upload_root: &Path, picture_id: &str) -> PathBuf {
    upload_root.join(secure_picture_file_name(picture_id))
}

pub fn transaction_picture_mime_type(filename: &str) -> &'static str {
    match filename
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    }
}

pub fn transaction_picture_data_url_from_base64(
    filename: &str,
    encoded_base64: impl AsRef<str>,
) -> String {
    format!(
        "data:{};base64,{}",
        transaction_picture_mime_type(filename),
        encoded_base64.as_ref()
    )
}

pub fn transaction_picture_upload_success_payload(
    picture_id: impl Into<String>,
    original_url: impl Into<String>,
) -> Value {
    success_result_body(
        serde_json::to_value(TransactionPictureUploadResult {
            picture_id: picture_id.into(),
            original_url: original_url.into(),
        })
        .expect("transaction picture upload result should serialize"),
    )
}

pub fn transaction_picture_upload_success_response(
    picture_id: impl Into<String>,
    original_url: impl Into<String>,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 200,
        body: transaction_picture_upload_success_payload(picture_id, original_url),
    }
}

pub fn missing_transaction_picture_file_response() -> RouteResponseContract {
    simple_route_error_response(400, "Missing picture file")
}

pub fn invalid_transaction_picture_file_response() -> RouteResponseContract {
    simple_route_error_response(400, "Invalid picture file")
}

pub fn unsupported_transaction_picture_type_response() -> RouteResponseContract {
    simple_route_error_response(400, unsupported_transaction_picture_type_message())
}

pub fn missing_unused_transaction_picture_id_response() -> RouteResponseContract {
    simple_route_error_response(400, "Missing picture id")
}

pub fn remove_unused_transaction_picture_success_payload() -> Value {
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Bool(true));
    Value::Object(payload)
}

pub fn remove_unused_transaction_picture_success_response() -> RouteResponseContract {
    RouteResponseContract {
        status_code: 200,
        body: remove_unused_transaction_picture_success_payload(),
    }
}

pub fn transaction_picture_internal_error_response(
    error: impl Into<String>,
) -> RouteResponseContract {
    simple_route_error_response(500, error)
}
