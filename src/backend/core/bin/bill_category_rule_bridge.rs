use std::io::{self, Read};

use bill_analyser_core::category_rules::compile_rule_expression;
use serde_json::{json, Value};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let command = std::env::args()
        .nth(1)
        .ok_or_else(|| "missing category rule bridge command".to_string())?;

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|_| "failed to read bridge input".to_string())?;

    let payload: Value =
        serde_json::from_str(&input).map_err(|_| "failed to parse bridge input".to_string())?;

    match command.as_str() {
        "compile-rule-expression" => {
            let expr = payload
                .get("expr")
                .and_then(Value::as_str)
                .ok_or_else(|| "missing rule expression".to_string())?;
            let regex_enabled = payload
                .get("regex_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let compiled = compile_rule_expression(expr, regex_enabled);
            println!("{}", json!({"success": true, "result": compiled}));
            Ok(())
        }
        _ => Err("unknown category rule bridge command".to_string()),
    }
}
