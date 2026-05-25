// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。

#[tracing::instrument(level = "debug", skip_all)]
fn insert_register_preset_categories(
    connection: &Connection,
    user_id: i64,
    categories: &[RegisterPresetCategory],
    created_at: &str,
) -> DbResult<bool> {
    for item in categories {
        let main_category = item.name.trim();
        if main_category.is_empty() {
            continue;
        }
        insert_category_ignore(
            connection,
            &CategoryInsertDraft {
                user_id,
                type_code: item.type_code,
                main_category,
                sub_category: "",
                priority: 0,
                icon: &item.icon,
                color: &item.color,
                created_at,
            },
        )?;
        for sub_item in &item.sub_categories {
            let sub_category = sub_item.name.trim();
            if sub_category.is_empty() {
                continue;
            }
            insert_category_ignore(
                connection,
                &CategoryInsertDraft {
                    user_id,
                    type_code: item.type_code,
                    main_category,
                    sub_category,
                    priority: 0,
                    icon: if sub_item.icon.is_empty() {
                        &item.icon
                    } else {
                        &sub_item.icon
                    },
                    color: if sub_item.color.is_empty() {
                        &item.color
                    } else {
                        &sub_item.color
                    },
                    created_at,
                },
            )?;
        }
    }
    Ok(true)
}

#[tracing::instrument(level = "debug", skip_all)]
pub(crate) fn ensure_default_category_seed(
    connection: &Connection,
    user_id: i64,
    created_at: &str,
) -> DbResult<RegisterDefaultSeedSummary> {
    #[cfg(not(coverage))]
    tracing::info!(domain = "auth", operation = "ensure_default_category_seed", "business operation entered");
    let mut summary = RegisterDefaultSeedSummary {
        categories_created: 0,
        categories_skipped: 0,
        rules_created: 0,
        rules_skipped: 0,
        rules_missing_categories: 0,
    };
    for category in DEFAULT_DAILY_CATEGORIES {
        if insert_category_ignore(
            connection,
            &CategoryInsertDraft {
                user_id,
                type_code: category.type_code,
                main_category: category.name,
                sub_category: "",
                priority: category.priority,
                icon: category.icon,
                color: category.color,
                created_at,
            },
        )? {
            summary.categories_created += 1;
        } else {
            summary.categories_skipped += 1;
        }
        for (offset, sub_category) in category.sub_categories.iter().enumerate() {
            if insert_category_ignore(
                connection,
                &CategoryInsertDraft {
                    user_id,
                    type_code: category.type_code,
                    main_category: category.name,
                    sub_category: sub_category.name,
                    priority: category.priority + offset as i64 + 1,
                    icon: sub_category.icon,
                    color: sub_category.color,
                    created_at,
                },
            )? {
                summary.categories_created += 1;
            } else {
                summary.categories_skipped += 1;
            }
        }
    }
    for rule in DEFAULT_DAILY_CATEGORY_RULES {
        let Some(category_id) =
            find_category_id(connection, user_id, rule.main_category, rule.sub_category)?
        else {
            summary.rules_missing_categories += 1;
            continue;
        };
        if auth_rule_name_exists(connection, user_id, rule.name)? {
            summary.rules_skipped += 1;
            continue;
        }
        let changed = connection.execute(
            r#"
            INSERT INTO category_rules (
                user_id, category_id, name, priority, rule_expression,
                regex_enabled, enabled, applied_count, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, 0, ?6, ?6)
            "#,
            params![
                user_id,
                category_id,
                rule.name,
                rule.priority,
                rule.rule_expression,
                created_at,
            ],
        )?;
        if changed > 0 {
            summary.rules_created += 1;
        } else {
            summary.rules_skipped += 1;
        }
    }
    Ok(summary)
}

struct CategoryInsertDraft<'a> {
    user_id: i64,
    type_code: i64,
    main_category: &'a str,
    sub_category: &'a str,
    priority: i64,
    icon: &'a str,
    color: &'a str,
    created_at: &'a str,
}

#[tracing::instrument(level = "debug", skip_all)]
fn insert_category_ignore(
    connection: &Connection,
    draft: &CategoryInsertDraft<'_>,
) -> DbResult<bool> {
    let changed = connection.execute(
        r#"
        INSERT OR IGNORE INTO categories (
            user_id, type, main_category, sub_category, description, priority,
            keywords, hidden, icon, color, created_at
        ) VALUES (?1, ?2, ?3, ?4, '', ?5, '', 0, ?6, ?7, ?8)
        "#,
        params![
            draft.user_id,
            draft.type_code,
            draft.main_category,
            draft.sub_category,
            draft.priority,
            draft.icon,
            draft.color,
            draft.created_at,
        ],
    )?;
    Ok(changed > 0)
}

#[tracing::instrument(level = "debug", skip_all)]
fn find_category_id(
    connection: &Connection,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
) -> DbResult<Option<i64>> {
    connection
        .query_row(
            "SELECT id FROM categories WHERE user_id = ?1 AND main_category = ?2 AND sub_category = ?3",
            params![user_id, main_category, sub_category],
            |row| row.get(0),
        )
        .optional()
        .map_err(DbError::from)
}

fn auth_rule_name_exists(connection: &Connection, user_id: i64, name: &str) -> DbResult<bool> {
    connection
        .query_row(
            "SELECT 1 FROM category_rules WHERE user_id = ?1 AND name = ?2 LIMIT 1",
            params![user_id, name],
            |_| Ok(()),
        )
        .optional()
        .map(|value| value.is_some())
        .map_err(DbError::from)
}
