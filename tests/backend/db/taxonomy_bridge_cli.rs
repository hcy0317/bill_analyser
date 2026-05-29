use std::{
    error::Error,
    io::Write,
    process::{Command, Stdio},
};

use serde_json::{json, Value};

#[test]
fn taxonomy_bridge_cli_dispatches_repository_and_settings_commands() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir
        .path()
        .join("taxonomy-bridge.db")
        .display()
        .to_string();
    let base = json!({"db_path": db_path, "user_id": 42});

    let mut cases = vec![
        ("list-accounts", base.clone()),
        ("get-account", with_fields(&base, json!({"account_id": 1}))),
        (
            "get-sub-accounts",
            with_fields(&base, json!({"parent_id": 1})),
        ),
        ("create-account", with_fields(&base, json!({"payload": {}}))),
        (
            "update-account",
            with_fields(&base, json!({"account_id": 1, "payload": {}})),
        ),
        (
            "delete-account",
            with_fields(&base, json!({"account_id": 1})),
        ),
        (
            "update-account-display-orders",
            with_fields(&base, json!({"orders": []})),
        ),
        ("list-tags", base.clone()),
        ("get-tag", with_fields(&base, json!({"tag_id": 1}))),
        ("create-tag", with_fields(&base, json!({"payload": {}}))),
        (
            "update-tag",
            with_fields(&base, json!({"tag_id": 1, "payload": {}})),
        ),
        ("delete-tag", with_fields(&base, json!({"tag_id": 1}))),
        (
            "update-display-orders",
            with_fields(&base, json!({"orders": []})),
        ),
        ("list-categories", base.clone()),
        (
            "get-category",
            with_fields(&base, json!({"category_id": 1})),
        ),
        (
            "get-category-by-name",
            with_fields(&base, json!({"main_category": "餐饮", "sub_category": ""})),
        ),
        (
            "create-category",
            with_fields(&base, json!({"payload": {}})),
        ),
        (
            "ensure-categories",
            with_fields(&base, json!({"payload": []})),
        ),
        (
            "update-category",
            with_fields(&base, json!({"category_id": 1, "payload": {}})),
        ),
        (
            "delete-category",
            with_fields(&base, json!({"category_id": 1})),
        ),
        (
            "delete-categories-by-main-category",
            with_fields(&base, json!({"main_category": "餐饮"})),
        ),
        (
            "update-main-category-name",
            with_fields(&base, json!({"old_name": "餐饮", "new_name": "外食"})),
        ),
        (
            "list-templates",
            with_fields(&base, json!({"template_type": 1})),
        ),
        (
            "get-template",
            with_fields(&base, json!({"template_id": 1, "template_type": 1})),
        ),
        ("list-enabled-recurring-templates", base.clone()),
        (
            "create-template",
            with_fields(&base, json!({"payload": {}})),
        ),
        (
            "update-template",
            with_fields(
                &base,
                json!({"template_id": 1, "template_type": 1, "payload": {}}),
            ),
        ),
        (
            "delete-template",
            with_fields(&base, json!({"template_id": 1, "template_type": 1})),
        ),
        (
            "update-template-display-orders",
            with_fields(&base, json!({"template_type": 1, "orders": []})),
        ),
    ];
    cases.extend([
        (
            "settings-normalize-sections",
            json!({"bundle": {"accounts": [], "categories": [], "tags": [], "templates": []}}),
        ),
        ("settings-export-taxonomy-sections", json!({"payload": {}})),
        ("settings-normalize-account-import", json!({"payload": {}})),
        ("settings-normalize-category-import", json!({"payload": {}})),
        ("settings-normalize-tag-import", json!({"payload": {}})),
        ("settings-resolve-template-payload", json!({"payload": {}})),
    ]);

    for (command, payload) in cases {
        let response = run_taxonomy_bridge(command, &payload)?;
        assert!(
            response.get("success").is_some(),
            "{command} did not return bridge envelope: {response}"
        );
    }

    let legacy_alias_update = run_taxonomy_bridge(
        "update-account",
        &with_fields(
            &base,
            json!({
                "account_id": 1,
                "payload": {
                    "subAccounts": [{
                        "name": "Child",
                        "aliases": ["legacy-child"]
                    }]
                }
            }),
        ),
    )?;
    assert_eq!(legacy_alias_update["success"], false);
    assert!(legacy_alias_update["error"]
        .as_str()
        .unwrap_or_default()
        .contains("aliases are no longer supported"));

    let unknown = run_taxonomy_bridge_error("missing-command", &json!({}))?;
    assert!(unknown.contains("unknown taxonomy bridge command"));
    Ok(())
}

fn with_fields(base: &Value, fields: Value) -> Value {
    let mut object = base.as_object().expect("base object").clone();
    object.extend(fields.as_object().expect("fields object").clone());
    Value::Object(object)
}

fn run_taxonomy_bridge(command: &str, payload: &Value) -> Result<Value, Box<dyn Error>> {
    let output = spawn_taxonomy_bridge(command, payload)?;
    assert!(
        output.status.success(),
        "bridge failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn run_taxonomy_bridge_error(command: &str, payload: &Value) -> Result<String, Box<dyn Error>> {
    let output = spawn_taxonomy_bridge(command, payload)?;
    assert!(!output.status.success());
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

fn spawn_taxonomy_bridge(
    command: &str,
    payload: &Value,
) -> Result<std::process::Output, Box<dyn Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bill_taxonomy_bridge"))
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .expect("stdin is piped")
        .write_all(payload.to_string().as_bytes())?;
    Ok(child.wait_with_output()?)
}
