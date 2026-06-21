#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：导入分类 section，维护 categoryRef/categoryName 引用映射供模板和分类规则重映射使用。
async fn import_postgres_settings_categories(
    transaction: &mut PgTransaction<'_, Postgres>,
    categories: &[Value],
    user_id: i64,
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
) -> DbResult<BTreeMap<String, i64>> {
    let mut existing = load_existing_postgres_categories(transaction, user_id).await?;
    let mut ref_map = existing
        .by_id
        .keys()
        .map(|category_id| (local_id_ref("category", *category_id), *category_id))
        .collect::<BTreeMap<_, _>>();
    ref_map.extend(existing.by_id.iter().filter_map(|(category_id, (main, sub))| {
        let name = category_name(main, sub);
        (!name.is_empty()).then(|| (format!("categoryName:{name}"), *category_id))
    }));

    for item in categories {
        if let Some(category_id) = upsert_postgres_settings_category(
            transaction,
            item,
            user_id,
            result.get_mut("transactionCategories"),
            &mut existing,
            warnings,
        )
        .await?
        {
            let category_ref = external_ref(item, "category");
            if !category_ref.is_empty() {
                ref_map.insert(category_ref, category_id);
            }
            let main = safe_text(get_any(item, &["mainCategory", "main_category"]), "");
            let sub = safe_text(get_any(item, &["subCategory", "sub_category"]), "");
            let name = category_name(&main, &sub);
            if !name.is_empty() {
                ref_map.insert(format!("categoryName:{name}"), category_id);
            }
        }
    }

    Ok(ref_map)
}

async fn load_existing_postgres_categories(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ExistingCategories> {
    let rows = sqlx::query("SELECT id, name, path FROM categories WHERE user_id = $1 ORDER BY id")
        .bind(user_id)
        .fetch_all(&mut **transaction)
        .await?;
    let mut existing = ExistingCategories::default();
    for row in rows {
        let category_id: i64 = row.try_get("id")?;
        let name: String = row.try_get("name")?;
        let path: Option<String> = row.try_get("path")?;
        let (main, sub) = postgres_category_parts(path.as_deref(), &name);
        existing
            .by_id
            .insert(category_id, (main.clone(), sub.clone()));
        existing.by_key.insert((main, sub), category_id);
    }
    Ok(existing)
}

// 中文说明：按主/子分类执行幂等 upsert，保持父分类、path、隐藏和样式字段与当前分类模型一致。
async fn upsert_postgres_settings_category(
    transaction: &mut PgTransaction<'_, Postgres>,
    item: &Value,
    user_id: i64,
    section: &mut SectionCounts,
    existing: &mut ExistingCategories,
    warnings: &mut Vec<String>,
) -> DbResult<Option<i64>> {
    let normalized = normalize_category_import(&json!({ "item": item }));
    let main = safe_text(normalized.get("main_category"), "");
    let sub = safe_text(normalized.get("sub_category"), "");
    if main.is_empty() {
        section.skipped += 1;
        warnings.push("Skipped category without mainCategory".to_string());
        return Ok(None);
    }
    let parent_id = if sub.is_empty() {
        None
    } else {
        Some(
            ensure_postgres_parent_category(transaction, user_id, &main, existing)
                .await?,
        )
    };
    let name = if sub.is_empty() { main.clone() } else { sub.clone() };
    let path = category_name(&main, &sub);
    let metadata = json!({
        "description": safe_text(normalized.get("description"), ""),
    });

    if let Some(category_id) = existing.by_key.get(&(main.clone(), sub.clone())).copied() {
        sqlx::query(
            r#"
            UPDATE categories
            SET parent_id = $1,
                name = $2,
                category_type = $3,
                path = $4,
                icon = $5,
                color = $6,
                display_order = $7,
                is_active = $8,
                metadata = $9,
                updated_at = now(),
                version = version + 1
            WHERE id = $10 AND user_id = $11
            "#,
        )
        .bind(parent_id)
        .bind(name)
        .bind(safe_text(normalized.get("type"), "3"))
        .bind(path)
        .bind(safe_text(normalized.get("icon"), ""))
        .bind(safe_text(normalized.get("color"), ""))
        .bind(settings_i64_to_i32(safe_int(normalized.get("priority"), 0)))
        .bind(safe_int(normalized.get("hidden"), 0) == 0)
        .bind(metadata)
        .bind(category_id)
        .bind(user_id)
        .execute(&mut **transaction)
        .await?;
        section.updated += 1;
        return Ok(Some(category_id));
    }

    let row = sqlx::query(
        r#"
        INSERT INTO categories (
            user_id, parent_id, name, category_type, path, icon, color,
            display_order, is_active, metadata
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(parent_id)
    .bind(name)
    .bind(safe_text(normalized.get("type"), "3"))
    .bind(path)
    .bind(safe_text(normalized.get("icon"), ""))
    .bind(safe_text(normalized.get("color"), ""))
    .bind(settings_i64_to_i32(safe_int(normalized.get("priority"), 0)))
    .bind(safe_int(normalized.get("hidden"), 0) == 0)
    .bind(metadata)
    .fetch_one(&mut **transaction)
    .await?;
    let category_id: i64 = row.try_get("id")?;
    existing
        .by_id
        .insert(category_id, (main.clone(), sub.clone()));
    existing.by_key.insert((main, sub), category_id);
    section.created += 1;
    Ok(Some(category_id))
}

// 中文说明：导入子分类前确保主分类存在，避免 settings bundle 中只有子分类时引用断裂。
async fn ensure_postgres_parent_category(
    transaction: &mut PgTransaction<'_, Postgres>,
    user_id: i64,
    main: &str,
    existing: &mut ExistingCategories,
) -> DbResult<i64> {
    if let Some(parent_id) = existing
        .by_key
        .get(&(main.to_string(), String::new()))
        .copied()
    {
        return Ok(parent_id);
    }
    let row = sqlx::query(
        r#"
        INSERT INTO categories (user_id, name, category_type, path, display_order, is_active)
        VALUES ($1, $2, '3', $2, 0, true)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(main)
    .fetch_one(&mut **transaction)
    .await?;
    let parent_id: i64 = row.try_get("id")?;
    existing
        .by_id
        .insert(parent_id, (main.to_string(), String::new()));
    existing
        .by_key
        .insert((main.to_string(), String::new()), parent_id);
    Ok(parent_id)
}
