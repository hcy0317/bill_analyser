#[tracing::instrument(level = "debug", skip_all)]
pub async fn import_postgres_settings_bundle(
    pool: &PostgresPool,
    bundle: &Value,
    user_id: i64,
    dry_run: bool,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "taxonomy", operation = "import_postgres_settings_bundle", "business operation entered");
    let sections = normalize_settings_bundle_sections(bundle)?;
    let mut transaction = pool.begin().await?;
    let mut result_sections = ImportSections::new();
    let mut warnings = Vec::new();

    let account_ref_map = import_postgres_settings_accounts(
        &mut transaction,
        section_items(&sections, "accounts"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )
    .await?;
    let category_ref_map = import_postgres_settings_categories(
        &mut transaction,
        section_items(&sections, "transactionCategories"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )
    .await?;
    let tag_ref_map = import_postgres_settings_tags(
        &mut transaction,
        section_items(&sections, "transactionTags"),
        user_id,
        &mut result_sections,
        &mut warnings,
    )
    .await?;
    import_postgres_settings_templates(
        &mut transaction,
        "transactionTemplates",
        section_items(&sections, "transactionTemplates"),
        user_id,
        1,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
        &category_ref_map,
        &tag_ref_map,
    )
    .await?;
    import_postgres_settings_templates(
        &mut transaction,
        "scheduledTransactions",
        section_items(&sections, "scheduledTransactions"),
        user_id,
        2,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
        &category_ref_map,
        &tag_ref_map,
    )
    .await?;
    import_postgres_settings_category_rules(
        &mut transaction,
        section_items(&sections, "categoryRecognitionRules"),
        user_id,
        &mut result_sections,
        &mut warnings,
        &category_ref_map,
    )
    .await?;
    import_postgres_settings_account_rules(
        &mut transaction,
        section_items(&sections, "accountRecognitionRules"),
        user_id,
        &mut result_sections,
        &mut warnings,
        &account_ref_map,
    )
    .await?;
    skip_postgres_settings_section(
        "llmConfigs",
        section_items(&sections, "llmConfigs"),
        &mut result_sections,
        &mut warnings,
        "PostgreSQL settings bundle LLM config import is not supported for this section",
    );
    skip_postgres_settings_section(
        "ocrConfig",
        section_items(&sections, "ocrConfig"),
        &mut result_sections,
        &mut warnings,
        "PostgreSQL settings bundle OCR config import is not supported for this section",
    );

    if dry_run {
        transaction.rollback().await?;
    } else {
        transaction.commit().await?;
    }

    Ok(json!({
        "dryRun": dry_run,
        "schemaVersion": SETTINGS_BUNDLE_SCHEMA_VERSION,
        "sections": result_sections.into_value(),
        "warnings": warnings,
        "resolvedRefs": {
            "accounts": account_ref_map.len(),
            "transactionCategories": category_ref_map.len(),
            "transactionTags": tag_ref_map.len(),
        },
    }))
}

fn skip_postgres_settings_section(
    section_key: &str,
    items: &[Value],
    result: &mut ImportSections,
    warnings: &mut Vec<String>,
    message: &str,
) {
    if items.is_empty() {
        return;
    }
    result.get_mut(section_key).skipped += i64::try_from(items.len()).unwrap_or(i64::MAX);
    warnings.push(message.to_string());
}
