async fn load_import_identity_maps_on_tx(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    user_id: i64,
) -> DbResult<ImportIdentityMaps> {
    let account_rows =
        sqlx::query("SELECT id FROM accounts WHERE user_id = $1 AND is_active = true")
            .bind(user_id)
            .fetch_all(&mut **tx)
            .await?;
    let category_rows = sqlx::query(
        "SELECT id, category_type FROM categories WHERE user_id = $1 AND is_active = true",
    )
    .bind(user_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut maps = ImportIdentityMaps::default();
    for row in account_rows {
        maps.active_accounts.insert(row.try_get("id")?);
    }
    for row in category_rows {
        let id = row.try_get::<i64, _>("id")?;
        let category_type = row
            .try_get::<Option<String>, _>("category_type")?
            .as_deref()
            .and_then(preview_category_type_code);
        maps.active_categories.insert(id, category_type);
    }
    Ok(maps)
}
