use std::collections::BTreeSet;

use super::types::{
    BackupArchiveSummary, BackupFileInfoContract, BackupFileInfoInput, BACKUP_ENCRYPTED_SUFFIX,
};

#[tracing::instrument(level = "debug", skip_all)]
/// 校验 zip 成员名是否只指向备份包内部相对路径，阻断绝对路径、盘符、ADS、控制字符和 `..` 穿越。
pub fn is_safe_backup_archive_member(member_name: &str) -> bool {
    let normalized_raw = member_name.replace('\\', "/");
    if normalized_raw.chars().any(char::is_control) {
        return false;
    }

    let normalized = normalized_raw.trim().to_string();
    if normalized.is_empty() || normalized.starts_with('/') {
        return false;
    }
    if normalized.contains(':') {
        return false;
    }

    !normalized.split('/').any(|part| part == "..")
}

#[tracing::instrument(level = "debug", skip_all)]
/// 从 zip 成员列表生成备份包摘要，并在发现不安全成员时输出与 HTTP 响应一致的失败摘要。
pub fn backup_archive_summary_from_entries<I, S>(entries: I) -> BackupArchiveSummary
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let raw_names = entries
        .into_iter()
        .map(|item| item.as_ref().to_string())
        .collect::<Vec<_>>();
    if let Some(unsafe_name) = raw_names
        .iter()
        .find(|name| !is_safe_backup_archive_member(name))
    {
        return invalid_backup_archive_summary(&format!("备份文件包含不安全路径: {unsafe_name}"));
    }

    let names = raw_names
        .into_iter()
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    let contains_data_dir = names.iter().any(|name| name.starts_with("data/"));
    let top_level_entries = names
        .iter()
        .filter_map(|name| name.split('/').next())
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    BackupArchiveSummary {
        valid_zip: true,
        contains_data_dir,
        entry_count: names.len(),
        top_level_entries,
        error: String::new(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 构造统一的无效备份包摘要，供 zip 读取失败和安全校验失败复用。
pub fn invalid_backup_archive_summary(error: &str) -> BackupArchiveSummary {
    BackupArchiveSummary {
        valid_zip: false,
        contains_data_dir: false,
        entry_count: 0,
        top_level_entries: Vec::new(),
        error: error.to_string(),
    }
}

#[tracing::instrument(level = "debug", skip_all)]
/// 将本地文件系统信息、checksum metadata 和 archive 摘要投影为前端可见的备份文件合同。
pub fn build_backup_file_info(input: BackupFileInfoInput) -> BackupFileInfoContract {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "backup_user_data",
        operation = "build_backup_file_info",
        "business operation entered"
    );
    let encrypted = input.filename.ends_with(BACKUP_ENCRYPTED_SUFFIX);
    let metadata_checksum_matched = input
        .metadata_checksum
        .as_deref()
        .is_some_and(|checksum| !checksum.is_empty() && checksum == input.checksum);

    BackupFileInfoContract {
        filename: input.filename,
        size: input.size,
        created_at: input.created_at,
        path: input.path,
        checksum: input.checksum,
        encrypted,
        valid_zip: input.archive_summary.valid_zip,
        contains_data_dir: input.archive_summary.contains_data_dir,
        entry_count: input.archive_summary.entry_count,
        top_level_entries: input.archive_summary.top_level_entries,
        error: input.archive_summary.error,
        metadata_checksum_matched,
    }
}
