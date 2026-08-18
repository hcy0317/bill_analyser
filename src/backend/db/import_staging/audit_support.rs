struct ImportAuditMigrationLedger {
    expected_migration_version: i64,
    expected_migration_count: u64,
    actual_migration_version: i64,
    migration_count: u64,
}

async fn validate_import_audit_migration_ledger(
    connection: &mut sqlx::PgConnection,
    audit_name: &str,
) -> DbResult<ImportAuditMigrationLedger> {
    let expected_versions = crate::postgres_migration_manifest()
        .iter()
        .map(|migration| migration.version)
        .collect::<Vec<_>>();
    let actual_versions: Vec<i64> = sqlx::query_scalar(
        "SELECT version FROM _sqlx_migrations WHERE success=true ORDER BY version",
    )
    .fetch_all(&mut *connection)
    .await?;
    if actual_versions != expected_versions {
        return Err(DbError::InvalidOperation(format!(
            "{audit_name} requires exact migration ledger {:?}; found {:?}",
            expected_versions, actual_versions
        )));
    }
    Ok(ImportAuditMigrationLedger {
        expected_migration_version: expected_versions.last().copied().unwrap_or_default(),
        expected_migration_count: u64::try_from(expected_versions.len()).unwrap_or(u64::MAX),
        actual_migration_version: actual_versions.last().copied().unwrap_or_default(),
        migration_count: u64::try_from(actual_versions.len()).unwrap_or(u64::MAX),
    })
}

fn import_audit_count(row: &PgRow, column: &str, audit_name: &str) -> DbResult<u64> {
    let value: i64 = row.try_get(column)?;
    u64::try_from(value).map_err(|_| {
        DbError::InvalidOperation(format!(
            "invalid negative {audit_name} count {column}: {value}"
        ))
    })
}
