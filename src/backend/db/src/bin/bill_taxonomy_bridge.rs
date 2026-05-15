use std::io::{self, Read};

use bill_analyser_db::taxonomy::accounts::{
    open_accounts_connection, parse_account_display_orders, AccountsRepository,
};
use bill_analyser_db::taxonomy::categories::{open_categories_connection, CategoriesRepository};
use bill_analyser_db::taxonomy::settings_bundle::{
    export_taxonomy_sections, normalize_account_import, normalize_category_import,
    normalize_settings_bundle_sections, normalize_tag_import, resolve_template_payload,
};
use bill_analyser_db::taxonomy::tags::{
    open_tags_connection, parse_display_orders, TagsRepository,
};
use bill_analyser_db::taxonomy::templates::{
    open_templates_connection, parse_template_display_orders, TemplatesRepository,
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
        "list-categories" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| repository.list_categories(user_id),
        )),
        "get-category" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.get_category_by_id(required_i64(&payload, "category_id")?, user_id)
            },
        )),
        "get-category-by-name" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.get_category_by_name(
                    required_str_allow_empty(&payload, "main_category")?,
                    required_str_allow_empty(&payload, "sub_category")?,
                    user_id,
                )
            },
        )),
        "create-category" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.create_category(required_value(&payload, "payload")?, user_id)
            },
        )),
        "ensure-categories" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.ensure_categories(required_value(&payload, "payload")?, user_id)
            },
        )),
        "update-category" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.update_category(
                    required_i64(&payload, "category_id")?,
                    required_value(&payload, "payload")?,
                    user_id,
                )
            },
        )),
        "delete-category" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.delete_category(required_i64(&payload, "category_id")?, user_id)
            },
        )),
        "delete-categories-by-main-category" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.delete_categories_by_main_category(
                    required_str(&payload, "main_category")?,
                    user_id,
                )
            },
        )),
        "update-main-category-name" => result_response(with_categories_repository(
            &payload,
            |repository, user_id| {
                repository.update_main_category_name(
                    required_str(&payload, "old_name")?,
                    required_str(&payload, "new_name")?,
                    user_id,
                )
            },
        )),
        "list-templates" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| {
                repository.list_templates(user_id, optional_i64(&payload, "template_type")?)
            },
        )),
        "get-template" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| {
                repository.get_template_by_id(
                    required_i64(&payload, "template_id")?,
                    user_id,
                    optional_i64(&payload, "template_type")?,
                )
            },
        )),
        "list-enabled-recurring-templates" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| repository.list_enabled_recurring_templates(user_id),
        )),
        "create-template" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| {
                repository.create_template(required_value(&payload, "payload")?, user_id)
            },
        )),
        "update-template" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| {
                repository.update_template(
                    required_i64(&payload, "template_id")?,
                    required_value(&payload, "payload")?,
                    user_id,
                    optional_i64(&payload, "template_type")?,
                )
            },
        )),
        "delete-template" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| {
                repository.delete_template(
                    required_i64(&payload, "template_id")?,
                    user_id,
                    optional_i64(&payload, "template_type")?,
                )
            },
        )),
        "update-template-display-orders" => result_response(with_templates_repository(
            &payload,
            |repository, user_id| {
                let orders = parse_template_display_orders(required_value(&payload, "orders")?)?;
                repository.update_display_orders(
                    &orders,
                    required_i64(&payload, "template_type")?,
                    user_id,
                )
            },
        )),
        "settings-normalize-sections" => result_response(
            required_value(&payload, "bundle").and_then(normalize_settings_bundle_sections),
        ),
        "settings-export-taxonomy-sections" => {
            result_response(required_value(&payload, "payload").and_then(export_taxonomy_sections))
        }
        "settings-normalize-account-import" => json!({
            "success": true,
            "result": normalize_account_import(payload.get("payload").unwrap_or(&Value::Null)),
        }),
        "settings-normalize-category-import" => json!({
            "success": true,
            "result": normalize_category_import(payload.get("payload").unwrap_or(&Value::Null)),
        }),
        "settings-normalize-tag-import" => json!({
            "success": true,
            "result": normalize_tag_import(payload.get("payload").unwrap_or(&Value::Null)),
        }),
        "settings-resolve-template-payload" => json!({
            "success": true,
            "result": resolve_template_payload(payload.get("payload").unwrap_or(&Value::Null)),
        }),
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

fn with_categories_repository<T>(
    payload: &Value,
    operation: impl FnOnce(&mut CategoriesRepository<'_>, i64) -> DbResult<T>,
) -> DbResult<T> {
    let db_path = required_str(payload, "db_path")?;
    let user_id = required_i64(payload, "user_id")?;
    let mut connection = open_categories_connection(db_path)?;
    let mut repository = CategoriesRepository::new(&mut connection);
    operation(&mut repository, user_id)
}

fn with_templates_repository<T>(
    payload: &Value,
    operation: impl FnOnce(&mut TemplatesRepository<'_>, i64) -> DbResult<T>,
) -> DbResult<T> {
    let db_path = required_str(payload, "db_path")?;
    let user_id = required_i64(payload, "user_id")?;
    let mut connection = open_templates_connection(db_path)?;
    let mut repository = TemplatesRepository::new(&mut connection);
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

fn required_str_allow_empty<'payload>(
    payload: &'payload Value,
    key: &str,
) -> DbResult<&'payload str> {
    required_value(payload, key)?
        .as_str()
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be text")))
}

fn required_i64(payload: &Value, key: &str) -> DbResult<i64> {
    required_value(payload, key)?
        .as_i64()
        .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be integer")))
}

fn optional_i64(payload: &Value, key: &str) -> DbResult<Option<i64>> {
    match payload.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(value) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| DbError::InvalidOperation(format!("{key} must be integer"))),
    }
}

fn db_error_message(error: &DbError) -> String {
    match error {
        DbError::InvalidOperation(message) | DbError::UnsafePath(message) => message.clone(),
        DbError::Sqlite(error) => error.to_string(),
        DbError::Io(error) => error.to_string(),
    }
}
