/// 提取交易图片扩展名并统一成小写，作为类型 allowlist 的输入。
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

/// 判断上传文件名是否属于允许的交易图片格式。
pub fn is_allowed_transaction_picture_filename(filename: &str) -> bool {
    transaction_picture_extension(filename).is_some()
}

/// 返回图片类型不支持时对前端展示稳定的错误文案。
pub fn unsupported_transaction_picture_type_message() -> String {
    format!(
        "Picture type not allowed. Supported: {}",
        ALLOWED_TRANSACTION_PICTURE_EXTENSIONS.join(", ")
    )
}

/// 为交易图片生成带时间戳和随机后缀的上传 id，避免原始文件名碰撞。
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

/// 清洗图片文件名中可能影响路径或展示安全的字符。
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

/// 根据上传根目录和图片 id 构造待删除文件路径。
pub fn transaction_picture_delete_path(upload_root: &Path, picture_id: &str) -> PathBuf {
    upload_root.join(secure_picture_file_name(picture_id))
}

/// 根据交易图片扩展名返回前端 data URL 使用的 MIME 类型。
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

/// 把后端 base64 图片内容包装成浏览器可直接显示的 data URL。
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

/// 生成图片上传成功 payload，保留图片 id 和 data URL 字段。
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

/// 生成图片上传成功路由响应。
pub fn transaction_picture_upload_success_response(
    picture_id: impl Into<String>,
    original_url: impl Into<String>,
) -> RouteResponseContract {
    RouteResponseContract {
        status_code: 200,
        body: transaction_picture_upload_success_payload(picture_id, original_url),
    }
}

/// 生成缺少图片文件时的路由错误响应。
pub fn missing_transaction_picture_file_response() -> RouteResponseContract {
    simple_route_error_response(400, "Missing picture file")
}

/// 生成上传图片文件名或类型非法时的路由错误响应。
pub fn invalid_transaction_picture_file_response() -> RouteResponseContract {
    simple_route_error_response(400, "Invalid picture file")
}

/// 生成图片类型不在 allowlist 内时的路由错误响应。
pub fn unsupported_transaction_picture_type_response() -> RouteResponseContract {
    simple_route_error_response(400, unsupported_transaction_picture_type_message())
}

/// 生成删除未使用图片时缺少 pictureId 的路由错误响应。
pub fn missing_unused_transaction_picture_id_response() -> RouteResponseContract {
    simple_route_error_response(400, "Missing picture id")
}

/// 生成删除未使用图片成功后的兼容 payload。
pub fn remove_unused_transaction_picture_success_payload() -> Value {
    let mut payload = Map::new();
    payload.insert("success".to_string(), Value::Bool(true));
    payload.insert("result".to_string(), Value::Bool(true));
    Value::Object(payload)
}

/// 生成删除未使用图片成功后的路由响应。
pub fn remove_unused_transaction_picture_success_response() -> RouteResponseContract {
    RouteResponseContract {
        status_code: 200,
        body: remove_unused_transaction_picture_success_payload(),
    }
}

/// 生成图片上传/删除内部错误的统一路由响应。
pub fn transaction_picture_internal_error_response(
    error: impl Into<String>,
) -> RouteResponseContract {
    simple_route_error_response(500, error)
}
