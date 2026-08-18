use std::{env, error::Error, io, process};

use bill_analyser_db::{
    audit_import_confirm_receipt_target_snapshot, ImportConfirmReceiptReadAuditReport,
    PostgresPool, IMPORT_CONFIRM_RECEIPT_READ_AUDIT_MAX_BATCH_SIZE,
};
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;

mod support;

use support::redact_database_error;

const DEFAULT_BATCH_SIZE: u32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandConfig {
    batch_size: u32,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
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
    eligible_for_typed_read_cutover: bool,
    #[serde(flatten)]
    report: &'a ImportConfirmReceiptReadAuditReport,
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
    let report = audit_import_confirm_receipt_target_snapshot(pool, config.batch_size).await?;
    let eligible_for_typed_read_cutover = report.is_match();
    println!(
        "{}",
        serde_json::to_string(&AuditOutput {
            event: "import_confirm_receipt_read_target_audit",
            eligible_for_typed_read_cutover,
            report: &report,
        })?
    );
    if !eligible_for_typed_read_cutover {
        return Err(io::Error::other(format!(
            "confirm receipt read target audit failed: {} unmaterialized, {} unexpected typed, {} mismatched receipts",
            report.unmaterialized_target_receipts,
            report.unexpected_typed_receipts,
            report.mismatch_receipts
        ))
        .into());
    }
    Ok(())
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
            "--batch-size" => {
                config.batch_size = parse_bounded_u32(
                    value,
                    name,
                    IMPORT_CONFIRM_RECEIPT_READ_AUDIT_MAX_BATCH_SIZE,
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
                "bill_import_confirm_receipt_read_audit [--batch-size <1..{IMPORT_CONFIRM_RECEIPT_READ_AUDIT_MAX_BATCH_SIZE}>]"
            ),
            "env": ["BILL_ANALYSER_POSTGRES_URL"],
            "runs_migrations": false,
            "writes_business_data": false,
            "snapshot": "One REPEATABLE READ, READ ONLY transaction; no checkpoint resume."
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
    fn defaults_are_bounded() {
        let CommandAction::Run(config) = parse_args(&[]).unwrap() else {
            panic!("expected runnable config");
        };
        assert_eq!(config, CommandConfig::default());
        assert_eq!(config.batch_size, 1_000);
    }

    #[test]
    fn parses_inline_and_separate_values() {
        let CommandAction::Run(inline) = parse_args(&args(&["--batch-size=64"])).unwrap() else {
            panic!("expected runnable config");
        };
        let CommandAction::Run(separate) = parse_args(&args(&["--batch-size", "25"])).unwrap()
        else {
            panic!("expected runnable config");
        };
        assert_eq!(inline.batch_size, 64);
        assert_eq!(separate.batch_size, 25);
        assert!(matches!(
            parse_args(&args(&["-h"])),
            Ok(CommandAction::Help)
        ));
    }

    #[test]
    fn rejects_missing_unknown_and_out_of_range_values() {
        for invalid in [
            args(&["--batch-size"]),
            args(&["--unknown", "1"]),
            args(&["--batch-size", "not-a-number"]),
            args(&["--batch-size", "0"]),
            args(&["--batch-size", "10001"]),
        ] {
            assert!(parse_args(&invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn database_errors_redact_full_urls_and_password_fragments() {
        let database_url = "postgresql://audit_user:c6d-fake-password@database.internal/read_audit";
        let message =
            format!("connection failed for {database_url}: password c6d-fake-password rejected");
        let redacted = redact_database_error(&message, Some(database_url));
        assert!(!redacted.contains(database_url));
        assert!(!redacted.contains("c6d-fake-password"));
        assert!(redacted.contains("[REDACTED_DATABASE_URL]"));
        assert!(redacted.contains("[REDACTED_DATABASE_PASSWORD]"));
    }
}
