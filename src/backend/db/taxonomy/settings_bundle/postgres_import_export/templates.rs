#[tracing::instrument(level = "debug", skip_all)]
#[allow(clippy::too_many_arguments)]
// 中文说明：导入交易模板或周期模板 section，消费账户、分类、标签引用映射后再写入模板表。
async fn import_postgres_settings_templates(
    transaction: &mut PgTransaction<'_, Postgres>,
    section_key: &str,
    templates: &[Value],
    user_id: i64,
    template_type: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    account_ref_map: &BTreeMap<String, i64>,
    category_ref_map: &BTreeMap<String, i64>,
    tag_ref_map: &BTreeMap<String, i64>,
) -> DbResult<()> {
    if templates.is_empty() {
        return Ok(());
    }

    let mut existing = load_existing_postgres_templates(transaction, user_id).await?;
    for item in templates {
        let name = safe_text(item.get("name"), "");
        let fallback_display_order = existing
            .by_key
            .get(&(template_type, name.clone()))
            .map_or_else(
                || existing.next_display_order(template_type),
                |record| record.display_order,
            );
        let values = {
            let section = result.get_mut(section_key);
            settings_template_values_from_item(
                item,
                template_type,
                fallback_display_order,
                account_ref_map,
                category_ref_map,
                tag_ref_map,
                section,
                warnings,
            )
        };
        let Some(values) = values else {
            continue;
        };
        let key = (values.template_type, values.name.clone());

        if let Some(record) = existing.by_key.get(&key).cloned() {
            update_postgres_settings_template(transaction, user_id, record.id, &values).await?;
            existing.by_key.insert(
                key,
                ExistingTemplate {
                    id: record.id,
                    display_order: values.display_order,
                },
            );
            result.get_mut(section_key).updated += 1;
            continue;
        }

        let template_id = insert_postgres_settings_template(transaction, user_id, &values).await?;
        existing.by_key.insert(
            key,
            ExistingTemplate {
                id: template_id,
                display_order: values.display_order,
            },
        );
        result.get_mut(section_key).created += 1;
    }

    Ok(())
}

async fn load_existing_postgres_templates(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingTemplates> {
    let rows = sqlx::query(
        r#"
        SELECT id, template_type, name, display_order
        FROM transaction_templates
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(&mut **transaction)
    .await?;
    let mut existing = ExistingTemplates::default();
    for row in rows {
        let template_type = i64::from(row.try_get::<i32, _>("template_type")?);
        let name: String = row.try_get("name")?;
        existing.by_key.insert(
            (template_type, name),
            ExistingTemplate {
                id: row.try_get("id")?,
                display_order: i64::from(row.try_get::<i32, _>("display_order")?),
            },
        );
    }
    Ok(existing)
}

#[allow(clippy::too_many_arguments)]
// 中文说明：把 settings bundle 模板条目解析成 PostgreSQL 写入值，严格保留 minor units 和引用解析结果。
fn settings_template_values_from_item(
    item: &Value,
    template_type: i64,
    fallback_display_order: i64,
    account_ref_map: &BTreeMap<String, i64>,
    category_ref_map: &BTreeMap<String, i64>,
    tag_ref_map: &BTreeMap<String, i64>,
    section: &mut SectionCounts,
    warnings: &mut Vec<String>,
) -> Option<SettingsTemplateImportValues> {
    let name = safe_text(item.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped template without name".to_string());
        return None;
    }

    let resolved = resolve_template_payload(&json!({
        "item": item,
        "account_ref_map": account_ref_map,
        "category_ref_map": category_ref_map,
        "tag_ref_map": tag_ref_map,
    }));
    warnings.extend(
        resolved
            .get("warnings")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned),
    );
    if safe_bool(resolved.get("unresolved")) {
        section.skipped += 1;
        return None;
    }

    let payload = resolved.get("payload").unwrap_or(&Value::Null);
    let scheduled = template_type == 2;
    let display_order = get_any(item, &["displayOrder", "display_order"])
        .map(|value| safe_int(Some(value), fallback_display_order))
        .unwrap_or(fallback_display_order);
    let source_account_id = settings_zero_id_text(payload.get("account"));
    let destination_account_id = settings_zero_id_text(payload.get("counterparty"));

    Some(SettingsTemplateImportValues {
        name,
        template_type,
        description: settings_optional_text(payload.get("description")),
        transaction_type: Some(safe_text(payload.get("type"), "3")),
        category_id: settings_optional_text(payload.get("category")),
        source_account_id,
        destination_account_id,
        source_amount_minor_units: settings_strict_minor_units(payload.get("source_amount_cents")),
        destination_amount_minor_units: settings_strict_minor_units(
            payload.get("destination_amount_cents"),
        ),
        hide_amount: safe_int(payload.get("hide_amount"), 0) != 0,
        tag_ids: settings_template_tag_ids(payload),
        comment: settings_optional_text(payload.get("comment")),
        scheduled_frequency_type: scheduled
            .then(|| safe_int(payload.get("scheduled_frequency_type"), 0)),
        scheduled_frequency: scheduled
            .then(|| settings_optional_text(payload.get("frequency")))
            .flatten(),
        scheduled_start_date: scheduled
            .then(|| settings_optional_text(payload.get("start_date")))
            .flatten(),
        scheduled_end_date: scheduled
            .then(|| settings_optional_text(payload.get("end_date")))
            .flatten(),
        scheduled_next_date: scheduled
            .then(|| settings_optional_text(payload.get("next_date")))
            .flatten(),
        enabled: safe_int(payload.get("enabled"), 1) != 0,
        auto_create: safe_int(payload.get("auto_create"), 0) != 0,
        display_order,
        hidden: safe_int(payload.get("hidden"), 0) != 0,
        utc_offset: safe_int(payload.get("utc_offset"), 0),
    })
}

// 中文说明：按模板 ID 更新 settings bundle 导入命中的现有模板，避免重建导致引用和排序漂移。
async fn update_postgres_settings_template(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
    template_id: i64,
    values: &SettingsTemplateImportValues,
) -> DbResult<()> {
    sqlx::query(
        r#"
        UPDATE transaction_templates
        SET name = $1,
            description = $2,
            transaction_type = $3,
            category_id = $4,
            source_account_id = $5,
            destination_account_id = $6,
            source_amount_minor_units = $7,
            destination_amount_minor_units = $8,
            hide_amount = $9,
            tag_ids = $10,
            comment = $11,
            scheduled_frequency_type = $12,
            scheduled_frequency = $13,
            scheduled_start_date = $14,
            scheduled_end_date = $15,
            scheduled_next_date = $16,
            enabled = $17,
            auto_create = $18,
            display_order = $19,
            hidden = $20,
            utc_offset = $21,
            updated_at = now(),
            version = version + 1
        WHERE id = $22 AND user_id = $23 AND template_type = $24
        "#,
    )
    .bind(&values.name)
    .bind(&values.description)
    .bind(&values.transaction_type)
    .bind(&values.category_id)
    .bind(&values.source_account_id)
    .bind(&values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(&values.tag_ids)
    .bind(&values.comment)
    .bind(values.scheduled_frequency_type.map(settings_i64_to_i32))
    .bind(&values.scheduled_frequency)
    .bind(&values.scheduled_start_date)
    .bind(&values.scheduled_end_date)
    .bind(&values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(settings_i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(settings_i64_to_i32(values.utc_offset))
    .bind(template_id)
    .bind(user_id)
    .bind(settings_i64_to_i32(values.template_type))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

// 中文说明：插入 settings bundle 中的新模板，普通模板和周期模板共用同一 minor units 写入合同。
async fn insert_postgres_settings_template(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
    values: &SettingsTemplateImportValues,
) -> DbResult<i64> {
    let row = sqlx::query(
        r#"
        INSERT INTO transaction_templates (
            user_id, template_type, name, description, transaction_type,
            category_id, source_account_id, destination_account_id,
            source_amount_minor_units, destination_amount_minor_units,
            hide_amount, tag_ids, comment, scheduled_frequency_type,
            scheduled_frequency, scheduled_start_date, scheduled_end_date,
            scheduled_next_date, enabled, auto_create, display_order, hidden,
            utc_offset
        )
        VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8,
            $9, $10,
            $11, $12, $13, $14,
            $15, $16, $17,
            $18, $19, $20, $21, $22,
            $23
        )
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(settings_i64_to_i32(values.template_type))
    .bind(&values.name)
    .bind(&values.description)
    .bind(&values.transaction_type)
    .bind(&values.category_id)
    .bind(&values.source_account_id)
    .bind(&values.destination_account_id)
    .bind(values.source_amount_minor_units)
    .bind(values.destination_amount_minor_units)
    .bind(values.hide_amount)
    .bind(&values.tag_ids)
    .bind(&values.comment)
    .bind(values.scheduled_frequency_type.map(settings_i64_to_i32))
    .bind(&values.scheduled_frequency)
    .bind(&values.scheduled_start_date)
    .bind(&values.scheduled_end_date)
    .bind(&values.scheduled_next_date)
    .bind(values.enabled)
    .bind(values.auto_create)
    .bind(settings_i64_to_i32(values.display_order))
    .bind(values.hidden)
    .bind(settings_i64_to_i32(values.utc_offset))
    .fetch_one(&mut **transaction)
    .await?;
    row.try_get("id").map_err(Into::into)
}
