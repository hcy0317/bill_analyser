use std::{env, error::Error, io, process};

use bill_analyser_db::{
    audit_import_preview_signal_target_snapshot, ImportPreviewSignalTargetAuditReport,
    PostgresPool, IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE,
    IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_ROW_BATCH_SIZE,
};
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;

const DEFAULT_ROW_BATCH_SIZE: u32 = 1_000;
const DEFAULT_QUERY_PAGE_SIZE: u32 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandConfig {
    row_batch_size: u32,
    query_page_size: u32,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            row_batch_size: DEFAULT_ROW_BATCH_SIZE,
            query_page_size: DEFAULT_QUERY_PAGE_SIZE,
        }
    }
}

enum CommandAction {
    Run(CommandConfig),
    Help,
}

#[derive(Serialize)]
struct AuditOutput<'a> {
    event: &'static str,
    eligible_for_read_cutover: bool,
    #[serde(flatten)]
    report: &'a ImportPreviewSignalTargetAuditReport,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        let message = redact_database_error(
            &error.to_string(),
            env::var("BILL_ANALYSER_POSTGRES_URL").ok().as_deref(),
        );
        eprintln!(
            "{}",
            serde_json::to_string(&json!({"event":"error","message":message}))
                .unwrap_or_else(|_| "{\"event\":\"error\"}".to_string())
        );
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let CommandAction::Run(config) = parse_args(&args)? else {
        print_help();
        return Ok(());
    };
    let postgres_url = env::var("BILL_ANALYSER_POSTGRES_URL")
        .map_err(|_| "BILL_ANALYSER_POSTGRES_URL is required")?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&postgres_url)
        .await?;
    let result = execute(&pool, &config).await;
    pool.close().await;
    result
}

async fn execute(pool: &PostgresPool, config: &CommandConfig) -> Result<(), Box<dyn Error>> {
    let report = audit_import_preview_signal_target_snapshot(
        pool,
        config.row_batch_size,
        config.query_page_size,
    )
    .await?;
    let eligible_for_read_cutover = report.is_match();
    println!(
        "{}",
        serde_json::to_string(&AuditOutput {
            event: "import_signal_read_target_audit",
            eligible_for_read_cutover,
            report: &report,
        })?
    );
    if !eligible_for_read_cutover {
        return Err(io::Error::other(format!(
            "import signal target audit failed: {} unmaterialized rows, {} row mismatches, {} query mismatches",
            report.unmaterialized_rows, report.row_mismatch_rows, report.query_mismatch_cases
        ))
        .into());
    }
    Ok(())
}

fn redact_database_error(message: &str, database_url: Option<&str>) -> String {
    let Some(database_url) = database_url.filter(|value| !value.is_empty()) else {
        return message.to_string();
    };
    let mut redacted = message.replace(database_url, "[REDACTED_DATABASE_URL]");
    if let Some(authority) = database_url
        .split_once("://")
        .map(|(_, remainder)| remainder)
        .and_then(|remainder| remainder.split_once('@').map(|(authority, _)| authority))
    {
        if let Some(password) = authority.split_once(':').map(|(_, password)| password) {
            if !password.is_empty() {
                redacted = redacted.replace(password, "[REDACTED_DATABASE_PASSWORD]");
            }
        }
    }
    redacted
}

fn parse_args(args: &[String]) -> Result<CommandAction, String> {
    let mut config = CommandConfig::default();
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--help" || argument == "-h" {
            return Ok(CommandAction::Help);
        }
        let (name, inline_value) = argument
            .split_once('=')
            .map_or((argument.as_str(), None), |(name, value)| {
                (name, Some(value))
            });
        let value = if let Some(value) = inline_value {
            value
        } else {
            index += 1;
            args.get(index)
                .ok_or_else(|| format!("missing value for {name}"))?
        };
        match name {
            "--row-batch-size" => {
                config.row_batch_size = parse_bounded_u32(
                    value,
                    name,
                    IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_ROW_BATCH_SIZE,
                )?;
            }
            "--query-page-size" => {
                config.query_page_size = parse_bounded_u32(
                    value,
                    name,
                    IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE,
                )?;
            }
            _ => return Err(format!("unknown argument: {name}")),
        }
        index += 1;
    }
    Ok(CommandAction::Run(config))
}

fn parse_bounded_u32(value: &str, name: &str, maximum: u32) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("{name} must be an integer from 1 to {maximum}"))?;
    if !(1..=maximum).contains(&parsed) {
        return Err(format!("{name} must be an integer from 1 to {maximum}"));
    }
    Ok(parsed)
}

fn print_help() {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "usage": format!(
                "bill_import_signal_read_audit [--row-batch-size <1..{IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_ROW_BATCH_SIZE}>] [--query-page-size <1..{IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE}>]"
            ),
            "env": ["BILL_ANALYSER_POSTGRES_URL"],
            "writes_business_data": false,
            "snapshot": "One REPEATABLE READ, READ ONLY transaction; no cross-process checkpoint resume."
        }))
        .expect("help json is serializable")
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn defaults_are_bounded_for_target_audit() {
        let CommandAction::Run(config) = parse_args(&[]).unwrap() else {
            panic!("expected runnable config");
        };
        assert_eq!(config, CommandConfig::default());
        assert_eq!(config.row_batch_size, 1_000);
        assert_eq!(config.query_page_size, 100);
    }

    #[test]
    fn parses_inline_and_separate_values() {
        let CommandAction::Run(config) =
            parse_args(&args(&["--row-batch-size=64", "--query-page-size", "25"])).unwrap()
        else {
            panic!("expected runnable config");
        };
        assert_eq!(
            config,
            CommandConfig {
                row_batch_size: 64,
                query_page_size: 25,
            }
        );
        assert!(matches!(
            parse_args(&args(&["-h"])),
            Ok(CommandAction::Help)
        ));
    }

    #[test]
    fn rejects_missing_unknown_and_out_of_range_values() {
        for invalid in [
            args(&["--row-batch-size"]),
            args(&["--unknown", "1"]),
            args(&["--row-batch-size", "not-a-number"]),
            args(&["--row-batch-size", "0"]),
            args(&["--row-batch-size", "10001"]),
            args(&["--query-page-size", "not-a-number"]),
            args(&["--query-page-size", "0"]),
            args(&["--query-page-size", "1001"]),
        ] {
            assert!(parse_args(&invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn database_errors_redact_full_urls_and_password_fragments() {
        let database_url =
            "postgresql://audit_user:c5fb-fake-password-sentinel@database.internal/audit";
        let message = format!(
            "connection failed for {database_url}: password c5fb-fake-password-sentinel rejected"
        );
        let redacted = redact_database_error(&message, Some(database_url));
        assert!(!redacted.contains(database_url));
        assert!(!redacted.contains("c5fb-fake-password-sentinel"));
        assert!(redacted.contains("[REDACTED_DATABASE_URL]"));
        assert!(redacted.contains("[REDACTED_DATABASE_PASSWORD]"));
        assert_eq!(redact_database_error("plain error", None), "plain error");
    }
}
