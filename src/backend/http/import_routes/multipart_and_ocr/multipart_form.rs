use super::*;

/// 解析 multipart/form-data body，提取文本字段和文件 part，保留原始文件字节。
pub(super) fn parse_multipart_form_data(
    content_type: &str,
    body: &[u8],
) -> Result<MultipartForm, ImportV2RouteResponse> {
    let boundary = multipart_boundary_from_content_type(content_type)
        .ok_or_else(|| import_v2_error_response(400, "Missing multipart boundary"))?;
    let marker = format!("--{boundary}");
    let mut form = MultipartForm::default();
    for raw_part in split_bytes(body, marker.as_bytes()) {
        let raw_part = trim_part_boundary(raw_part);
        if raw_part.is_empty() || raw_part == b"--" {
            continue;
        }
        let Some((headers, part_body)) =
            split_once_bytes(raw_part, b"\r\n\r\n").or_else(|| split_once_bytes(raw_part, b"\n\n"))
        else {
            continue;
        };
        let headers_text = String::from_utf8_lossy(headers);
        let Some(disposition) = headers_text.lines().find(|line| {
            line.to_ascii_lowercase()
                .starts_with("content-disposition:")
        }) else {
            continue;
        };
        let Some(name) = disposition_param(disposition, "name") else {
            continue;
        };
        let filename = disposition_param(disposition, "filename")
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let content_type = headers_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.trim().eq_ignore_ascii_case("content-type") {
                    Some(value.trim().to_string())
                } else {
                    None
                }
            })
            .filter(|value| !value.is_empty());
        form.parts.push(MultipartPart {
            name,
            filename,
            content_type,
            body: strip_trailing_newline(part_body).to_vec(),
        });
    }
    Ok(form)
}

/// 从 Content-Type 中读取 boundary，支持带引号的浏览器标准格式。
fn multipart_boundary_from_content_type(content_type: &str) -> Option<String> {
    content_type.split(';').find_map(|part| {
        let part = part.trim();
        let value = part.strip_prefix("boundary=")?;
        Some(value.trim_matches('"').to_string())
    })
}

fn disposition_param(disposition: &str, param: &str) -> Option<String> {
    let prefix = format!("{param}=");
    disposition.split(';').find_map(|part| {
        let part = part.trim();
        let raw_value = part.strip_prefix(&prefix)?;
        Some(raw_value.trim_matches('"').to_string())
    })
}

/// 按字节 marker 拆分 body，避免把上传图片内容当作 UTF-8 文本处理。
fn split_bytes<'a>(body: &'a [u8], marker: &[u8]) -> Vec<&'a [u8]> {
    if marker.is_empty() {
        return vec![body];
    }
    let mut parts = Vec::new();
    let mut start = 0;
    while let Some(offset) = find_bytes(&body[start..], marker) {
        parts.push(&body[start..start + offset]);
        start += offset + marker.len();
    }
    parts.push(&body[start..]);
    parts
}

/// 按字节 marker 拆分一次，用于 multipart header/body 分界。
fn split_once_bytes<'a>(body: &'a [u8], marker: &[u8]) -> Option<(&'a [u8], &'a [u8])> {
    let offset = find_bytes(body, marker)?;
    Some((&body[..offset], &body[offset + marker.len()..]))
}

/// 在二进制 body 中查找 marker，确保 multipart 解析不依赖字符串转换。
#[tracing::instrument(level = "debug", skip_all)]
fn find_bytes(body: &[u8], marker: &[u8]) -> Option<usize> {
    if marker.is_empty() || marker.len() > body.len() {
        return None;
    }
    body.windows(marker.len())
        .position(|window| window == marker)
}

/// 去除 part 前导换行，避免边界分隔符后的 CRLF 干扰 header 解析。
fn trim_part_boundary(mut part: &[u8]) -> &[u8] {
    while part.starts_with(b"\r\n") {
        part = &part[2..];
    }
    while part.starts_with(b"\n") {
        part = &part[1..];
    }
    part
}

/// 去除 part body 末尾换行，只清理 multipart 分隔带来的包装字符。
fn strip_trailing_newline(mut body: &[u8]) -> &[u8] {
    if body.ends_with(b"\r\n") {
        body = &body[..body.len().saturating_sub(2)];
    } else if body.ends_with(b"\n") {
        body = &body[..body.len().saturating_sub(1)];
    }
    body
}

/// 解码导入文本字段，优先 UTF-8，失败时回退 GBK 并去除 BOM。
pub(super) fn decode_import_text(bytes: &[u8]) -> String {
    let decoded = if let Ok(text) = std::str::from_utf8(bytes) {
        text.to_string()
    } else {
        let (text, _, _) = GBK.decode(bytes);
        text.into_owned()
    };
    decoded.trim_start_matches('\u{feff}').to_string()
}
