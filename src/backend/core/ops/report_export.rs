use super::types::{OpsContractError, ReportExportContract};

#[tracing::instrument(level = "debug", skip_all)]
/// 规范化报表导出格式，固定扩展名和 MIME，拒绝未支持格式进入下载响应。
pub fn normalize_report_export_format(
    format_type: &str,
) -> Result<ReportExportContract, OpsContractError> {
    let normalized = format_type.trim().to_lowercase();
    match normalized.as_str() {
        "pdf" => Ok(ReportExportContract {
            format_type: normalized,
            extension: "pdf".to_string(),
            mimetype: "application/pdf".to_string(),
        }),
        "excel" | "xlsx" => Ok(ReportExportContract {
            format_type: "excel".to_string(),
            extension: "xlsx".to_string(),
            mimetype: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .to_string(),
        }),
        "html" => Ok(ReportExportContract {
            format_type: normalized,
            extension: "html".to_string(),
            mimetype: "text/html".to_string(),
        }),
        _ => Err(OpsContractError::new(
            "Unsupported report format",
            format!("不支持的格式: {format_type}"),
            400,
        )),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将报表下载文件名收敛为安全叶子名，兼容 Windows 保留名和跨平台非法字符。
pub fn secure_report_filename(filename: &str) -> String {
    let fallback = "report";
    let trimmed = filename.trim();
    if trimmed.is_empty() || matches!(trimmed, "." | "..") {
        return fallback.to_string();
    }

    let leaf = trimmed
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .map(str::trim)
        .unwrap_or(fallback);
    if leaf.is_empty() || matches!(leaf, "." | "..") {
        return fallback.to_string();
    }

    let mut cleaned = String::new();
    let mut previous_underscore = false;
    for ch in leaf.chars() {
        let replacement = if matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            || ch.is_control()
        {
            '_'
        } else {
            ch
        };
        if replacement == '_' {
            if !previous_underscore {
                cleaned.push('_');
            }
            previous_underscore = true;
        } else {
            cleaned.push(replacement);
            previous_underscore = false;
        }
    }

    let cleaned = cleaned.trim_matches(|ch| matches!(ch, ' ' | '.' | '_'));
    let reserved_name = cleaned
        .split_once('.')
        .map(|(prefix, _)| prefix)
        .unwrap_or(cleaned)
        .trim_end_matches([' ', '.'])
        .to_lowercase();

    if cleaned.is_empty()
        || matches!(cleaned, "." | "..")
        || is_windows_reserved_report_name(&reserved_name)
    {
        fallback.to_string()
    } else {
        cleaned.to_string()
    }
}

/// 识别 Windows 设备保留名，避免导出的报表文件名在 Windows 客户端落盘失败。
fn is_windows_reserved_report_name(name: &str) -> bool {
    matches!(name, "con" | "prn" | "aux" | "nul")
        || (name.len() == 4
            && (name.starts_with("com") || name.starts_with("lpt"))
            && name.as_bytes()[3].is_ascii_digit()
            && name.as_bytes()[3] != b'0')
}
