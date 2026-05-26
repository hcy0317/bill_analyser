use std::{env, path::PathBuf, process};

use bill_analyser_db::{
    export_sqlite_to_postgres_bundle, import_postgres_bundle_to_postgres,
    import_postgres_bundle_to_sink, load_sqlite_to_postgres_bundle_json,
    sqlite_to_postgres_dry_run, DbError, DbResult, PostgresImportSink, SqliteToPostgresTableExport,
};
use sqlx::postgres::PgPoolOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandMode {
    DryRun,
    Export,
    ImportCheck,
    Import,
    Help,
}

#[derive(Debug, Default)]
struct CliArgs {
    mode: Option<CommandMode>,
    sqlite_path: Option<PathBuf>,
    bundle_path: Option<PathBuf>,
    postgres_url: Option<String>,
    output_path: Option<PathBuf>,
}

#[derive(Debug)]
struct CommandOutput {
    output_path: Option<PathBuf>,
    json: String,
}

#[derive(Default)]
struct CountingImportSink;

impl PostgresImportSink for CountingImportSink {
    fn import_table(&mut self, table: &SqliteToPostgresTableExport) -> DbResult<usize> {
        Ok(table.row_count)
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("sqlite-to-postgres migration error: {error}");
        process::exit(2);
    }
}

async fn run() -> DbResult<()> {
    let output = execute(env::args().skip(1)).await?;
    emit_output(output)
}

async fn execute(args: impl IntoIterator<Item = String>) -> DbResult<CommandOutput> {
    let args = parse_args(args)?;
    match args.mode.ok_or_else(|| invalid("missing --mode"))? {
        CommandMode::DryRun => {
            let sqlite_path = args
                .sqlite_path
                .ok_or_else(|| invalid("dry-run requires --sqlite"))?;
            let report = sqlite_to_postgres_dry_run(sqlite_path)?;
            command_output(args.output_path, &report)
        }
        CommandMode::Export => {
            let sqlite_path = args
                .sqlite_path
                .ok_or_else(|| invalid("export requires --sqlite"))?;
            let bundle = export_sqlite_to_postgres_bundle(sqlite_path)?;
            command_output(args.output_path, &bundle)
        }
        CommandMode::ImportCheck => {
            let bundle_path = args
                .bundle_path
                .ok_or_else(|| invalid("import-check requires --bundle"))?;
            let bundle = load_sqlite_to_postgres_bundle_json(bundle_path)?;
            let mut sink = CountingImportSink;
            let report = import_postgres_bundle_to_sink(&bundle, &mut sink)?;
            command_output(args.output_path, &report)
        }
        CommandMode::Import => {
            let bundle_path = args
                .bundle_path
                .ok_or_else(|| invalid("import requires --bundle"))?;
            let postgres_url = args
                .postgres_url
                .ok_or_else(|| invalid("import requires --postgres-url"))?;
            let bundle = load_sqlite_to_postgres_bundle_json(bundle_path)?;
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&postgres_url)
                .await
                .map_err(|error| {
                    DbError::InvalidOperation(format!("connect Postgres for import: {error}"))
                })?;
            let report = import_postgres_bundle_to_postgres(&pool, &bundle).await?;
            command_output(args.output_path, &report)
        }
        CommandMode::Help => Ok(CommandOutput {
            output_path: None,
            json: help_text().to_string(),
        }),
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> DbResult<CliArgs> {
    let mut parsed = CliArgs::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" => {
                let value = args
                    .next()
                    .ok_or_else(|| invalid("--mode requires a value"))?;
                parsed.mode = Some(match value.as_str() {
                    "dry-run" => CommandMode::DryRun,
                    "export" => CommandMode::Export,
                    "import-check" => CommandMode::ImportCheck,
                    "import" => CommandMode::Import,
                    _ => {
                        return Err(invalid(
                            "unsupported --mode; use dry-run, export, import-check, or import",
                        ))
                    }
                });
            }
            "--sqlite" => {
                parsed.sqlite_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| invalid("--sqlite requires a path"))?,
                ));
            }
            "--bundle" => {
                parsed.bundle_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| invalid("--bundle requires a path"))?,
                ));
            }
            "--postgres-url" => {
                parsed.postgres_url = Some(
                    args.next()
                        .ok_or_else(|| invalid("--postgres-url requires a connection string"))?,
                );
            }
            "--output" => {
                parsed.output_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| invalid("--output requires a path"))?,
                ));
            }
            "--help" | "-h" => {
                parsed.mode = Some(CommandMode::Help);
            }
            _ => return Err(invalid(format!("unknown argument {arg}"))),
        }
    }
    Ok(parsed)
}

fn command_output<T: serde::Serialize>(
    output_path: Option<PathBuf>,
    value: &T,
) -> DbResult<CommandOutput> {
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        DbError::InvalidOperation(format!("serialize migration CLI output: {error}"))
    })?;
    Ok(CommandOutput { output_path, json })
}

fn emit_output(output: CommandOutput) -> DbResult<()> {
    if let Some(output_path) = output.output_path {
        std::fs::write(output_path, output.json)?;
        Ok(())
    } else {
        println!("{}", output.json);
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> DbError {
    DbError::InvalidOperation(format!(
        "sqlite-to-postgres migration CLI: {}",
        message.into()
    ))
}

fn help_text() -> &'static str {
    "Usage:\n  bill_sqlite_to_postgres_migrate --mode dry-run --sqlite <db> [--output report.json]\n  bill_sqlite_to_postgres_migrate --mode export --sqlite <db> [--output bundle.json]\n  bill_sqlite_to_postgres_migrate --mode import-check --bundle <bundle.json> [--output report.json]\n  bill_sqlite_to_postgres_migrate --mode import --bundle <bundle.json> --postgres-url <url> [--output import-report.json]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use bill_analyser_db::{
        write_sqlite_to_postgres_json, SqliteToPostgresExportBundle, SqliteToPostgresTableExport,
    };
    use rusqlite::Connection;

    #[tokio::test]
    async fn cli_private_edges_cover_parse_sink_and_execute_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sqlite_path = temp_dir.path().join("source.db");
        Connection::open(&sqlite_path).unwrap();

        let dry_run = execute([
            "--mode".to_string(),
            "dry-run".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
        ])
        .await
        .unwrap();
        assert!(dry_run.json.contains("\"schema_version\""));

        let export = execute([
            "--mode".to_string(),
            "export".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
        ])
        .await
        .unwrap();
        assert!(export.json.contains("\"total_rows\": 0"));

        let table = SqliteToPostgresTableExport {
            source_table: "users".to_string(),
            target_table: "users".to_string(),
            row_count: 2,
            checksum: "users-checksum".to_string(),
            rows: Vec::new(),
        };
        let mut sink = CountingImportSink;
        assert_eq!(sink.import_table(&table).unwrap(), 2);

        let bundle_path = temp_dir.path().join("bundle.json");
        let output_path = temp_dir.path().join("check.json");
        write_sqlite_to_postgres_json(
            &bundle_path,
            &SqliteToPostgresExportBundle {
                schema_version: 1,
                table_count: 1,
                total_rows: 2,
                checksum: "empty".to_string(),
                tables: vec![table],
            },
        )
        .unwrap();
        let output = execute([
            "--mode".to_string(),
            "import-check".to_string(),
            "--bundle".to_string(),
            bundle_path.display().to_string(),
            "--output".to_string(),
            output_path.display().to_string(),
        ])
        .await
        .unwrap();
        emit_output(output).unwrap();
        assert!(std::fs::read_to_string(output_path)
            .unwrap()
            .contains("\"imported_rows\": 2"));

        assert!(execute(["--help".to_string()])
            .await
            .unwrap()
            .json
            .contains("Usage:"));
        assert!(parse_args(["--unknown".to_string()])
            .unwrap_err()
            .to_string()
            .contains("unknown argument"));
        assert!(parse_args(["--mode".to_string(), "apply".to_string()])
            .unwrap_err()
            .to_string()
            .contains("unsupported --mode"));
        assert!(parse_args(["--postgres-url".to_string()])
            .unwrap_err()
            .to_string()
            .contains("--postgres-url requires"));
        assert!(execute([
            "--mode".to_string(),
            "import".to_string(),
            "--bundle".to_string(),
            bundle_path.display().to_string(),
        ])
        .await
        .unwrap_err()
        .to_string()
        .contains("import requires --postgres-url"));
        assert!(execute([
            "--mode".to_string(),
            "import".to_string(),
            "--bundle".to_string(),
            bundle_path.display().to_string(),
            "--postgres-url".to_string(),
            "not-a-postgres-url".to_string(),
        ])
        .await
        .unwrap_err()
        .to_string()
        .contains("connect Postgres for import"));
    }
}
