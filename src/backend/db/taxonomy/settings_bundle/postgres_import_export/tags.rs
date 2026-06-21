#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：导入标签 section，维护 tagRef/tagName 引用映射供模板 tag_ids 重映射使用。
async fn import_postgres_settings_tags(
    transaction: &mut PgTransaction<'_, Postgres>,
    tags: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_postgres_tags(transaction, user_id).await?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|tag_id| (local_id_ref("tag", *tag_id), *tag_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(
        existing
            .by_id
            .iter()
            .filter(|(_, name)| !name.is_empty())
            .map(|(tag_id, name)| (format!("tagName:{name}"), *tag_id)),
    );

    for item in tags {
        if let Some(tag_id) = upsert_postgres_settings_tag(
            transaction,
            item,
            user_id,
            result.get_mut("transactionTags"),
            &mut existing,
            warnings,
        )
        .await?
        {
            let tag_ref = external_ref(item, "tag");
            if !tag_ref.is_empty() {
                ref_map.insert(tag_ref, tag_id);
            }
            let name = safe_text(item.get("name"), "");
            if !name.is_empty() {
                ref_map.insert(format!("tagName:{name}"), tag_id);
            }
        }
    }
    Ok(ref_map)
}

async fn load_existing_postgres_tags(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingTags> {
    let rows = sqlx::query("SELECT id, name FROM tags WHERE user_id = $1 ORDER BY id")
        .bind(user_id)
        .fetch_all(&mut **transaction)
        .await?;
    let mut existing = ExistingTags::default();
    for row in rows {
        let tag_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        existing.by_id.insert(tag_id, name.clone());
        existing.by_name.insert(name, tag_id);
    }
    Ok(existing)
}

// 中文说明：按标签名称执行幂等 upsert，并把隐藏状态和图标保存在 metadata 中。
async fn upsert_postgres_settings_tag(
    transaction: &mut PgTransaction<'_, Postgres>,
    item: &Value,
    user_id: i64,
    section: &mut SectionCounts,
    existing: &mut ExistingTags,
    warnings: &mut Vec<String>,
) -> DbResult<Option<i64>> {
    let normalized = normalize_tag_import(&json!({ "item": item }));
    let name = safe_text(normalized.get("name"), "");
    if name.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped tag without name".to_string());
        return Ok(None);
    }
    let metadata = json!({
        "icon": safe_text(normalized.get("icon"), ""),
        "hidden": safe_bool(normalized.get("hidden")),
    });
    if let Some(tag_id) = existing.by_name.get(&name).copied() {
        sqlx::query(
            r#"
            UPDATE tags
            SET color = $1,
                display_order = $2,
                metadata = $3,
                updated_at = now(),
                version = version + 1
            WHERE id = $4 AND user_id = $5
            "#,
        )
        .bind(safe_text(normalized.get("color"), ""))
        .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
        .bind(metadata)
        .bind(tag_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        section.updated += 1;
        return Ok(Some(tag_id));
    }
    let row = sqlx::query(
        r#"
        INSERT INTO tags (user_id, name, color, display_order, metadata)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(name.trim())
    .bind(safe_text(normalized.get("color"), ""))
    .bind(settings_i64_to_i32(safe_int(normalized.get("display_order"), 0)))
    .bind(metadata)
    .fetch_one(&mut **transaction)
    .await?;
    let tag_id: i64 = row.try_get("id")?;
    existing.by_id.insert(tag_id, name.clone());
    existing.by_name.insert(name, tag_id);
    section.created += 1;
    Ok(Some(tag_id))
}
