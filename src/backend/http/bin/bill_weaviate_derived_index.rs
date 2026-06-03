use std::{env, error::Error, process};

use bill_analyser_db::{DatabaseRuntimeConfig, DatabaseRuntimeProvider, DbError, PostgresPool};
use bill_analyser_http::{
    probe_weaviate_health, process_weaviate_outbox_once, rebuild_weaviate_from_postgres,
    HttpShellConfig, WeaviateHttpClient,
};
use serde_json::json;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }
    let mode = option_value(&args, "--mode")
        .or_else(|| args.first().cloned())
        .unwrap_or_else(|| "health".to_string());
    let config = HttpShellConfig::from_env()?;

    match mode.as_str() {
        "health" => {
            let status = probe_weaviate_health(&config).await;
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        "bootstrap" => {
            let report = WeaviateHttpClient::new(&config.weaviate)?
                .bootstrap_schema()
                .await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "process-outbox" => {
            let pool = postgres_pool(&config)?;
            let report = process_weaviate_outbox_once(&pool, &config.weaviate).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        "rebuild" => {
            let pool = postgres_pool(&config)?;
            let user_id = option_value(&args, "--user-id")
                .map(|value| value.parse::<i64>())
                .transpose()?;
            let report = rebuild_weaviate_from_postgres(&pool, &config.weaviate, user_id).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        other => {
            return Err(format!("unknown mode: {other}").into());
        }
    }

    Ok(())
}

fn postgres_pool(config: &HttpShellConfig) -> Result<PostgresPool, DbError> {
    let postgres_url = config.postgres_url.as_deref().ok_or_else(|| {
        DbError::InvalidOperation(
            "BILL_ANALYSER_POSTGRES_URL is required for Weaviate outbox/rebuild".to_string(),
        )
    })?;
    let provider = DatabaseRuntimeProvider::new(DatabaseRuntimeConfig::postgres(
        postgres_url.to_string(),
        config.timeout,
    ))?;
    Ok(provider.postgres_runtime()?.pool().clone())
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.windows(2)
        .find_map(|pair| (pair[0] == name).then(|| pair[1].clone()))
        .or_else(|| {
            args.iter()
                .find_map(|arg| arg.strip_prefix(&format!("{name}=")).map(ToOwned::to_owned))
        })
}

fn print_help() {
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "usage": "bill_weaviate_derived_index --mode <health|bootstrap|process-outbox|rebuild> [--user-id <id>]",
            "env": [
                "BILL_ANALYSER_WEAVIATE_ENABLED",
                "BILL_ANALYSER_WEAVIATE_ENDPOINT",
                "BILL_ANALYSER_WEAVIATE_API_KEY",
                "BILL_ANALYSER_WEAVIATE_COLLECTION_PREFIX",
                "BILL_ANALYSER_POSTGRES_URL"
            ]
        }))
        .expect("help json is serializable")
    );
}
