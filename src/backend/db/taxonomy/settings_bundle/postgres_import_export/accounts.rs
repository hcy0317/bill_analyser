#[tracing::instrument(level = "debug", skip_all)]
async fn import_postgres_settings_accounts(
    transaction: &mut PgTransaction<'_, Postgres>,
    accounts: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_postgres_accounts(transaction, user_id).await?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|account_id| (local_id_ref("account", *account_id), *account_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(
        existing
            .by_id
            .iter()
            .filter(|(_, (name, _))| !name.is_empty())
            .map(|(account_id, (name, _))| (format!("accountName:{name}"), *account_id)),
    );

    let mut pending = accounts.iter().collect::<Vec<_>>();
    for _ in 0..=pending.len() {
        let mut next_pending = Vec::new();
        let mut progressed = false;
        for item in pending {
            let parent_ref = safe_text(get_any(item, &["parentRef", "parent_ref"]), "");
            if !parent_ref.is_empty() && !ref_map.contains_key(&parent_ref) {
                next_pending.push(item);
                continue;
            }
            if let Some(account_id) = upsert_postgres_settings_account(
                transaction,
                item,
                user_id,
                result.get_mut("accounts"),
                &mut existing,
                &ref_map,
                warnings,
            )
            .await?
            {
                add_account_refs(&mut ref_map, item, account_id);
                progressed = true;
            }
        }
        pending = next_pending;
        if pending.is_empty() || !progressed {
            break;
        }
    }

    for item in pending {
        warnings.push(format!(
            "Account parent not found; imported as root: {}",
            safe_text(item.get("name"), "")
        ));
        let mut root_item = item.as_object().cloned().unwrap_or_default();
        root_item.insert("parentRef".to_string(), Value::String(String::new()));
        if let Some(account_id) = upsert_postgres_settings_account(
            transaction,
            &Value::Object(root_item),
            user_id,
            result.get_mut("accounts"),
            &mut existing,
            &ref_map,
            warnings,
        )
        .await?
        {
            add_account_refs(&mut ref_map, item, account_id);
        }
    }

    Ok(ref_map)
}

async fn load_existing_postgres_accounts(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingAccounts> {
    let rows = sqlx::query(
        "SELECT id, name, metadata FROM accounts WHERE user_id = $1 ORDER BY id",
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = ExistingAccounts::default();
    for row in rows {
        let account_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let metadata: Value = row.try_get("metadata")?;
        let parent_id = postgres_metadata_parent_id(&metadata);
        existing
            .by_id
            .insert(account_id, (name.clone(), parent_id));
        existing.by_key.insert((name.clone(), parent_id), account_id);
        existing.by_key.insert((name, 0), account_id);
    }
    Ok(existing)
}

#[allow(clippy::too_many_arguments)]
async fn upsert_postgres_settings_account(
    transaction: &mut PgTransaction<'_, Postgres>,
    item: &Value,
    user_id: i64,
    section: &mut SectionCounts,
    existing: &mut ExistingAccounts,
    ref_map: &BTreeMap<String, i64>,
    warnings: &mut Vec<String>,
) -> DbResult<Option<i64>> {
    let name = safe_text(item.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped account without name".to_string());
        return Ok(None);
    }
    let normalized = match normalize_account_import(&json!({
        "item": item,
        "ref_map": ref_map,
    })) {
        Ok(normalized) => normalized,
        Err(account_warnings) => {
            section.skipped += 1;
            warnings.extend(account_warnings);
            return Ok(None);
        }
    };
    let parent_id = safe_int(normalized.get("parent_id"), 0);
    let metadata = postgres_account_metadata(&normalized, parent_id);
    let balance_cents = safe_int(normalized.get("balance_cents"), 0);

    if let Some(account_id) = existing
        .by_key
        .get(&(name.clone(), parent_id))
        .or_else(|| existing.by_key.get(&(name.clone(), 0)))
        .copied()
    {
        sqlx::query(
            r#"
            UPDATE accounts
            SET account_type = $1,
                currency = $2,
                balance_cents = $3,
                is_active = $4,
                display_order = $5,
                metadata = $6,
                updated_at = now(),
                version = version + 1
            WHERE id = $7 AND user_id = $8
            "#,
        )
        .bind(safe_text(normalized.get("type"), "1"))
        .bind(safe_text(normalized.get("currency"), "CNY"))
        .bind(balance_cents)
        .bind(safe_int(normalized.get("hidden"), 0) == 0)
        .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
        .bind(metadata)
        .bind(account_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        existing
            .by_id
            .insert(account_id, (name.clone(), parent_id));
        existing.by_key.insert((name, parent_id), account_id);
        section.updated += 1;
        return Ok(Some(account_id));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO accounts (
            user_id, name, account_type, currency, balance_cents, is_active,
            display_order, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(safe_text(normalized.get("type"), "1"))
    .bind(safe_text(normalized.get("currency"), "CNY"))
    .bind(balance_cents)
    .bind(safe_int(normalized.get("hidden"), 0) == 0)
    .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
    .bind(metadata)
    .fetch_one(&mut **transaction)
    .await?;
    let account_id: i64 = row.try_get("id")?;
    existing
        .by_id
        .insert(account_id, (name.clone(), parent_id));
    existing.by_key.insert((name.clone(), parent_id), account_id);
    existing.by_key.insert((name, 0), account_id);
    section.created += 1;
    Ok(Some(account_id))
}

fn postgres_account_metadata(normalized: &Value, parent_id: i64) -> Value {
    let mut metadata = Map::new();
    for key in ["category", "icon", "color", "comment"] {
        metadata.insert(
            key.to_string(),
            normalized.get(key).cloned().unwrap_or(Value::Null),
        );
    }
    metadata.insert(
        "initial_balance_cents".to_string(),
        json!(safe_int(normalized.get("initial_balance_cents"), 0)),
    );
    if parent_id > 0 {
        metadata.insert("parent_id".to_string(), Value::Number(parent_id.into()));
    }
    Value::Object(metadata)
}
