fn update_sub_accounts(
    repository: &mut AccountsRepository<'_>,
    account_id: i64,
    user_id: i64,
    sub_accounts: &[Value],
) -> RouteResult<()> {
    let existing_sub_accounts = repository
        .get_sub_accounts(account_id, user_id)
        .map_err(|_| Box::new(db_error_response()))?;
    let existing_sub_ids = existing_sub_accounts
        .iter()
        .filter_map(|account| account.get("id").and_then(value_as_i64))
        .collect::<Vec<_>>();
    let mut updated_sub_ids = Vec::new();

    for sub_account in sub_accounts {
        let sub_account_id = sub_account.get("id").and_then(value_as_i64);
        let mut sub_payload = frontend_account_to_backend(sub_account)
            .map_err(|message| Box::new(bad_request(message)))?;
        if let Some(sub_account_id) =
            sub_account_id.filter(|candidate| existing_sub_ids.contains(candidate))
        {
            repository
                .update_account(sub_account_id, &Value::Object(sub_payload), user_id)
                .map_err(|_| Box::new(db_error_response()))?;
            updated_sub_ids.push(sub_account_id);
            continue;
        }

        sub_payload.insert(
            "parent_id".to_string(),
            Value::Number(Number::from(account_id)),
        );
        repository
            .create_account(&Value::Object(sub_payload), user_id)
            .map_err(|_| Box::new(db_error_response()))?;
    }

    for old_sub_id in existing_sub_ids {
        if !updated_sub_ids.contains(&old_sub_id) {
            repository
                .delete_account(old_sub_id, user_id)
                .map_err(|_| Box::new(db_error_response()))?;
        }
    }
    Ok(())
}

fn load_account_with_sub_accounts(
    repository: &mut AccountsRepository<'_>,
    account_id: i64,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<AccountRecord>> {
    let Some(mut account) = repository.get_account(account_id, user_id)? else {
        return Ok(None);
    };
    let sub_accounts = repository.get_sub_accounts(account_id, user_id)?;
    if !sub_accounts.is_empty() {
        account.insert(
            "subAccounts".to_string(),
            Value::Array(sub_accounts.into_iter().map(Value::Object).collect()),
        );
    }
    Ok(Some(account))
}

fn format_account_list_response(accounts: Vec<AccountRecord>) -> Value {
    let formatted = accounts
        .into_iter()
        .map(backend_account_to_frontend)
        .collect::<Vec<_>>();
    let mut children_by_parent: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut top_level = Vec::new();

    for account in formatted {
        let parent_id = account
            .get("parentId")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        if parent_id.is_empty() || parent_id == "0" {
            top_level.push(account);
        } else {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(Value::Object(account));
        }
    }

    for account in &mut top_level {
        if let Some(account_id) = account
            .get("id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
        {
            if let Some(children) = children_by_parent.remove(&account_id) {
                if !children.is_empty() {
                    account.insert("subAccounts".to_string(), Value::Array(children));
                }
            }
        }
    }

    Value::Array(top_level.into_iter().map(Value::Object).collect())
}

fn format_sync_account_balances_response(result: SyncAllAccountBalancesResult) -> Value {
    json!({
        "total_accounts": result.total_accounts,
        "synced_accounts": result.synced_accounts,
        "discrepancies": result
            .discrepancies
            .into_iter()
            .map(format_account_balance_discrepancy)
            .collect::<Vec<_>>(),
        "errors": result.errors,
    })
}

fn format_account_balance_discrepancy(discrepancy: AccountBalanceDiscrepancy) -> Value {
    json!({
        "account_id": discrepancy.account_id,
        "name": discrepancy.name,
        "old_balance": json_number(discrepancy.old_balance),
        "new_balance": json_number(discrepancy.new_balance),
        "diff": json_number(discrepancy.diff),
    })
}

fn frontend_account_to_backend(payload: &Value) -> Result<Map<String, Value>, String> {
    let Some(object) = payload.as_object() else {
        return Err("Account payload must be an object".to_string());
    };
    let balance_cents = object
        .get("balance")
        .or_else(|| object.get("initial_balance"))
        .and_then(value_as_f64)
        .unwrap_or_default();
    let balance_yuan = round2(balance_cents / 100.0);
    let hidden = object
        .get("hidden")
        .map(value_truthy)
        .unwrap_or_else(|| !object.get("visible").map(value_truthy).unwrap_or(true));

    let mut result = Map::new();
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(object.get("name"), "")),
    );
    result.insert(
        "parent_id".to_string(),
        Value::Number(Number::from(
            object
                .get("parentId")
                .or_else(|| object.get("parent_id"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert(
        "category".to_string(),
        object.get("category").cloned().unwrap_or(Value::Null),
    );
    result.insert(
        "type".to_string(),
        object
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(object.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(object.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(object.get("currency"), "CNY")),
    );
    result.insert("balance".to_string(), json_number(balance_yuan));
    result.insert("initial_balance".to_string(), json_number(balance_yuan));
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(object.get("comment"), "")),
    );
    result.insert(
        "aliases".to_string(),
        Value::String(
            serde_json::to_string(&parse_aliases(object.get("aliases")))
                .unwrap_or_else(|_| "[]".to_string()),
        ),
    );
    result.insert(
        "display_order".to_string(),
        Value::Number(Number::from(
            object
                .get("displayOrder")
                .or_else(|| object.get("display_order"))
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    if let Some(Value::Array(sub_accounts)) = object.get("subAccounts") {
        let converted_sub_accounts = sub_accounts
            .iter()
            .map(frontend_account_to_backend)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(Value::Object)
            .collect();
        result.insert(
            "subAccounts".to_string(),
            Value::Array(converted_sub_accounts),
        );
    }
    if let Some(statement_date) = object
        .get("creditCardStatementDate")
        .or_else(|| object.get("credit_card_statement_date"))
    {
        result.insert(
            "credit_card_statement_date".to_string(),
            statement_date.clone(),
        );
    }
    Ok(result)
}

fn backend_account_to_frontend(mut account: AccountRecord) -> Map<String, Value> {
    let hidden = account.get("hidden").map(value_truthy).unwrap_or(false);
    let sub_accounts = account.remove("subAccounts");
    let mut result = Map::new();
    result.insert(
        "id".to_string(),
        Value::String(value_string(account.get("id"), "")),
    );
    result.insert(
        "name".to_string(),
        Value::String(string_or_default(account.get("name"), "")),
    );
    result.insert(
        "parentId".to_string(),
        Value::String(value_string(account.get("parent_id"), "0")),
    );
    result.insert(
        "category".to_string(),
        account
            .get("category")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "type".to_string(),
        account
            .get("type")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(account.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(account.get("color"), "")),
    );
    result.insert(
        "currency".to_string(),
        Value::String(string_or_default(account.get("currency"), "CNY")),
    );
    result.insert(
        "balance".to_string(),
        Value::Number(Number::from(yuan_to_cents(account.get("balance")))),
    );
    result.insert(
        "comment".to_string(),
        Value::String(string_or_default(account.get("comment"), "")),
    );
    result.insert(
        "aliases".to_string(),
        Value::Array(
            parse_aliases(account.get("aliases"))
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    result.insert(
        "creditCardStatementDate".to_string(),
        account
            .get("credit_card_statement_date")
            .cloned()
            .unwrap_or(Value::Null),
    );
    result.insert(
        "displayOrder".to_string(),
        account
            .get("display_order")
            .cloned()
            .unwrap_or_else(|| Value::Number(Number::from(0))),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));

    if let Some(Value::Array(sub_accounts)) = sub_accounts.filter(|value| {
        value
            .as_array()
            .is_some_and(|sub_accounts| !sub_accounts.is_empty())
    }) {
        result.insert(
            "subAccounts".to_string(),
            Value::Array(
                sub_accounts
                    .into_iter()
                    .filter_map(|sub_account| {
                        sub_account
                            .as_object()
                            .cloned()
                            .map(backend_account_to_frontend)
                            .map(Value::Object)
                    })
                    .collect(),
            ),
        );
    }

    result
}

fn format_tag_list_response(tags: Vec<TagRecord>) -> Value {
    Value::Array(
        tags.into_iter()
            .map(backend_tag_to_frontend)
            .map(Value::Object)
            .collect(),
    )
}

fn format_template_list_response(templates: Vec<TemplateRecord>) -> Value {
    Value::Array(templates.into_iter().map(Value::Object).collect())
}

fn backend_tag_to_frontend(tag: TagRecord) -> Map<String, Value> {
    let hidden = tag.hidden != 0;
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(tag.id.to_string()));
    result.insert("name".to_string(), Value::String(tag.name));
    result.insert(
        "color".to_string(),
        tag.color.map_or(Value::Null, Value::String),
    );
    result.insert(
        "icon".to_string(),
        tag.icon.map_or(Value::Null, Value::String),
    );
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(tag.display_order)),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));
    result
}

fn update_virtual_category_handler(
    repository: &mut CategoriesRepository<'_>,
    user_id: i64,
    old_name: &str,
    body: &Value,
) -> Response {
    let new_name = category_name_from_body(body).unwrap_or_else(|| old_name.to_string());
    let mut renamed_group = false;

    if new_name != old_name {
        let real_categories = match repository.list_categories(user_id) {
            Ok(value) => value,
            Err(_) => return category_db_error_response(),
        };
        let has_old_group = real_categories
            .iter()
            .any(|category| category_text(category, "main_category") == old_name);
        let has_target_group = real_categories
            .iter()
            .any(|category| category_text(category, "main_category") == new_name);
        if has_old_group && has_target_group {
            return error_response(StatusCode::CONFLICT, "Category rename conflict");
        }
        match repository.update_main_category_name(old_name, &new_name, user_id) {
            Ok(true) => renamed_group = has_old_group,
            Ok(false) if has_old_group => {
                return error_response(StatusCode::CONFLICT, "Category rename conflict");
            }
            Ok(false) => {}
            Err(_) => {
                return error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to rename category",
                )
            }
        }
    }

    let updates = Value::Object(virtual_category_update_payload(body, &new_name));
    let category_id = match repository.get_category_by_name(&new_name, "", user_id) {
        Ok(Some(existing)) => {
            let Some(category_id) = existing.get("id").and_then(value_as_i64) else {
                return category_db_error_response();
            };
            match repository.update_category(category_id, &updates, user_id) {
                Ok(true) => category_id,
                Ok(false) => match repository.get_category_by_name(&new_name, "", user_id) {
                    Ok(Some(refreshed)) => {
                        let Some(category_id) = refreshed.get("id").and_then(value_as_i64) else {
                            return category_db_error_response();
                        };
                        match repository.update_category(category_id, &updates, user_id) {
                            Ok(true) => category_id,
                            Ok(false) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                            Err(_) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        rollback_category_rename(
                            repository,
                            renamed_group,
                            &new_name,
                            old_name,
                            user_id,
                        );
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                    Err(_) => {
                        rollback_category_rename(
                            repository,
                            renamed_group,
                            &new_name,
                            old_name,
                            user_id,
                        );
                        return category_db_error_response();
                    }
                },
                Err(_) => {
                    rollback_category_rename(
                        repository,
                        renamed_group,
                        &new_name,
                        old_name,
                        user_id,
                    );
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to save category",
                    );
                }
            }
        }
        Ok(None) => {
            let create_payload = Value::Object(virtual_category_create_payload(body, &new_name));
            match repository.create_category(&create_payload, user_id) {
                Ok(Some(category_id)) => category_id,
                Ok(None) => match repository.get_category_by_name(&new_name, "", user_id) {
                    Ok(Some(refreshed)) => {
                        let Some(category_id) = refreshed.get("id").and_then(value_as_i64) else {
                            return category_db_error_response();
                        };
                        match repository.update_category(category_id, &updates, user_id) {
                            Ok(true) => category_id,
                            Ok(false) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                            Err(_) => {
                                rollback_category_rename(
                                    repository,
                                    renamed_group,
                                    &new_name,
                                    old_name,
                                    user_id,
                                );
                                return error_response(
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to save category",
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        rollback_category_rename(
                            repository,
                            renamed_group,
                            &new_name,
                            old_name,
                            user_id,
                        );
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to save category",
                        );
                    }
                    Err(_) => return category_db_error_response(),
                },
                Err(_) => {
                    rollback_category_rename(
                        repository,
                        renamed_group,
                        &new_name,
                        old_name,
                        user_id,
                    );
                    return error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to save category",
                    );
                }
            }
        }
        Err(_) => return category_db_error_response(),
    };

    match repository.get_category_by_id(category_id, user_id) {
        Ok(Some(category)) => success_result(
            StatusCode::OK,
            Value::Object(backend_category_to_frontend(&category, "0")),
        ),
        Ok(None) => {
            rollback_category_rename(repository, renamed_group, &new_name, old_name, user_id);
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load updated category",
            )
        }
        Err(_) => category_db_error_response(),
    }
}

fn update_real_category_handler(
    repository: &mut CategoriesRepository<'_>,
    user_id: i64,
    category_id: i64,
    body: &Value,
) -> Response {
    let mut updates = category_update_payload_from_frontend(body);
    let mut renamed_main_category = false;
    let mut original_main_category = String::new();
    let mut renamed_to_main_category = String::new();

    if let Some(new_name) = category_name_from_body(body) {
        let category = match repository.get_category_by_id(category_id, user_id) {
            Ok(Some(value)) => value,
            Ok(None) => return not_found("Category not found"),
            Err(_) => return category_db_error_response(),
        };
        if category_text(&category, "sub_category").is_empty() {
            let old_name = category_text(&category, "main_category");
            if new_name != old_name {
                let real_categories = match repository.list_categories(user_id) {
                    Ok(value) => value,
                    Err(_) => return category_db_error_response(),
                };
                if real_categories
                    .iter()
                    .any(|category| category_text(category, "main_category") == new_name)
                {
                    return error_response(StatusCode::CONFLICT, "Category rename conflict");
                }
                match repository.update_main_category_name(&old_name, &new_name, user_id) {
                    Ok(true) => {
                        renamed_main_category = true;
                        original_main_category = old_name;
                        renamed_to_main_category = new_name;
                    }
                    Ok(false) => {
                        return error_response(StatusCode::CONFLICT, "Category rename conflict")
                    }
                    Err(_) => {
                        return error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to rename category",
                        );
                    }
                }
            }
        } else {
            updates.insert("sub_category".to_string(), Value::String(new_name));
        }
    }

    let payload = Value::Object(updates);
    match repository.update_category(category_id, &payload, user_id) {
        Ok(true) => match repository.get_category_by_id(category_id, user_id) {
            Ok(Some(category)) => success_result(
                StatusCode::OK,
                Value::Object(backend_category_to_frontend(&category, "0")),
            ),
            Ok(None) => {
                rollback_category_rename(
                    repository,
                    renamed_main_category,
                    &renamed_to_main_category,
                    &original_main_category,
                    user_id,
                );
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to load updated category",
                )
            }
            Err(_) => category_db_error_response(),
        },
        Ok(false) => {
            rollback_category_rename(
                repository,
                renamed_main_category,
                &renamed_to_main_category,
                &original_main_category,
                user_id,
            );
            not_found("Category not found")
        }
        Err(error) => {
            rollback_category_rename(
                repository,
                renamed_main_category,
                &renamed_to_main_category,
                &original_main_category,
                user_id,
            );
            let text = error.to_string();
            if text.contains("constraint") || text.contains("UNIQUE") {
                error_response(StatusCode::CONFLICT, "Category update conflict")
            } else {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update category",
                )
            }
        }
    }
}

fn rollback_category_rename(
    repository: &mut CategoriesRepository<'_>,
    renamed: bool,
    current_name: &str,
    previous_name: &str,
    user_id: i64,
) {
    if renamed {
        let _ = repository.update_main_category_name(current_name, previous_name, user_id);
    }
}

#[derive(Clone, Copy)]
enum CategoryPayloadMode {
    FrontendDefaults,
    ImportDefaults,
}

fn resolve_parent_category(
    repository: &mut CategoriesRepository<'_>,
    parent_id: &str,
    user_id: i64,
) -> bill_analyser_db::DbResult<Option<(String, i64)>> {
    if let Some(parent_name) = virtual_category_name(parent_id) {
        return Ok(Some((parent_name, 1)));
    }
    let Ok(parent_id) = parent_id.parse::<i64>() else {
        return Ok(None);
    };
    Ok(repository
        .get_category_by_id(parent_id, user_id)?
        .map(|parent| {
            (
                category_text(&parent, "main_category"),
                parent.get("type").and_then(value_as_i64).unwrap_or(1),
            )
        }))
}

fn category_name_from_body(payload: &Value) -> Option<String> {
    payload
        .as_object()
        .and_then(|object| object.get("name"))
        .map(|value| string_or_default(Some(value), ""))
}

fn virtual_category_name(category_id: &str) -> Option<String> {
    category_id
        .strip_prefix("virtual_")
        .map(ToString::to_string)
}

fn category_parent_id_for_get(
    repository: &mut CategoriesRepository<'_>,
    category: &CategoryRecord,
    user_id: i64,
) -> String {
    let sub_category = category_text(category, "sub_category");
    if sub_category.is_empty() {
        return "0".to_string();
    }
    let main_category = category_text(category, "main_category");
    match repository.get_category_by_name(&main_category, "", user_id) {
        Ok(Some(parent)) => parent
            .get("id")
            .map(|value| value_string(Some(value), "0"))
            .unwrap_or_else(|| format!("virtual_{main_category}")),
        Ok(None) | Err(_) => format!("virtual_{main_category}"),
    }
}

fn frontend_category_to_backend(
    payload: &Value,
    main_category: &str,
    sub_category: &str,
    category_type: i64,
    mode: CategoryPayloadMode,
) -> Map<String, Value> {
    let mut result = Map::new();
    result.insert(
        "type".to_string(),
        Value::Number(Number::from(category_type)),
    );
    result.insert(
        "main_category".to_string(),
        Value::String(main_category.to_string()),
    );
    result.insert(
        "sub_category".to_string(),
        Value::String(sub_category.to_string()),
    );
    result.insert(
        "description".to_string(),
        Value::String(string_or_default(payload.get("comment"), "")),
    );
    result.insert(
        "priority".to_string(),
        Value::Number(Number::from(
            payload
                .get("displayOrder")
                .and_then(value_as_i64)
                .unwrap_or(0),
        )),
    );
    result.insert(
        "keywords".to_string(),
        Value::String(string_or_default(payload.get("keywords"), "")),
    );
    let hidden = match mode {
        CategoryPayloadMode::FrontendDefaults => payload
            .get("visible")
            .map(|visible| !value_truthy(visible))
            .or_else(|| payload.get("hidden").map(value_truthy))
            .unwrap_or(false),
        CategoryPayloadMode::ImportDefaults => {
            payload.get("hidden").map(value_truthy).unwrap_or(false)
        }
    };
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert(
        "icon".to_string(),
        Value::String(string_or_default(payload.get("icon"), "")),
    );
    result.insert(
        "color".to_string(),
        Value::String(string_or_default(payload.get("color"), "")),
    );
    result
}

fn virtual_category_update_payload(payload: &Value, main_category: &str) -> Map<String, Value> {
    let category_type = payload.get("type").and_then(value_as_i64).unwrap_or(1);
    frontend_category_to_backend(
        payload,
        main_category,
        "",
        category_type,
        CategoryPayloadMode::FrontendDefaults,
    )
}

fn virtual_category_create_payload(payload: &Value, main_category: &str) -> Map<String, Value> {
    virtual_category_update_payload(payload, main_category)
}

fn category_update_payload_from_frontend(payload: &Value) -> Map<String, Value> {
    let mut result = Map::new();
    if let Some(comment) = payload.get("comment") {
        result.insert(
            "description".to_string(),
            Value::String(string_or_default(Some(comment), "")),
        );
    }
    if let Some(display_order) = payload.get("displayOrder").and_then(value_as_i64) {
        result.insert(
            "priority".to_string(),
            Value::Number(Number::from(display_order)),
        );
    }
    if let Some(keywords) = payload.get("keywords") {
        result.insert(
            "keywords".to_string(),
            Value::String(string_or_default(Some(keywords), "")),
        );
    }
    if let Some(category_type) = payload.get("type").and_then(value_as_i64) {
        result.insert(
            "type".to_string(),
            Value::Number(Number::from(category_type)),
        );
    }
    if let Some(visible) = payload.get("visible") {
        result.insert("hidden".to_string(), Value::Bool(!value_truthy(visible)));
    }
    if let Some(icon) = payload.get("icon") {
        result.insert(
            "icon".to_string(),
            Value::String(string_or_default(Some(icon), "")),
        );
    }
    if let Some(color) = payload.get("color") {
        result.insert(
            "color".to_string(),
            Value::String(string_or_default(Some(color), "")),
        );
    }
    result
}

fn import_category_payload(
    payload: &Value,
    main_category: &str,
    sub_category: &str,
) -> Map<String, Value> {
    let category_type = payload.get("type").and_then(value_as_i64).unwrap_or(3);
    let mut result = frontend_category_to_backend(
        payload,
        main_category,
        sub_category,
        category_type,
        CategoryPayloadMode::ImportDefaults,
    );
    if let Some(description) = payload.get("description") {
        result.insert(
            "description".to_string(),
            Value::String(string_or_default(Some(description), "")),
        );
    }
    if let Some(priority) = payload.get("priority").and_then(value_as_i64) {
        result.insert(
            "priority".to_string(),
            Value::Number(Number::from(priority)),
        );
    }
    result
}

fn format_category_tree_response(categories: Vec<CategoryRecord>) -> Value {
    let mut grouped: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
    let mut main_indices: BTreeMap<(i64, String), (i64, usize)> = BTreeMap::new();

    for category in categories {
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(0);
        let main_name = category_text(&category, "main_category");
        let sub_name = category_text(&category, "sub_category");
        let key = (category_type, main_name.clone());

        if !main_indices.contains_key(&key) {
            let parent_id = if sub_name.is_empty() {
                value_string(category.get("id"), &format!("virtual_{main_name}"))
            } else {
                format!("virtual_{main_name}")
            };
            let mut node = backend_category_to_frontend(&category, "0");
            node.insert("id".to_string(), Value::String(parent_id));
            node.insert("name".to_string(), Value::String(main_name.clone()));
            node.insert("parentId".to_string(), Value::String("0".to_string()));
            node.insert("comment".to_string(), Value::String(String::new()));
            node.insert("hidden".to_string(), Value::Bool(false));
            node.insert("visible".to_string(), Value::Bool(true));
            node.insert("keywords".to_string(), Value::String(String::new()));
            node.insert("subCategories".to_string(), Value::Array(Vec::new()));
            let bucket = grouped.entry(category_type).or_default();
            let index = bucket.len();
            bucket.push(Value::Object(node));
            main_indices.insert(key.clone(), (category_type, index));
        }

        let Some((bucket_key, index)) = main_indices.get(&key).copied() else {
            continue;
        };
        let Some(bucket) = grouped.get_mut(&bucket_key) else {
            continue;
        };
        let Some(node) = bucket.get_mut(index).and_then(Value::as_object_mut) else {
            continue;
        };

        if sub_name.is_empty() {
            let sub_categories = node
                .remove("subCategories")
                .unwrap_or_else(|| Value::Array(Vec::new()));
            *node = backend_category_to_frontend(&category, "0");
            node.insert("subCategories".to_string(), sub_categories);
        } else {
            let parent_id = node
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("virtual_{main_name}"));
            let sub_node = backend_category_to_frontend(&category, &parent_id);
            node.entry("subCategories".to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("subCategories must stay an array")
                .push(Value::Object(sub_node));
        }
    }

    let mut result = Map::new();
    for (category_type, values) in grouped {
        result.insert(category_type.to_string(), Value::Array(values));
    }
    Value::Object(result)
}

fn format_category_flat_response(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(
        categories
            .iter()
            .map(|category| {
                let parent_id = if category_text(category, "sub_category").is_empty() {
                    "0".to_string()
                } else {
                    format!("virtual_{}", category_text(category, "main_category"))
                };
                Value::Object(backend_category_to_frontend(category, &parent_id))
            })
            .collect(),
    )
}

fn categories_to_value(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(categories.into_iter().map(Value::Object).collect())
}

fn format_category_statistics_response(statistics: Vec<CategoryStatistic>) -> Value {
    let mut result = Map::new();

    for statistic in statistics {
        let entry = result
            .entry(statistic.main_category.clone())
            .or_insert_with(|| {
                json!({
                    "total_amount": 0.0,
                    "count": 0,
                    "sub_categories": {}
                })
            });
        let Some(entry_object) = entry.as_object_mut() else {
            continue;
        };

        let total_amount = entry_object
            .get("total_amount")
            .and_then(value_as_f64)
            .unwrap_or_default()
            + statistic.total_amount.abs();
        entry_object.insert(
            "total_amount".to_string(),
            json_number(round2(total_amount)),
        );

        let count = entry_object
            .get("count")
            .and_then(value_as_i64)
            .unwrap_or_default()
            + statistic.count;
        entry_object.insert("count".to_string(), Value::Number(Number::from(count)));

        if statistic.sub_category.is_empty() {
            continue;
        }
        let sub_categories = entry_object
            .entry("sub_categories".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(sub_categories) = sub_categories.as_object_mut() {
            sub_categories.insert(
                statistic.sub_category.clone(),
                json!({
                    "total_amount": round2(statistic.total_amount.abs()),
                    "count": statistic.count
                }),
            );
        }
    }

    Value::Object(result)
}

