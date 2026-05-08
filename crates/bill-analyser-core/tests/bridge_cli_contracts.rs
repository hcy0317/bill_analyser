use std::{
    error::Error,
    io::Write,
    process::{Command, Stdio},
};

use serde_json::{json, Value};

#[test]
fn auth_bridge_cli_validates_refresh_claims_and_reports_errors() -> Result<(), Box<dyn Error>> {
    let ok = run_bridge(
        env!("CARGO_BIN_EXE_bill_auth_bridge"),
        "validate-refresh-token-claims",
        &json!({"type": "refresh", "user_id": 42, "username": "alice"}),
    )?;
    assert_eq!(ok["success"], true);
    assert_eq!(ok["result"]["user_id"], 42);

    let rejected = run_bridge(
        env!("CARGO_BIN_EXE_bill_auth_bridge"),
        "validate-refresh-token-claims",
        &json!({"type": "access", "user_id": 42, "username": "alice"}),
    )?;
    assert_eq!(rejected["success"], false);
    assert_eq!(rejected["error"]["status"], 400);

    let unknown = run_bridge_error(
        env!("CARGO_BIN_EXE_bill_auth_bridge"),
        "missing-command",
        &json!({}),
    )?;
    assert!(unknown.contains("unknown auth bridge command"));
    Ok(())
}

#[test]
fn category_rule_bridge_cli_compiles_expression_and_reports_errors() -> Result<(), Box<dyn Error>> {
    let ok = run_bridge(
        env!("CARGO_BIN_EXE_bill_category_rule_bridge"),
        "compile-rule-expression",
        &json!({"expr": "星巴克", "regex_enabled": false}),
    )?;
    assert_eq!(ok["success"], true);
    assert_eq!(ok["result"]["or_blocks"][0][0], "星巴克");

    let missing_expr = run_bridge_error(
        env!("CARGO_BIN_EXE_bill_category_rule_bridge"),
        "compile-rule-expression",
        &json!({}),
    )?;
    assert!(missing_expr.contains("missing rule expression"));

    let unknown = run_bridge_error(
        env!("CARGO_BIN_EXE_bill_category_rule_bridge"),
        "missing-command",
        &json!({}),
    )?;
    assert!(unknown.contains("unknown category rule bridge command"));
    Ok(())
}

fn run_bridge(binary: &str, command: &str, payload: &Value) -> Result<Value, Box<dyn Error>> {
    let output = spawn_bridge(binary, command, payload)?;
    assert!(
        output.status.success(),
        "bridge failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn run_bridge_error(
    binary: &str,
    command: &str,
    payload: &Value,
) -> Result<String, Box<dyn Error>> {
    let output = spawn_bridge(binary, command, payload)?;
    assert!(!output.status.success());
    Ok(String::from_utf8_lossy(&output.stderr).to_string())
}

fn spawn_bridge(
    binary: &str,
    command: &str,
    payload: &Value,
) -> Result<std::process::Output, Box<dyn Error>> {
    let mut child = Command::new(binary)
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
