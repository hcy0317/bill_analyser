use std::{env, error::Error, str::FromStr};

use bill_analyser_db::{run_postgres_migrations, PostgresPool};
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, Executor};

pub struct IsolatedPostgres {
    pub pool: PostgresPool,
    admin_pool: PostgresPool,
    pub db_name: String,
}

impl IsolatedPostgres {
    pub async fn cleanup(self) -> Result<(), Box<dyn Error>> {
        self.pool.close().await;
        self.admin_pool
            .execute(format!(r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#, self.db_name).as_str())
            .await?;
        Ok(())
    }
}

pub async fn isolated_postgres_database(
    prefix: &str,
) -> Result<Option<IsolatedPostgres>, Box<dyn Error>> {
    let Ok(postgres_url) = env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
        eprintln!("skipping PostgreSQL contract: BILL_ANALYSER_TEST_POSTGRES_URL is not set");
        return Ok(None);
    };

    let unique = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .abs();
    let db_name = format!("{prefix}_{unique}");
    let base_options = PgConnectOptions::from_str(&postgres_url)?;
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(base_options.clone().database("postgres"))
        .await?;
    admin_pool
        .execute(format!(r#"CREATE DATABASE "{}""#, db_name).as_str())
        .await?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(base_options.database(&db_name))
        .await?;
    run_postgres_migrations(&pool).await?;

    Ok(Some(IsolatedPostgres {
        pool,
        admin_pool,
        db_name,
    }))
}
