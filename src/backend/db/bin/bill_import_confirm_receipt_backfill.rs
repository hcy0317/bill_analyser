use std::{env, error::Error, process, time::Duration};

use bill_analyser_db::{
    backfill_import_confirm_receipt_batch, ImportConfirmReceiptBackfillBatchReport, PostgresPool,
    IMPORT_CONFIRM_RECEIPT_BACKFILL_MAX_BATCH_SIZE,
};
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;

mod support;

use support::redact_database_error;

const DEFAULT_BATCH_SIZE: u32 = 200;
const DEFAULT_SLEEP_MS: u64 = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandConfig {
    after_session_id: i64,
    batch_size: u32,
    max_batches: Option<u64>,
    sleep_ms: u64,
}

impl Default for CommandConfig {
    fn default() -> Self {
        Self {
            after_session_id: 0,
            batch_size: DEFAULT_BATCH_SIZE,
            max_batches: None,
            sleep_ms: DEFAULT_SLEEP_MS,
        }
    }
}

enum CommandAction {
    Run(CommandConfig),
    Help,
}

#[derive(Serialize)]
struct BatchOutput<'a> {
    event: &'static str,
    batch_number: u64,
    #[serde(flatten)]
    report: &'a ImportConfirmReceiptBackfillBatchReport,
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
        .max_connections(2)
        .connect(&postgres_url)
        .await?;
    let result = execute(&pool, &config).await;
    pool.close().await;
    result
}

async fn execute(pool: &PostgresPool, config: &CommandConfig) -> Result<(), Box<dyn Error>> {
    let mut after_session_id = config.after_session_id;
    let mut batch_number = 0_u64;
    loop {
        batch_number += 1;
        let report =
            backfill_import_confirm_receipt_batch(pool, after_session_id, config.batch_size)
                .await?;
        println!(
            "{}",
            serde_json::to_string(&BatchOutput {
                event: "import_confirm_receipt_backfill_batch",
                batch_number,
                report: &report,
            })?
        );
        after_session_id = report.next_after_session_id;
        if !report.has_remaining_sessions
            || config
                .max_batches
                .is_some_and(|maximum| batch_number >= maximum)
        {
            break;
        }
        if config.sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(config.sleep_ms)).await;
        }
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
            "--after-session-id" => {
                config.after_session_id = value
                    .parse()
                    .map_err(|_| "--after-session-id must be a non-negative integer".to_string())?;
                if config.after_session_id < 0 {
                    return Err("--after-session-id must be a non-negative integer".to_string());
                }
            }
            "--batch-size" => {
                config.batch_size =
                    parse_bounded_u32(value, name, IMPORT_CONFIRM_RECEIPT_BACKFILL_MAX_BATCH_SIZE)?;
            }
            "--max-batches" => {
                let parsed = value
                    .parse()
                    .map_err(|_| "--max-batches must be a positive integer".to_string())?;
                if parsed == 0 {
                    return Err("--max-batches must be a positive integer".to_string());
                }
                config.max_batches = Some(parsed);
            }
            "--sleep-ms" => {
                config.sleep_ms = value
                    .parse()
                    .map_err(|_| "--sleep-ms must be a non-negative integer".to_string())?;
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
                "bill_import_confirm_receipt_backfill [--after-session-id <id>] [--batch-size <1..{IMPORT_CONFIRM_RECEIPT_BACKFILL_MAX_BATCH_SIZE}>] [--max-batches <count>] [--sleep-ms <milliseconds>]"
            ),
            "env": ["BILL_ANALYSER_POSTGRES_URL"],
            "checkpoint": "Persist next_after_session_id from each committed JSON batch before resuming.",
            "runs_migrations": false,
            "writes_business_data": true
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
    fn defaults_are_rate_limited_and_start_at_zero() {
        let CommandAction::Run(config) = parse_args(&[]).unwrap() else {
            panic!("expected runnable config");
        };
        assert_eq!(config, CommandConfig::default());
        assert_eq!(config.after_session_id, 0);
        assert_eq!(config.batch_size, 200);
        assert_eq!(config.max_batches, None);
        assert_eq!(config.sleep_ms, 50);
    }

    #[test]
    fn parses_inline_and_separate_values() {
        let CommandAction::Run(config) = parse_args(&args(&[
            "--after-session-id=42",
            "--batch-size",
            "64",
            "--max-batches=3",
            "--sleep-ms",
            "0",
        ]))
        .unwrap() else {
            panic!("expected runnable config");
        };
        assert_eq!(
            config,
            CommandConfig {
                after_session_id: 42,
                batch_size: 64,
                max_batches: Some(3),
                sleep_ms: 0,
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
            args(&["--batch-size"]),
            args(&["--unknown", "1"]),
            args(&["--after-session-id", "not-a-number"]),
            args(&["--after-session-id", "-1"]),
            args(&["--batch-size", "not-a-number"]),
            args(&["--batch-size", "0"]),
            args(&["--batch-size", "1001"]),
            args(&["--max-batches", "not-a-number"]),
            args(&["--max-batches", "0"]),
            args(&["--sleep-ms", "not-a-number"]),
        ] {
            assert!(parse_args(&invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn database_errors_redact_full_urls_and_password_fragments() {
        let database_url =
            "postgresql://backfill_user:c6c-fake-password@database.internal/backfill";
        let message =
            format!("connection failed for {database_url}: password c6c-fake-password rejected");
        let redacted = redact_database_error(&message, Some(database_url));
        assert!(!redacted.contains(database_url));
        assert!(!redacted.contains("c6c-fake-password"));
        assert!(redacted.contains("[REDACTED_DATABASE_URL]"));
        assert!(redacted.contains("[REDACTED_DATABASE_PASSWORD]"));
    }
}
