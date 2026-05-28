use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::{self, Command},
};

use bill_analyser_db::{
    apply_postgres_account_recovery, build_postgres_account_recovery_dry_run,
    inspect_postgres_account_recovery_source, AccountRecoveryApplyOptions,
    AccountRecoveryTargetRef, DbError, DbResult, ACCOUNT_RECOVERY_DEFAULT_SOURCE_USER_ID,
};
use sqlx::postgres::PgPoolOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandMode {
    Preflight,
    DryRun,
    Apply,
    Help,
}

#[derive(Debug, Default)]
struct CliArgs {
    mode: Option<CommandMode>,
    sqlite_path: Option<PathBuf>,
    postgres_url: Option<String>,
    target_user_ref: Option<String>,
    source_user_id: Option<i64>,
    manifest_id: Option<String>,
    confirm_target_user_id: Option<i64>,
    confirm_target_username: Option<String>,
    confirm_target_email: Option<String>,
    snapshot_dir: Option<PathBuf>,
    workspace_root: Option<PathBuf>,
    allow_unexpected_source_shape: bool,
    output_path: Option<PathBuf>,
}

#[derive(Debug)]
struct CommandOutput {
    output_path: Option<PathBuf>,
    json: String,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("postgres account recovery error: {error}");
        process::exit(2);
    }
}

async fn run() -> DbResult<()> {
    let output = execute(env::args().skip(1)).await?;
    emit_output(output)
}

async fn execute(args: impl IntoIterator<Item = String>) -> DbResult<CommandOutput> {
    let args = parse_args(args)?;
    let source_user_id = args
        .source_user_id
        .unwrap_or(ACCOUNT_RECOVERY_DEFAULT_SOURCE_USER_ID);
    match args.mode.ok_or_else(|| invalid("missing --mode"))? {
        CommandMode::Preflight => {
            let sqlite_path = args
                .sqlite_path
                .ok_or_else(|| invalid("preflight requires --sqlite"))?;
            let report = inspect_postgres_account_recovery_source(sqlite_path, source_user_id)?;
            command_output(args.output_path, &report)
        }
        CommandMode::DryRun => {
            let sqlite_path = args
                .sqlite_path
                .ok_or_else(|| invalid("dry-run requires --sqlite"))?;
            let target_user_ref = target_ref(args.target_user_ref, "dry-run")?;
            let postgres_url = postgres_url_arg(args.postgres_url, "dry-run")?;
            let pool = connect_postgres(&postgres_url).await?;
            let report = build_postgres_account_recovery_dry_run(
                sqlite_path,
                source_user_id,
                &pool,
                &target_user_ref,
            )
            .await?;
            command_output(args.output_path, &report)
        }
        CommandMode::Apply => {
            let sqlite_path = args
                .sqlite_path
                .ok_or_else(|| invalid("apply requires --sqlite"))?;
            let target_user_ref = target_ref(args.target_user_ref, "apply")?;
            let postgres_url = postgres_url_arg(args.postgres_url, "apply")?;
            let manifest_id = args
                .manifest_id
                .ok_or_else(|| invalid("apply requires --manifest-id"))?;
            let snapshot_dir = args.snapshot_dir.unwrap_or_else(|| {
                PathBuf::from(format!(".git/ai/recovery-snapshots/{manifest_id}"))
            });
            let workspace_root = args.workspace_root.unwrap_or(env::current_dir()?);
            let options = AccountRecoveryApplyOptions {
                source_user_id,
                target_user_ref,
                manifest_id,
                confirm_target_user_id: args
                    .confirm_target_user_id
                    .ok_or_else(|| invalid("apply requires --confirm-target-user-id"))?,
                confirm_target_username: args
                    .confirm_target_username
                    .ok_or_else(|| invalid("apply requires --confirm-target-username"))?,
                confirm_target_email: args.confirm_target_email.unwrap_or_default(),
                snapshot_dir,
                workspace_root,
                allow_unexpected_source_shape: args.allow_unexpected_source_shape,
            };
            let pool = connect_postgres(&postgres_url).await?;
            let report = apply_postgres_account_recovery(sqlite_path, &pool, options).await?;
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
                    "preflight" => CommandMode::Preflight,
                    "dry-run" => CommandMode::DryRun,
                    "apply" => CommandMode::Apply,
                    _ => {
                        return Err(invalid(
                            "unsupported --mode; use preflight, dry-run, or apply",
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
            "--postgres-url" => {
                parsed.postgres_url = Some(
                    args.next()
                        .ok_or_else(|| invalid("--postgres-url requires a connection string"))?,
                );
            }
            "--target-user" => {
                parsed.target_user_ref = Some(
                    args.next()
                        .ok_or_else(|| invalid("--target-user requires a user ref"))?,
                );
            }
            "--source-user-id" => {
                parsed.source_user_id = Some(parse_i64_arg(&mut args, "--source-user-id")?);
            }
            "--manifest-id" => {
                parsed.manifest_id = Some(
                    args.next()
                        .ok_or_else(|| invalid("--manifest-id requires a value"))?,
                );
            }
            "--confirm-target-user-id" => {
                parsed.confirm_target_user_id =
                    Some(parse_i64_arg(&mut args, "--confirm-target-user-id")?);
            }
            "--confirm-target-username" => {
                parsed.confirm_target_username = Some(
                    args.next()
                        .ok_or_else(|| invalid("--confirm-target-username requires a value"))?,
                );
            }
            "--confirm-target-email" => {
                parsed.confirm_target_email = Some(
                    args.next()
                        .ok_or_else(|| invalid("--confirm-target-email requires a value"))?,
                );
            }
            "--snapshot-dir" => {
                parsed.snapshot_dir = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| invalid("--snapshot-dir requires a path"))?,
                ));
            }
            "--workspace-root" => {
                parsed.workspace_root =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        invalid("--workspace-root requires a path")
                    })?));
            }
            "--output" => {
                parsed.output_path = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| invalid("--output requires a path"))?,
                ));
            }
            "--allow-unexpected-source-shape" => {
                parsed.allow_unexpected_source_shape = true;
            }
            "--help" | "-h" => {
                parsed.mode = Some(CommandMode::Help);
            }
            _ => return Err(invalid(format!("unknown argument {arg}"))),
        }
    }
    Ok(parsed)
}

async fn connect_postgres(postgres_url: &str) -> DbResult<bill_analyser_db::PostgresPool> {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(postgres_url)
        .await
        .map_err(|error| {
            DbError::InvalidOperation(format!("connect Postgres for account recovery: {error}"))
        })
}

fn target_ref(value: Option<String>, mode: &str) -> DbResult<AccountRecoveryTargetRef> {
    AccountRecoveryTargetRef::new(
        value.ok_or_else(|| invalid(format!("{mode} requires --target-user")))?,
    )
}

fn parse_i64_arg(args: &mut impl Iterator<Item = String>, name: &'static str) -> DbResult<i64> {
    let value = args
        .next()
        .ok_or_else(|| invalid(format!("{name} requires a value")))?;
    value
        .parse::<i64>()
        .map_err(|error| invalid(format!("{name} must be an integer: {error}")))
}

fn command_output<T: serde::Serialize>(
    output_path: Option<PathBuf>,
    value: &T,
) -> DbResult<CommandOutput> {
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        DbError::InvalidOperation(format!("serialize account recovery CLI output: {error}"))
    })?;
    Ok(CommandOutput { output_path, json })
}

fn emit_output(output: CommandOutput) -> DbResult<()> {
    if let Some(output_path) = output.output_path {
        ensure_output_path_allowed(&env::current_dir()?, &output_path)?;
        std::fs::write(output_path, output.json)?;
    } else {
        println!("{}", output.json);
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> DbError {
    DbError::InvalidOperation(format!("postgres account recovery CLI: {}", message.into()))
}

fn postgres_url_arg(value: Option<String>, mode: &str) -> DbResult<String> {
    value
        .or_else(|| env::var("BILL_ANALYSER_POSTGRES_URL").ok())
        .ok_or_else(|| {
            invalid(format!(
                "{mode} requires --postgres-url or BILL_ANALYSER_POSTGRES_URL"
            ))
        })
}

fn ensure_output_path_allowed(workspace_root: &Path, output_path: &Path) -> DbResult<()> {
    reject_parent_dir_components(output_path, "output path")?;
    let workspace_root = canonical_or_absolutize(workspace_root)?;
    let output_path = if output_path.is_absolute() {
        output_path.to_path_buf()
    } else {
        workspace_root.join(output_path)
    };
    reject_parent_dir_components(&output_path, "output path")?;
    if !output_path.starts_with(&workspace_root) {
        return Ok(());
    }
    let relative = output_path.strip_prefix(&workspace_root).map_err(|error| {
        DbError::InvalidOperation(format!(
            "account recovery output path strip failed: {error}"
        ))
    })?;
    if relative.starts_with(".git") {
        return Ok(());
    }
    let output = Command::new("git")
        .arg("check-ignore")
        .arg("-v")
        .arg("--")
        .arg(relative)
        .current_dir(&workspace_root)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(DbError::InvalidOperation(format!(
        "account recovery output path {} is inside the workspace but is not git-ignored",
        output_path.display()
    )))
}

fn reject_parent_dir_components(path: &Path, label: &str) -> DbResult<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(DbError::InvalidOperation(format!(
            "account recovery {label} must not contain parent-directory components"
        )));
    }
    Ok(())
}

fn canonical_or_absolutize(path: &Path) -> DbResult<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    fs::canonicalize(&path).or(Ok(path))
}

fn help_text() -> &'static str {
    "Usage:\n  bill_postgres_account_recovery --mode preflight --sqlite <db> [--source-user-id 5] [--output <ignored-or-outside-worktree>.json]\n  bill_postgres_account_recovery --mode dry-run --sqlite <db> --target-user <id|username|email> [--postgres-url <url>|BILL_ANALYSER_POSTGRES_URL] [--source-user-id 5] [--output <ignored-or-outside-worktree>.json]\n  bill_postgres_account_recovery --mode apply --sqlite <db> --target-user <id|username|email> [--postgres-url <url>|BILL_ANALYSER_POSTGRES_URL] --manifest-id <manifest> --confirm-target-user-id <id> --confirm-target-username <username> [--confirm-target-email <email>] [--snapshot-dir .git/ai/recovery-snapshots/<manifest>] [--workspace-root <repo>] [--allow-unexpected-source-shape] [--output <ignored-or-outside-worktree>.json]"
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::Value;

    #[tokio::test]
    async fn cli_private_edges_cover_parse_and_preflight_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sqlite_path = temp_dir.path().join("source.db");
        let connection = Connection::open(&sqlite_path).unwrap();
        connection
            .execute_batch(
                "
                CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, email TEXT);
                CREATE TABLE accounts (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, name TEXT NOT NULL, aliases TEXT);
                INSERT INTO users VALUES (5, 'source', 'source@example.test');
                INSERT INTO accounts VALUES (1, 5, 'Wallet', '[\"Cash\"]');
                ",
            )
            .unwrap();
        drop(connection);

        let help = execute(["--help".to_string()]).await.unwrap();
        assert!(help.json.contains("Usage:"));

        let preflight = execute([
            "--mode".to_string(),
            "preflight".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
        ])
        .await
        .unwrap();
        assert!(preflight.json.contains("\"source_user_id\": 5"));
        assert!(preflight.json.contains("\"generated_account_rules\": 1"));

        assert!(execute([
            "--mode".to_string(),
            "dry-run".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
        ])
        .await
        .unwrap_err()
        .to_string()
        .contains("dry-run requires --target-user"));

        assert!(execute([
            "--mode".to_string(),
            "apply".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
            "--postgres-url".to_string(),
            "not-a-postgres-url".to_string(),
            "--target-user".to_string(),
            "target".to_string(),
        ])
        .await
        .unwrap_err()
        .to_string()
        .contains("apply requires --manifest-id"));

        assert!(
            parse_args(["--source-user-id".to_string(), "nope".to_string()])
                .unwrap_err()
                .to_string()
                .contains("must be an integer")
        );

        assert!(parse_args(["--mode".to_string(), "bogus".to_string()])
            .unwrap_err()
            .to_string()
            .contains("unsupported --mode"));
        assert!(parse_args(["--workspace-root".to_string()])
            .unwrap_err()
            .to_string()
            .contains("--workspace-root requires a path"));
        assert!(parse_args(["--unknown".to_string()])
            .unwrap_err()
            .to_string()
            .contains("unknown argument"));
        assert!(execute([
            "--mode".to_string(),
            "dry-run".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
            "--target-user".to_string(),
            "target".to_string(),
        ])
        .await
        .unwrap_err()
        .to_string()
        .contains("requires --postgres-url"));
        assert!(execute([
            "--mode".to_string(),
            "apply".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
            "--target-user".to_string(),
            "target".to_string(),
            "--postgres-url".to_string(),
            "not-a-postgres-url".to_string(),
            "--manifest-id".to_string(),
            "manifest".to_string(),
            "--confirm-target-user-id".to_string(),
            "1".to_string(),
            "--confirm-target-username".to_string(),
            "target".to_string(),
        ])
        .await
        .unwrap_err()
        .to_string()
        .contains("connect Postgres"));
    }

    #[tokio::test]
    async fn cli_execute_covers_dry_run_apply_and_output_edges_when_postgres_is_available() {
        let Ok(postgres_url) = std::env::var("BILL_ANALYSER_TEST_POSTGRES_URL") else {
            return;
        };
        let pool = connect_postgres(&postgres_url).await.unwrap();
        bill_analyser_db::run_postgres_migrations(&pool)
            .await
            .unwrap();

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let target_username = format!("cli-recovery-target-{suffix}");
        let target_email = format!("cli-recovery-target-{suffix}@example.test");
        let target_user_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, 'hash') RETURNING id",
        )
        .bind(&target_username)
        .bind(&target_email)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO accounts (user_id, name, currency, balance_cents) VALUES ($1, 'old account', 'CNY', 1)",
        )
        .bind(target_user_id)
        .execute(&pool)
        .await
        .unwrap();

        let temp_dir = tempfile::tempdir().unwrap();
        let sqlite_path = temp_dir.path().join("source.db");
        create_apply_fixture(&sqlite_path);
        let workspace_root = temp_dir.path().join("workspace");
        let dry_run_output = workspace_root.join(".git").join("ai").join("dry-run.json");
        let dry_run = execute([
            "--mode".to_string(),
            "dry-run".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
            "--target-user".to_string(),
            target_user_id.to_string(),
            "--postgres-url".to_string(),
            postgres_url.clone(),
            "--source-user-id".to_string(),
            "5".to_string(),
            "--output".to_string(),
            dry_run_output.display().to_string(),
        ])
        .await
        .unwrap();
        assert_eq!(dry_run.output_path, Some(dry_run_output));
        let dry_run_json: Value = serde_json::from_str(&dry_run.json).unwrap();
        let manifest_id = dry_run_json["manifest_id"].as_str().unwrap();
        assert!(manifest_id.starts_with("account-recovery-"));

        let snapshot_dir = workspace_root
            .join(".git")
            .join("ai")
            .join("recovery-snapshots")
            .join(manifest_id);
        let apply = execute([
            "--mode".to_string(),
            "apply".to_string(),
            "--sqlite".to_string(),
            sqlite_path.display().to_string(),
            "--target-user".to_string(),
            target_user_id.to_string(),
            "--postgres-url".to_string(),
            postgres_url,
            "--manifest-id".to_string(),
            manifest_id.to_string(),
            "--confirm-target-user-id".to_string(),
            target_user_id.to_string(),
            "--confirm-target-username".to_string(),
            target_username.clone(),
            "--confirm-target-email".to_string(),
            target_email,
            "--snapshot-dir".to_string(),
            snapshot_dir.display().to_string(),
            "--workspace-root".to_string(),
            workspace_root.display().to_string(),
            "--allow-unexpected-source-shape".to_string(),
        ])
        .await
        .unwrap();
        let apply_json: Value = serde_json::from_str(&apply.json).unwrap();
        assert_eq!(apply_json["target_post_counts"]["accounts"], 1);
        assert_eq!(apply_json["target_post_counts"]["bills"], 1);
        assert!(snapshot_dir.join("manifest.redacted.json").exists());

        let emitted = temp_dir.path().join("outside-output.json");
        emit_output(CommandOutput {
            output_path: Some(emitted.clone()),
            json: apply.json,
        })
        .unwrap();
        assert!(emitted.exists());

        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(target_user_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    #[test]
    fn cli_output_path_guard_covers_allowed_and_rejected_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        assert!(ensure_output_path_allowed(temp_dir.path(), Path::new(".git/ai/out.json")).is_ok());
        fs::write(temp_dir.path().join(".gitignore"), "ignored.json\n").unwrap();
        let git_init = Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        assert!(git_init.status.success());
        assert!(ensure_output_path_allowed(temp_dir.path(), Path::new("ignored.json")).is_ok());
        assert!(
            ensure_output_path_allowed(temp_dir.path(), Path::new(".git/../out.json"))
                .unwrap_err()
                .to_string()
                .contains("parent-directory")
        );
        assert!(
            ensure_output_path_allowed(temp_dir.path(), Path::new("tracked.json"))
                .unwrap_err()
                .to_string()
                .contains("not git-ignored")
        );
        assert_eq!(
            postgres_url_arg(Some("postgres://example".to_string()), "dry-run").unwrap(),
            "postgres://example"
        );
        if std::env::var_os("BILL_ANALYSER_POSTGRES_URL").is_none() {
            assert!(postgres_url_arg(None, "dry-run")
                .unwrap_err()
                .to_string()
                .contains("requires --postgres-url"));
        }
        assert!(canonical_or_absolutize(Path::new("Cargo.toml"))
            .unwrap()
            .is_absolute());
        emit_output(CommandOutput {
            output_path: None,
            json: "{}".to_string(),
        })
        .unwrap();
        assert!(command_output(None, &BrokenSerialize)
            .unwrap_err()
            .to_string()
            .contains("serialize account recovery CLI output"));
    }

    struct BrokenSerialize;

    impl serde::Serialize for BrokenSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("broken"))
        }
    }

    fn create_apply_fixture(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT NOT NULL, email TEXT);
                CREATE TABLE accounts (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    type INTEGER,
                    currency TEXT,
                    balance REAL,
                    aliases TEXT,
                    hidden INTEGER,
                    display_order INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    type INTEGER,
                    main_category TEXT,
                    sub_category TEXT,
                    priority INTEGER,
                    hidden INTEGER,
                    created_at TEXT NOT NULL
                );
                CREATE TABLE bills (
                    id INTEGER PRIMARY KEY,
                    user_id INTEGER NOT NULL,
                    date TEXT NOT NULL,
                    type TEXT NOT NULL,
                    amount REAL NOT NULL,
                    counterparty TEXT,
                    description TEXT,
                    payment_method TEXT,
                    main_category TEXT,
                    sub_category TEXT,
                    hash TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    source_account_id INTEGER,
                    destination_account_id INTEGER
                );
                INSERT INTO users VALUES (5, 'source', 'source@example.test');
                INSERT INTO accounts VALUES (42, 5, 'Wallet', 1, 'CNY', 123.45, '["Cash"]', 0, 1, '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z');
                INSERT INTO categories VALUES (50, 5, 1, 'Food', 'Lunch', 1, 0, '2026-01-01T00:00:00Z');
                INSERT INTO bills VALUES (70, 5, '2026-01-03T12:00:00Z', '支出', 19.99, 'Cafe', 'Lunch', 'Wallet', 'Food', 'Lunch', 'hash70', '2026-01-03T12:01:00Z', '2026-01-03T12:02:00Z', 42, 0);
                "#,
            )
            .unwrap();
    }
}
