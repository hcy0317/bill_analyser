use std::collections::BTreeSet;

use super::types::{
    BackupCleanupDecision, BackupCleanupPlan, BackupFileCandidate, BackupRecordContract,
};

#[tracing::instrument(level = "debug", skip_all)]
/// 按“数据库记录优先”的保留策略生成清理计划，区分缺失记录、超出保留数和 stray 文件。
pub fn plan_backup_cleanup(
    records: &[BackupRecordContract],
    stray_files: &[BackupFileCandidate],
    keep_count: usize,
) -> BackupCleanupPlan {
    let mut decisions = Vec::new();
    let mut active_records = records
        .iter()
        .filter(|record| record.storage_type.trim().is_empty() || record.storage_type == "local")
        .filter(|record| record.status != "deleted")
        .collect::<Vec<_>>();

    for record in active_records
        .iter()
        .filter(|record| !record.file_exists)
        .copied()
    {
        decisions.push(BackupCleanupDecision {
            filename: record.backup_name.clone(),
            action: "mark_deleted".to_string(),
            reason: "missing_file".to_string(),
        });
    }

    active_records.retain(|record| record.file_exists);
    active_records.sort_by(|left, right| {
        right
            .created_at
            .cmp(&left.created_at)
            .then_with(|| right.id.cmp(&left.id))
    });

    let kept_record_names = active_records
        .iter()
        .take(keep_count)
        .map(|record| record.backup_name.clone())
        .collect::<BTreeSet<_>>();
    let active_record_names = active_records
        .iter()
        .map(|record| record.backup_name.clone())
        .collect::<BTreeSet<_>>();

    let mut deleted_count = 0_usize;
    for record in active_records.iter().skip(keep_count) {
        deleted_count += 1;
        decisions.push(BackupCleanupDecision {
            filename: record.backup_name.clone(),
            action: "delete_file_and_mark_deleted".to_string(),
            reason: "retention_cleanup".to_string(),
        });
    }

    let mut stray = stray_files
        .iter()
        .filter(|candidate| !kept_record_names.contains(&candidate.filename))
        .filter(|candidate| !active_record_names.contains(&candidate.filename))
        .collect::<Vec<_>>();
    stray.sort_by_key(|candidate| std::cmp::Reverse(candidate.modified_at));

    let remaining_slots = keep_count.saturating_sub(kept_record_names.len());
    for candidate in stray.iter().skip(remaining_slots) {
        deleted_count += 1;
        decisions.push(BackupCleanupDecision {
            filename: candidate.filename.clone(),
            action: "delete_stray_file".to_string(),
            reason: "retention_cleanup".to_string(),
        });
    }

    let kept_count = kept_record_names.len() + stray.len().min(remaining_slots);
    BackupCleanupPlan {
        deleted_count,
        kept_count: kept_count.min(keep_count),
        decisions,
    }
}
