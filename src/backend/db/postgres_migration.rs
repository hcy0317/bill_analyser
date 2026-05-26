use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use rusqlite::{types::ValueRef, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Number, Value};
use sha2::{Digest, Sha256};
use sqlx::{types::Json, Postgres, Transaction};

use crate::{DbError, DbResult, PostgresPool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationTableStatus {
    Ready,
    MissingTable,
    MissingColumns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteToPostgresTableReport {
    pub source_table: String,
    pub target_table: String,
    pub status: MigrationTableStatus,
    pub row_count: u64,
    pub expected_columns: Vec<String>,
    pub present_columns: Vec<String>,
    pub missing_columns: Vec<String>,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteToPostgresDryRunReport {
    pub schema_version: u16,
    pub table_count: usize,
    pub ready_table_count: usize,
    pub total_rows: u64,
    pub checksum: String,
    pub tables: Vec<SqliteToPostgresTableReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostgresTargetRow {
    pub source_table: String,
    pub target_table: String,
    pub source_id: Option<i64>,
    pub values: BTreeMap<String, Value>,
    pub checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteToPostgresTableExport {
    pub source_table: String,
    pub target_table: String,
    pub row_count: usize,
    pub checksum: String,
    pub rows: Vec<PostgresTargetRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteToPostgresExportBundle {
    pub schema_version: u16,
    pub table_count: usize,
    pub total_rows: usize,
    pub checksum: String,
    pub tables: Vec<SqliteToPostgresTableExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SqliteToPostgresImportCheckReport {
    pub table_count: usize,
    pub imported_rows: usize,
    pub checksum: String,
    pub table_checksums: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy)]
struct MigrationTableSpec {
    source_table: &'static str,
    target_table: &'static str,
    expected_columns: &'static [&'static str],
}

const MIGRATION_SCHEMA_VERSION: u16 = 1;
const SQLITE_TO_POSTGRES_MIGRATION_NAME: &str = "sqlite_to_postgres_authoritative_import_v1";

const MIGRATION_TABLE_SPECS: &[MigrationTableSpec] = &[
    MigrationTableSpec {
        source_table: "users",
        target_table: "users",
        expected_columns: &[
            "id",
            "username",
            "email",
            "password_hash",
            "created_at",
            "updated_at",
        ],
    },
    MigrationTableSpec {
        source_table: "accounts",
        target_table: "accounts",
        expected_columns: &[
            "id",
            "user_id",
            "name",
            "type",
            "balance",
            "aliases",
            "created_at",
            "updated_at",
        ],
    },
    MigrationTableSpec {
        source_table: "categories",
        target_table: "categories",
        expected_columns: &[
            "id",
            "user_id",
            "type",
            "main_category",
            "sub_category",
            "created_at",
        ],
    },
    MigrationTableSpec {
        source_table: "tags",
        target_table: "tags",
        expected_columns: &["id", "user_id", "name", "created_at", "updated_at"],
    },
    MigrationTableSpec {
        source_table: "bills",
        target_table: "bills",
        expected_columns: &[
            "id",
            "user_id",
            "date",
            "type",
            "amount",
            "counterparty",
            "description",
            "created_at",
            "updated_at",
        ],
    },
    MigrationTableSpec {
        source_table: "app_settings",
        target_table: "settings",
        expected_columns: &[
            "id",
            "key",
            "value",
            "value_type",
            "updated_at",
            "created_at",
        ],
    },
    MigrationTableSpec {
        source_table: "bills_parser_template",
        target_table: "parser_templates",
        expected_columns: &[
            "id",
            "session_id",
            "user_id",
            "parser_date",
            "parser_amount",
            "parser_type",
            "parser_id",
            "created_at",
        ],
    },
    MigrationTableSpec {
        source_table: "category_rules",
        target_table: "category_rules",
        expected_columns: &[
            "id",
            "user_id",
            "category_id",
            "name",
            "priority",
            "rule_expression",
            "enabled",
            "created_at",
            "updated_at",
        ],
    },
];

const POSTGRES_TARGET_TABLES: &[&str] = &[
    "users",
    "accounts",
    "categories",
    "tags",
    "bills",
    "settings",
    "parser_templates",
    "category_rules",
    "account_rules",
];

pub fn sqlite_to_postgres_dry_run(
    sqlite_path: impl AsRef<Path>,
) -> DbResult<SqliteToPostgresDryRunReport> {
    let connection = Connection::open(sqlite_path)?;
    sqlite_to_postgres_dry_run_from_connection(&connection)
}

pub fn sqlite_to_postgres_dry_run_from_connection(
    connection: &Connection,
) -> DbResult<SqliteToPostgresDryRunReport> {
    let mut tables = Vec::with_capacity(MIGRATION_TABLE_SPECS.len());
    for spec in MIGRATION_TABLE_SPECS {
        tables.push(build_table_report(connection, spec)?);
    }

    let ready_table_count = tables
        .iter()
        .filter(|table| table.status == MigrationTableStatus::Ready)
        .count();
    let total_rows = tables.iter().map(|table| table.row_count).sum();
    let checksum = checksum_json(&tables)?;

    Ok(SqliteToPostgresDryRunReport {
        schema_version: MIGRATION_SCHEMA_VERSION,
        table_count: tables.len(),
        ready_table_count,
        total_rows,
        checksum,
        tables,
    })
}

pub fn export_sqlite_to_postgres_bundle(
    sqlite_path: impl AsRef<Path>,
) -> DbResult<SqliteToPostgresExportBundle> {
    let connection = Connection::open(sqlite_path)?;
    export_sqlite_to_postgres_bundle_from_connection(&connection)
}

pub fn export_sqlite_to_postgres_bundle_from_connection(
    connection: &Connection,
) -> DbResult<SqliteToPostgresExportBundle> {
    let mut exports = Vec::new();
    for spec in MIGRATION_TABLE_SPECS {
        let report = build_table_report(connection, spec)?;
        if report.status != MigrationTableStatus::Ready {
            continue;
        }
        exports.extend(export_table(connection, spec)?);
    }
    exports.sort_by_key(|table| postgres_target_table_rank(&table.target_table));

    let total_rows = exports.iter().map(|table| table.row_count).sum();
    let checksum = checksum_json(&exports)?;
    Ok(SqliteToPostgresExportBundle {
        schema_version: MIGRATION_SCHEMA_VERSION,
        table_count: exports.len(),
        total_rows,
        checksum,
        tables: exports,
    })
}

pub trait PostgresImportSink {
    fn import_table(&mut self, table: &SqliteToPostgresTableExport) -> DbResult<usize>;
}

pub fn import_postgres_bundle_to_sink(
    bundle: &SqliteToPostgresExportBundle,
    sink: &mut impl PostgresImportSink,
) -> DbResult<SqliteToPostgresImportCheckReport> {
    let mut imported_rows = 0;
    let mut table_checksums = BTreeMap::new();

    for table in &bundle.tables {
        let written = sink.import_table(table)?;
        if written != table.row_count {
            return Err(DbError::InvalidOperation(format!(
                "Postgres import sink wrote {written} rows for {} but expected {}",
                table.target_table, table.row_count
            )));
        }
        imported_rows += written;
        table_checksums.insert(table.target_table.clone(), table.checksum.clone());
    }

    let checksum = checksum_json(&table_checksums)?;
    Ok(SqliteToPostgresImportCheckReport {
        table_count: bundle.tables.len(),
        imported_rows,
        checksum,
        table_checksums,
    })
}

pub async fn import_postgres_bundle_to_postgres(
    pool: &PostgresPool,
    bundle: &SqliteToPostgresExportBundle,
) -> DbResult<SqliteToPostgresImportCheckReport> {
    import_postgres_bundle_to_postgres_with_name(pool, bundle, SQLITE_TO_POSTGRES_MIGRATION_NAME)
        .await
}

pub async fn import_postgres_bundle_to_postgres_with_name(
    pool: &PostgresPool,
    bundle: &SqliteToPostgresExportBundle,
    migration_name: &str,
) -> DbResult<SqliteToPostgresImportCheckReport> {
    let mut transaction = pool.begin().await.map_err(postgres_error)?;
    insert_migration_audit_event(
        &mut transaction,
        migration_name,
        "import",
        "started",
        json!({
            "schema_version": bundle.schema_version,
            "table_count": bundle.table_count,
            "total_rows": bundle.total_rows,
            "checksum": bundle.checksum,
        }),
    )
    .await?;

    let result = import_postgres_bundle_in_transaction(&mut transaction, bundle).await;
    match result {
        Ok(report) => {
            insert_migration_audit_event(
                &mut transaction,
                migration_name,
                "import",
                "succeeded",
                json!({
                    "table_count": report.table_count,
                    "imported_rows": report.imported_rows,
                    "checksum": report.checksum,
                    "table_checksums": report.table_checksums,
                }),
            )
            .await?;
            transaction.commit().await.map_err(postgres_error)?;
            Ok(report)
        }
        Err(error) => {
            let _ = transaction.rollback().await;
            let audit_payload = json!({
                "schema_version": bundle.schema_version,
                "table_count": bundle.table_count,
                "total_rows": bundle.total_rows,
                "checksum": bundle.checksum,
                "error": error.to_string(),
            });
            let _ = insert_migration_audit_event_pool(
                pool,
                migration_name,
                "import",
                "failed_retryable",
                audit_payload,
            )
            .await;
            Err(error)
        }
    }
}

async fn import_postgres_bundle_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    bundle: &SqliteToPostgresExportBundle,
) -> DbResult<SqliteToPostgresImportCheckReport> {
    let mut imported_rows = 0;
    let mut table_checksums = BTreeMap::new();

    for table in &bundle.tables {
        validate_postgres_table_export(table)?;
        let mut written = 0;
        for row in &table.rows {
            validate_postgres_target_row(table, row)?;
            written += insert_postgres_target_row(transaction, row).await?;
        }
        if written != table.row_count {
            return Err(DbError::InvalidOperation(format!(
                "Postgres import wrote {written} rows for {} but expected {}",
                table.target_table, table.row_count
            )));
        }
        refresh_postgres_identity_sequence(transaction, &table.target_table).await?;
        imported_rows += written;
        table_checksums.insert(table.target_table.clone(), table.checksum.clone());
    }

    let checksum = checksum_json(&table_checksums)?;
    Ok(SqliteToPostgresImportCheckReport {
        table_count: bundle.tables.len(),
        imported_rows,
        checksum,
        table_checksums,
    })
}

pub fn load_sqlite_to_postgres_bundle_json(
    path: impl AsRef<Path>,
) -> DbResult<SqliteToPostgresExportBundle> {
    let raw = fs::read_to_string(path)?;
    serde_json::from_str(&raw).map_err(|error| {
        DbError::InvalidOperation(format!("invalid sqlite-to-postgres bundle json: {error}"))
    })
}

pub fn write_sqlite_to_postgres_json<T: Serialize>(
    path: impl AsRef<Path>,
    value: &T,
) -> DbResult<()> {
    let raw = serde_json::to_string_pretty(value).map_err(|error| {
        DbError::InvalidOperation(format!("serialize sqlite-to-postgres json: {error}"))
    })?;
    fs::write(path, raw)?;
    Ok(())
}

async fn insert_postgres_target_row(
    transaction: &mut Transaction<'_, Postgres>,
    row: &PostgresTargetRow,
) -> DbResult<usize> {
    let sql = postgres_insert_sql(row)?;
    let row_json = Value::Object(row.values.clone().into_iter().collect());
    let result = sqlx::query(&sql)
        .bind(Json(row_json))
        .execute(&mut **transaction)
        .await
        .map_err(postgres_error)?;
    Ok(result.rows_affected() as usize)
}

async fn refresh_postgres_identity_sequence(
    transaction: &mut Transaction<'_, Postgres>,
    table_name: &str,
) -> DbResult<()> {
    if !POSTGRES_TARGET_TABLES.contains(&table_name) {
        return Err(DbError::InvalidOperation(format!(
            "unsupported PostgreSQL migration target table {table_name}"
        )));
    }
    let table_identifier = quote_postgres_identifier(table_name)?;
    let sql = format!(
        "SELECT setval(pg_get_serial_sequence($1, 'id'), COALESCE((SELECT MAX(id) FROM {table_identifier}), 1), (SELECT MAX(id) FROM {table_identifier}) IS NOT NULL)"
    );
    sqlx::query(&sql)
        .bind(table_name)
        .execute(&mut **transaction)
        .await
        .map_err(postgres_error)?;
    Ok(())
}

async fn insert_migration_audit_event(
    transaction: &mut Transaction<'_, Postgres>,
    migration_name: &str,
    stage: &str,
    status: &str,
    payload: Value,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO migration_audit_events (migration_name, stage, status, payload) VALUES ($1, $2, $3, $4)",
    )
    .bind(migration_name)
    .bind(stage)
    .bind(status)
    .bind(Json(payload))
    .execute(&mut **transaction)
    .await
    .map_err(postgres_error)?;
    Ok(())
}

async fn insert_migration_audit_event_pool(
    pool: &PostgresPool,
    migration_name: &str,
    stage: &str,
    status: &str,
    payload: Value,
) -> DbResult<()> {
    sqlx::query(
        "INSERT INTO migration_audit_events (migration_name, stage, status, payload) VALUES ($1, $2, $3, $4)",
    )
    .bind(migration_name)
    .bind(stage)
    .bind(status)
    .bind(Json(payload))
    .execute(pool)
    .await
    .map_err(postgres_error)?;
    Ok(())
}

fn validate_postgres_table_export(table: &SqliteToPostgresTableExport) -> DbResult<()> {
    if table.rows.len() != table.row_count {
        return Err(DbError::InvalidOperation(format!(
            "Postgres migration table {} declares {} rows but contains {} rows",
            table.target_table,
            table.row_count,
            table.rows.len()
        )));
    }
    if !POSTGRES_TARGET_TABLES.contains(&table.target_table.as_str()) {
        return Err(DbError::InvalidOperation(format!(
            "unsupported PostgreSQL migration target table {}",
            table.target_table
        )));
    }
    Ok(())
}

fn validate_postgres_target_row(
    table: &SqliteToPostgresTableExport,
    row: &PostgresTargetRow,
) -> DbResult<()> {
    if row.target_table != table.target_table {
        return Err(DbError::InvalidOperation(format!(
            "Postgres migration row target {} does not match table {}",
            row.target_table, table.target_table
        )));
    }
    if row.values.is_empty() {
        return Err(DbError::InvalidOperation(format!(
            "Postgres migration row for {} has no values",
            row.target_table
        )));
    }
    let checksum = checksum_json(&row.values)?;
    if checksum != row.checksum {
        return Err(DbError::InvalidOperation(format!(
            "Postgres migration row checksum mismatch for {} source {:?}",
            row.target_table, row.source_id
        )));
    }
    Ok(())
}

fn postgres_insert_sql(row: &PostgresTargetRow) -> DbResult<String> {
    if !POSTGRES_TARGET_TABLES.contains(&row.target_table.as_str()) {
        return Err(DbError::InvalidOperation(format!(
            "unsupported PostgreSQL migration target table {}",
            row.target_table
        )));
    }
    let table_identifier = quote_postgres_identifier(&row.target_table)?;
    let columns = row
        .values
        .keys()
        .map(|column| quote_postgres_identifier(column))
        .collect::<DbResult<Vec<_>>>()?;
    if columns.is_empty() {
        return Err(DbError::InvalidOperation(format!(
            "Postgres migration row for {} has no columns",
            row.target_table
        )));
    }
    let column_list = columns.join(", ");
    Ok(format!(
        "INSERT INTO {table_identifier} ({column_list}) SELECT {column_list} FROM jsonb_populate_record(NULL::{table_identifier}, $1::jsonb)"
    ))
}

fn quote_postgres_identifier(identifier: &str) -> DbResult<String> {
    if !is_safe_postgres_identifier(identifier) {
        return Err(DbError::InvalidOperation(format!(
            "unsafe PostgreSQL identifier in migration import: {identifier}"
        )));
    }
    Ok(format!("\"{}\"", identifier.replace('"', "\"\"")))
}

fn is_safe_postgres_identifier(identifier: &str) -> bool {
    let mut chars = identifier.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_lowercase())
        && chars.all(|character| {
            character == '_' || character.is_ascii_lowercase() || character.is_ascii_digit()
        })
}

fn postgres_error(error: sqlx::Error) -> DbError {
    DbError::InvalidOperation(format!("postgres migration error: {error}"))
}

fn postgres_target_table_rank(table_name: &str) -> usize {
    POSTGRES_TARGET_TABLES
        .iter()
        .position(|target_table| *target_table == table_name)
        .unwrap_or(usize::MAX)
}

fn build_table_report(
    connection: &Connection,
    spec: &MigrationTableSpec,
) -> DbResult<SqliteToPostgresTableReport> {
    if !sqlite_table_exists(connection, spec.source_table)? {
        return Ok(SqliteToPostgresTableReport {
            source_table: spec.source_table.to_string(),
            target_table: spec.target_table.to_string(),
            status: MigrationTableStatus::MissingTable,
            row_count: 0,
            expected_columns: spec
                .expected_columns
                .iter()
                .map(|column| (*column).to_string())
                .collect(),
            present_columns: Vec::new(),
            missing_columns: spec
                .expected_columns
                .iter()
                .map(|column| (*column).to_string())
                .collect(),
            checksum: checksum_json(&Vec::<String>::new())?,
        });
    }

    let present_columns = sqlite_table_columns(connection, spec.source_table)?;
    let present_set: BTreeSet<_> = present_columns.iter().cloned().collect();
    let missing_columns: Vec<String> = spec
        .expected_columns
        .iter()
        .filter(|column| !present_set.contains(**column))
        .map(|column| (*column).to_string())
        .collect();
    let rows = read_sqlite_rows(connection, spec.source_table)?;
    let checksum = checksum_json(&rows)?;

    Ok(SqliteToPostgresTableReport {
        source_table: spec.source_table.to_string(),
        target_table: spec.target_table.to_string(),
        status: if missing_columns.is_empty() {
            MigrationTableStatus::Ready
        } else {
            MigrationTableStatus::MissingColumns
        },
        row_count: rows.len() as u64,
        expected_columns: spec
            .expected_columns
            .iter()
            .map(|column| (*column).to_string())
            .collect(),
        present_columns,
        missing_columns,
        checksum,
    })
}

fn export_table(
    connection: &Connection,
    spec: &MigrationTableSpec,
) -> DbResult<Vec<SqliteToPostgresTableExport>> {
    let rows = read_sqlite_rows(connection, spec.source_table)?;
    let mut grouped: BTreeMap<String, Vec<PostgresTargetRow>> = BTreeMap::new();

    for row in rows {
        for target_row in map_source_row(spec.source_table, spec.target_table, &row)? {
            grouped
                .entry(target_row.target_table.clone())
                .or_default()
                .push(target_row);
        }
    }

    grouped
        .into_iter()
        .map(|(target_table, rows)| {
            let checksum = checksum_json(&rows)?;
            Ok(SqliteToPostgresTableExport {
                source_table: spec.source_table.to_string(),
                target_table,
                row_count: rows.len(),
                checksum,
                rows,
            })
        })
        .collect()
}

fn map_source_row(
    source_table: &str,
    target_table: &str,
    row: &BTreeMap<String, Value>,
) -> DbResult<Vec<PostgresTargetRow>> {
    let mut mapped_rows = match source_table {
        "users" => vec![target_row(
            source_table,
            target_table,
            row,
            map_user_row(row)?,
        )?],
        "accounts" => {
            let mut rows = vec![target_row(
                source_table,
                target_table,
                row,
                map_account_row(row)?,
            )?];
            rows.extend(map_account_alias_rules(source_table, row)?);
            rows
        }
        "categories" => vec![target_row(
            source_table,
            target_table,
            row,
            map_category_row(row)?,
        )?],
        "tags" => vec![target_row(
            source_table,
            target_table,
            row,
            map_tag_row(row)?,
        )?],
        "bills" => vec![target_row(
            source_table,
            target_table,
            row,
            map_bill_row(row)?,
        )?],
        "app_settings" => vec![target_row(
            source_table,
            target_table,
            row,
            map_setting_row(row)?,
        )?],
        "bills_parser_template" => {
            vec![target_row(
                source_table,
                target_table,
                row,
                map_parser_template_row(row)?,
            )?]
        }
        "category_rules" => vec![target_row(
            source_table,
            target_table,
            row,
            map_category_rule_row(row)?,
        )?],
        _ => Vec::new(),
    };
    mapped_rows.sort_by(|left, right| {
        left.target_table
            .cmp(&right.target_table)
            .then(left.source_id.cmp(&right.source_id))
            .then(left.checksum.cmp(&right.checksum))
    });
    Ok(mapped_rows)
}

fn map_user_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    let id = required_i64(row, "id")?;
    insert_i64(&mut values, "id", id);
    insert_i64(&mut values, "legacy_id", id);
    insert_string(&mut values, "username", required_string(row, "username")?);
    insert_string(
        &mut values,
        "email",
        optional_string(row, "email").unwrap_or_default(),
    );
    insert_optional_string(
        &mut values,
        "display_name",
        optional_string(row, "nickname"),
    );
    insert_string(
        &mut values,
        "password_hash",
        optional_string(row, "password_hash").unwrap_or_default(),
    );
    insert_json(
        &mut values,
        "metadata",
        metadata_without(
            row,
            &[
                "id",
                "username",
                "email",
                "nickname",
                "password_hash",
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
    Ok(values)
}

fn map_account_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    let id = required_i64(row, "id")?;
    insert_i64(&mut values, "id", id);
    insert_i64(&mut values, "legacy_id", id);
    insert_i64(
        &mut values,
        "user_id",
        optional_i64(row, "user_id").unwrap_or(1),
    );
    insert_string(&mut values, "name", required_string(row, "name")?);
    insert_string(
        &mut values,
        "account_type",
        optional_i64(row, "type").unwrap_or_default().to_string(),
    );
    insert_optional_string(&mut values, "currency", optional_string(row, "currency"));
    insert_i64(
        &mut values,
        "balance_cents",
        yuan_value_to_cents(row.get("balance")),
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
                "currency",
                "balance",
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
    Ok(values)
}

fn map_account_alias_rules(
    source_table: &str,
    row: &BTreeMap<String, Value>,
) -> DbResult<Vec<PostgresTargetRow>> {
    let aliases = parse_aliases(optional_string(row, "aliases").as_deref());
    let account_id = required_i64(row, "id")?;
    let user_id = optional_i64(row, "user_id").unwrap_or(1);
    let created_at = optional_string(row, "created_at").unwrap_or_else(default_timestamp);
    let updated_at = optional_string(row, "updated_at").unwrap_or_else(default_timestamp);

    aliases
        .into_iter()
        .map(|alias| {
            let mut values = BTreeMap::new();
            insert_i64(&mut values, "user_id", user_id);
            insert_i64(&mut values, "account_id", account_id);
            insert_string(&mut values, "name", format!("legacy alias: {alias}"));
            insert_string(&mut values, "account_role_scope", "any");
            insert_string(&mut values, "transaction_type_scope", "all");
            insert_json(&mut values, "field_scope", json!(["counterparty", "payment_method", "description", "parser"]));
            insert_json(
                &mut values,
                "rule_expression",
                json!({"operator":"contains_any","values":[alias],"source":"legacy_account_aliases"}),
            );
            insert_bool(&mut values, "regex_enabled", false);
            insert_i64(&mut values, "priority", 1000);
            insert_bool(&mut values, "enabled", true);
            insert_string(&mut values, "source", "legacy_account_aliases");
            insert_string(
                &mut values,
                "source_key",
                format!("legacy_alias:{}", alias.trim().to_ascii_lowercase()),
            );
            insert_i64(&mut values, "match_count", 0);
            insert_string(&mut values, "created_at", created_at.clone());
            insert_string(&mut values, "updated_at", updated_at.clone());
            insert_i64(&mut values, "version", 1);
            target_row(source_table, "account_rules", row, values)
        })
        .collect()
}

fn map_category_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    let id = required_i64(row, "id")?;
    let main = optional_string(row, "main_category").unwrap_or_default();
    let sub = optional_string(row, "sub_category").unwrap_or_default();
    insert_i64(&mut values, "id", id);
    insert_i64(&mut values, "legacy_id", id);
    insert_i64(
        &mut values,
        "user_id",
        optional_i64(row, "user_id").unwrap_or(1),
    );
    insert_optional_i64(&mut values, "parent_id", None);
    insert_string(
        &mut values,
        "name",
        if sub.is_empty() {
            main.clone()
        } else {
            sub.clone()
        },
    );
    insert_string(
        &mut values,
        "category_type",
        optional_i64(row, "type").unwrap_or_default().to_string(),
    );
    insert_string(
        &mut values,
        "path",
        [main.as_str(), sub.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("/"),
    );
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
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    Ok(values)
}

fn map_tag_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    let id = required_i64(row, "id")?;
    insert_i64(&mut values, "id", id);
    insert_i64(&mut values, "legacy_id", id);
    insert_i64(
        &mut values,
        "user_id",
        optional_i64(row, "user_id").unwrap_or(1),
    );
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
    Ok(values)
}

fn map_bill_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    let id = required_i64(row, "id")?;
    let bill_type = optional_string(row, "type").unwrap_or_default();
    insert_i64(&mut values, "id", id);
    insert_i64(&mut values, "legacy_id", id);
    insert_i64(
        &mut values,
        "user_id",
        optional_i64(row, "user_id").unwrap_or(1),
    );
    insert_string(&mut values, "occurred_at", required_string(row, "date")?);
    insert_i64(
        &mut values,
        "amount_cents",
        yuan_value_to_cents(row.get("amount")),
    );
    insert_string(&mut values, "direction", bill_direction(&bill_type));
    insert_string(
        &mut values,
        "transaction_type",
        bill_transaction_type(&bill_type),
    );
    insert_optional_positive_i64(
        &mut values,
        "account_id",
        optional_i64(row, "source_account_id"),
    );
    insert_optional_positive_i64(
        &mut values,
        "transfer_target_account_id",
        optional_i64(row, "destination_account_id"),
    );
    insert_optional_positive_i64(
        &mut values,
        "source_account_id",
        optional_i64(row, "source_account_id"),
    );
    insert_optional_positive_i64(
        &mut values,
        "target_account_id",
        optional_i64(row, "destination_account_id"),
    );
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
    Ok(values)
}

fn map_setting_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", required_i64(row, "id")?);
    insert_i64(&mut values, "user_id", 1);
    insert_string(&mut values, "key", required_string(row, "key")?);
    insert_json(
        &mut values,
        "value",
        parse_json_or_string(optional_string(row, "value")),
    );
    insert_bool(&mut values, "sensitive", truthy(row.get("is_encrypted")));
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
    Ok(values)
}

fn map_parser_template_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    let id = required_i64(row, "id")?;
    let session_id = required_string(row, "session_id")?;
    let parser_id = required_string(row, "parser_id")?;
    insert_i64(&mut values, "id", id);
    insert_i64(
        &mut values,
        "user_id",
        optional_i64(row, "user_id").unwrap_or(1),
    );
    insert_string(&mut values, "parser_id", parser_id.clone());
    insert_string(&mut values, "source_name", session_id.clone());
    insert_string(&mut values, "template_name", format!("{parser_id}:{id}"));
    insert_string(
        &mut values,
        "feature_signature",
        format!("{session_id}:{parser_id}:{id}"),
    );
    insert_optional_string(&mut values, "parser_version", None);
    insert_json(
        &mut values,
        "metadata",
        metadata_without(
            row,
            &["id", "user_id", "session_id", "parser_id", "created_at"],
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
        optional_string(row, "created_at").unwrap_or_else(default_timestamp),
    );
    insert_i64(&mut values, "version", 1);
    Ok(values)
}

fn map_category_rule_row(row: &BTreeMap<String, Value>) -> DbResult<BTreeMap<String, Value>> {
    let mut values = BTreeMap::new();
    insert_i64(&mut values, "id", required_i64(row, "id")?);
    insert_i64(
        &mut values,
        "user_id",
        optional_i64(row, "user_id").unwrap_or(1),
    );
    insert_i64(
        &mut values,
        "category_id",
        required_i64(row, "category_id")?,
    );
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
            "regex_enabled": truthy(row.get("regex_enabled"))
        }),
    );
    insert_i64(
        &mut values,
        "priority",
        optional_i64(row, "priority").unwrap_or_default(),
    );
    insert_bool(&mut values, "enabled", truthy(row.get("enabled")));
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
    Ok(values)
}

fn target_row(
    source_table: &str,
    target_table: &str,
    source: &BTreeMap<String, Value>,
    values: BTreeMap<String, Value>,
) -> DbResult<PostgresTargetRow> {
    let checksum = checksum_json(&values)?;
    Ok(PostgresTargetRow {
        source_table: source_table.to_string(),
        target_table: target_table.to_string(),
        source_id: optional_i64(source, "id"),
        values,
        checksum,
    })
}

fn sqlite_table_exists(connection: &Connection, table_name: &str) -> DbResult<bool> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table_name],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

fn sqlite_table_columns(connection: &Connection, table_name: &str) -> DbResult<Vec<String>> {
    let sql = format!("PRAGMA table_info({})", quote_sqlite_identifier(table_name));
    let mut statement = connection.prepare(&sql)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(columns)
}

fn read_sqlite_rows(
    connection: &Connection,
    table_name: &str,
) -> DbResult<Vec<BTreeMap<String, Value>>> {
    let order_by = if sqlite_table_columns(connection, table_name)?
        .iter()
        .any(|column| column == "id")
    {
        "id"
    } else {
        "rowid"
    };
    let sql = format!(
        "SELECT * FROM {} ORDER BY {}",
        quote_sqlite_identifier(table_name),
        quote_sqlite_identifier(order_by)
    );
    let mut statement = connection.prepare(&sql)?;
    let columns = statement
        .column_names()
        .into_iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let rows = statement
        .query_map([], |row| {
            let mut values = BTreeMap::new();
            for (index, column) in columns.iter().enumerate() {
                values.insert(column.clone(), sqlite_value_to_json(row.get_ref(index)?));
            }
            Ok(values)
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
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

fn checksum_json(value: &impl Serialize) -> DbResult<String> {
    let raw = serde_json::to_vec(value).map_err(|error| {
        DbError::InvalidOperation(format!(
            "serialize sqlite-to-postgres checksum payload: {error}"
        ))
    })?;
    Ok(hex_sha256(&raw))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

fn required_string(row: &BTreeMap<String, Value>, key: &str) -> DbResult<String> {
    optional_string(row, key).ok_or_else(|| {
        DbError::InvalidOperation(format!("missing required SQLite column value {key}"))
    })
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
            "missing required SQLite integer column value {key}"
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

fn yuan_value_to_cents(value: Option<&Value>) -> i64 {
    (optional_f64(value).unwrap_or_default() * 100.0).round() as i64
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

fn parse_json_or_string(value: Option<String>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    serde_json::from_str(&value).unwrap_or(Value::String(value))
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
    if bill_type.contains("转账") || normalized.contains("transfer") {
        "transfer"
    } else if bill_type.contains("投资") || normalized.contains("investment") {
        "investment"
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

fn insert_optional_positive_i64(
    values: &mut BTreeMap<String, Value>,
    key: &str,
    value: Option<i64>,
) {
    insert_optional_i64(values, key, value.filter(|value| *value > 0));
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

fn insert_json(values: &mut BTreeMap<String, Value>, key: &str, value: Value) {
    values.insert(key.to_string(), value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RecordingSink {
        rows: BTreeMap<String, usize>,
    }

    impl PostgresImportSink for RecordingSink {
        fn import_table(&mut self, table: &SqliteToPostgresTableExport) -> DbResult<usize> {
            self.rows
                .insert(table.target_table.clone(), table.row_count);
            Ok(table.row_count)
        }
    }

    #[test]
    fn dry_run_reports_counts_columns_and_checksums_without_writing_postgres() {
        let connection = fixture_connection();

        let report = sqlite_to_postgres_dry_run_from_connection(&connection).unwrap();

        assert_eq!(report.schema_version, 1);
        assert_eq!(report.table_count, 8);
        assert_eq!(report.ready_table_count, 8);
        assert_eq!(report.total_rows, 8);
        assert_eq!(report.checksum.len(), 64);
        let bills = report
            .tables
            .iter()
            .find(|table| table.source_table == "bills")
            .unwrap();
        assert_eq!(bills.row_count, 1);
        assert_eq!(bills.status, MigrationTableStatus::Ready);
        assert!(bills.missing_columns.is_empty());
        assert_eq!(bills.checksum.len(), 64);
    }

    #[test]
    fn export_bundle_normalizes_amounts_aliases_rules_and_target_checksums() {
        let connection = fixture_connection();

        let bundle = export_sqlite_to_postgres_bundle_from_connection(&connection).unwrap();

        assert_eq!(bundle.schema_version, 1);
        assert_eq!(bundle.table_count, 9);
        assert_eq!(bundle.total_rows, 10);
        let account_position = bundle
            .tables
            .iter()
            .position(|table| table.target_table == "accounts")
            .unwrap();
        let account_rules_position = bundle
            .tables
            .iter()
            .position(|table| table.target_table == "account_rules")
            .unwrap();
        assert!(account_position < account_rules_position);
        let accounts = table_export(&bundle, "accounts");
        assert_eq!(accounts.row_count, 1);
        assert_eq!(accounts.rows[0].values["balance_cents"], json!(12345));
        let account_rules = table_export(&bundle, "account_rules");
        assert_eq!(account_rules.row_count, 2);
        assert!(account_rules
            .rows
            .iter()
            .any(|row| row.values["rule_expression"]["values"]
                .as_array()
                .unwrap()
                .contains(&json!("Cash"))));
        let bills = table_export(&bundle, "bills");
        assert_eq!(bills.rows[0].values["amount_cents"], json!(1999));
        assert_eq!(bills.rows[0].values["direction"], json!("expense"));
        assert_eq!(bills.rows[0].values["transaction_type"], json!("expense"));
        assert_eq!(bundle.checksum.len(), 64);
    }

    #[test]
    fn import_check_replays_bundle_into_sink_and_detects_count_mismatch() {
        let connection = fixture_connection();
        let bundle = export_sqlite_to_postgres_bundle_from_connection(&connection).unwrap();
        let mut sink = RecordingSink::default();

        let report = import_postgres_bundle_to_sink(&bundle, &mut sink).unwrap();

        assert_eq!(report.table_count, bundle.table_count);
        assert_eq!(report.imported_rows, bundle.total_rows);
        assert_eq!(sink.rows["bills"], 1);
        assert_eq!(report.checksum.len(), 64);

        struct BadSink;
        impl PostgresImportSink for BadSink {
            fn import_table(&mut self, _table: &SqliteToPostgresTableExport) -> DbResult<usize> {
                Ok(0)
            }
        }
        let error = import_postgres_bundle_to_sink(&bundle, &mut BadSink).unwrap_err();
        assert!(error
            .to_string()
            .contains("Postgres import sink wrote 0 rows"));
    }

    #[test]
    fn postgres_import_sql_validates_tables_rows_columns_and_checksums() {
        let connection = fixture_connection();
        let bundle = export_sqlite_to_postgres_bundle_from_connection(&connection).unwrap();
        let users = table_export(&bundle, "users");
        let row = &users.rows[0];

        validate_postgres_table_export(users).unwrap();
        validate_postgres_target_row(users, row).unwrap();
        let sql = postgres_insert_sql(row).unwrap();
        assert!(sql.contains("INSERT INTO \"users\""));
        assert!(sql.contains("jsonb_populate_record(NULL::\"users\""));
        assert!(sql.contains("\"username\""));

        let mut bad_count = users.clone();
        bad_count.row_count += 1;
        let error = validate_postgres_table_export(&bad_count).unwrap_err();
        assert!(error.to_string().contains("declares"));

        let mut bad_table = users.clone();
        bad_table.target_table = "users; drop table users".to_string();
        let error = validate_postgres_table_export(&bad_table).unwrap_err();
        assert!(error.to_string().contains("unsupported PostgreSQL"));

        let mut bad_row = row.clone();
        bad_row.target_table = "accounts".to_string();
        let error = validate_postgres_target_row(users, &bad_row).unwrap_err();
        assert!(error.to_string().contains("does not match"));

        let mut bad_checksum = row.clone();
        bad_checksum.checksum = "bad".to_string();
        let error = validate_postgres_target_row(users, &bad_checksum).unwrap_err();
        assert!(error.to_string().contains("checksum mismatch"));

        let mut unsafe_column = row.clone();
        unsafe_column
            .values
            .insert("bad-column".to_string(), json!("unsafe"));
        let error = postgres_insert_sql(&unsafe_column).unwrap_err();
        assert!(error.to_string().contains("unsafe PostgreSQL identifier"));
    }

    #[tokio::test]
    async fn postgres_import_writes_bundle_and_audit_when_test_url_is_set() {
        let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return;
        };
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&postgres_url)
            .await
            .unwrap();
        crate::run_postgres_migrations(&pool).await.unwrap();
        sqlx::query(
            "TRUNCATE migration_audit_events, account_rules, category_rules, parser_templates, settings, bills, tags, categories, accounts, users RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .unwrap();

        let connection = fixture_connection();
        let bundle = export_sqlite_to_postgres_bundle_from_connection(&connection).unwrap();
        let report = import_postgres_bundle_to_postgres_with_name(
            &pool,
            &bundle,
            "test_sqlite_to_postgres_import",
        )
        .await
        .unwrap();

        assert_eq!(report.imported_rows, bundle.total_rows);
        assert_eq!(postgres_count(&pool, "SELECT COUNT(*) FROM users").await, 1);
        assert_eq!(
            postgres_count(&pool, "SELECT COUNT(*) FROM account_rules").await,
            2
        );
        assert_eq!(
            postgres_count(
                &pool,
                "SELECT COUNT(*) FROM migration_audit_events WHERE status = 'succeeded'",
            )
            .await,
            1
        );

        sqlx::query(
            "TRUNCATE migration_audit_events, account_rules, category_rules, parser_templates, settings, bills, tags, categories, accounts, users RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .unwrap();
    }

    #[test]
    fn missing_table_and_missing_columns_are_reported_as_retryable_dry_run_failures() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL
                );
                ",
            )
            .unwrap();

        let report = sqlite_to_postgres_dry_run_from_connection(&connection).unwrap();

        let users = report
            .tables
            .iter()
            .find(|table| table.source_table == "users")
            .unwrap();
        assert_eq!(users.status, MigrationTableStatus::MissingColumns);
        assert!(users.missing_columns.contains(&"email".to_string()));
        let accounts = report
            .tables
            .iter()
            .find(|table| table.source_table == "accounts")
            .unwrap();
        assert_eq!(accounts.status, MigrationTableStatus::MissingTable);
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

    async fn postgres_count(pool: &PostgresPool, sql: &str) -> i64 {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(pool)
            .await
            .unwrap()
    }

    fn fixture_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE users (
                    id INTEGER PRIMARY KEY,
                    username TEXT NOT NULL,
                    email TEXT NOT NULL,
                    password_hash TEXT NOT NULL,
                    nickname TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    type INTEGER NOT NULL,
                    currency TEXT,
                    balance REAL,
                    hidden INTEGER,
                    aliases TEXT,
                    display_order INTEGER,
                    comment TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    type INTEGER,
                    main_category TEXT NOT NULL,
                    sub_category TEXT NOT NULL,
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
                    counterparty TEXT NOT NULL,
                    description TEXT NOT NULL,
                    payment_method TEXT,
                    main_category TEXT,
                    sub_category TEXT,
                    hash TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    source_account_id INTEGER,
                    destination_account_id INTEGER
                );
                CREATE TABLE app_settings (
                    id INTEGER PRIMARY KEY,
                    key TEXT NOT NULL,
                    value TEXT,
                    value_type TEXT,
                    is_encrypted INTEGER,
                    updated_at TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE bills_parser_template (
                    id INTEGER PRIMARY KEY,
                    session_id TEXT NOT NULL,
                    user_id INTEGER NOT NULL,
                    parser_date TEXT NOT NULL,
                    parser_amount REAL NOT NULL,
                    parser_type TEXT NOT NULL,
                    parser_id TEXT NOT NULL,
                    parser_description TEXT,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE category_rules (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    category_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    priority INTEGER NOT NULL,
                    rule_expression TEXT NOT NULL,
                    regex_enabled INTEGER,
                    enabled INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                INSERT INTO users VALUES (1, 'alice', 'a@example.test', 'hash', 'Alice', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO accounts VALUES (10, 1, 'Wallet', 1, 'CNY', 123.45, 0, '[\"Cash\",\"零钱\"]', 7, 'daily cash', '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO categories VALUES (20, 1, 1, '餐饮', '午餐', 5, 0, 'utensils', '#ffcc00', '2026-01-01T00:00:00Z');
                INSERT INTO tags VALUES (30, 1, 'work', '#336699', 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO bills VALUES (40, 1, '2026-01-03T12:00:00Z', '支出', 19.99, 'Cafe', 'Lunch', 'Wallet', '餐饮', '午餐', 'hash40', '2026-01-03T12:01:00Z', '2026-01-03T12:02:00Z', 10, 0);
                INSERT INTO app_settings VALUES (50, 'receipt_ocr_config', '{\"enabled\":true}', 'json', 0, '2026-01-02T00:00:00Z', '2026-01-01T00:00:00Z');
                INSERT INTO bills_parser_template VALUES (60, 'session-1', 1, '2026-01-03T12:00:00Z', 19.99, '支出', 'wechat', 'Lunch', '2026-01-03T12:01:00Z');
                INSERT INTO category_rules VALUES (70, 1, 20, 'Cafe rule', 10, 'Cafe', 0, 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                ",
            )
            .unwrap();
        connection
    }
}
