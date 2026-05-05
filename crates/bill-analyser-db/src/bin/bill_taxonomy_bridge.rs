use std::io::{self, Read};

use bill_analyser_db::taxonomy::accounts::{
    open_accounts_connection, parse_account_display_orders, AccountsRepository,
};
use bill_analyser_db::taxonomy::tags::{
    open_tags_connection, parse_display_orders, TagsRepository,
};
use bill_analyser_db::{DbError, DbResult};
use serde::Serialize;
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
        .ok_or_else(|| "missing taxonomy bridge command".to_string())?;

    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|_| "failed to read bridge input".to_string())?;

    let payload: Value =
        serde_json::from_str(&input).map_err(|_| "failed to parse bridge input".to_string())?;

    let response = match command.as_str() {
        "list-accounts" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                repository.list_accounts(user_id)
            }))
        }
        "get-account" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                repository.get_account(required_i64(&payload, "account_id")?, user_id)
            }))
        }
        "get-sub-accounts" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                repository.get_sub_accounts(required_i64(&payload, "parent_id")?, user_id)
            }))
        }
        "create-account" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                repository.create_account(required_value(&payload, "payload")?, user_id)
            }))
        }
        "update-account" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                repository.update_account(
                    required_i64(&payload, "account_id")?,
                    required_value(&payload, "payload")?,
                    user_id,
                )
            }))
        }
        "delete-account" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                repository.delete_account(required_i64(&payload, "account_id")?, user_id)
            }))
        }
        "update-account-display-orders" => {
            result_response(with_accounts_repository(&payload, |repository, user_id| {
                let orders = parse_account_display_orders(required_value(&payload, "orders")?)?;
                repository.update_display_orders(&orders, user_id)
            }))
        }
        "list-tags" => result_response(with_tags_repository(&payload, |repository, user_id| {
            repository.list_tags(user_id)
        })),
        "get-tag" => result_response(with_tags_repository(&payload, |repository, user_id| {
            repository.get_tag(required_i64(&payload, "tag_id")?, user_id)
        })),
        "create-tag" => result_response(with_tags_repository(&payload, |repository, user_id| {
            repository.create_tag(required_value(&payload, "payload")?, user_id)
        })),
        "update-tag" => result_response(with_tags_repository(&payload, |repository, user_id| {
            repository.update_tag(
                required_i64(&payload, "tag_id")?,
                required_value(&payload, "payload")?,
                user_id,
            )
        })),
        "delete-tag" => result_response(with_tags_repository(&payload, |repository, user_id| {
            repository.delete_tag(required_i64(&payload, "tag_id")?, user_id)
        })),
        "update-display-orders" => {
            result_response(with_tags_repository(&payload, |repository, user_id| {
                let orders = parse_display_orders(required_value(&payload, "orders")?)?;
                repository.update_display_orders(&orders, user_id)
            }))
        }
        _ => return Err("unknown taxonomy bridge command".to_string()),
    };

    println!("{response}");
    Ok(())
}

fn with_accounts_repository<T>(
    payload: &Value,
    operation: impl FnOnce(&mut AccountsRepository<'_>, i64) -> DbResult<T>,
) -> DbResult<T> {
    let db_path = required_str(payload, "db_path")?;
    let user_id = required_i64(payload, "user_id")?;
    let mut connection = open_accounts_connection(db_path)?;
    let mut repository = AccountsRepository::new(&mut connection);
    operation(&mut repository, user_id)
}

fn with_tags_repository<T>(
    payload: &Value,
    operation: impl FnOnce(&mut TagsRepository<'_>, i64) -> DbResult<T>,
) -> DbResult<T> {
    let db_path = required_str(payload, "db_path")?;
    let user_id = required_i64(payload, "user_id")?;
    let mut connection = open_tags_connection(db_path)?;
    let mut repository = TagsRepository::new(&mut connection);
    operation(&mut repository, user_id)
}

fn result_response<T: Serialize>(result: DbResult<T>) -> Value {
    match result {
        Ok(result) => json!({"success": true, "result": result}),
        Err(error) => json!({"success": false, "error": db_error_message(&error)}),
    }
}

fn required_value<'payload>(payload: &'payload Value, key: &str) -> DbResult<&'payload Value> {
    payload
        .get(key)
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} is required")))
}

fn required_str<'payload>(payload: &'payload Value, key: &str) -> DbResult<&'payload str> {
    required_value(payload, key)?
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be text")))
}

fn required_i64(payload: &Value, key: &str) -> DbResult<i64> {
    required_value(payload, key)?
        .as_i64()
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be integer")))
}

fn db_error_message(error: &DbError) -> String {
    match error {
        DbError::InvalidOperation(message) | DbError::UnsafePath(message) => message.clone(),
        DbError::Sqlite(error) => error.to_string(),
        DbError::Io(error) => error.to_string(),
    }
}
