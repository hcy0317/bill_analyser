use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
};

use rusqlite::{params, types::ValueRef, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Number, Value};
use sha2::{Digest, Sha256};
use sqlx::{types::Json, Postgres, Row};

use crate::{
    postgres_migration::import_postgres_bundle_in_transaction, DbError, DbResult, PostgresPool,
    PostgresTargetRow, SqliteToPostgresExportBundle, SqliteToPostgresImportCheckReport,
    SqliteToPostgresTableExport,
};

pub const ACCOUNT_RECOVERY_DEFAULT_SOURCE_USER_ID: i64 = 5;
pub const ACCOUNT_RECOVERY_ID_BLOCK_SIZE: i64 = 1_000_000_000;

const ACCOUNT_RECOVERY_SCHEMA_VERSION: u16 = 1;
const ACCOUNT_RECOVERY_MIGRATION_NAME: &str = "postgres_account_recovery_v1";
const ACCOUNT_RECOVERY_LOCK_CLASS_ID: i32 = 41_720;

const TARGET_TABLE_ALLOWLIST: &[&str] = &[
    "accounts",
    "categories",
    "tags",
    "bills",
    "bill_tags",
    "budgets",
    "budget_history",
    "category_rules",
    "account_rules",
];

const TARGET_TABLE_DENYLIST: &[&str] = &[
    "users",
    "token_sessions",
    "user_two_factor_recovery_codes",
    "user_external_auths",
    "business_audit_events",
    "backup_records",
    "backup_jobs",
    "backup_audit_logs",
    "settings",
];

const USER_DATA_DELETE_TABLES: &[&str] = &[
    "vector_outbox_events",
    "import_learning_feedback_events",
    "import_learning_features",
    "import_learning_suggestions",
    "import_learning_lifecycle",
    "import_learning_suppressions",
    "import_learning_samples",
    "matching_feedback_events",
    "matching_pairs",
    "matching_suppressions",
    "preview_matching_feedback",
    "recurring_suggestions",
    "import_confirm_operations",
    "import_history_materializations",
    "import_decision_groups",
    "import_sessions",
    "budget_history",
    "budgets",
    "bill_tags",
    "bills",
    "account_rules",
    "category_rules",
    "tags",
    "account_aliases_legacy",
    "categories",
    "accounts",
];

const SETTINGS_ALLOWLIST_KEYS: &[&str] = &[
    "default_currency",
    "currency",
    "locale",
    "timezone",
    "theme",
    "date_format",
    "number_format",
];

const SETTINGS_DENYLIST_TERMS: &[&str] = &[
    "auth",
    "2fa",
    "two_factor",
    "operation_password",
    "cloud",
    "backup",
    "llm",
    "ocr",
    "provider",
    "api_key",
    "apikey",
    "token",
    "secret",
    "password",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecoveryExpectedCounts {
    pub bills: u64,
    pub accounts: u64,
    pub categories: u64,
    pub budgets: u64,
    pub category_rules: u64,
}

impl Default for AccountRecoveryExpectedCounts {
    fn default() -> Self {
        Self {
            bills: 26,
            accounts: 19,
            categories: 93,
            budgets: 23,
            category_rules: 78,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountRecoverySourceCounts {
    pub bills: u64,
    pub accounts: u64,
    pub categories: u64,
    pub tags: u64,
    pub bill_tags: u64,
    pub budgets: u64,
    pub budget_history: u64,
    pub budget_history_orphaned: u64,
    pub category_rules: u64,
    pub accounts_with_aliases: u64,
    pub generated_account_rules: u64,
    pub settings_allowed: u64,
    pub settings_denied: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecoverySourceReport {
    pub schema_version: u16,
    pub source_user_id: i64,
    pub source_identity_hash: String,
    pub source_sqlite_sha256: String,
    pub counts: AccountRecoverySourceCounts,
    pub expected_counts: AccountRecoveryExpectedCounts,
    pub expected_counts_match: bool,
    pub table_checksums: BTreeMap<String, String>,
    pub settings_allowlist: Vec<String>,
    pub settings_denylist_terms: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecoveryTargetTriplet {
    pub user_id: i64,
    pub username_sha256: String,
    pub email_sha256: Option<String>,
    pub auth_source_evidence: String,
    pub triplet_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AccountRecoveryTargetCounts {
    pub accounts: i64,
    pub categories: i64,
    pub tags: i64,
    pub bills: i64,
    pub bill_tags: i64,
    pub budgets: i64,
    pub budget_history: i64,
    pub category_rules: i64,
    pub account_rules: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecoveryIdPlan {
    pub source_user_id: i64,
    pub target_user_id: i64,
    pub id_block_size: i64,
    pub id_base: i64,
    pub max_source_id: i64,
    pub max_remapped_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecoveryDryRunReport {
    pub schema_version: u16,
    pub manifest_id: String,
    pub source: AccountRecoverySourceReport,
    pub target: AccountRecoveryTargetTriplet,
    pub target_pre_counts: AccountRecoveryTargetCounts,
    pub id_plan: AccountRecoveryIdPlan,
    pub table_allowlist: Vec<String>,
    pub table_denylist: Vec<String>,
    pub snapshot_default_path: String,
    pub dry_run_hash: String,
    pub redaction_policy: Vec<String>,
    pub apply_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRecoveryTargetRef {
    value: String,
}

impl AccountRecoveryTargetRef {
    pub fn new(value: impl Into<String>) -> DbResult<Self> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DbError::InvalidOperation(
                "account recovery target user ref is required".to_string(),
            ));
        }
        Ok(Self { value })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountRecoveryApplyOptions {
    pub source_user_id: i64,
    pub target_user_ref: AccountRecoveryTargetRef,
    pub manifest_id: String,
    pub confirm_target_user_id: i64,
    pub confirm_target_username: String,
    pub confirm_target_email: String,
    pub snapshot_dir: PathBuf,
    pub workspace_root: PathBuf,
    pub allow_unexpected_source_shape: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRecoveryApplyReport {
    pub schema_version: u16,
    pub manifest_id: String,
    pub target: AccountRecoveryTargetTriplet,
    pub source_counts: AccountRecoverySourceCounts,
    pub target_pre_counts: AccountRecoveryTargetCounts,
    pub target_post_counts: AccountRecoveryTargetCounts,
    pub import_check: SqliteToPostgresImportCheckReport,
    pub snapshot_manifest_sha256: String,
    pub dry_run_hash: String,
    pub allow_unexpected_source_shape: bool,
}

#[derive(Debug, Clone)]
struct ResolvedTarget {
    user_id: i64,
    username: String,
    email: Option<String>,
    password_hash_present: bool,
}

pub fn inspect_postgres_account_recovery_source(
    sqlite_path: impl AsRef<Path>,
    source_user_id: i64,
) -> DbResult<AccountRecoverySourceReport> {
    let sqlite_path = sqlite_path.as_ref();
    let source_sqlite_sha256 = file_sha256(sqlite_path)?;
    let connection = Connection::open(sqlite_path)?;
    inspect_postgres_account_recovery_source_from_connection(
        &connection,
        source_sqlite_sha256,
        source_user_id,
    )
}

pub fn inspect_postgres_account_recovery_source_from_connection(
    connection: &Connection,
    source_sqlite_sha256: String,
    source_user_id: i64,
) -> DbResult<AccountRecoverySourceReport> {
    validate_positive_id(source_user_id, "source_user_id")?;
    let source_identity_hash = source_user_identity_hash(connection, source_user_id)?;
    let counts = source_counts(connection, source_user_id)?;
    let expected_counts = AccountRecoveryExpectedCounts::default();
    let expected_counts_match = counts.bills == expected_counts.bills
        && counts.accounts == expected_counts.accounts
        && counts.categories == expected_counts.categories
        && counts.budgets == expected_counts.budgets
        && counts.category_rules == expected_counts.category_rules;
    let table_checksums = source_table_checksums(connection, source_user_id)?;

    Ok(AccountRecoverySourceReport {
        schema_version: ACCOUNT_RECOVERY_SCHEMA_VERSION,
        source_user_id,
        source_identity_hash,
        source_sqlite_sha256,
        counts,
        expected_counts,
        expected_counts_match,
        table_checksums,
        settings_allowlist: SETTINGS_ALLOWLIST_KEYS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        settings_denylist_terms: SETTINGS_DENYLIST_TERMS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
    })
}

pub async fn resolve_postgres_account_recovery_target(
    pool: &PostgresPool,
    target_ref: &AccountRecoveryTargetRef,
) -> DbResult<AccountRecoveryTargetTriplet> {
    Ok(resolve_target_raw(pool, target_ref).await?.triplet())
}

pub async fn build_postgres_account_recovery_dry_run(
    sqlite_path: impl AsRef<Path>,
    source_user_id: i64,
    pool: &PostgresPool,
    target_ref: &AccountRecoveryTargetRef,
) -> DbResult<AccountRecoveryDryRunReport> {
    let sqlite_path = sqlite_path.as_ref();
    let source_sqlite_sha256 = file_sha256(sqlite_path)?;
    let connection = Connection::open(sqlite_path)?;
    let target = resolve_postgres_account_recovery_target(pool, target_ref).await?;
    let target_pre_counts = target_counts(pool, target.user_id).await?;
    build_postgres_account_recovery_dry_run_from_connection(
        &connection,
        source_sqlite_sha256,
        source_user_id,
        target,
        target_pre_counts,
    )
}

pub fn build_postgres_account_recovery_dry_run_from_connection(
    connection: &Connection,
    source_sqlite_sha256: String,
    source_user_id: i64,
    target: AccountRecoveryTargetTriplet,
    target_pre_counts: AccountRecoveryTargetCounts,
) -> DbResult<AccountRecoveryDryRunReport> {
    let source = inspect_postgres_account_recovery_source_from_connection(
        connection,
        source_sqlite_sha256,
        source_user_id,
    )?;
    let id_plan = build_id_plan(connection, source_user_id, target.user_id)?;
    let table_allowlist = TARGET_TABLE_ALLOWLIST
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let table_denylist = TARGET_TABLE_DENYLIST
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let redaction_policy = vec![
        "manifest stores source and target identity hashes, never raw usernames or emails".to_string(),
        "manifest omits transaction descriptions, merchants, account aliases, password hashes, tokens, and provider credentials".to_string(),
        "settings recovery is default-deny and reports only policy terms and counts".to_string(),
    ];
    let apply_requirements = vec![
        "--manifest-id must equal this manifest_id".to_string(),
        "--confirm-target-user-id must equal the resolved target id".to_string(),
        "--confirm-target-username and --confirm-target-email must match the live users row"
            .to_string(),
        "--snapshot-dir must be git-ignored or under .git/".to_string(),
    ];

    let material = json!({
        "schema_version": ACCOUNT_RECOVERY_SCHEMA_VERSION,
        "source": source,
        "target": target,
        "target_pre_counts": target_pre_counts,
        "id_plan": id_plan,
        "table_allowlist": table_allowlist,
        "table_denylist": table_denylist,
        "redaction_policy": redaction_policy,
        "apply_requirements": apply_requirements,
    });
    let dry_run_hash = checksum_json(&material)?;
    let manifest_id = format!("account-recovery-{}", &dry_run_hash[..16]);
    let snapshot_default_path = format!(".git/ai/recovery-snapshots/{manifest_id}");

    Ok(AccountRecoveryDryRunReport {
        schema_version: ACCOUNT_RECOVERY_SCHEMA_VERSION,
        manifest_id,
        source: serde_json::from_value(material["source"].clone()).map_err(json_error)?,
        target: serde_json::from_value(material["target"].clone()).map_err(json_error)?,
        target_pre_counts: serde_json::from_value(material["target_pre_counts"].clone())
            .map_err(json_error)?,
        id_plan: serde_json::from_value(material["id_plan"].clone()).map_err(json_error)?,
        table_allowlist: serde_json::from_value(material["table_allowlist"].clone())
            .map_err(json_error)?,
        table_denylist: serde_json::from_value(material["table_denylist"].clone())
            .map_err(json_error)?,
        snapshot_default_path,
        dry_run_hash,
        redaction_policy: serde_json::from_value(material["redaction_policy"].clone())
            .map_err(json_error)?,
        apply_requirements: serde_json::from_value(material["apply_requirements"].clone())
            .map_err(json_error)?,
    })
}

pub fn build_postgres_account_recovery_bundle_from_connection(
    connection: &Connection,
    source_user_id: i64,
    target_user_id: i64,
) -> DbResult<SqliteToPostgresExportBundle> {
    let id_plan = build_id_plan(connection, source_user_id, target_user_id)?;
    let accounts = read_user_rows(connection, "accounts", source_user_id)?;
    let categories = read_user_rows(connection, "categories", source_user_id)?;
    let tags = read_user_rows(connection, "tags", source_user_id)?;
    let bills = read_user_rows(connection, "bills", source_user_id)?;
    let bill_tags = read_bill_tag_rows(connection, source_user_id)?;
    let budgets = read_user_rows(connection, "budgets", source_user_id)?;
    let budget_history = read_budget_history_rows(connection, source_user_id)?;
    let category_rules = read_user_rows(connection, "category_rules", source_user_id)?;

    let account_ids = id_lookup(&accounts, &id_plan)?;
    let category_ids = id_lookup(&categories, &id_plan)?;
    let tag_ids = id_lookup(&tags, &id_plan)?;
    let bill_ids = id_lookup(&bills, &id_plan)?;
    let budget_ids = id_lookup(&budgets, &id_plan)?;
    let category_by_name = category_lookup(&categories, &id_plan)?;

    let mut exports = Vec::new();
    push_table(
        &mut exports,
        "accounts",
        "accounts",
        accounts
            .iter()
            .map(|row| map_account_row(row, &id_plan))
            .collect::<DbResult<Vec<_>>>()?,
    )?;
    push_table(
        &mut exports,
        "categories",
        "categories",
        categories
            .iter()
            .map(|row| map_category_row(row, &id_plan))
            .collect::<DbResult<Vec<_>>>()?,
    )?;
    push_table(
        &mut exports,
        "tags",
        "tags",
        tags.iter()
            .map(|row| map_tag_row(row, &id_plan))
            .collect::<DbResult<Vec<_>>>()?,
    )?;
    push_table(
        &mut exports,
        "bills",
        "bills",
        bills
            .iter()
            .map(|row| map_bill_row(row, &id_plan, &account_ids, &category_by_name))
            .collect::<DbResult<Vec<_>>>()?,
    )?;
    push_table(
        &mut exports,
        "bill_tags",
        "bill_tags",
        bill_tags
            .iter()
            .map(|row| map_bill_tag_row(row, &bill_ids, &tag_ids, target_user_id))
            .collect::<DbResult<Vec<_>>>()?,
    )?;
    push_table(
        &mut exports,
        "budgets",
        "budgets",
        budgets
            .iter()
            .map(|row| map_budget_row(row, &id_plan))
            .collect::<DbResult<Vec<_>>>()?,
    )?;
    let mut mapped_budget_history = Vec::new();
    for row in &budget_history {
        let source_budget_id = required_i64(row, "budget_id")?;
        if budget_ids.contains_key(&source_budget_id) {
            mapped_budget_history.push(map_budget_history_row(row, &id_plan, &budget_ids)?);
        }
    }
    push_table(
        &mut exports,
        "budget_history",
        "budget_history",
        mapped_budget_history,
    )?;
    push_table(
        &mut exports,
        "category_rules",
        "category_rules",
        category_rules
            .iter()
            .map(|row| map_category_rule_row(row, &id_plan, &category_ids))
            .collect::<DbResult<Vec<_>>>()?,
    )?;

    let mut account_rules = Vec::new();
    for row in &accounts {
        if let Some(rows) = map_account_alias_rules(row, &id_plan, &account_ids) {
            account_rules.extend(rows?);
        }
    }
    push_table(&mut exports, "accounts", "account_rules", account_rules)?;

    exports.sort_by_key(|table| target_table_rank(&table.target_table));
    let total_rows = exports.iter().map(|table| table.row_count).sum();
    let checksum = checksum_json(&exports)?;
    Ok(SqliteToPostgresExportBundle {
        schema_version: ACCOUNT_RECOVERY_SCHEMA_VERSION,
        table_count: exports.len(),
        total_rows,
        checksum,
        tables: exports,
    })
}

pub async fn apply_postgres_account_recovery(
    sqlite_path: impl AsRef<Path>,
    pool: &PostgresPool,
    options: AccountRecoveryApplyOptions,
) -> DbResult<AccountRecoveryApplyReport> {
    let sqlite_path = sqlite_path.as_ref();
    validate_apply_options(&options)?;
    let source_sqlite_sha256 = file_sha256(sqlite_path)?;
    let connection = Connection::open(sqlite_path)?;
    let resolved = resolve_target_raw(pool, &options.target_user_ref).await?;
    validate_target_confirmation(&resolved, &options)?;
    let mut transaction = pool.begin().await?;
    acquire_recovery_lock(&mut transaction, resolved.user_id).await?;
    lock_target_user_row(&mut transaction, resolved.user_id).await?;
    let target_pre_counts =
        target_counts_in_transaction(&mut transaction, resolved.user_id).await?;
    let dry_run = build_postgres_account_recovery_dry_run_from_connection(
        &connection,
        source_sqlite_sha256,
        options.source_user_id,
        resolved.triplet(),
        target_pre_counts.clone(),
    )?;
    if dry_run.manifest_id != options.manifest_id {
        return Err(DbError::InvalidOperation(format!(
            "account recovery manifest mismatch: got {}, expected {}",
            options.manifest_id, dry_run.manifest_id
        )));
    }
    if !dry_run.source.expected_counts_match && !options.allow_unexpected_source_shape {
        return Err(DbError::InvalidOperation(
            "account recovery source counts do not match the expected recovery shape; rerun only with an explicit override for non-production fixtures".to_string(),
        ));
    }
    let snapshot_dir = ensure_snapshot_dir_allowed(&options.workspace_root, &options.snapshot_dir)?;
    fs::create_dir_all(&snapshot_dir)?;
    let snapshot_manifest = json!({
        "schema_version": ACCOUNT_RECOVERY_SCHEMA_VERSION,
        "manifest_id": dry_run.manifest_id,
        "source_sqlite_sha256": dry_run.source.source_sqlite_sha256,
        "source_counts": dry_run.source.counts,
        "target": dry_run.target,
        "target_pre_counts": dry_run.target_pre_counts,
        "dry_run_hash": dry_run.dry_run_hash,
        "allow_unexpected_source_shape": options.allow_unexpected_source_shape,
        "redacted": true,
    });
    let snapshot_manifest_sha256 = checksum_json(&snapshot_manifest)?;
    let snapshot_manifest_path = snapshot_dir.join("manifest.redacted.json");
    write_json(&snapshot_manifest_path, &snapshot_manifest)?;

    let bundle = build_postgres_account_recovery_bundle_from_connection(
        &connection,
        options.source_user_id,
        resolved.user_id,
    )?;
    validate_no_remapped_id_collisions(&mut transaction, &bundle, resolved.user_id).await?;
    insert_migration_audit_event(
        &mut transaction,
        "started",
        json!({
            "manifest_id": dry_run.manifest_id,
            "dry_run_hash": dry_run.dry_run_hash,
            "source_sqlite_sha256": dry_run.source.source_sqlite_sha256,
            "source_counts": dry_run.source.counts,
            "target_user_id": resolved.user_id,
            "target_triplet_hash": dry_run.target.triplet_hash,
            "snapshot_manifest_sha256": snapshot_manifest_sha256,
            "allow_unexpected_source_shape": options.allow_unexpected_source_shape,
        }),
    )
    .await?;
    delete_target_user_business_rows(&mut transaction, resolved.user_id).await?;
    let import_check = import_postgres_bundle_in_transaction(&mut transaction, &bundle).await?;
    reset_recovery_identity_sequences(&mut transaction, &bundle, &dry_run.id_plan).await?;
    insert_migration_audit_event(
        &mut transaction,
        "succeeded",
        json!({
            "manifest_id": dry_run.manifest_id,
            "imported_rows": import_check.imported_rows,
            "import_checksum": import_check.checksum,
            "table_checksums": import_check.table_checksums,
            "target_user_id": resolved.user_id,
        }),
    )
    .await?;
    transaction.commit().await?;

    let target_post_counts = target_counts(pool, resolved.user_id).await?;
    Ok(AccountRecoveryApplyReport {
        schema_version: ACCOUNT_RECOVERY_SCHEMA_VERSION,
        manifest_id: dry_run.manifest_id,
        target: dry_run.target,
        source_counts: dry_run.source.counts,
        target_pre_counts,
        target_post_counts,
        import_check,
        snapshot_manifest_sha256,
        dry_run_hash: dry_run.dry_run_hash,
        allow_unexpected_source_shape: options.allow_unexpected_source_shape,
    })
}

pub fn account_recovery_setting_allowed(key: &str, is_encrypted: bool) -> bool {
    if is_encrypted {
        return false;
    }
    let normalized = key.trim().to_ascii_lowercase();
    if SETTINGS_DENYLIST_TERMS
        .iter()
        .any(|term| normalized.contains(term))
    {
        return false;
    }
    SETTINGS_ALLOWLIST_KEYS.contains(&normalized.as_str())
}

fn source_counts(
    connection: &Connection,
    source_user_id: i64,
) -> DbResult<AccountRecoverySourceCounts> {
    let accounts = read_user_rows(connection, "accounts", source_user_id)?;
    let budgets = read_user_rows(connection, "budgets", source_user_id)?;
    let budget_ids = budgets
        .iter()
        .map(|row| required_i64(row, "id"))
        .collect::<DbResult<BTreeSet<_>>>()?;
    let budget_history = read_budget_history_rows(connection, source_user_id)?;
    let budget_history_budget_ids = budget_history
        .iter()
        .map(|row| required_i64(row, "budget_id"))
        .collect::<DbResult<Vec<_>>>()?;
    let budget_history_orphaned = budget_history_budget_ids
        .iter()
        .filter(|budget_id| !budget_ids.contains(budget_id))
        .count() as u64;
    let generated_account_rules = accounts
        .iter()
        .map(|row| parse_aliases(optional_string(row, "aliases").as_deref()).len() as u64)
        .sum();
    let accounts_with_aliases = accounts
        .iter()
        .filter(|row| !parse_aliases(optional_string(row, "aliases").as_deref()).is_empty())
        .count() as u64;
    let (settings_allowed, settings_denied) = settings_policy_counts(connection)?;
    Ok(AccountRecoverySourceCounts {
        bills: read_user_rows(connection, "bills", source_user_id)?.len() as u64,
        accounts: accounts.len() as u64,
        categories: read_user_rows(connection, "categories", source_user_id)?.len() as u64,
        tags: read_user_rows(connection, "tags", source_user_id)?.len() as u64,
        bill_tags: read_bill_tag_rows(connection, source_user_id)?.len() as u64,
        budgets: budgets.len() as u64,
        budget_history: budget_history.len() as u64,
        budget_history_orphaned,
        category_rules: read_user_rows(connection, "category_rules", source_user_id)?.len() as u64,
        accounts_with_aliases,
        generated_account_rules,
        settings_allowed,
        settings_denied,
    })
}

fn settings_policy_counts(connection: &Connection) -> DbResult<(u64, u64)> {
    if !sqlite_table_exists(connection, "app_settings")? {
        return Ok((0, 0));
    }
    let mut statement = connection
        .prepare("SELECT key, COALESCE(is_encrypted, 0) FROM app_settings ORDER BY key")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? != 0))
    })?;
    let mut allowed = 0;
    let mut denied = 0;
    for row in rows {
        let (key, encrypted) = row?;
        if account_recovery_setting_allowed(&key, encrypted) {
            allowed += 1;
        } else {
            denied += 1;
        }
    }
    Ok((allowed, denied))
}

fn source_table_checksums(
    connection: &Connection,
    source_user_id: i64,
) -> DbResult<BTreeMap<String, String>> {
    let mut checksums = BTreeMap::new();
    for table in [
        "accounts",
        "categories",
        "tags",
        "bills",
        "budgets",
        "budget_history",
        "category_rules",
    ] {
        let rows = if table == "budget_history" {
            read_budget_history_rows(connection, source_user_id)?
        } else {
            read_user_rows(connection, table, source_user_id)?
        };
        checksums.insert(table.to_string(), checksum_json(&rows)?);
    }
    checksums.insert(
        "bill_tags".to_string(),
        checksum_json(&read_bill_tag_rows(connection, source_user_id)?)?,
    );
    Ok(checksums)
}

fn source_user_identity_hash(connection: &Connection, source_user_id: i64) -> DbResult<String> {
    if !sqlite_table_exists(connection, "users")? {
        return Err(DbError::InvalidOperation(
            "account recovery source users table is missing".to_string(),
        ));
    }
    let user = connection
        .query_row(
            "SELECT username, email FROM users WHERE id = ?1",
            params![source_user_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .optional()?
        .ok_or_else(|| {
            DbError::InvalidOperation(format!(
                "account recovery source user {source_user_id} was not found"
            ))
        })?;
    Ok(hash_identity_parts(&[
        source_user_id.to_string(),
        user.0,
        user.1.unwrap_or_default(),
    ]))
}

async fn resolve_target_raw(
    pool: &PostgresPool,
    target_ref: &AccountRecoveryTargetRef,
) -> DbResult<ResolvedTarget> {
    let rows = sqlx::query(
        "SELECT id, username, email, password_hash IS NOT NULL AS password_hash_present FROM users WHERE id::text = $1 OR username = $1 OR email = $1 ORDER BY id ASC",
    )
    .bind(target_ref.as_str())
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Err(DbError::InvalidOperation(
            "account recovery target user ref did not match a PostgreSQL users row".to_string(),
        ));
    }
    if rows.len() > 1 {
        return Err(DbError::InvalidOperation(
            "account recovery target user ref matched multiple PostgreSQL users rows".to_string(),
        ));
    }
    let row = &rows[0];
    Ok(ResolvedTarget {
        user_id: row.try_get("id")?,
        username: row.try_get("username")?,
        email: row.try_get("email")?,
        password_hash_present: row.try_get("password_hash_present")?,
    })
}

impl ResolvedTarget {
    fn triplet(&self) -> AccountRecoveryTargetTriplet {
        let email_sha256 = self.email.as_ref().map(|value| hash_text(value));
        let auth_source_evidence = if self.password_hash_present {
            "postgres.users:password_hash_present"
        } else {
            "postgres.users:password_hash_absent"
        }
        .to_string();
        let triplet_hash = hash_identity_parts(&[
            self.user_id.to_string(),
            self.username.clone(),
            self.email.clone().unwrap_or_default(),
            auth_source_evidence.clone(),
        ]);
        AccountRecoveryTargetTriplet {
            user_id: self.user_id,
            username_sha256: hash_text(&self.username),
            email_sha256,
            auth_source_evidence,
            triplet_hash,
        }
    }
}

async fn target_counts(
    pool: &PostgresPool,
    target_user_id: i64,
) -> DbResult<AccountRecoveryTargetCounts> {
    Ok(AccountRecoveryTargetCounts {
        accounts: target_count(pool, "accounts", target_user_id).await?,
        categories: target_count(pool, "categories", target_user_id).await?,
        tags: target_count(pool, "tags", target_user_id).await?,
        bills: target_count(pool, "bills", target_user_id).await?,
        bill_tags: target_count(pool, "bill_tags", target_user_id).await?,
        budgets: target_count(pool, "budgets", target_user_id).await?,
        budget_history: target_count(pool, "budget_history", target_user_id).await?,
        category_rules: target_count(pool, "category_rules", target_user_id).await?,
        account_rules: target_count(pool, "account_rules", target_user_id).await?,
    })
}

async fn target_count(pool: &PostgresPool, table_name: &str, target_user_id: i64) -> DbResult<i64> {
    if !TARGET_TABLE_ALLOWLIST.contains(&table_name) {
        return Err(DbError::InvalidOperation(format!(
            "account recovery target count table is not allowlisted: {table_name}"
        )));
    }
    let sql = format!("SELECT COUNT(*)::BIGINT FROM {table_name} WHERE user_id = $1");
    let count = sqlx::query_scalar(&sql)
        .bind(target_user_id)
        .fetch_one(pool)
        .await?;
    Ok(count)
}

async fn target_counts_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    target_user_id: i64,
) -> DbResult<AccountRecoveryTargetCounts> {
    Ok(AccountRecoveryTargetCounts {
        accounts: target_count_in_transaction(transaction, "accounts", target_user_id).await?,
        categories: target_count_in_transaction(transaction, "categories", target_user_id).await?,
        tags: target_count_in_transaction(transaction, "tags", target_user_id).await?,
        bills: target_count_in_transaction(transaction, "bills", target_user_id).await?,
        bill_tags: target_count_in_transaction(transaction, "bill_tags", target_user_id).await?,
        budgets: target_count_in_transaction(transaction, "budgets", target_user_id).await?,
        budget_history: target_count_in_transaction(transaction, "budget_history", target_user_id)
            .await?,
        category_rules: target_count_in_transaction(transaction, "category_rules", target_user_id)
            .await?,
        account_rules: target_count_in_transaction(transaction, "account_rules", target_user_id)
            .await?,
    })
}

async fn target_count_in_transaction(
    transaction: &mut sqlx::Transaction<'_, Postgres>,
    table_name: &str,
    target_user_id: i64,
) -> DbResult<i64> {
    if !TARGET_TABLE_ALLOWLIST.contains(&table_name) {
        return Err(DbError::InvalidOperation(format!(
            "account recovery target count table is not allowlisted: {table_name}"
        )));
    }
    let sql = format!("SELECT COUNT(*)::BIGINT FROM {table_name} WHERE user_id = $1");
    let count = sqlx::query_scalar(&sql)
        .bind(target_user_id)
        .fetch_one(&mut **transaction)
        .await?;
    Ok(count)
}

fn build_id_plan(
    connection: &Connection,
    source_user_id: i64,
    target_user_id: i64,
) -> DbResult<AccountRecoveryIdPlan> {
    validate_positive_id(source_user_id, "source_user_id")?;
    validate_positive_id(target_user_id, "target_user_id")?;
    let id_base = target_user_id
        .checked_mul(ACCOUNT_RECOVERY_ID_BLOCK_SIZE)
        .ok_or_else(|| {
            DbError::InvalidOperation(format!(
                "account recovery target user id {target_user_id} overflows id remap base"
            ))
        })?;
    let max_source_id = max_source_id(connection, source_user_id)?;
    if max_source_id >= ACCOUNT_RECOVERY_ID_BLOCK_SIZE {
        return Err(DbError::InvalidOperation(format!(
            "account recovery max source id {max_source_id} exceeds remap block size {}",
            ACCOUNT_RECOVERY_ID_BLOCK_SIZE
        )));
    }
    let max_remapped_id = id_base.checked_add(max_source_id).ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "account recovery id remap overflows for target user {target_user_id}"
        ))
    })?;
    Ok(AccountRecoveryIdPlan {
        source_user_id,
        target_user_id,
        id_block_size: ACCOUNT_RECOVERY_ID_BLOCK_SIZE,
        id_base,
        max_source_id,
        max_remapped_id,
    })
}

fn max_source_id(connection: &Connection, source_user_id: i64) -> DbResult<i64> {
    let mut max_id = 0;
    for table in [
        "accounts",
        "categories",
        "tags",
        "bills",
        "budgets",
        "budget_history",
        "category_rules",
    ] {
        if !sqlite_table_exists(connection, table)?
            || !sqlite_table_has_column(connection, table, "id")?
            || (table != "budget_history"
                && !sqlite_table_has_column(connection, table, "user_id")?)
        {
            continue;
        }
        let table_identifier = quote_sqlite_identifier(table);
        let sql = if table == "budget_history" {
            if sqlite_table_has_column(connection, table, "user_id")? {
                format!("SELECT COALESCE(MAX(id), 0) FROM {table_identifier} WHERE user_id = ?1")
            } else {
                format!(
                    "SELECT COALESCE(MAX(bh.id), 0) FROM {table_identifier} bh JOIN budgets b ON b.id = bh.budget_id WHERE b.user_id = ?1"
                )
            }
        } else {
            format!("SELECT COALESCE(MAX(id), 0) FROM {table_identifier} WHERE user_id = ?1")
        };
        let table_max: i64 =
            connection.query_row(&sql, params![source_user_id], |row| row.get(0))?;
        max_id = max_id.max(table_max);
    }
    Ok(max_id)
}

fn read_user_rows(
    connection: &Connection,
    table_name: &str,
    source_user_id: i64,
) -> DbResult<Vec<BTreeMap<String, Value>>> {
    if !sqlite_table_exists(connection, table_name)? {
        return Ok(Vec::new());
    }
    if !sqlite_table_has_column(connection, table_name, "user_id")? {
        return Err(DbError::InvalidOperation(format!(
            "account recovery source table {table_name} has no user_id column"
        )));
    }
    let sql = format!(
        "SELECT * FROM {} WHERE user_id = ?1 ORDER BY id ASC",
        quote_sqlite_identifier(table_name)
    );
    query_rows_user_id(connection, &sql, source_user_id)
}

fn read_bill_tag_rows(
    connection: &Connection,
    source_user_id: i64,
) -> DbResult<Vec<BTreeMap<String, Value>>> {
    if !sqlite_table_exists(connection, "bill_tags")? || !sqlite_table_exists(connection, "bills")?
    {
        return Ok(Vec::new());
    }
    query_rows_user_id(
        connection,
        "SELECT bt.* FROM bill_tags bt JOIN bills b ON b.id = bt.bill_id WHERE b.user_id = ?1 ORDER BY bt.bill_id ASC, bt.tag_id ASC",
        source_user_id,
    )
}

fn read_budget_history_rows(
    connection: &Connection,
    source_user_id: i64,
) -> DbResult<Vec<BTreeMap<String, Value>>> {
    if !sqlite_table_exists(connection, "budget_history")? {
        return Ok(Vec::new());
    }
    if sqlite_table_has_column(connection, "budget_history", "user_id")? {
        return query_rows_user_id(
            connection,
            "SELECT * FROM budget_history WHERE user_id = ?1 ORDER BY id ASC",
            source_user_id,
        );
    }
    if !sqlite_table_exists(connection, "budgets")? {
        return Ok(Vec::new());
    }
    query_rows_user_id(
        connection,
        "SELECT bh.* FROM budget_history bh JOIN budgets b ON b.id = bh.budget_id WHERE b.user_id = ?1 ORDER BY bh.id ASC",
        source_user_id,
    )
}

fn query_rows_user_id(
    connection: &Connection,
    sql: &str,
    source_user_id: i64,
) -> DbResult<Vec<BTreeMap<String, Value>>> {
    let mut statement = connection.prepare(sql)?;
    let columns = statement
        .column_names()
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let rows = statement
        .query_map(params![source_user_id], |row| {
            let mut values = BTreeMap::new();
            for (index, column) in columns.iter().enumerate() {
                values.insert(column.clone(), sqlite_value_to_json(row.get_ref(index)?));
            }
            Ok(values)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn id_lookup(
    rows: &[BTreeMap<String, Value>],
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<BTreeMap<i64, i64>> {
    rows.iter()
        .map(|row| {
            let source_id = required_i64(row, "id")?;
            Ok((source_id, remap_id(id_plan, source_id)?))
        })
        .collect()
}

fn category_lookup(
    rows: &[BTreeMap<String, Value>],
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<BTreeMap<(String, String), i64>> {
    let mut lookup = BTreeMap::new();
    for row in rows {
        let source_id = required_i64(row, "id")?;
        lookup.insert(category_key(row), remap_id(id_plan, source_id)?);
    }
    Ok(lookup)
}

fn map_account_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "legacy_id", source_id);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_string(&mut values, "name", required_string(row, "name")?);
    insert_string(
        &mut values,
        "account_type",
        optional_i64(row, "type").unwrap_or_default().to_string(),
    );
    insert_optional_string(
        &mut values,
        "payment_method",
        optional_string(row, "category"),
    );
    insert_string(
        &mut values,
        "currency",
        optional_string(row, "currency").unwrap_or_else(|| "CNY".to_string()),
    );
    insert_i64(
        &mut values,
        "balance_cents",
        required_yuan_value_to_cents(row.get("balance"), "accounts.balance")?,
    );
    insert_bool(&mut values, "is_active", !truthy(row.get("hidden")));
    insert_i64(
        &mut values,
        "display_order",
        optional_i64(row, "display_order").unwrap_or_default(),
    );
    insert_json(
        &mut values,
        "metadata",
        metadata_without(
            row,
            &[
                "id",
                "user_id",
                "name",
                "type",
                "category",
                "currency",
                "balance",
                "aliases",
                "hidden",
                "display_order",
                "created_at",
                "updated_at",
            ],
        ),
    );
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "updated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("accounts", "accounts", Some(source_id), values)
}

fn map_category_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let main = optional_string(row, "main_category").unwrap_or_default();
    let sub = optional_string(row, "sub_category").unwrap_or_default();
    let path = [main.as_str(), sub.as_str()]
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("/");
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "legacy_id", source_id);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_optional_i64(&mut values, "parent_id", None);
    insert_string(
        &mut values,
        "name",
        if path.is_empty() {
            format!("category:{source_id}")
        } else {
            path.clone()
        },
    );
    insert_string(
        &mut values,
        "category_type",
        optional_i64(row, "type").unwrap_or_default().to_string(),
    );
    insert_optional_string(&mut values, "path", (!path.is_empty()).then_some(path));
    insert_optional_string(&mut values, "icon", optional_string(row, "icon"));
    insert_optional_string(&mut values, "color", optional_string(row, "color"));
    insert_i64(
        &mut values,
        "display_order",
        optional_i64(row, "priority").unwrap_or_default(),
    );
    insert_bool(&mut values, "is_active", !truthy(row.get("hidden")));
    insert_json(
        &mut values,
        "metadata",
        metadata_without(
            row,
            &[
                "id",
                "user_id",
                "type",
                "main_category",
                "sub_category",
                "priority",
                "hidden",
                "icon",
                "color",
                "created_at",
                "updated_at",
            ],
        ),
    );
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "updated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("categories", "categories", Some(source_id), values)
}

fn map_tag_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "legacy_id", source_id);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_string(&mut values, "name", required_string(row, "name")?);
    insert_optional_string(&mut values, "color", optional_string(row, "color"));
    insert_i64(
        &mut values,
        "display_order",
        optional_i64(row, "display_order").unwrap_or_default(),
    );
    insert_json(
        &mut values,
        "metadata",
        metadata_without(
            row,
            &[
                "id",
                "user_id",
                "name",
                "color",
                "display_order",
                "created_at",
                "updated_at",
            ],
        ),
    );
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "updated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("tags", "tags", Some(source_id), values)
}

fn map_bill_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
    account_ids: &BTreeMap<i64, i64>,
    category_by_name: &BTreeMap<(String, String), i64>,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let bill_type = optional_string(row, "type").unwrap_or_default();
    let source_account_id = optional_i64(row, "source_account_id")
        .filter(|value| *value > 0)
        .and_then(|value| account_ids.get(&value).copied());
    let target_account_id = optional_i64(row, "destination_account_id")
        .filter(|value| *value > 0)
        .and_then(|value| account_ids.get(&value).copied());
    let category_id = category_by_name.get(&category_key(row)).copied();
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "legacy_id", source_id);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_string(&mut values, "occurred_at", required_string(row, "date")?);
    insert_i64(
        &mut values,
        "amount_cents",
        required_yuan_value_to_cents(row.get("amount"), "bills.amount")?,
    );
    insert_string(&mut values, "direction", bill_direction(&bill_type));
    insert_string(
        &mut values,
        "transaction_type",
        bill_transaction_type(&bill_type),
    );
    insert_optional_i64(&mut values, "account_id", source_account_id);
    insert_optional_i64(&mut values, "category_id", category_id);
    insert_optional_i64(&mut values, "transfer_target_account_id", target_account_id);
    insert_optional_i64(&mut values, "source_account_id", source_account_id);
    insert_optional_i64(&mut values, "target_account_id", target_account_id);
    insert_optional_string(
        &mut values,
        "merchant",
        optional_string(row, "counterparty"),
    );
    insert_optional_string(
        &mut values,
        "payment_method",
        optional_string(row, "payment_method"),
    );
    insert_optional_string(
        &mut values,
        "description",
        optional_string(row, "description"),
    );
    insert_optional_string(&mut values, "source_hash", optional_string(row, "hash"));
    insert_json(
        &mut values,
        "standard_payload",
        metadata_without(
            row,
            &[
                "id",
                "user_id",
                "date",
                "type",
                "amount",
                "counterparty",
                "description",
                "payment_method",
                "hash",
                "created_at",
                "updated_at",
                "source_account_id",
                "destination_account_id",
            ],
        ),
    );
    insert_json(&mut values, "raw_payload", json!({}));
    insert_bool(&mut values, "is_deleted", false);
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "updated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("bills", "bills", Some(source_id), values)
}

fn map_bill_tag_row(
    row: &BTreeMap<String, Value>,
    bill_ids: &BTreeMap<i64, i64>,
    tag_ids: &BTreeMap<i64, i64>,
    target_user_id: i64,
) -> DbResult<PostgresTargetRow> {
    let source_bill_id = required_i64(row, "bill_id")?;
    let source_tag_id = required_i64(row, "tag_id")?;
    let mut values = BTreeMap::new();
    insert_i64(
        &mut values,
        "bill_id",
        lookup_mapped_id(bill_ids, source_bill_id, "bill_tags.bill_id")?,
    );
    insert_i64(
        &mut values,
        "tag_id",
        lookup_mapped_id(tag_ids, source_tag_id, "bill_tags.tag_id")?,
    );
    insert_i64(&mut values, "user_id", target_user_id);
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    target_row("bill_tags", "bill_tags", Some(source_bill_id), values)
}

fn map_budget_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "legacy_id", source_id);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_string(&mut values, "name", recovered_budget_name(row, source_id));
    insert_optional_string(&mut values, "category", optional_string(row, "category"));
    insert_string(
        &mut values,
        "sub_category",
        optional_string(row, "sub_category").unwrap_or_default(),
    );
    insert_string(
        &mut values,
        "period_type",
        required_string(row, "period_type")?,
    );
    insert_i64(
        &mut values,
        "amount_cents",
        required_yuan_value_to_cents(row.get("amount"), "budgets.amount")?,
    );
    insert_string(
        &mut values,
        "start_date",
        required_string(row, "start_date")?,
    );
    insert_optional_string(&mut values, "end_date", optional_string(row, "end_date"));
    insert_i64(
        &mut values,
        "alert_threshold",
        optional_i64(row, "alert_threshold").unwrap_or(80),
    );
    insert_bool(
        &mut values,
        "enabled",
        row.get("enabled")
            .map(|value| truthy(Some(value)))
            .unwrap_or(true),
    );
    insert_json(
        &mut values,
        "metadata",
        metadata_without(
            row,
            &[
                "id",
                "user_id",
                "name",
                "category",
                "sub_category",
                "period_type",
                "amount",
                "start_date",
                "end_date",
                "alert_threshold",
                "enabled",
                "created_at",
                "updated_at",
            ],
        ),
    );
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "updated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("budgets", "budgets", Some(source_id), values)
}

fn map_budget_history_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
    budget_ids: &BTreeMap<i64, i64>,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let source_budget_id = required_i64(row, "budget_id")?;
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_i64(
        &mut values,
        "budget_id",
        lookup_mapped_id(budget_ids, source_budget_id, "budget_history.budget_id")?,
    );
    insert_string(
        &mut values,
        "period_start",
        required_string(row, "period_start")?,
    );
    insert_string(
        &mut values,
        "period_end",
        required_string(row, "period_end")?,
    );
    insert_i64(
        &mut values,
        "budget_amount_cents",
        required_yuan_value_to_cents(row.get("budget_amount"), "budget_history.budget_amount")?,
    );
    insert_i64(
        &mut values,
        "spent_amount_cents",
        required_yuan_value_to_cents(row.get("spent_amount"), "budget_history.spent_amount")?,
    );
    insert_i64(
        &mut values,
        "remaining_amount_cents",
        required_yuan_value_to_cents(
            row.get("remaining_amount"),
            "budget_history.remaining_amount",
        )?,
    );
    insert_number(
        &mut values,
        "execution_rate",
        optional_f64(row.get("execution_rate")).unwrap_or_default(),
    );
    insert_string(
        &mut values,
        "status",
        optional_string(row, "status").unwrap_or_else(|| "within_budget".to_string()),
    );
    insert_string(
        &mut values,
        "filter_summary",
        optional_string(row, "filter_summary").unwrap_or_default(),
    );
    insert_string(
        &mut values,
        "calculated_at",
        optional_string(row, "calculated_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "calculated_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "calculated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("budget_history", "budget_history", Some(source_id), values)
}

fn map_category_rule_row(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
    category_ids: &BTreeMap<i64, i64>,
) -> DbResult<PostgresTargetRow> {
    let source_id = required_i64(row, "id")?;
    let category_id = optional_i64(row, "category_id")
        .and_then(|source_category_id| category_ids.get(&source_category_id).copied());
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", remap_id(id_plan, source_id)?);
    insert_i64(&mut values, "user_id", id_plan.target_user_id);
    insert_optional_i64(&mut values, "category_id", category_id);
    insert_string(
        &mut values,
        "name",
        optional_string(row, "name").unwrap_or_default(),
    );
    insert_string(&mut values, "transaction_type_scope", "all");
    insert_json(
        &mut values,
        "field_scope",
        json!(["counterparty", "payment_method", "description"]),
    );
    insert_json(
        &mut values,
        "rule_expression",
        json!({
            "legacy_expression": optional_string(row, "rule_expression").unwrap_or_default(),
            "regex_enabled": truthy(row.get("regex_enabled")),
            "source": "sqlite_account_recovery"
        }),
    );
    insert_i64(
        &mut values,
        "priority",
        optional_i64(row, "priority").unwrap_or_default(),
    );
    insert_bool(
        &mut values,
        "enabled",
        row.get("enabled")
            .map(|value| truthy(Some(value)))
            .unwrap_or(true),
    );
    insert_string(
        &mut values,
        "created_at",
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_string(
        &mut values,
        "updated_at",
        optional_string(row, "updated_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    target_row("category_rules", "category_rules", Some(source_id), values)
}

fn map_account_alias_rules(
    row: &BTreeMap<String, Value>,
    id_plan: &AccountRecoveryIdPlan,
    account_ids: &BTreeMap<i64, i64>,
) -> Option<DbResult<Vec<PostgresTargetRow>>> {
    let aliases = parse_aliases(optional_string(row, "aliases").as_deref());
    if aliases.is_empty() {
        return None;
    }
    Some(
        aliases
            .into_iter()
            .map(|alias| {
                let source_account_id = required_i64(row, "id")?;
                let target_account_id =
                    lookup_mapped_id(account_ids, source_account_id, "account_rules.account_id")?;
                let created_at =
                    optional_string(row, "created_at").unwrap_or_else(default_timestamp);
                let updated_at =
                    optional_string(row, "updated_at").unwrap_or_else(default_timestamp);
                let mut values = BTreeMap::new();
                insert_i64(&mut values, "user_id", id_plan.target_user_id);
                insert_i64(&mut values, "account_id", target_account_id);
                insert_string(
                    &mut values,
                    "name",
                    format!("recovered account rule: {alias}"),
                );
                insert_string(&mut values, "account_role_scope", "any");
                insert_string(&mut values, "transaction_type_scope", "all");
                insert_json(
                    &mut values,
                    "field_scope",
                    json!(["counterparty", "payment_method", "description", "parser"]),
                );
                insert_json(
                    &mut values,
                    "rule_expression",
                    json!({
                        "operator": "contains_any",
                        "values": [alias],
                        "source": "sqlite_account_recovery"
                    }),
                );
                insert_bool(&mut values, "regex_enabled", false);
                insert_i64(&mut values, "priority", 1000);
                insert_bool(&mut values, "enabled", true);
                insert_string(&mut values, "source", "sqlite_account_recovery");
                let alias_hash = hash_text(&alias.to_ascii_lowercase());
                insert_string(
                    &mut values,
                    "source_key",
                    format!("sqlite:{}:{}", source_account_id, &alias_hash[..16]),
                );
                insert_i64(&mut values, "match_count", 0);
                insert_string(&mut values, "created_at", created_at);
                insert_string(&mut values, "updated_at", updated_at);
                insert_i64(&mut values, "version", 1);
                target_row("accounts", "account_rules", Some(source_account_id), values)
            })
            .collect(),
    )
}

fn push_table(
    exports: &mut Vec<SqliteToPostgresTableExport>,
    source_table: &str,
    target_table: &str,
    mut rows: Vec<PostgresTargetRow>,
) -> DbResult<()> {
    if rows.is_empty() {
        return Ok(());
    }
    rows.sort_by(|left, right| {
        left.source_id
            .cmp(&right.source_id)
            .then(left.checksum.cmp(&right.checksum))
    });
    let checksum = checksum_json(&rows)?;
    exports.push(SqliteToPostgresTableExport {
        source_table: source_table.to_string(),
        target_table: target_table.to_string(),
        row_count: rows.len(),
        checksum,
        rows,
    });
    Ok(())
}

fn target_row(
    source_table: &str,
    target_table: &str,
    source_id: Option<i64>,
    values: BTreeMap<String, Value>,
) -> DbResult<PostgresTargetRow> {
    let checksum = checksum_json(&values)?;
    Ok(PostgresTargetRow {
        source_table: source_table.to_string(),
        target_table: target_table.to_string(),
        source_id,
        values,
        checksum,
    })
}

fn target_table_rank(table_name: &str) -> usize {
    TARGET_TABLE_ALLOWLIST
        .iter()
        .position(|target_table| *target_table == table_name)
        .unwrap_or(usize::MAX)
}

fn remap_id(id_plan: &AccountRecoveryIdPlan, source_id: i64) -> DbResult<i64> {
    validate_positive_id(source_id, "source_id")?;
    if source_id >= id_plan.id_block_size {
        return Err(DbError::InvalidOperation(format!(
            "account recovery source id {source_id} exceeds remap block size {}",
            id_plan.id_block_size
        )));
    }
    id_plan.id_base.checked_add(source_id).ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "account recovery remapped id overflow for source id {source_id}"
        ))
    })
}

fn lookup_mapped_id(lookup: &BTreeMap<i64, i64>, source_id: i64, field: &str) -> DbResult<i64> {
    lookup.get(&source_id).copied().ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "account recovery missing mapped id for {field} source id {source_id}"
        ))
    })
}

fn category_key(row: &BTreeMap<String, Value>) -> (String, String) {
    (
        optional_string(row, "main_category").unwrap_or_default(),
        optional_string(row, "sub_category").unwrap_or_default(),
    )
}

async fn acquire_recovery_lock(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_user_id: i64,
) -> DbResult<()> {
    let lock_user = i32::try_from(target_user_id).map_err(|_| {
        DbError::InvalidOperation(format!(
            "account recovery target user id {target_user_id} cannot be used as advisory lock key"
        ))
    })?;
    sqlx::query("SELECT pg_advisory_xact_lock($1, $2)")
        .bind(ACCOUNT_RECOVERY_LOCK_CLASS_ID)
        .bind(lock_user)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn lock_target_user_row(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_user_id: i64,
) -> DbResult<()> {
    let found = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(target_user_id)
        .fetch_optional(&mut **transaction)
        .await?;
    if found.is_none() {
        return Err(DbError::InvalidOperation(
            "account recovery target user disappeared before apply".to_string(),
        ));
    }
    Ok(())
}

async fn validate_no_remapped_id_collisions(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bundle: &SqliteToPostgresExportBundle,
    target_user_id: i64,
) -> DbResult<()> {
    let mut ids_by_table = BTreeMap::<String, Vec<i64>>::new();
    for table in &bundle.tables {
        if table.target_table == "bill_tags" {
            continue;
        }
        if !TARGET_TABLE_ALLOWLIST.contains(&table.target_table.as_str()) {
            return Err(DbError::InvalidOperation(format!(
                "account recovery collision check table is not allowlisted: {}",
                table.target_table
            )));
        }
        for row in &table.rows {
            if let Some(id) = row.values.get("id").and_then(Value::as_i64) {
                ids_by_table
                    .entry(table.target_table.clone())
                    .or_default()
                    .push(id);
            }
        }
    }

    for (table_name, ids) in ids_by_table {
        if ids.is_empty() {
            continue;
        }
        let table_identifier = quote_postgres_identifier(&table_name)?;
        let sql = format!(
            "SELECT COUNT(*)::BIGINT FROM {table_identifier} WHERE id = ANY($1) AND user_id <> $2"
        );
        let count: i64 = sqlx::query_scalar(&sql)
            .bind(ids)
            .bind(target_user_id)
            .fetch_one(&mut **transaction)
            .await?;
        if count > 0 {
            return Err(DbError::InvalidOperation(format!(
                "account recovery remapped ids collide with non-target rows in {table_name}"
            )));
        }
    }
    Ok(())
}

async fn reset_recovery_identity_sequences(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    bundle: &SqliteToPostgresExportBundle,
    id_plan: &AccountRecoveryIdPlan,
) -> DbResult<()> {
    let mut tables = BTreeSet::new();
    for table in &bundle.tables {
        if table.target_table != "bill_tags" {
            tables.insert(table.target_table.as_str());
        }
    }
    for table_name in tables {
        if !TARGET_TABLE_ALLOWLIST.contains(&table_name) {
            return Err(DbError::InvalidOperation(format!(
                "account recovery sequence reset table is not allowlisted: {table_name}"
            )));
        }
        let sequence_name: Option<String> =
            sqlx::query_scalar("SELECT pg_get_serial_sequence($1, 'id')")
                .bind(table_name)
                .fetch_one(&mut **transaction)
                .await?;
        let Some(sequence_name) = sequence_name else {
            continue;
        };
        let table_identifier = quote_postgres_identifier(table_name)?;
        let sql = format!(
            "SELECT setval($1, COALESCE((SELECT MAX(id) FROM {table_identifier} WHERE id < $2 OR id > $3), 1), (SELECT MAX(id) FROM {table_identifier} WHERE id < $2 OR id > $3) IS NOT NULL)"
        );
        sqlx::query(&sql)
            .bind(sequence_name)
            .bind(id_plan.id_base)
            .bind(id_plan.max_remapped_id)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

fn quote_postgres_identifier(identifier: &str) -> DbResult<String> {
    if !TARGET_TABLE_ALLOWLIST.contains(&identifier) {
        return Err(DbError::InvalidOperation(format!(
            "account recovery unsafe PostgreSQL identifier: {identifier}"
        )));
    }
    Ok(format!("\"{}\"", identifier.replace('"', "\"\"")))
}

async fn delete_target_user_business_rows(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    target_user_id: i64,
) -> DbResult<()> {
    for table_name in USER_DATA_DELETE_TABLES {
        let sql = format!("DELETE FROM {table_name} WHERE user_id = $1");
        sqlx::query(&sql)
            .bind(target_user_id)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

async fn insert_migration_audit_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    status: &str,
    payload: Value,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO migration_audit_events (migration_name, stage, status, payload) VALUES ($1, 'account_recovery', $2, $3)",
    )
    .bind(ACCOUNT_RECOVERY_MIGRATION_NAME)
    .bind(status)
    .bind(Json(payload))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn validate_apply_options(options: &AccountRecoveryApplyOptions) -> DbResult<()> {
    validate_positive_id(options.source_user_id, "source_user_id")?;
    validate_positive_id(options.confirm_target_user_id, "confirm_target_user_id")?;
    if options.manifest_id.trim().is_empty() {
        return Err(DbError::InvalidOperation(
            "account recovery apply requires --manifest-id".to_string(),
        ));
    }
    if options.confirm_target_username.trim().is_empty() {
        return Err(DbError::InvalidOperation(
            "account recovery apply requires --confirm-target-username".to_string(),
        ));
    }
    if options.snapshot_dir.as_os_str().is_empty() {
        return Err(DbError::InvalidOperation(
            "account recovery apply requires --snapshot-dir".to_string(),
        ));
    }
    Ok(())
}

fn validate_target_confirmation(
    resolved: &ResolvedTarget,
    options: &AccountRecoveryApplyOptions,
) -> DbResult<()> {
    let resolved_email = resolved.email.clone().unwrap_or_default();
    if resolved.user_id != options.confirm_target_user_id
        || resolved.username != options.confirm_target_username
        || resolved_email != options.confirm_target_email
    {
        return Err(DbError::InvalidOperation(
            "account recovery target confirmation does not match the live PostgreSQL users row"
                .to_string(),
        ));
    }
    Ok(())
}

fn ensure_snapshot_dir_allowed(workspace_root: &Path, snapshot_dir: &Path) -> DbResult<PathBuf> {
    reject_parent_dir_components(snapshot_dir, "snapshot path")?;
    let workspace_root = canonical_or_absolutize(workspace_root)?;
    let snapshot_dir = if snapshot_dir.is_absolute() {
        snapshot_dir.to_path_buf()
    } else {
        workspace_root.join(snapshot_dir)
    };
    reject_parent_dir_components(&snapshot_dir, "snapshot path")?;
    if !snapshot_dir.starts_with(&workspace_root) {
        return Ok(snapshot_dir);
    }
    let relative = snapshot_dir
        .strip_prefix(&workspace_root)
        .map_err(|error| {
            DbError::InvalidOperation(format!(
                "account recovery snapshot path strip failed: {error}"
            ))
        })?;
    if relative.starts_with(".git") {
        return Ok(snapshot_dir);
    }
    let output = Command::new("git")
        .arg("check-ignore")
        .arg("-v")
        .arg("--")
        .arg(relative)
        .current_dir(&workspace_root)
        .output()?;
    if output.status.success() {
        return Ok(snapshot_dir);
    }
    Err(DbError::InvalidOperation(format!(
        "account recovery snapshot path {} is inside the workspace but is not git-ignored",
        snapshot_dir.display()
    )))
}

fn reject_parent_dir_components(path: &Path, label: &str) -> DbResult<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(DbError::InvalidOperation(format!(
            "account recovery {label} must not contain parent-directory components"
        )));
    }
    Ok(())
}

fn canonical_or_absolutize(path: &Path) -> DbResult<PathBuf> {
    let path = absolutize(path)?;
    fs::canonicalize(&path).or(Ok(path))
}

fn validate_positive_id(value: i64, name: &str) -> DbResult<()> {
    if value <= 0 {
        return Err(DbError::InvalidOperation(format!(
            "account recovery {name} must be positive, got {value}"
        )));
    }
    Ok(())
}

fn sqlite_table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            params![table_name],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

fn sqlite_table_has_column(
    connection: &Connection,
    table_name: &str,
    column_name: &str,
) -> DbResult<bool> {
    let sql = format!("PRAGMA table_info({})", quote_sqlite_identifier(table_name));
    let mut statement = connection.prepare(&sql)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(columns.contains(column_name))
}

fn quote_sqlite_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn sqlite_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => json!(value),
        ValueRef::Real(value) => Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).to_string()),
        ValueRef::Blob(value) => Value::String(format!("<blob:{}>", value.len())),
    }
}

fn required_string(row: &BTreeMap<String, Value>, key: &str) -> DbResult<String> {
    optional_string(row, key).ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "missing required SQLite account recovery column value {key}"
        ))
    })
}

fn recovered_budget_name(row: &BTreeMap<String, Value>, source_id: i64) -> String {
    if let Some(name) = optional_string(row, "name") {
        return name;
    }

    let category = optional_string(row, "category");
    let sub_category = optional_string(row, "sub_category");
    let category_path = [category.as_deref(), sub_category.as_deref()]
        .into_iter()
        .flatten()
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("/");
    let period_type = optional_string(row, "period_type");
    let start_date = optional_string(row, "start_date");

    match (
        category_path.is_empty(),
        period_type.as_deref(),
        start_date.as_deref(),
    ) {
        (false, Some(period), Some(start)) => format!("{category_path} {period} {start}"),
        (false, Some(period), None) => format!("{category_path} {period}"),
        (false, None, _) => category_path,
        (true, Some(period), Some(start)) => format!("budget:{source_id} {period} {start}"),
        (true, Some(period), None) => format!("budget:{source_id} {period}"),
        (true, None, _) => format!("budget:{source_id}"),
    }
}

fn optional_string(row: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    match row.get(key) {
        Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
        Some(Value::Number(value)) => Some(value.to_string()),
        Some(Value::Bool(value)) => Some(value.to_string()),
        _ => None,
    }
}

fn required_i64(row: &BTreeMap<String, Value>, key: &str) -> DbResult<i64> {
    optional_i64(row, key).ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "missing required SQLite account recovery integer column value {key}"
        ))
    })
}

fn optional_i64(row: &BTreeMap<String, Value>, key: &str) -> Option<i64> {
    match row.get(key) {
        Some(Value::Number(value)) => value
            .as_i64()
            .or_else(|| value.as_f64().map(|number| number as i64)),
        Some(Value::String(value)) => value.parse::<i64>().ok(),
        Some(Value::Bool(value)) => Some(i64::from(*value)),
        _ => None,
    }
}

fn optional_f64(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(value)) => value.as_f64(),
        Some(Value::String(value)) => value.parse::<f64>().ok(),
        Some(Value::Bool(value)) => Some(if *value { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn required_yuan_value_to_cents(value: Option<&Value>, field: &str) -> DbResult<i64> {
    let Some(value) = value else {
        return Err(DbError::InvalidOperation(format!(
            "account recovery required money field {field} is missing"
        )));
    };
    let raw = match value {
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Bool(_) | Value::Null | Value::Array(_) | Value::Object(_) => {
            return Err(DbError::InvalidOperation(format!(
                "account recovery required money field {field} is not a decimal value"
            )))
        }
    };
    decimal_yuan_to_cents(&raw, field)
}

fn decimal_yuan_to_cents(raw: &str, field: &str) -> DbResult<i64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(DbError::InvalidOperation(format!(
            "account recovery required money field {field} is empty"
        )));
    }
    if raw.contains(['e', 'E']) {
        return Err(DbError::InvalidOperation(format!(
            "account recovery required money field {field} must not use exponent notation"
        )));
    }
    let (negative, digits) = raw
        .strip_prefix('-')
        .map_or((false, raw), |stripped| (true, stripped));
    let digits = digits.strip_prefix('+').unwrap_or(digits);
    let mut parts = digits.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next().unwrap_or_default();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.chars().all(|character| character.is_ascii_digit())
        || !fraction.chars().all(|character| character.is_ascii_digit())
    {
        return Err(DbError::InvalidOperation(format!(
            "account recovery required money field {field} must be a decimal value"
        )));
    }
    let yuan = integer.parse::<i64>().map_err(|error| {
        DbError::InvalidOperation(format!(
            "account recovery required money field {field} cannot be parsed: {error}"
        ))
    })?;
    let mut cents = yuan.checked_mul(100).ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "account recovery required money field {field} overflows cents"
        ))
    })?;
    let mut fraction_digits = fraction.chars();
    let first = fraction_digits
        .next()
        .and_then(|digit| digit.to_digit(10))
        .unwrap_or(0) as i64;
    let second = fraction_digits
        .next()
        .and_then(|digit| digit.to_digit(10))
        .unwrap_or(0) as i64;
    cents = cents.checked_add(first * 10 + second).ok_or_else(|| {
        DbError::InvalidOperation(format!(
            "account recovery required money field {field} overflows cents"
        ))
    })?;
    if fraction_digits
        .next()
        .and_then(|digit| digit.to_digit(10))
        .is_some_and(|digit| digit >= 5)
    {
        cents = cents.checked_add(1).ok_or_else(|| {
            DbError::InvalidOperation(format!(
                "account recovery required money field {field} overflows cents"
            ))
        })?;
    }
    if negative {
        cents = cents.checked_neg().ok_or_else(|| {
            DbError::InvalidOperation(format!(
                "account recovery required money field {field} overflows cents"
            ))
        })?;
    }
    Ok(cents)
}

fn truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(value)) => value.as_i64().is_some_and(|value| value != 0),
        Some(Value::String(value)) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        _ => false,
    }
}

fn parse_aliases(value: Option<&str>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };
    if let Ok(parsed) = serde_json::from_str::<Vec<String>>(value) {
        return dedupe_non_empty(parsed);
    }
    dedupe_non_empty(
        value
            .split([',', ';', '|', '，', '；', '、', '\n', '\r', '\t'])
            .map(ToString::to_string)
            .collect(),
    )
}

fn dedupe_non_empty(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.to_ascii_lowercase()))
        .collect()
}

fn metadata_without(row: &BTreeMap<String, Value>, excluded_keys: &[&str]) -> Value {
    let excluded: BTreeSet<&str> = excluded_keys.iter().copied().collect();
    let metadata = row
        .iter()
        .filter(|(key, value)| !excluded.contains(key.as_str()) && !value.is_null())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<BTreeMap<_, _>>();
    json!(metadata)
}

fn bill_direction(bill_type: &str) -> &'static str {
    let normalized = bill_type.to_ascii_lowercase();
    if bill_type.contains('收') || normalized.contains("income") {
        "income"
    } else {
        "expense"
    }
}

fn bill_transaction_type(bill_type: &str) -> &'static str {
    let normalized = bill_type.to_ascii_lowercase();
    if bill_type.contains('转') || normalized.contains("transfer") {
        "transfer"
    } else if bill_type.contains('收') || normalized.contains("income") {
        "income"
    } else {
        "expense"
    }
}

fn default_timestamp() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

fn insert_i64(values: &mut BTreeMap<String, Value>, key: &str, value: i64) {
    values.insert(key.to_string(), json!(value));
}

fn insert_optional_i64(values: &mut BTreeMap<String, Value>, key: &str, value: Option<i64>) {
    values.insert(key.to_string(), value.map_or(Value::Null, Value::from));
}

fn insert_string(values: &mut BTreeMap<String, Value>, key: &str, value: impl Into<String>) {
    values.insert(key.to_string(), Value::String(value.into()));
}

fn insert_optional_string(values: &mut BTreeMap<String, Value>, key: &str, value: Option<String>) {
    values.insert(key.to_string(), value.map_or(Value::Null, Value::String));
}

fn insert_bool(values: &mut BTreeMap<String, Value>, key: &str, value: bool) {
    values.insert(key.to_string(), Value::Bool(value));
}

fn insert_number(values: &mut BTreeMap<String, Value>, key: &str, value: f64) {
    values.insert(
        key.to_string(),
        Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
    );
}

fn insert_json(values: &mut BTreeMap<String, Value>, key: &str, value: Value) {
    values.insert(key.to_string(), value);
}

fn checksum_json(value: &impl Serialize) -> DbResult<String> {
    let raw = serde_json::to_vec(value).map_err(json_error)?;
    Ok(hex_sha256(&raw))
}

fn hash_identity_parts(parts: &[String]) -> String {
    hash_text(&parts.join("\n"))
}

fn hash_text(value: &str) -> String {
    hex_sha256(value.as_bytes())
}

fn file_sha256(path: &Path) -> DbResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(hex_digest(&digest))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex_digest(&digest)
}

fn hex_digest(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn write_json(path: &Path, value: &impl Serialize) -> DbResult<()> {
    let raw = serde_json::to_string_pretty(value).map_err(json_error)?;
    fs::write(path, raw)?;
    Ok(())
}

fn absolutize(path: &Path) -> DbResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    Ok(std::env::current_dir()?.join(path))
}

fn json_error(error: serde_json::Error) -> DbError {
    DbError::InvalidOperation(format!("account recovery json error: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_inspection_filters_user_and_redacts_identity() {
        let connection = fixture_connection();

        let report = inspect_postgres_account_recovery_source_from_connection(
            &connection,
            "source-checksum".to_string(),
            5,
        )
        .unwrap();

        assert_eq!(report.source_user_id, 5);
        assert_eq!(report.counts.accounts, 1);
        assert_eq!(report.counts.bills, 1);
        assert_eq!(report.counts.categories, 1);
        assert_eq!(report.counts.generated_account_rules, 2);
        assert_eq!(report.counts.accounts_with_aliases, 1);
        assert_eq!(report.counts.settings_allowed, 1);
        assert_eq!(report.counts.settings_denied, 2);
        assert_ne!(report.source_identity_hash, "Cyansl0t");
        assert!(report.source_identity_hash.len() == 64);
    }

    #[test]
    fn dry_run_manifest_is_deterministic_and_records_target_counts() {
        let connection = fixture_connection();
        let target = AccountRecoveryTargetTriplet {
            user_id: 9,
            username_sha256: hash_text("target"),
            email_sha256: Some(hash_text("target@example.test")),
            auth_source_evidence: "postgres.users:password_hash_present".to_string(),
            triplet_hash: hash_text("triplet"),
        };
        let counts = AccountRecoveryTargetCounts {
            accounts: 3,
            ..AccountRecoveryTargetCounts::default()
        };

        let first = build_postgres_account_recovery_dry_run_from_connection(
            &connection,
            "source-checksum".to_string(),
            5,
            target.clone(),
            counts.clone(),
        )
        .unwrap();
        let second = build_postgres_account_recovery_dry_run_from_connection(
            &connection,
            "source-checksum".to_string(),
            5,
            target,
            counts,
        )
        .unwrap();

        assert_eq!(first.manifest_id, second.manifest_id);
        assert!(first.manifest_id.starts_with("account-recovery-"));
        assert_eq!(first.source.counts.budget_history, 2);
        assert_eq!(first.source.counts.budget_history_orphaned, 1);
        assert_eq!(first.target_pre_counts.accounts, 3);
        assert_eq!(first.id_plan.id_base, 9 * ACCOUNT_RECOVERY_ID_BLOCK_SIZE);
        assert!(first.table_allowlist.contains(&"account_rules".to_string()));
        assert!(first.table_denylist.contains(&"users".to_string()));
    }

    #[test]
    fn recovery_bundle_remaps_user_ids_amounts_and_alias_rules() {
        let connection = fixture_connection();

        let bundle =
            build_postgres_account_recovery_bundle_from_connection(&connection, 5, 9).unwrap();

        let accounts = table_export(&bundle, "accounts");
        assert_eq!(accounts.row_count, 1);
        assert_eq!(
            accounts.rows[0].values["id"],
            json!(9 * ACCOUNT_RECOVERY_ID_BLOCK_SIZE + 42)
        );
        assert_eq!(accounts.rows[0].values["user_id"], json!(9));
        assert_eq!(accounts.rows[0].values["balance_cents"], json!(12345));

        let bills = table_export(&bundle, "bills");
        assert_eq!(bills.row_count, 1);
        assert_eq!(bills.rows[0].values["amount_cents"], json!(1999));
        assert_eq!(
            bills.rows[0].values["account_id"],
            accounts.rows[0].values["id"]
        );

        let account_rules = table_export(&bundle, "account_rules");
        assert_eq!(account_rules.row_count, 2);
        assert!(account_rules
            .rows
            .iter()
            .all(|row| row.values["source"] == json!("sqlite_account_recovery")));
        assert!(account_rules
            .rows
            .iter()
            .any(|row| row.values["rule_expression"]["values"] == json!(["Cash"])));

        let category_rules = table_export(&bundle, "category_rules");
        assert_eq!(category_rules.row_count, 1);
        assert_eq!(category_rules.rows[0].values["user_id"], json!(9));

        let budgets = table_export(&bundle, "budgets");
        assert_eq!(budgets.row_count, 2);
        assert_eq!(budgets.rows[0].values["name"], json!("Lunch budget"));
        assert_eq!(
            budgets.rows[1].values["name"],
            json!("Food/Dinner monthly 2026-02-01")
        );

        let budget_history = table_export(&bundle, "budget_history");
        assert_eq!(budget_history.row_count, 1);
        assert_eq!(
            budget_history.rows[0].values["spent_amount_cents"],
            json!(12000)
        );
    }

    #[test]
    fn setting_policy_is_default_deny_for_sensitive_keys() {
        assert!(account_recovery_setting_allowed("default_currency", false));
        assert!(!account_recovery_setting_allowed(
            "receipt_ocr_config",
            false
        ));
        assert!(!account_recovery_setting_allowed("provider_api_key", false));
        assert!(!account_recovery_setting_allowed("theme", true));
    }

    #[test]
    fn guard_edges_reject_missing_users_bad_ids_and_unignored_snapshots() {
        assert!(AccountRecoveryTargetRef::new("   ")
            .unwrap_err()
            .to_string()
            .contains("target user ref is required"));

        let connection = fixture_connection();
        assert!(inspect_postgres_account_recovery_source_from_connection(
            &connection,
            "source-checksum".to_string(),
            0,
        )
        .unwrap_err()
        .to_string()
        .contains("source_user_id must be positive"));
        assert!(inspect_postgres_account_recovery_source_from_connection(
            &connection,
            "source-checksum".to_string(),
            404,
        )
        .unwrap_err()
        .to_string()
        .contains("source user 404 was not found"));

        let no_users = Connection::open_in_memory().unwrap();
        assert!(inspect_postgres_account_recovery_source_from_connection(
            &no_users,
            "source-checksum".to_string(),
            5,
        )
        .unwrap_err()
        .to_string()
        .contains("users table is missing"));

        let temp_dir = tempfile::tempdir().unwrap();
        assert!(ensure_snapshot_dir_allowed(temp_dir.path(), Path::new(".git/ai/snap")).is_ok());
        assert!(
            ensure_snapshot_dir_allowed(temp_dir.path(), Path::new(".git/../docs/recovery"))
                .unwrap_err()
                .to_string()
                .contains("parent-directory")
        );
        assert!(
            ensure_snapshot_dir_allowed(temp_dir.path(), Path::new("snapshots/raw"))
                .unwrap_err()
                .to_string()
                .contains("not git-ignored")
        );
    }

    #[test]
    fn source_reader_edges_handle_missing_and_legacy_budget_history_shapes() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, email TEXT);
                CREATE TABLE budgets (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, name TEXT);
                CREATE TABLE budget_history (id INTEGER PRIMARY KEY, budget_id INTEGER NOT NULL);
                CREATE TABLE accounts (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
                INSERT INTO users VALUES (5, 'source', NULL);
                INSERT INTO budgets VALUES (10, 5, 'Budget');
                INSERT INTO budget_history VALUES (11, 10);
                INSERT INTO accounts VALUES (1, 'missing user id');
                "#,
            )
            .unwrap();

        let history = read_budget_history_rows(&connection, 5).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["id"], json!(11));
        assert!(read_bill_tag_rows(&connection, 5).unwrap().is_empty());
        assert!(read_user_rows(&connection, "accounts", 5)
            .unwrap_err()
            .to_string()
            .contains("has no user_id column"));
    }

    #[test]
    fn value_helpers_cover_scalar_and_empty_edges() {
        let mut row = BTreeMap::new();
        row.insert("number".to_string(), json!(42));
        row.insert("bool".to_string(), json!(true));
        row.insert("float".to_string(), json!(12.345));
        row.insert("empty".to_string(), Value::String(String::new()));
        assert_eq!(optional_string(&row, "number"), Some("42".to_string()));
        assert_eq!(optional_string(&row, "bool"), Some("true".to_string()));
        assert_eq!(optional_string(&row, "empty"), None);
        assert_eq!(optional_i64(&row, "float"), Some(12));
        assert_eq!(optional_i64(&row, "bool"), Some(1));
        assert_eq!(optional_f64(row.get("bool")), Some(1.0));
        assert_eq!(
            required_yuan_value_to_cents(row.get("float"), "test.amount").unwrap(),
            1235
        );
        assert_eq!(
            decimal_yuan_to_cents("-1.235", "test.amount").unwrap(),
            -124
        );
        assert!(required_yuan_value_to_cents(None, "test.amount")
            .unwrap_err()
            .to_string()
            .contains("is missing"));
        assert!(decimal_yuan_to_cents("1e2", "test.amount")
            .unwrap_err()
            .to_string()
            .contains("exponent"));
        assert!(decimal_yuan_to_cents("abc", "test.amount")
            .unwrap_err()
            .to_string()
            .contains("decimal"));
        assert!(truthy(row.get("bool")));
        assert!(parse_aliases(None).is_empty());
        assert_eq!(
            parse_aliases(Some(" Cash; cash | 零钱\n")),
            vec!["Cash".to_string(), "零钱".to_string()]
        );
        assert_eq!(recovered_budget_name(&row, 7), "budget:7".to_string());
        row.insert("period_type".to_string(), json!("yearly"));
        row.insert("start_date".to_string(), json!("2026-01-01"));
        assert_eq!(
            recovered_budget_name(&row, 7),
            "budget:7 yearly 2026-01-01".to_string()
        );
        row.remove("start_date");
        assert_eq!(
            recovered_budget_name(&row, 7),
            "budget:7 yearly".to_string()
        );
        row.insert("category".to_string(), json!("Food"));
        row.insert("sub_category".to_string(), json!("Dinner"));
        assert_eq!(
            recovered_budget_name(&row, 7),
            "Food/Dinner yearly".to_string()
        );
        row.remove("period_type");
        assert_eq!(recovered_budget_name(&row, 7), "Food/Dinner".to_string());
        row.insert("name".to_string(), json!("Custom budget"));
        assert_eq!(recovered_budget_name(&row, 7), "Custom budget".to_string());
        assert!(required_string(&row, "missing")
            .unwrap_err()
            .to_string()
            .contains("missing required"));
        assert!(required_i64(&row, "missing")
            .unwrap_err()
            .to_string()
            .contains("missing required"));
        let mut values = BTreeMap::new();
        insert_number(&mut values, "nan", f64::NAN);
        assert_eq!(values["nan"], Value::Null);
        assert_eq!(bill_direction("收入"), "income");
        assert_eq!(bill_transaction_type("转账"), "transfer");
        assert_eq!(bill_transaction_type("income"), "income");
    }

    fn table_export<'a>(
        bundle: &'a SqliteToPostgresExportBundle,
        target_table: &str,
    ) -> &'a SqliteToPostgresTableExport {
        bundle
            .tables
            .iter()
            .find(|table| table.target_table == target_table)
            .unwrap()
    }

    fn fixture_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL,
                    email TEXT
                );
                CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    type INTEGER NOT NULL,
                    category TEXT,
                    currency TEXT,
                    balance REAL,
                    aliases TEXT,
                    hidden INTEGER,
                    display_order INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    type INTEGER,
                    main_category TEXT,
                    sub_category TEXT,
                    priority INTEGER,
                    hidden INTEGER,
                    icon TEXT,
                    color TEXT,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE tags (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    color TEXT,
                    display_order INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE bills (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    date TEXT NOT NULL,
                    type TEXT NOT NULL,
                    amount REAL NOT NULL,
                    counterparty TEXT,
                    description TEXT,
                    payment_method TEXT,
                    main_category TEXT,
                    sub_category TEXT,
                    hash TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    source_account_id INTEGER,
                    destination_account_id INTEGER
                );
                CREATE TABLE bill_tags (
                    bill_id INTEGER NOT NULL,
                    tag_id INTEGER NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE budgets (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    category TEXT,
                    sub_category TEXT,
                    period_type TEXT NOT NULL,
                    amount REAL NOT NULL,
                    start_date TEXT NOT NULL,
                    end_date TEXT,
                    alert_threshold INTEGER,
                    enabled INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE budget_history (
                    id INTEGER PRIMARY KEY,
                    budget_id INTEGER NOT NULL,
                    user_id INTEGER NOT NULL,
                    period_start TEXT NOT NULL,
                    period_end TEXT NOT NULL,
                    budget_amount REAL,
                    spent_amount REAL,
                    remaining_amount REAL,
                    execution_rate REAL,
                    status TEXT,
                    calculated_at TEXT,
                    filter_summary TEXT
                );
                CREATE TABLE category_rules (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    category_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    priority INTEGER,
                    rule_expression TEXT,
                    regex_enabled INTEGER,
                    enabled INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE app_settings (
                    id INTEGER PRIMARY KEY,
                    key TEXT NOT NULL,
                    value TEXT,
                    is_encrypted INTEGER
                );

                INSERT INTO users VALUES (5, 'Cyansl0t', 'hcy84872684@example.test');
                INSERT INTO users VALUES (6, 'other', 'other@example.test');
                INSERT INTO accounts VALUES (42, 5, 'Wallet', 1, 'cash', 'CNY', 123.45, '["Cash", "零钱"]', 0, 7, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO accounts VALUES (43, 6, 'Other Wallet', 1, 'cash', 'CNY', 999.99, '["Other"]', 0, 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO categories VALUES (50, 5, 1, 'Food', 'Lunch', 1, 0, 'utensils', '#fff', '2026-01-01T00:00:00Z');
                INSERT INTO tags VALUES (60, 5, 'work', '#336699', 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO bills VALUES (70, 5, '2026-01-03T12:00:00Z', '支出', 19.99, 'Cafe', 'Lunch', 'Wallet', 'Food', 'Lunch', 'hash70', '2026-01-03T12:01:00Z', '2026-01-03T12:02:00Z', 42, 0);
                INSERT INTO bills VALUES (71, 6, '2026-01-03T12:00:00Z', '支出', 99.99, 'Other', 'Other', 'Other', 'Food', 'Lunch', 'hash71', '2026-01-03T12:01:00Z', '2026-01-03T12:02:00Z', 43, 0);
                INSERT INTO bill_tags VALUES (70, 60, '2026-01-03T12:03:00Z');
                INSERT INTO budgets VALUES (80, 5, 'Lunch budget', 'Food', 'Lunch', 'monthly', 250.0, '2026-01-01', '2026-01-31', 80, 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO budgets VALUES (81, 5, '', 'Food', 'Dinner', 'monthly', 350.0, '2026-02-01', '2026-02-28', 80, 1, '2026-02-01T00:00:00Z', '2026-02-02T00:00:00Z');
                INSERT INTO budget_history VALUES (90, 80, 5, '2026-01-01', '2026-01-31', 250.0, 120.0, 130.0, 48.0, 'within_budget', '2026-02-01T00:00:00Z', 'Food/Lunch');
                INSERT INTO budget_history VALUES (91, 999, 5, '2026-01-01', '2026-01-31', 250.0, 10.0, 240.0, 4.0, 'within_budget', '2026-02-01T00:00:00Z', 'orphan');
                INSERT INTO category_rules VALUES (100, 5, 50, 'Cafe rule', 10, 'Cafe', 0, 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO app_settings VALUES (1, 'default_currency', 'CNY', 0);
                INSERT INTO app_settings VALUES (2, 'receipt_ocr_config', '{}', 0);
                INSERT INTO app_settings VALUES (3, 'provider_api_key', 'secret', 1);
                "#,
            )
            .unwrap();
        connection
    }
}
