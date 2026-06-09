// 中文导读：导入 Postgres staging 的结构性合同测试，锁定 session/source/preview/group 生命周期。
// 维护重点：这些测试读取迁移清单和权威 schema，避免 import stage2 拆分时破坏 staging 表关系。
// 不变式：staging 子表必须由 import_sessions 级联清理，preview 与决策/历史 materialization 保持可重建。

use std::fs;

use bill_analyser_db::{postgres_initial_schema_path, postgres_migration_manifest};

fn initial_schema() -> String {
    fs::read_to_string(postgres_initial_schema_path()).expect("initial PostgreSQL schema")
}

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing section start {start}"));
    let rest = &source[start_index..];
    let end_index = rest
        .find(end)
        .unwrap_or_else(|| panic!("missing section end {end}"));
    &rest[..end_index]
}

fn ordered_position(source: &str, needles: &[&str]) -> Vec<usize> {
    needles
        .iter()
        .map(|needle| {
            source
                .find(needle)
                .unwrap_or_else(|| panic!("missing marker {needle}"))
        })
        .collect()
}

#[test]
fn import_staging_tables_are_registered_in_lifecycle_order() {
    let initial = postgres_migration_manifest()
        .iter()
        .find(|descriptor| descriptor.version == 1)
        .expect("initial schema descriptor");
    let import_tables = [
        "import_sessions",
        "import_sources",
        "import_standard_rows",
        "import_preview_rows",
        "import_decision_groups",
        "import_decision_group_members",
        "import_history_materializations",
        "import_confirm_operations",
    ];

    let positions = import_tables
        .iter()
        .map(|table| {
            initial
                .required_tables
                .iter()
                .position(|registered| registered == table)
                .unwrap_or_else(|| panic!("migration manifest missing {table}"))
        })
        .collect::<Vec<_>>();
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    for index in [
        "idx_import_preview_rows_session_page_sort_key",
        "idx_import_decision_groups_session_group_type",
        "idx_import_decision_group_members_group",
        "idx_import_history_materializations_session_history_bill",
    ] {
        assert!(
            initial.required_indexes.contains(&index),
            "migration manifest missing {index}"
        );
    }
}

#[test]
fn import_staging_schema_keeps_rebuildable_preview_materialization_edges() {
    let schema = initial_schema();
    let positions = ordered_position(
        &schema,
        &[
            "CREATE TABLE IF NOT EXISTS import_sessions",
            "CREATE TABLE IF NOT EXISTS import_sources",
            "CREATE TABLE IF NOT EXISTS import_standard_rows",
            "CREATE TABLE IF NOT EXISTS import_preview_rows",
            "CREATE TABLE IF NOT EXISTS import_decision_groups",
            "CREATE TABLE IF NOT EXISTS import_decision_group_members",
            "CREATE TABLE IF NOT EXISTS import_history_materializations",
            "CREATE TABLE IF NOT EXISTS import_confirm_operations",
        ],
    );
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));

    let sources = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_sources",
        "CREATE TABLE IF NOT EXISTS import_standard_rows",
    );
    assert!(sources
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(sources.contains("UNIQUE (session_id, feature_signature)"));

    let standard_rows = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_standard_rows",
        "COMMENT ON COLUMN import_standard_rows.amount_cents",
    );
    assert!(standard_rows
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(standard_rows
        .contains("source_id BIGINT NOT NULL REFERENCES import_sources(id) ON DELETE CASCADE"));
    assert!(standard_rows.contains("UNIQUE (source_id, source_row_index)"));

    let preview = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_preview_rows",
        "COMMENT ON COLUMN import_preview_rows.amount_cents",
    );
    assert!(preview
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(preview.contains(
        "base_standard_row_id BIGINT REFERENCES import_standard_rows(id) ON DELETE SET NULL"
    ));
    assert!(preview.contains("account_id BIGINT REFERENCES accounts(id) ON DELETE SET NULL"));
    assert!(preview
        .contains("transfer_target_account_id BIGINT REFERENCES accounts(id) ON DELETE SET NULL"));

    let decision_members = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_decision_group_members",
        "CREATE TABLE IF NOT EXISTS import_history_materializations",
    );
    assert!(decision_members.contains(
        "group_id BIGINT NOT NULL REFERENCES import_decision_groups(id) ON DELETE CASCADE"
    ));
    assert!(decision_members
        .contains("preview_row_id BIGINT REFERENCES import_preview_rows(id) ON DELETE CASCADE"));
    assert!(decision_members
        .contains("standard_row_id BIGINT REFERENCES import_standard_rows(id) ON DELETE SET NULL"));

    let history = section_between(
        &schema,
        "CREATE TABLE IF NOT EXISTS import_history_materializations",
        "CREATE TABLE IF NOT EXISTS import_confirm_operations",
    );
    assert!(history
        .contains("session_id BIGINT NOT NULL REFERENCES import_sessions(id) ON DELETE CASCADE"));
    assert!(history.contains("UNIQUE (session_id, history_bill_id)"));
}
