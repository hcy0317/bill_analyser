use unicode_normalization::UnicodeNormalization;

use super::types::{
    BackupFilenameResolution, OpsContractError, BACKUP_ENCRYPTED_SUFFIX, BACKUP_PREFIX,
    BACKUP_ZIP_SUFFIX,
};

#[tracing::instrument(level = "debug", skip_all)]
/// 将用户传入的备份文件名折叠为安全叶子文件名，只保留备份下载/删除链路允许落盘的字符集合。
pub fn secure_backup_filename(filename: &str) -> String {
    let normalized = filename.nfkd().collect::<String>();
    let mut cleaned = String::new();
    for ch in normalized.chars() {
        if matches!(ch, '/' | '\\') || ch.is_whitespace() {
            cleaned.push(' ');
        } else if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
            cleaned.push(ch);
        }
    }

    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .trim_matches(|ch| matches!(ch, '.' | '_'))
        .to_string()
}

#[tracing::instrument(level = "debug", skip_all)]
/// 校验备份文件名是否为安全的本地叶子名，并返回加密后缀等路由层需要复用的解析结果。
pub fn resolve_backup_filename(
    filename: &str,
) -> Result<BackupFilenameResolution, OpsContractError> {
    let raw_filename = filename.trim();
    let bytes = raw_filename.as_bytes();
    let has_windows_drive = bytes.len() >= 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic();
    if raw_filename.is_empty()
        || raw_filename.starts_with('/')
        || has_windows_drive
        || raw_filename.contains('/')
        || raw_filename.contains('\\')
        || raw_filename.contains(':')
        || raw_filename.chars().any(char::is_control)
    {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }

    let sanitized = secure_backup_filename(raw_filename);
    if sanitized != raw_filename {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }

    if !is_allowed_backup_filename(&sanitized) {
        return Err(OpsContractError::new(
            "Invalid filename",
            "无效的文件名",
            400,
        ));
    }

    Ok(BackupFilenameResolution {
        original: filename.to_string(),
        encrypted: sanitized.ends_with(BACKUP_ENCRYPTED_SUFFIX),
        sanitized,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
/// 判断文件名是否仍属于 Bill Analyser 备份命名空间，避免任意文件被下载或删除。
pub fn is_allowed_backup_filename(filename: &str) -> bool {
    filename.starts_with(BACKUP_PREFIX)
        && (filename.ends_with(BACKUP_ZIP_SUFFIX) || filename.ends_with(BACKUP_ENCRYPTED_SUFFIX))
}
