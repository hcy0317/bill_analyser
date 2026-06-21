#[tracing::instrument(level = "debug", skip_all)]
pub async fn postgres_category_filters_for_ids(
    pool: &PostgresPool,
    user_id: i64,
    category_ids: &[i64],
) -> DbResult<Vec<BillCategoryFilter>> {
    let category_ids = normalize_ids(category_ids);
    if category_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut builder =
        QueryBuilder::<Postgres>::new("SELECT path, name FROM categories WHERE user_id = ");
    builder.push_bind(user_id);
    builder.push(" AND id IN (");
    push_bind_list(&mut builder, &category_ids);
    builder.push(") ORDER BY display_order ASC, id ASC");
    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            let path: Option<String> = row.try_get("path")?;
            let name: String = row.try_get("name")?;
            let (main, sub) = category_names_from_path(path.as_deref(), &name);
            Ok(BillCategoryFilter {
                main,
                sub: (!sub.trim().is_empty()).then_some(sub),
            })
        })
        .collect()
}
