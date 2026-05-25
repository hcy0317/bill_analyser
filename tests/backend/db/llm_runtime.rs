use std::time::Duration;

use bill_analyser_db::{
    accept_llm_candidate, activate_llm_config, count_llm_candidates, create_llm_candidate,
    create_llm_config, default_llm_runtime_config, delete_llm_config,
    effective_llm_config_from_saved, get_active_llm_config, get_llm_candidate_by_id,
    init_llm_runtime_schema, list_llm_candidates, list_llm_configs, reject_llm_candidate,
    update_llm_candidate_status, update_llm_config, LlmCandidateDraft, LlmConfigDraft,
    LlmConfigUpdate, SqliteConnectionConfig, SqliteDbPath, SqliteRuntime,
};
use rusqlite::{params, Connection};
use serde_json::json;

#[test]
fn llm_runtime_persists_configs_and_candidates_with_review_decisions(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("llm-runtime.db");
    let runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: SqliteDbPath::temporary_file(&db_path)?,
        create_if_missing: true,
        busy_timeout: Duration::from_secs(1),
    })?;
    init_llm_runtime_schema(runtime.connection())?;
    seed_rule_tables(&runtime)?;

    let first = create_llm_config(
        runtime.connection(),
        42,
        &LlmConfigDraft {
            name: "primary".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4.1-mini".to_string(),
            api_key: "secret-one".to_string(),
            base_url: "https://api.example.test".to_string(),
            credential_config: json!({}),
            advanced_settings: json!({
                "reasoning_depth": "high",
                "temperature": 0.7,
                "max_tokens": 1024,
                "ignored": true,
            }),
            is_active: true,
        },
    )?;
    assert_eq!(first["is_active"], 1);
    assert_eq!(first["advanced_settings"]["reasoning_depth"], "high");
    assert_eq!(first["credential_config"]["access_token"], "secret-one");
    assert_eq!(first["credential_config"]["credential_mode"], "api_key");
    assert_eq!(
        first["advanced_settings"]["ignored"],
        serde_json::Value::Null
    );

    let second = create_llm_config(
        runtime.connection(),
        42,
        &LlmConfigDraft {
            name: "backup".to_string(),
            provider: "claude".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            api_key: "secret-two".to_string(),
            base_url: "".to_string(),
            credential_config: json!({}),
            advanced_settings: json!({}),
            is_active: true,
        },
    )?;
    assert_eq!(second["is_active"], 1);
    let configs = list_llm_configs(runtime.connection(), 42)?;
    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0]["name"], "backup");
    assert_eq!(configs[1]["is_active"], 0);

    let updated = update_llm_config(
        runtime.connection(),
        second["id"].as_i64().expect("config id"),
        42,
        &LlmConfigUpdate {
            model: Some("claude-3-7-sonnet".to_string()),
            api_key: None,
            ..LlmConfigUpdate::default()
        },
    )?
    .expect("updated config");
    assert_eq!(updated["model"], "claude-3-7-sonnet");
    assert_eq!(updated["api_key"], "secret-two");
    assert!(activate_llm_config(
        runtime.connection(),
        first["id"].as_i64().expect("config id"),
        42
    )?);
    assert_eq!(
        get_active_llm_config(runtime.connection(), 42)?.expect("active")["name"],
        "primary"
    );

    let candidate_id = seed_rule_candidate(&runtime, 42)?;
    let candidates = list_llm_candidates(runtime.connection(), 42, Some("pending"), None, 50, 0)?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        count_llm_candidates(runtime.connection(), 42, Some("pending"), None)?,
        1
    );
    let accepted =
        accept_llm_candidate(runtime.connection(), candidate_id, 42)?.expect("accepted candidate");
    assert_eq!(accepted["status"], "accepted");
    assert!(accepted["created_rule_id"].as_i64().is_some());
    let duplicate_rule_id =
        seed_rule_candidate_with_expression(&runtime, 42, "餐饮", "咖啡", " OR={咖啡} ")?;
    let duplicate_rule = accept_llm_candidate(runtime.connection(), duplicate_rule_id, 42)?
        .expect("duplicate rule candidate");
    assert_eq!(duplicate_rule["status"], "accepted");
    assert!(duplicate_rule.get("created_rule_id").is_none());
    let invalid_rule_id =
        seed_rule_candidate_with_expression(&runtime, 42, "餐饮", "咖啡", "(OR={broken}")?;
    let invalid_rule =
        accept_llm_candidate(runtime.connection(), invalid_rule_id, 42)?.expect("invalid rule");
    assert_eq!(invalid_rule["status"], "accepted");
    assert!(invalid_rule.get("created_rule_id").is_none());

    let rejected_id = seed_plain_candidate(&runtime, 42)?;
    assert_eq!(
        reject_llm_candidate(runtime.connection(), rejected_id, 42)?,
        Some(true)
    );
    assert_eq!(
        count_llm_candidates(runtime.connection(), 42, Some("rejected"), None)?,
        1
    );
    Ok(())
}

#[test]
fn llm_runtime_persists_provider_auth_profile_and_keeps_legacy_api_key(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("llm-auth-profile.db");
    let runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: SqliteDbPath::temporary_file(&db_path)?,
        create_if_missing: true,
        busy_timeout: Duration::from_secs(1),
    })?;
    init_llm_runtime_schema(runtime.connection())?;

    let config = create_llm_config(
        runtime.connection(),
        42,
        &LlmConfigDraft {
            name: "token profile".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: "".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            credential_config: json!({
                "credentialMode": "auth_json",
                "auth": {
                    "accessToken": "profile-access",
                    "refreshToken": "profile-refresh",
                    "expiresAt": "2099-01-01T00:00:00Z"
                },
                "tokenEndpoint": "https://api.openai.com/oauth/token",
                "refreshBody": {"client_secret": "client-secret"}
            }),
            advanced_settings: json!({}),
            is_active: true,
        },
    )?;

    assert_eq!(config["credential_config"]["credential_mode"], "auth_json");
    assert_eq!(
        config["credential_config"]["access_token"],
        "profile-access"
    );
    assert_eq!(
        config["credential_config"]["refresh_token"],
        "profile-refresh"
    );
    let runtime_config = effective_llm_config_from_saved(runtime.connection(), 42)?;
    assert_eq!(
        runtime_config["provider_config"]["api_key"],
        "profile-access"
    );
    assert_eq!(
        runtime_config["credential_config"]["token_endpoint"],
        "https://api.openai.com/oauth/token"
    );

    let updated = update_llm_config(
        runtime.connection(),
        config["id"].as_i64().expect("config id"),
        42,
        &LlmConfigUpdate {
            credential_config: Some(json!({
                "credential_mode": "access_token",
                "access_token": "rotated-access",
                "refresh_token": "rotated-refresh"
            })),
            ..LlmConfigUpdate::default()
        },
    )?
    .expect("updated config");
    assert_eq!(
        updated["credential_config"]["access_token"],
        "rotated-access"
    );
    assert_eq!(
        effective_llm_config_from_saved(runtime.connection(), 42)?["provider_config"]["api_key"],
        "rotated-access"
    );

    Ok(())
}

#[test]
fn llm_runtime_handles_legacy_schema_and_edge_decisions() -> Result<(), Box<dyn std::error::Error>>
{
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("llm-runtime-legacy.db");
    let runtime = SqliteRuntime::open(SqliteConnectionConfig {
        path: SqliteDbPath::temporary_file(&db_path)?,
        create_if_missing: true,
        busy_timeout: Duration::from_secs(1),
    })?;
    runtime.connection().execute_batch(
        "
        CREATE TABLE llm_configs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL DEFAULT 1,
            name TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT 'openai',
            model TEXT NOT NULL DEFAULT '',
            api_key TEXT DEFAULT '',
            base_url TEXT DEFAULT '',
            is_active INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
            UNIQUE(user_id, name)
        );
        ",
    )?;
    init_llm_runtime_schema(runtime.connection())?;
    seed_rule_tables(&runtime)?;

    assert_eq!(
        effective_llm_config_from_saved(runtime.connection(), 7)?,
        default_llm_runtime_config()
    );

    let active = create_llm_config(
        runtime.connection(),
        7,
        &LlmConfigDraft {
            name: "active".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4.1-mini".to_string(),
            api_key: "active-secret".to_string(),
            base_url: "https://active.example.test".to_string(),
            credential_config: json!({}),
            advanced_settings: json!({"temperature": 0.2}),
            is_active: true,
        },
    )?;
    let inactive = create_llm_config(
        runtime.connection(),
        7,
        &LlmConfigDraft {
            name: "legacy".to_string(),
            provider: "openai".to_string(),
            model: "gpt-4.1".to_string(),
            api_key: "legacy-secret".to_string(),
            base_url: "".to_string(),
            credential_config: json!({}),
            advanced_settings: json!({"max_tokens": 512}),
            is_active: false,
        },
    )?;

    let inactive_id = inactive["id"].as_i64().expect("inactive config id");
    let updated = update_llm_config(
        runtime.connection(),
        inactive_id,
        7,
        &LlmConfigUpdate {
            name: Some("renamed".to_string()),
            provider: Some("claude".to_string()),
            model: Some("claude-3-7-sonnet".to_string()),
            api_key: Some("new-secret".to_string()),
            base_url: Some("https://llm.example.test".to_string()),
            advanced_settings: Some(json!({
                "reasoning_depth": "medium",
                "temperature": 0.4,
                "max_tokens": 2048,
                "unknown": true
            })),
            is_active: Some(true),
            ..LlmConfigUpdate::default()
        },
    )?
    .expect("updated config");
    assert_eq!(updated["name"], "renamed");
    assert_eq!(updated["provider"], "claude");
    assert_eq!(updated["api_key"], "new-secret");
    assert_eq!(updated["base_url"], "https://llm.example.test");
    assert_eq!(updated["advanced_settings"]["reasoning_depth"], "medium");
    assert_eq!(
        updated["advanced_settings"]["unknown"],
        serde_json::Value::Null
    );
    assert_eq!(updated["is_active"], 1);
    assert_eq!(
        get_active_llm_config(runtime.connection(), 7)?.expect("active")["id"],
        json!(inactive_id)
    );

    let active_id = active["id"].as_i64().expect("active config id");
    assert_eq!(
        update_llm_config(
            runtime.connection(),
            active_id,
            7,
            &LlmConfigUpdate {
                is_active: Some(false),
                ..LlmConfigUpdate::default()
            },
        )?
        .expect("deactivated config")["is_active"],
        0
    );
    assert_eq!(
        update_llm_config(runtime.connection(), 9999, 7, &LlmConfigUpdate::default())?,
        None
    );
    assert!(!activate_llm_config(runtime.connection(), 9999, 7)?);
    assert!(delete_llm_config(runtime.connection(), active_id, 7)?);
    assert!(!delete_llm_config(runtime.connection(), active_id, 7)?);

    let missing_category_id = seed_rule_candidate_for_category(&runtime, 7, "餐饮", "晚餐")?;
    assert_eq!(
        count_llm_candidates(runtime.connection(), 7, None, Some("rule_synthesis"))?,
        1
    );
    assert_eq!(
        list_llm_candidates(runtime.connection(), 7, None, Some("rule_synthesis"), 25, 0)?.len(),
        1
    );
    let accepted = accept_llm_candidate(runtime.connection(), missing_category_id, 7)?
        .expect("accepted candidate without matching category");
    assert_eq!(accepted["status"], "accepted");
    assert!(accepted.get("created_rule_id").is_none());

    let classification_id = seed_plain_candidate(&runtime, 7)?;
    assert!(update_llm_candidate_status(
        runtime.connection(),
        classification_id,
        "pending",
        7
    )?);
    assert_eq!(
        get_llm_candidate_by_id(runtime.connection(), classification_id, 7)?.expect("candidate")
            ["status"],
        "pending"
    );
    assert_eq!(accept_llm_candidate(runtime.connection(), 9999, 7)?, None);
    assert_eq!(reject_llm_candidate(runtime.connection(), 9999, 7)?, None);
    Ok(())
}

#[test]
fn llm_candidate_creation_reports_schema_and_reload_errors(
) -> Result<(), Box<dyn std::error::Error>> {
    let missing_schema = Connection::open_in_memory()?;
    assert!(create_llm_candidate(&missing_schema, &candidate_draft(42)).is_err());

    let with_trigger = Connection::open_in_memory()?;
    init_llm_runtime_schema(&with_trigger)?;
    with_trigger.execute_batch(
        "
        CREATE TRIGGER delete_llm_candidate_after_insert
        AFTER INSERT ON llm_candidates
        BEGIN
            DELETE FROM llm_candidates WHERE id = NEW.id;
        END;
        ",
    )?;
    assert!(create_llm_candidate(&with_trigger, &candidate_draft(42)).is_err());
    Ok(())
}

fn candidate_draft(user_id: i64) -> LlmCandidateDraft {
    LlmCandidateDraft {
        user_id,
        candidate_type: "classification".to_string(),
        source_bill_ids: vec![1, 2],
        suggested_main_category: "餐饮".to_string(),
        suggested_sub_category: "咖啡".to_string(),
        suggested_rule_expression: String::new(),
        confidence: 0.8,
        llm_provider: "openai".to_string(),
        llm_model: "gpt".to_string(),
        llm_response_raw: "{}".to_string(),
    }
}

fn seed_rule_tables(runtime: &SqliteRuntime) -> rusqlite::Result<()> {
    runtime.connection().execute_batch(
        "
        CREATE TABLE IF NOT EXISTS categories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            main_category TEXT NOT NULL,
            sub_category TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS category_rules (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            category_id INTEGER NOT NULL,
            name TEXT NOT NULL DEFAULT '',
            priority INTEGER NOT NULL DEFAULT 100,
            rule_expression TEXT NOT NULL,
            regex_enabled INTEGER DEFAULT 0,
            enabled INTEGER DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        ",
    )?;
    runtime.connection().execute(
        "INSERT INTO categories(user_id, main_category, sub_category) VALUES (42, '餐饮', '咖啡')",
        [],
    )?;
    Ok(())
}

fn seed_rule_candidate(runtime: &SqliteRuntime, user_id: i64) -> rusqlite::Result<i64> {
    seed_rule_candidate_for_category(runtime, user_id, "餐饮", "咖啡")
}

fn seed_rule_candidate_for_category(
    runtime: &SqliteRuntime,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
) -> rusqlite::Result<i64> {
    seed_rule_candidate_with_expression(runtime, user_id, main_category, sub_category, "OR={咖啡}")
}

fn seed_rule_candidate_with_expression(
    runtime: &SqliteRuntime,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
    expression: &str,
) -> rusqlite::Result<i64> {
    runtime.connection().execute(
        "INSERT INTO llm_candidates(
            user_id, type, source_bill_ids, suggested_main_category, suggested_sub_category,
            suggested_rule_expression, confidence, llm_provider, llm_model, llm_response_raw, status
         )
         VALUES (?1, 'rule_synthesis', '[1,2]', ?2, ?3, ?4, 0.91, 'openai', 'gpt', '{}', 'pending')",
        params![user_id, main_category, sub_category, expression],
    )?;
    Ok(runtime.connection().last_insert_rowid())
}

fn seed_plain_candidate(runtime: &SqliteRuntime, user_id: i64) -> rusqlite::Result<i64> {
    runtime.connection().execute(
        "INSERT INTO llm_candidates(
            user_id, type, source_bill_ids, suggested_main_category, suggested_sub_category,
            suggested_rule_expression, confidence, llm_provider, llm_model, llm_response_raw, status
         )
         VALUES (?1, 'classification', '[3]', '交通', '打车', '', 0.7, 'openai', 'gpt', '{}', 'pending')",
        params![user_id],
    )?;
    Ok(runtime.connection().last_insert_rowid())
}
