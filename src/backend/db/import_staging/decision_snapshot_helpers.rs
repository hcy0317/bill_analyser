// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_transfer_snapshot(value: &Value) -> Option<Value> {
    value
        .as_object()
        .map(|object| Value::Object(object.clone()))
        .filter(|value| value.as_object().is_some_and(|object| !object.is_empty()))
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_transfer_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_type": preview.preview_type,
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
        "preview_recurring_id": preview.preview_recurring_id,
        "preview_recurring_name": preview.preview_recurring_name,
        "preview_recurring_candidate_count": preview.preview_recurring_candidate_count,
        "preview_recurring_match_score": preview.preview_recurring_match_score,
        "preview_recurring_match_reasons": preview.preview_recurring_match_reasons,
        "preview_recurring_matched_date": preview.preview_recurring_matched_date,
    })
}

fn transfer_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    let mut changes = vec![
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_type")),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::RecurringId,
            snapshot_optional_i64(snapshot, "preview_recurring_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::RecurringName,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_recurring_name")),
        ),
        (
            ImportPreviewPatchField::RecurringCandidateCount,
            ImportPreviewPatchValue::Integer(snapshot_i64(
                snapshot,
                "preview_recurring_candidate_count",
            )),
        ),
        (
            ImportPreviewPatchField::RecurringMatchScore,
            ImportPreviewPatchValue::Real(snapshot_f64(snapshot, "preview_recurring_match_score")),
        ),
        (
            ImportPreviewPatchField::RecurringMatchReasons,
            ImportPreviewPatchValue::Text(snapshot_text(
                snapshot,
                "preview_recurring_match_reasons",
            )),
        ),
        (
            ImportPreviewPatchField::RecurringMatchedDate,
            ImportPreviewPatchValue::Text(snapshot_text(
                snapshot,
                "preview_recurring_matched_date",
            )),
        ),
    ];

    if snapshot.get("preview_source_account_id").is_some() {
        changes.push((
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ));
    }
    if snapshot.get("preview_destination_account_id").is_some() {
        changes.push((
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ));
    }

    changes
}

#[derive(Debug, Clone)]
struct ImportPreviewTransferAccount {
    id: i64,
    aliases: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ImportPreviewTransferAccountResolution {
    source_account_id: Option<i64>,
    destination_account_id: Option<i64>,
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_transfer_accounts_for_preview(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    preview: &ImportPreviewRow,
) -> DbResult<ImportPreviewTransferAccountResolution> {
    let Some(transfer_feedback) = preview
        .preview_matching_feedback
        .get("transfer")
        .filter(|value| value.is_object())
    else {
        return Ok(ImportPreviewTransferAccountResolution::default());
    };
    let Some(source_chain) = transfer_feedback
        .get("source_chain")
        .and_then(Value::as_array)
        .filter(|chain| !chain.is_empty())
    else {
        return Ok(ImportPreviewTransferAccountResolution::default());
    };
    let accounts = load_transfer_resolution_accounts(tx, user_id)?;
    Ok(resolve_transfer_accounts_from_chain(
        source_chain,
        &accounts,
        preview.preview_source_account_id,
        preview.preview_destination_account_id,
    ))
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_transfer_resolution_accounts(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
) -> DbResult<Vec<ImportPreviewTransferAccount>> {
    if !import_staging_table_exists_tx(tx, "accounts")? {
        return Ok(Vec::new());
    }

    let aliases_expr = if import_staging_column_exists_tx(tx, "accounts", "aliases")? {
        "aliases".to_string()
    } else {
        "NULL AS aliases".to_string()
    };
    let hidden_expr = if import_staging_column_exists_tx(tx, "accounts", "hidden")? {
        "hidden".to_string()
    } else {
        "0 AS hidden".to_string()
    };
    let hidden_filter = if import_staging_column_exists_tx(tx, "accounts", "hidden")? {
        "hidden = 0"
    } else {
        "1 = 1"
    };

    let mut statement = tx.prepare(&format!(
        "
        SELECT id, name, {aliases_expr}, {hidden_expr}
        FROM accounts
        WHERE user_id = ?1 AND {hidden_filter}
        ORDER BY id ASC
        "
    ))?;
    let rows = statement.query_map(params![user_id_i64(user_id)?], |row| {
        let name = row.get::<_, Option<String>>("name")?.unwrap_or_default();
        let raw_aliases = row.get::<_, Option<String>>("aliases")?;
        let mut aliases = parse_transfer_account_aliases(raw_aliases.as_deref());
        aliases.push(name.clone());
        Ok(ImportPreviewTransferAccount {
            id: row.get("id")?,
            aliases,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_transfer_accounts_from_chain(
    source_chain: &[Value],
    accounts: &[ImportPreviewTransferAccount],
    current_source_account_id: Option<i64>,
    current_destination_account_id: Option<i64>,
) -> ImportPreviewTransferAccountResolution {
    let outgoing = transfer_chain_entry_for_roles(
        source_chain,
        &["outgoing", "source", "from", "debit", "out"],
        Some(0),
    );
    let incoming = transfer_chain_entry_for_roles(
        source_chain,
        &["incoming", "destination", "to", "credit", "in"],
        Some(1),
    );
    let resolved_source = current_source_account_id.or_else(|| {
        outgoing.and_then(|entry| resolve_transfer_account_from_entry(entry, accounts))
    });
    let resolved_destination = current_destination_account_id.or_else(|| {
        incoming.and_then(|entry| resolve_transfer_account_from_entry(entry, accounts))
    });
    let destination_account_id = resolved_destination.filter(|destination| {
        resolved_source.is_none_or(|source| source != *destination)
    });

    ImportPreviewTransferAccountResolution {
        source_account_id: (current_source_account_id.is_none())
            .then_some(resolved_source)
            .flatten(),
        destination_account_id: (current_destination_account_id.is_none())
            .then_some(destination_account_id)
            .flatten(),
    }
}

fn transfer_chain_entry_for_roles<'a>(
    source_chain: &'a [Value],
    roles: &[&str],
    fallback_index: Option<usize>,
) -> Option<&'a Value> {
    source_chain
        .iter()
        .find(|entry| {
            let role = transfer_account_value_text(entry.get("role"))
                .map(|value| value.trim().to_ascii_lowercase())
                .unwrap_or_default();
            roles.iter().any(|candidate| role == *candidate)
        })
        .or_else(|| fallback_index.and_then(|index| source_chain.get(index)))
}

#[tracing::instrument(level = "debug", skip_all)]
fn resolve_transfer_account_from_entry(
    entry: &Value,
    accounts: &[ImportPreviewTransferAccount],
) -> Option<i64> {
    for field in [
        "source_account_id",
        "account_id",
        "id",
        "preview_source_account_id",
        "preview_destination_account_id",
    ] {
        if let Some(account_id) = entry
            .get(field)
            .and_then(transfer_account_id_from_value)
            .filter(|account_id| {
                accounts.is_empty() || accounts.iter().any(|account| account.id == *account_id)
            })
        {
            return Some(account_id);
        }
    }

    let tokens = transfer_account_tokens_from_entry(entry);
    if tokens.is_empty() {
        return None;
    }
    accounts
        .iter()
        .find(|account| transfer_account_matches_tokens(account, &tokens))
        .map(|account| account.id)
}

fn transfer_account_id_from_value(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return (number > 0).then_some(number);
    }
    value.as_str().and_then(parse_positive_i64)
}

fn transfer_account_tokens_from_entry(entry: &Value) -> Vec<String> {
    let mut tokens = Vec::new();
    for field in [
        "account_name",
        "payment_method",
        "parser_id",
        "counterparty",
        "source_account_id",
        "account_id",
        "name",
        "label",
        "parser_label",
    ] {
        if let Some(text) = transfer_account_value_text(entry.get(field)) {
            tokens.extend(expand_transfer_account_token(&text));
        }
    }
    if let Some(tags) = entry.get("tags").and_then(Value::as_array) {
        for tag in tags {
            if let Some(text) = transfer_account_value_text(Some(tag)) {
                tokens.extend(expand_transfer_account_token(&text));
            }
        }
    }
    tokens
}

fn expand_transfer_account_token(value: &str) -> Vec<String> {
    let normalized = normalize_transfer_account_text(value);
    if normalized.is_empty() {
        return Vec::new();
    }
    let mut tokens = vec![normalized.clone()];
    for prefix in ["parser:", "channel:", "account:", "source:"] {
        if let Some(stripped) = normalized.strip_prefix(prefix) {
            if !stripped.is_empty() {
                tokens.push(stripped.to_string());
            }
        }
    }
    tokens
}

fn transfer_account_matches_tokens(
    account: &ImportPreviewTransferAccount,
    tokens: &[String],
) -> bool {
    account.aliases.iter().any(|alias| {
        let alias = normalize_transfer_account_text(alias);
        !alias.is_empty()
            && tokens
                .iter()
                .any(|token| token == &alias || token.contains(&alias) || alias.contains(token))
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn parse_transfer_account_aliases(raw_aliases: Option<&str>) -> Vec<String> {
    let raw_aliases = raw_aliases.unwrap_or("").trim();
    if raw_aliases.is_empty() {
        return Vec::new();
    }
    if let Ok(Value::Array(values)) = serde_json::from_str::<Value>(raw_aliases) {
        return values
            .iter()
            .filter_map(|value| transfer_account_value_text(Some(value)))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }
    raw_aliases
        .split([',', ';', '|', '，', '；'])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn transfer_account_value_text(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }
    if let Some(number) = value.as_i64() {
        return Some(number.to_string());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    value.as_f64().map(|number| number.to_string())
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_transfer_account_text(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_learning_snapshot(value: &Value) -> Option<Value> {
    value.as_object().map(|_| {
        serde_json::json!({
            "preview_type": snapshot_text(value, "preview_type"),
            "preview_main_category": snapshot_text(value, "preview_main_category"),
            "preview_sub_category": snapshot_text(value, "preview_sub_category"),
            "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
            "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
        })
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_learning_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_type": preview.preview_type,
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
    })
}

fn category_rule_account_baseline_snapshot(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    preview: &ImportPreviewRow,
) -> DbResult<Option<Value>> {
    let feedback = &preview.preview_matching_feedback;
    if let Some(snapshot) = feedback
        .get("stage2_baseline")
        .and_then(normalize_stage2_baseline_snapshot)
    {
        return Ok(Some(snapshot));
    }

    let category = feedback
        .get("category_rule")
        .filter(|value| value.is_object())
        .and_then(|value| value.get("category_id"))
        .and_then(value_to_positive_i64)
        .map(|category_id| import_preview_category_by_id(tx, user_id, category_id))
        .transpose()?
        .flatten();

    let account = feedback.get("account").filter(|value| value.is_object());
    let source_account_id = account
        .and_then(|value| value.get("source_account_id"))
        .and_then(value_to_positive_i64);
    let destination_account_id = account
        .and_then(|value| value.get("destination_account_id"))
        .and_then(value_to_positive_i64);
    if category.is_none() && source_account_id.is_none() && destination_account_id.is_none() {
        return Ok(None);
    }

    let (preview_type, main_category, sub_category) = category
        .as_ref()
        .map(|category| {
            (
                preview_type_name_for_category_type(category.type_code)
                    .unwrap_or(preview.preview_type.as_str())
                    .to_string(),
                category.main_category.clone(),
                category.sub_category.clone(),
            )
        })
        .unwrap_or_else(|| {
            (
                preview.preview_type.clone(),
                preview.preview_main_category.clone(),
                preview.preview_sub_category.clone(),
            )
        });
    let source_account_id = source_account_id.or(preview.preview_source_account_id);
    let destination_account_id = destination_account_id.or(preview.preview_destination_account_id);

    Ok(Some(serde_json::json!({
        "preview_type": preview_type,
        "preview_main_category": main_category,
        "preview_sub_category": sub_category,
        "preview_source_account_id": source_account_id,
        "preview_destination_account_id": destination_account_id,
    })))
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_stage2_baseline_snapshot(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    let has_baseline_field = [
        "preview_type",
        "preview_main_category",
        "preview_sub_category",
        "preview_source_account_id",
        "preview_destination_account_id",
    ]
    .iter()
    .any(|field| object.contains_key(*field));
    if !has_baseline_field {
        return None;
    }

    Some(serde_json::json!({
        "preview_type": snapshot_text(value, "preview_type"),
        "preview_main_category": snapshot_text(value, "preview_main_category"),
        "preview_sub_category": snapshot_text(value, "preview_sub_category"),
        "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
        "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
    }))
}

fn value_to_positive_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return (number > 0).then_some(number);
    }
    value.as_str().and_then(parse_positive_i64)
}

fn learning_accept_preview_snapshot(
    applied_result: Option<&ImportPreviewLearningApply>,
    preview: &ImportPreviewRow,
) -> Value {
    serde_json::json!({
        "preview_type": applied_result
            .and_then(|value| value.preview_type.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_type.clone()),
        "preview_main_category": applied_result
            .and_then(|value| value.preview_main_category.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_main_category.clone()),
        "preview_sub_category": applied_result
            .and_then(|value| value.preview_sub_category.as_ref())
            .cloned()
            .unwrap_or_else(|| preview.preview_sub_category.clone()),
        "preview_source_account_id": applied_result
            .and_then(|value| value.preview_source_account_id)
            .unwrap_or(preview.preview_source_account_id),
        "preview_destination_account_id": applied_result
            .and_then(|value| value.preview_destination_account_id)
            .unwrap_or(preview.preview_destination_account_id),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImportPreviewCanonicalCategory {
    id: i64,
    type_code: Option<i64>,
    main_category: String,
    sub_category: String,
}

fn transfer_decision_category(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
) -> DbResult<Option<ImportPreviewCanonicalCategory>> {
    if let Some(category_id) = user_cash_transfer_category_id(tx, user_id)? {
        if let Some(category) = import_preview_category_by_id(tx, user_id, category_id)? {
            if category.type_code.is_none_or(|type_code| type_code == 4) {
                return Ok(Some(category));
            }
        }
    }

    first_import_preview_category_for_type(tx, user_id, 4)
}

fn learning_rule_category(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    rule_id: i64,
) -> DbResult<Option<ImportPreviewCanonicalCategory>> {
    if rule_id <= 0
        || !import_staging_table_exists_tx(tx, "import_learning_rules")?
        || !import_staging_column_exists_tx(tx, "import_learning_rules", "learned_category_id")?
    {
        return Ok(None);
    }

    let category_id = tx
        .query_row(
            "
            SELECT learned_category_id
            FROM import_learning_rules
            WHERE id = ?1 AND user_id = ?2
            LIMIT 1
            ",
            params![rule_id, user_id_i64(user_id)?],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        .filter(|category_id| *category_id > 0);

    match category_id {
        Some(category_id) => import_preview_category_by_id(tx, user_id, category_id),
        None => Ok(None),
    }
}

fn canonicalize_learning_snapshot_category(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    snapshot: &mut Value,
) -> DbResult<()> {
    let main_category = snapshot_text(snapshot, "preview_main_category");
    let sub_category = snapshot_text(snapshot, "preview_sub_category");
    if main_category.trim().is_empty() && sub_category.trim().is_empty() {
        return Ok(());
    }

    let preview_type = snapshot_text(snapshot, "preview_type");
    let expected_type = preview_category_type_code(&preview_type);
    if let Some(category) =
        import_preview_category_by_path(tx, user_id, expected_type, &main_category, &sub_category)?
    {
        apply_category_to_learning_snapshot(snapshot, &category);
    } else {
        set_learning_snapshot_text(snapshot, "preview_main_category", String::new());
        set_learning_snapshot_text(snapshot, "preview_sub_category", String::new());
    }

    Ok(())
}

#[tracing::instrument(level = "debug", skip_all)]
fn apply_category_to_learning_snapshot(
    snapshot: &mut Value,
    category: &ImportPreviewCanonicalCategory,
) {
    let fallback_type = snapshot_text(snapshot, "preview_type");
    let preview_type = preview_type_name_for_category_type(category.type_code)
        .unwrap_or(fallback_type.as_str())
        .to_string();
    set_learning_snapshot_text(snapshot, "preview_type", preview_type);
    set_learning_snapshot_text(
        snapshot,
        "preview_main_category",
        category.main_category.clone(),
    );
    set_learning_snapshot_text(
        snapshot,
        "preview_sub_category",
        category.sub_category.clone(),
    );
}

fn set_learning_snapshot_text(snapshot: &mut Value, field: &str, value: String) {
    if let Some(object) = snapshot.as_object_mut() {
        object.insert(field.to_string(), Value::String(value));
    }
}

fn preview_type_name_for_category_type(type_code: Option<i64>) -> Option<&'static str> {
    match type_code {
        Some(2) => Some("收入"),
        Some(3) => Some("支出"),
        Some(4) => Some("转账"),
        Some(5) => Some("投资"),
        _ => None,
    }
}

fn preview_category_type_code(preview_type: &str) -> Option<i64> {
    match preview_type.trim().to_ascii_lowercase().as_str() {
        "收入" | "income" | "2" => Some(2),
        "支出" | "expense" | "3" => Some(3),
        "转账" | "transfer" | "4" => Some(4),
        "投资" | "investment" | "5" => Some(5),
        _ => None,
    }
}

fn user_cash_transfer_category_id(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
) -> DbResult<Option<i64>> {
    if !import_staging_table_exists_tx(tx, "users")?
        || !import_staging_column_exists_tx(tx, "users", "cash_transfer_category_id")?
    {
        return Ok(None);
    }

    Ok(tx
        .query_row(
            "SELECT cash_transfer_category_id FROM users WHERE id = ?1 LIMIT 1",
            params![user_id_i64(user_id)?],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten()
        .filter(|category_id| *category_id > 0))
}

fn first_import_preview_category_for_type(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    type_code: i64,
) -> DbResult<Option<ImportPreviewCanonicalCategory>> {
    if !import_staging_table_exists_tx(tx, "categories")?
        || !import_staging_column_exists_tx(tx, "categories", "type")?
    {
        return Ok(None);
    }

    let order_by = if import_staging_column_exists_tx(tx, "categories", "priority")? {
        "priority, id"
    } else {
        "id"
    };
    tx.query_row(
        &format!(
            "
            SELECT id, type, main_category, sub_category
            FROM categories
            WHERE user_id = ?1 AND type = ?2
            ORDER BY {order_by}
            LIMIT 1
            "
        ),
        params![user_id_i64(user_id)?, type_code],
        import_preview_category_from_row,
    )
    .optional()
    .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_category_by_id(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    category_id: i64,
) -> DbResult<Option<ImportPreviewCanonicalCategory>> {
    if category_id <= 0 || !import_staging_table_exists_tx(tx, "categories")? {
        return Ok(None);
    }

    if import_staging_column_exists_tx(tx, "categories", "type")? {
        tx.query_row(
            "
            SELECT id, type, main_category, sub_category
            FROM categories
            WHERE id = ?1 AND user_id = ?2
            LIMIT 1
            ",
            params![category_id, user_id_i64(user_id)?],
            import_preview_category_from_row,
        )
        .optional()
        .map_err(DbError::from)
    } else {
        tx.query_row(
            "
            SELECT id, main_category, sub_category
            FROM categories
            WHERE id = ?1 AND user_id = ?2
            LIMIT 1
            ",
            params![category_id, user_id_i64(user_id)?],
            |row| {
                Ok(ImportPreviewCanonicalCategory {
                    id: row.get(0)?,
                    type_code: None,
                    main_category: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    sub_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                })
            },
        )
        .optional()
        .map_err(DbError::from)
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_category_by_path(
    tx: &rusqlite::Transaction<'_>,
    user_id: UserId,
    expected_type: Option<i64>,
    main_category: &str,
    sub_category: &str,
) -> DbResult<Option<ImportPreviewCanonicalCategory>> {
    if !import_staging_table_exists_tx(tx, "categories")? {
        return Ok(None);
    }

    let user_id = user_id_i64(user_id)?;
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if import_staging_column_exists_tx(tx, "categories", "type")? {
        if let Some(expected_type) = expected_type {
            return tx
                .query_row(
                    "
                    SELECT id, type, main_category, sub_category
                    FROM categories
                    WHERE user_id = ?1
                      AND type = ?2
                      AND main_category = ?3
                      AND sub_category = ?4
                    LIMIT 1
                    ",
                    params![user_id, expected_type, main_category, sub_category],
                    import_preview_category_from_row,
                )
                .optional()
                .map_err(DbError::from);
        }

        return tx
            .query_row(
                "
                SELECT id, type, main_category, sub_category
                FROM categories
                WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3
                LIMIT 1
                ",
                params![user_id, main_category, sub_category],
                import_preview_category_from_row,
            )
            .optional()
            .map_err(DbError::from);
    }

    tx.query_row(
        "
        SELECT id, main_category, sub_category
        FROM categories
        WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3
        LIMIT 1
        ",
        params![user_id, main_category, sub_category],
        |row| {
            Ok(ImportPreviewCanonicalCategory {
                id: row.get(0)?,
                type_code: None,
                main_category: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                sub_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
            })
        },
    )
    .optional()
    .map_err(DbError::from)
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_preview_category_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ImportPreviewCanonicalCategory> {
    Ok(ImportPreviewCanonicalCategory {
        id: row.get(0)?,
        type_code: row.get::<_, Option<i64>>(1)?,
        main_category: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        sub_category: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_staging_table_exists_tx(
    tx: &rusqlite::Transaction<'_>,
    table_name: &str,
) -> DbResult<bool> {
    Ok(tx.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
        params![table_name],
        |_| Ok(()),
    )
    .optional()?
    .is_some())
}

#[tracing::instrument(level = "debug", skip_all)]
fn import_staging_column_exists_tx(
    tx: &rusqlite::Transaction<'_>,
    table_name: &str,
    column_name: &str,
) -> DbResult<bool> {
    if !matches!(
        table_name,
        "accounts" | "categories" | "users" | "import_learning_rules"
    ) {
        return Ok(false);
    }

    let mut statement = tx.prepare(&format!("PRAGMA table_info({table_name})"))?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column_name {
            return Ok(true);
        }
    }
    Ok(false)
}

fn learning_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::Type,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_type")),
        ),
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
    ]
}

fn learning_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_type") == preview.preview_type
        && snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
}

fn transfer_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_type") == preview.preview_type
        && snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
        && snapshot_optional_i64(snapshot, "preview_recurring_id") == preview.preview_recurring_id
        && snapshot_text(snapshot, "preview_recurring_name") == preview.preview_recurring_name
        && snapshot_i64(snapshot, "preview_recurring_candidate_count")
            == preview.preview_recurring_candidate_count
        && (snapshot_f64(snapshot, "preview_recurring_match_score")
            - preview.preview_recurring_match_score)
            .abs()
            < f64::EPSILON
        && snapshot_text(snapshot, "preview_recurring_match_reasons")
            == preview.preview_recurring_match_reasons
        && snapshot_text(snapshot, "preview_recurring_matched_date")
            == preview.preview_recurring_matched_date
}

fn preview_llm_not_found() -> ImportPreviewLlmDecisionResult {
    ImportPreviewLlmDecisionResult {
        preview: None,
        event_id: None,
        applied_fields: Vec::new(),
        restored: false,
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_llm_previous_snapshot(preview: &ImportPreviewRow) -> Value {
    serde_json::json!({
        "preview_main_category": preview.preview_main_category,
        "preview_sub_category": preview.preview_sub_category,
        "preview_source_account_id": preview.preview_source_account_id,
        "preview_destination_account_id": preview.preview_destination_account_id,
    })
}

#[tracing::instrument(level = "debug", skip_all)]
fn normalize_llm_snapshot(value: &Value) -> Option<Value> {
    value.as_object().map(|_| {
        serde_json::json!({
            "preview_main_category": snapshot_text(value, "preview_main_category"),
            "preview_sub_category": snapshot_text(value, "preview_sub_category"),
            "preview_source_account_id": snapshot_account_id(value, "preview_source_account_id"),
            "preview_destination_account_id": snapshot_account_id(value, "preview_destination_account_id"),
        })
    })
}

fn llm_snapshot_restore_changes(
    snapshot: &Value,
) -> Vec<(ImportPreviewPatchField, ImportPreviewPatchValue)> {
    vec![
        (
            ImportPreviewPatchField::MainCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_main_category")),
        ),
        (
            ImportPreviewPatchField::SubCategory,
            ImportPreviewPatchValue::Text(snapshot_text(snapshot, "preview_sub_category")),
        ),
        (
            ImportPreviewPatchField::SourceAccountId,
            snapshot_account_id(snapshot, "preview_source_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
        (
            ImportPreviewPatchField::DestinationAccountId,
            snapshot_account_id(snapshot, "preview_destination_account_id")
                .map(ImportPreviewPatchValue::Integer)
                .unwrap_or(ImportPreviewPatchValue::Null),
        ),
    ]
}

fn llm_preview_matches_snapshot(preview: &ImportPreviewRow, snapshot: &Value) -> bool {
    snapshot_text(snapshot, "preview_main_category") == preview.preview_main_category
        && snapshot_text(snapshot, "preview_sub_category") == preview.preview_sub_category
        && snapshot_account_id(snapshot, "preview_source_account_id")
            == preview.preview_source_account_id
        && snapshot_account_id(snapshot, "preview_destination_account_id")
            == preview.preview_destination_account_id
}
