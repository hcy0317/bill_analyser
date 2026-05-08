use std::{collections::BTreeSet, error::Error};

use bill_analyser_db::{
    get_app_setting, get_app_setting_row, init_app_settings_schema, load_ocr_config_setting,
    set_app_setting, store_ocr_config_setting, AppSettingDraft, DbError, SqliteConnectionConfig,
    SqliteDbPath, SqliteRuntime, OCR_CONFIG_SETTING_KEY,
};
use serde_json::{json, Value};

fn runtime_for(path: &std::path::Path) -> Result<SqliteRuntime, Box<dyn Error>> {
    let db_path = SqliteDbPath::temporary_file(path)?;
    Ok(SqliteRuntime::open(SqliteConnectionConfig {
        path: db_path,
        create_if_missing: true,
        busy_timeout: std::time::Duration::from_secs(1),
    })?)
}

fn table_columns(runtime: &SqliteRuntime, table: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut statement = runtime
        .connection()
        .prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    Ok(rows.collect::<Result<BTreeSet<_>, _>>()?)
}

fn table_indexes(
    runtime: &SqliteRuntime,
    table: &str,
) -> Result<Vec<(String, bool)>, Box<dyn Error>> {
    let mut statement = runtime
        .connection()
        .prepare(&format!("PRAGMA index_list({table})"))?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(2)? != 0))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[test]
fn app_settings_schema_matches_python_shape_and_upserts_by_key() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("settings.db"))?;
    init_app_settings_schema(runtime.connection())?;

    assert_eq!(
        table_columns(&runtime, "app_settings")?,
        BTreeSet::from([
            "id".to_string(),
            "key".to_string(),
            "value".to_string(),
            "value_type".to_string(),
            "description".to_string(),
            "is_encrypted".to_string(),
            "updated_at".to_string(),
            "created_at".to_string(),
        ])
    );
    assert!(table_indexes(&runtime, "app_settings")?
        .iter()
        .any(|(name, _)| name == "idx_app_settings_key"));

    assert!(set_app_setting(
        runtime.connection(),
        &AppSettingDraft {
            key: " receipt_ocr_config ".to_string(),
            value: r#"{"provider":"disabled"}"#.to_string(),
            value_type: " json ".to_string(),
            description: Some("Receipt OCR runtime configuration".to_string()),
            is_encrypted: false,
        },
    )?);
    let first = get_app_setting_row(runtime.connection(), OCR_CONFIG_SETTING_KEY)?
        .expect("row should be inserted");
    assert_eq!(first.key, OCR_CONFIG_SETTING_KEY);
    assert_eq!(first.value_type, "json");
    assert_eq!(
        first.description.as_deref(),
        Some("Receipt OCR runtime configuration")
    );
    assert!(!first.is_encrypted);

    assert!(set_app_setting(
        runtime.connection(),
        &AppSettingDraft {
            key: OCR_CONFIG_SETTING_KEY.to_string(),
            value: r#"{"provider":"tesseract","lang":"eng"}"#.to_string(),
            value_type: "".to_string(),
            description: None,
            is_encrypted: true,
        },
    )?);
    let updated = get_app_setting_row(runtime.connection(), OCR_CONFIG_SETTING_KEY)?
        .expect("row should still exist");
    assert_eq!(updated.id, first.id);
    assert_eq!(
        updated.value.as_deref(),
        Some(r#"{"provider":"tesseract","lang":"eng"}"#)
    );
    assert_eq!(updated.value_type, "string");
    assert!(updated.description.is_none());
    assert!(updated.is_encrypted);
    assert_eq!(
        get_app_setting(runtime.connection(), OCR_CONFIG_SETTING_KEY)?.as_deref(),
        Some(r#"{"provider":"tesseract","lang":"eng"}"#)
    );
    Ok(())
}

#[test]
fn ocr_config_setting_normalizes_rejects_unknown_and_stores_only_runtime_config(
) -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let runtime = runtime_for(&temp_dir.path().join("ocr-config.db"))?;
    init_app_settings_schema(runtime.connection())?;

    let default_config = load_ocr_config_setting(runtime.connection())?;
    assert_eq!(default_config.provider, "disabled");
    assert_eq!(default_config.lang, "chi_sim+eng");

    let stored = store_ocr_config_setting(
        runtime.connection(),
        Some(&json!({
            "provider": "TESSERACT",
            "lang": " eng+chi_sim ",
            "image": "should-not-persist",
            "result": {"amount": 12.5}
        })),
    )?;
    assert_eq!(stored.provider, "tesseract");
    assert_eq!(stored.lang, "eng+chi_sim");

    let row = get_app_setting_row(runtime.connection(), OCR_CONFIG_SETTING_KEY)?
        .expect("ocr config row should exist");
    assert_eq!(row.value_type, "json");
    assert_eq!(
        row.description.as_deref(),
        Some("Receipt OCR runtime configuration")
    );
    assert!(!row.is_encrypted);
    let persisted: Value = serde_json::from_str(row.value.as_deref().unwrap_or("{}"))?;
    assert_eq!(
        persisted,
        json!({
            "provider": "tesseract",
            "lang": "eng+chi_sim"
        })
    );
    assert!(persisted.get("image").is_none());
    assert!(persisted.get("result").is_none());

    let rejected =
        store_ocr_config_setting(runtime.connection(), Some(&json!({"provider": "bogus"})))
            .expect_err("unknown providers should keep Python route 400 semantics available");
    assert!(matches!(rejected, DbError::InvalidOperation(_)));
    assert_eq!(
        load_ocr_config_setting(runtime.connection())?,
        stored,
        "rejected providers must not overwrite the current OCR config"
    );

    set_app_setting(
        runtime.connection(),
        &AppSettingDraft {
            key: OCR_CONFIG_SETTING_KEY.to_string(),
            value: "{not-json".to_string(),
            value_type: "json".to_string(),
            description: None,
            is_encrypted: false,
        },
    )?;
    let fallback = load_ocr_config_setting(runtime.connection())?;
    assert_eq!(fallback.provider, "disabled");
    assert_eq!(fallback.lang, "chi_sim+eng");
    Ok(())
}
