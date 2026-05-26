use std::error::Error;
use std::time::Duration;

use bill_analyser_db::{
    DatabaseRuntimeBackend, DatabaseRuntimeConfig, DatabaseRuntimeProvider, DbError, SqliteDbPath,
    SqliteRuntimeOpenMode,
};

#[test]
fn database_backend_names_are_stable_for_health_and_logs() {
    assert_eq!(DatabaseRuntimeBackend::Sqlite.as_str(), "sqlite");
    assert_eq!(DatabaseRuntimeBackend::Postgres.as_str(), "postgres");
}

#[test]
fn sqlite_provider_opens_legacy_runtime_with_runtime_pragmas() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let sqlite_path = SqliteDbPath::temporary_file(temp_dir.path().join("runtime.db"))?;
    let provider = DatabaseRuntimeProvider::new(DatabaseRuntimeConfig::sqlite(
        sqlite_path,
        Duration::from_secs(1),
    ))?;

    assert_eq!(provider.backend(), DatabaseRuntimeBackend::Sqlite);
    let runtime = provider.open_sqlite_runtime(SqliteRuntimeOpenMode::CreateIfMissing)?;
    let pragmas = runtime.pragma_snapshot()?;

    assert_eq!(pragmas.journal_mode, "wal");
    assert!(pragmas.foreign_keys);
    assert_eq!(pragmas.synchronous, 1);
    Ok(())
}

#[test]
fn sqlite_provider_requires_a_sqlite_path() {
    let provider = DatabaseRuntimeProvider::new(DatabaseRuntimeConfig {
        backend: DatabaseRuntimeBackend::Sqlite,
        sqlite_path: None,
        postgres_url: None,
        busy_timeout: Duration::from_millis(1),
        postgres_max_connections: 5,
    })
    .expect("sqlite provider can be built without opening the path");
    assert!(format!("{provider:?}").contains("postgres_runtime_configured: false"));

    let error = match provider.open_sqlite_runtime(SqliteRuntimeOpenMode::ExistingOnly) {
        Ok(_) => panic!("sqlite runtime without path should fail"),
        Err(error) => error,
    };

    assert!(matches!(error, DbError::InvalidOperation(_)));
    assert!(error.to_string().contains("configured SQLite path"));
}

#[tokio::test]
async fn postgres_provider_builds_lazy_runtime_without_leaking_secret() -> Result<(), Box<dyn Error>>
{
    let config = DatabaseRuntimeConfig::postgres(
        "postgres://bill:secret@127.0.0.1:1/bill_analyser",
        Duration::from_secs(1),
    )
    .with_postgres_max_connections(0);

    let debug = format!("{config:?}");
    assert!(debug.contains("postgres_url_configured"));
    assert!(!debug.contains("secret"));

    let provider = DatabaseRuntimeProvider::new(config)?;
    assert_eq!(provider.backend(), DatabaseRuntimeBackend::Postgres);
    assert!(format!("{provider:?}").contains("postgres_runtime_configured: true"));
    let postgres_runtime = provider.postgres_runtime()?;
    assert!(format!("{postgres_runtime:?}").contains("pool_configured"));
    let _pool = postgres_runtime.pool();

    let fallback_error = match provider.open_sqlite_runtime(SqliteRuntimeOpenMode::CreateIfMissing)
    {
        Ok(_) => panic!("postgres provider must not open sqlite runtime"),
        Err(error) => error,
    };
    assert!(fallback_error
        .to_string()
        .contains("must not silently fall back"));
    Ok(())
}

#[test]
fn sqlite_provider_rejects_postgres_runtime_request() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let sqlite_path = SqliteDbPath::temporary_file(temp_dir.path().join("runtime.db"))?;
    let provider = DatabaseRuntimeProvider::new(DatabaseRuntimeConfig::sqlite(
        sqlite_path,
        Duration::from_secs(1),
    ))?;

    let error = provider.postgres_runtime().unwrap_err();

    assert!(matches!(error, DbError::InvalidOperation(_)));
    assert!(error.to_string().contains("not configured"));
    Ok(())
}

#[test]
fn postgres_provider_requires_postgres_url() {
    let error = DatabaseRuntimeProvider::new(DatabaseRuntimeConfig {
        backend: DatabaseRuntimeBackend::Postgres,
        sqlite_path: None,
        postgres_url: None,
        busy_timeout: Duration::from_millis(1),
        postgres_max_connections: 5,
    })
    .unwrap_err();

    assert!(matches!(error, DbError::InvalidOperation(_)));
    assert!(error.to_string().contains("Postgres URL"));
}
