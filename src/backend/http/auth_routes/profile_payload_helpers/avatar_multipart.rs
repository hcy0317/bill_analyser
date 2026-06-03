// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn avatar_data_url_from_multipart(headers: &HeaderMap, body: &[u8]) -> RouteResult<String> {
    let content_type = header_value(headers, header::CONTENT_TYPE.as_str());
    let boundary = multipart_boundary(&content_type).ok_or_else(|| {
        Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar file is required",
        )))
    })?;
    let parts = multipart_parts(body, boundary.as_bytes());
    for part in parts {
        let Some((raw_headers, raw_body)) = split_multipart_part(part) else {
            continue;
        };
        let header_text = String::from_utf8_lossy(raw_headers);
        if !header_text.contains("name=\"avatar\"") {
            continue;
        }
        let payload = trim_trailing_newline(raw_body);
        if payload.is_empty() {
            return Err(Box::new(auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Avatar file is empty",
            ))));
        }
        return avatar_data_url_from_payload(payload, multipart_part_content_type(&header_text));
    }
    Err(Box::new(auth_rest_error_response(AuthRestError::new(
        400,
        "Bad Request",
        "Avatar file is required",
    ))))
}

fn avatar_data_url_from_payload(
    payload: &[u8],
    declared_mime_type: Option<String>,
) -> RouteResult<String> {
    if payload.len() > MAX_AVATAR_BYTES {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Avatar file is too large",
        ))));
    }
    let Some(detected_mime_type) = detect_avatar_mime_type(payload) else {
        return Err(Box::new(auth_rest_error_response(AuthRestError::new(
            400,
            "Bad Request",
            "Unsupported avatar file type",
        ))));
    };
    if let Some(declared_mime_type) = declared_mime_type {
        if !declared_mime_type.eq_ignore_ascii_case(detected_mime_type) {
            return Err(Box::new(auth_rest_error_response(AuthRestError::new(
                400,
                "Bad Request",
                "Avatar MIME type does not match file content",
            ))));
        }
    }
    Ok(format!(
        "data:{detected_mime_type};base64,{}",
        general_purpose::STANDARD.encode(payload)
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
fn detect_avatar_mime_type(payload: &[u8]) -> Option<&'static str> {
    if payload.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if payload.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if payload.starts_with(b"GIF87a") || payload.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if payload.len() >= 12 && payload.starts_with(b"RIFF") && &payload[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

fn multipart_boundary(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|segment| {
        let segment = segment.trim();
        let value = segment.strip_prefix("boundary=")?;
        Some(value.trim_matches('"').to_string())
    })
}

fn multipart_parts<'a>(body: &'a [u8], boundary: &[u8]) -> Vec<&'a [u8]> {
    let delimiter = [b"--".as_slice(), boundary].concat();
    let mut parts = Vec::new();
    let mut search_start = 0;
    while let Some(boundary_start) = find_bytes(&body[search_start..], &delimiter) {
        let part_start = search_start + boundary_start + delimiter.len();
        if body.get(part_start..part_start + 2) == Some(b"--") {
            break;
        }
        let part_start = if body.get(part_start..part_start + 2) == Some(b"\r\n") {
            part_start + 2
        } else if body.get(part_start..part_start + 1) == Some(b"\n") {
            part_start + 1
        } else {
            part_start
        };
        let Some(next_boundary) = find_bytes(&body[part_start..], &delimiter) else {
            break;
        };
        let part_end = part_start + next_boundary;
        parts.push(&body[part_start..part_end]);
        search_start = part_end;
    }
    parts
}

fn split_multipart_part(part: &[u8]) -> Option<(&[u8], &[u8])> {
    if let Some(index) = find_bytes(part, b"\r\n\r\n") {
        return Some((&part[..index], &part[index + 4..]));
    }
    find_bytes(part, b"\n\n").map(|index| (&part[..index], &part[index + 2..]))
}

fn trim_trailing_newline(mut value: &[u8]) -> &[u8] {
    if value.ends_with(b"\r\n") {
        value = &value[..value.len().saturating_sub(2)];
    } else if value.ends_with(b"\n") {
        value = &value[..value.len().saturating_sub(1)];
    }
    value
}

fn multipart_part_content_type(header_text: &str) -> Option<String> {
    header_text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case("content-type") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        None
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
