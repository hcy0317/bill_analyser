use std::{env, error::Error, io, process};

use bill_analyser_db::{
    audit_import_preview_signal_performance, ImportPreviewSignalPerformanceAuditConfig,
    ImportPreviewSignalPerformanceAuditReport, PostgresPool,
    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_MEASURED_ITERATIONS,
    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT,
    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WARMUP_ITERATIONS,
    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS,
    IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE,
};
use serde::Serialize;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;

mod support;

use support::redact_database_error;

enum CommandAction {
    Run(ImportPreviewSignalPerformanceAuditConfig),
    Help,
}

#[derive(Serialize)]
struct AuditOutput<'a> {
    event: &'static str,
    performance_gate_passed: bool,
    #[serde(flatten)]
    report: &'a ImportPreviewSignalPerformanceAuditReport,
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

async fn execute(
    pool: &PostgresPool,
    config: &ImportPreviewSignalPerformanceAuditConfig,
) -> Result<(), Box<dyn Error>> {
    let report = audit_import_preview_signal_performance(pool, config).await?;
    let performance_gate_passed = report.passes_performance_gate();
    println!(
        "{}",
        serde_json::to_string(&AuditOutput {
            event: "import_signal_performance_target_audit",
            performance_gate_passed,
            report: &report,
        })?
    );
    if !performance_gate_passed {
        return Err(io::Error::other(format!(
            "import signal performance audit failed: {} query parity mismatches, {} query regressions, write gate passed={}",
            report.query_mismatch_cases, report.query_regression_cases, report.write.passed
        ))
        .into());
    }
    Ok(())
}

fn parse_args(args: &[String]) -> Result<CommandAction, String> {
    let mut config = ImportPreviewSignalPerformanceAuditConfig::default();
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
            "--warmup-iterations" => {
                config.warmup_iterations = parse_bounded_u32(
                    value,
                    name,
                    0,
                    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WARMUP_ITERATIONS,
                )?;
            }
            "--measured-iterations" => {
                config.measured_iterations = parse_bounded_u32(
                    value,
                    name,
                    1,
                    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_MEASURED_ITERATIONS,
                )?;
            }
            "--query-page-size" => {
                config.query_page_size = parse_bounded_u32(
                    value,
                    name,
                    1,
                    IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE,
                )?;
            }
            "--maximum-query-regression-percent" => {
                config.maximum_query_regression_percent = parse_bounded_f64(
                    value,
                    name,
                    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT,
                )?;
            }
            "--maximum-write-regression-percent" => {
                config.maximum_write_regression_percent = parse_bounded_f64(
                    value,
                    name,
                    IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT,
                )?;
            }
            _ => return Err(format!("unknown argument: {name}")),
        }
        index += 1;
    }
    Ok(CommandAction::Run(config))
}

fn parse_bounded_u32(value: &str, name: &str, minimum: u32, maximum: u32) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
        .map_err(|_| format!("{name} must be an integer from {minimum} to {maximum}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!(
            "{name} must be an integer from {minimum} to {maximum}"
        ));
    }
    Ok(parsed)
}

fn parse_bounded_f64(value: &str, name: &str, maximum: f64) -> Result<f64, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("{name} must be a number from 0 to {maximum}"))?;
    if !parsed.is_finite() || !(0.0..=maximum).contains(&parsed) {
        return Err(format!("{name} must be a number from 0 to {maximum}"));
    }
    Ok(parsed)
}

fn print_help() {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "usage": format!(
                "bill_import_signal_performance_audit [--warmup-iterations <0..{IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WARMUP_ITERATIONS}>] [--measured-iterations <1..{IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_MEASURED_ITERATIONS}>] [--query-page-size <1..{IMPORT_PREVIEW_SIGNAL_TARGET_AUDIT_MAX_PAGE_SIZE}>] [--maximum-query-regression-percent <0..{IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT}>] [--maximum-write-regression-percent <0..{IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_REGRESSION_PERCENT}>]"
            ),
            "env": ["BILL_ANALYSER_POSTGRES_URL"],
            "writes_business_data": false,
            "temporary_tables_rolled_back": true,
            "maximum_write_rows": IMPORT_PREVIEW_SIGNAL_PERFORMANCE_MAX_WRITE_ROWS,
            "read_cutover_authority": "none; combine this performance report with a separate eligible import signal target read audit",
            "defaults": ImportPreviewSignalPerformanceAuditConfig::default(),
            "timing": "One warmup plus five measured iterations by default; PostgreSQL JIT disabled; legacy/typed order alternates."
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
    fn defaults_match_frozen_c5_benchmark_contract() {
        let CommandAction::Run(config) = parse_args(&[]).unwrap() else {
            panic!("expected runnable config");
        };
        assert_eq!(config, ImportPreviewSignalPerformanceAuditConfig::default());
        assert_eq!(config.warmup_iterations, 1);
        assert_eq!(config.measured_iterations, 5);
        assert_eq!(config.maximum_query_regression_percent, 15.0);
        assert_eq!(config.maximum_write_regression_percent, 15.0);
    }

    #[test]
    fn parses_inline_and_separate_values() {
        let CommandAction::Run(config) = parse_args(&args(&[
            "--warmup-iterations=2",
            "--measured-iterations",
            "10",
            "--query-page-size=25",
            "--maximum-query-regression-percent",
            "14.5",
            "--maximum-write-regression-percent=12.5",
        ]))
        .unwrap() else {
            panic!("expected runnable config");
        };
        assert_eq!(config.warmup_iterations, 2);
        assert_eq!(config.measured_iterations, 10);
        assert_eq!(config.query_page_size, 25);
        assert_eq!(config.maximum_query_regression_percent, 14.5);
        assert_eq!(config.maximum_write_regression_percent, 12.5);
        assert!(matches!(
            parse_args(&args(&["-h"])),
            Ok(CommandAction::Help)
        ));
    }

    #[test]
    fn rejects_missing_unknown_non_finite_and_out_of_range_values() {
        for invalid in [
            args(&["--measured-iterations"]),
            args(&["--unknown", "1"]),
            args(&["--warmup-iterations", "4"]),
            args(&["--measured-iterations", "0"]),
            args(&["--query-page-size", "1001"]),
            args(&["--maximum-query-regression-percent", "NaN"]),
            args(&["--maximum-write-regression-percent", "15.1"]),
        ] {
            assert!(parse_args(&invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn shared_redaction_hides_full_urls_and_password_fragments() {
        let database_url =
            "postgresql://audit_user:c5fc-fake-password-sentinel@database.internal/audit";
        let message = format!(
            "connection failed for {database_url}: password c5fc-fake-password-sentinel rejected"
        );
        let redacted = redact_database_error(&message, Some(database_url));
        assert!(!redacted.contains(database_url));
        assert!(!redacted.contains("c5fc-fake-password-sentinel"));
        assert!(redacted.contains("[REDACTED_DATABASE_URL]"));
        assert!(redacted.contains("[REDACTED_DATABASE_PASSWORD]"));
    }
}
