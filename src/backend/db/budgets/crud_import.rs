// 中文导读：SQLite repository 层，负责 schema、事务、user-scope 查询、row helper 和跨表写入边界。
// 维护重点：SQL 与数据行映射集中在本层，HTTP handler 不应复制查询逻辑或绕过事务 helper。
// 不变式：业务写入默认 rollback-on-error，审计与兼容缓存只有在注释明确时才能作为 best-effort。


pub fn query_budgets_for_listing(
    connection: &Connection,
    user_id: UserId,
    filters: &BudgetFilters,
) -> DbResult<Vec<BudgetRecord>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let mut budgets = query_budgets_raw(connection, user_id, filters)?;
    enrich_budget_listing(connection, user_id, filters.budget_type, &mut budgets)
}

pub fn get_budget_by_id(
    connection: &Connection,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<Option<BudgetRecord>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    get_budget_by_id_on_connection(connection, user_id, budget_id)
}

pub fn create_budget(
    connection: &mut Connection,
    user_id: UserId,
    draft: &BudgetCreateDraft,
) -> DbResult<i64> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let now = now_text();
        let mut payload = normalize_create_payload(&draft.fields, &now)?;
        let budget_id = insert_budget_on_tx(tx, user_id, &mut payload)?;
        let effective_budget = get_budget_by_id_on_tx(tx, user_id, budget_id)?
            .ok_or_else(|| DbError::InvalidOperation("created budget not found".to_string()))?;
        synchronize_primary_budget_for_group(
            tx,
            build_budget_group_key(&effective_budget, user_id),
            Some(&effective_budget),
        )?;
        synchronize_budget_period_hierarchy(tx, &effective_budget, user_id)?;
        Ok(budget_id)
    })
}

pub fn update_budget(
    connection: &mut Connection,
    user_id: UserId,
    budget_id: i64,
    draft: &BudgetUpdateDraft,
) -> DbResult<bool> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(existing_budget) = get_budget_by_id_on_tx(tx, user_id, budget_id)? else {
            return Ok(false);
        };
        let mut payload = normalize_update_payload(&existing_budget, &draft.fields)?;
        if payload.is_empty() {
            return Ok(false);
        }
        let (assignments, values) = update_payload_to_sql(&mut payload)?;
        let mut values = values;
        values.push(SqlValue::Integer(budget_id));
        values.push(SqlValue::Integer(user_id));
        let updated = tx.execute(
            &format!(
                "UPDATE budgets SET {} WHERE id = ? AND user_id = ?",
                assignments.join(", ")
            ),
            params_from_iter(values),
        )?;
        if updated == 0 {
            return Ok(false);
        }
        let updated_budget = get_budget_by_id_on_tx(tx, user_id, budget_id)?
            .ok_or_else(|| DbError::InvalidOperation("updated budget not found".to_string()))?;
        for group_key in
            collect_budget_sync_group_keys(user_id, [&existing_budget, &updated_budget])
        {
            synchronize_primary_budget_for_group(tx, Some(group_key), Some(&updated_budget))?;
        }
        synchronize_budget_period_hierarchy(tx, &existing_budget, user_id)?;
        synchronize_budget_period_hierarchy(tx, &updated_budget, user_id)?;
        Ok(true)
    })
}

pub fn delete_budget(
    connection: &mut Connection,
    user_id: UserId,
    budget_id: i64,
) -> DbResult<bool> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let Some(budget) = get_budget_by_id_on_tx(tx, user_id, budget_id)? else {
            return Ok(false);
        };
        let category = record_text(&budget, "category");
        let period_type = record_text(&budget, "period_type");
        let start_date = record_text(&budget, "start_date");
        let sub_category = normalize_sub_category(record_value(&budget, "sub_category"));
        let affected_budgets = if sub_category.is_empty() {
            get_budgets_for_sync_group_on_tx(tx, user_id, &category, &period_type, &start_date)?
        } else {
            vec![budget.clone()]
        };

        let deleted = if sub_category.is_empty() {
            tx.execute(
                "
                DELETE FROM budgets
                WHERE category = ?1
                  AND period_type = ?2
                  AND start_date = ?3
                  AND user_id = ?4
                ",
                params![category, period_type, start_date, user_id],
            )?
        } else {
            let deleted = tx.execute(
                "DELETE FROM budgets WHERE id = ?1 AND user_id = ?2",
                params![budget_id, user_id],
            )?;
            synchronize_primary_budget_for_group(
                tx,
                build_budget_group_key(&budget, user_id),
                Some(&budget),
            )?;
            deleted
        };
        for affected_budget in &affected_budgets {
            synchronize_budget_period_hierarchy(tx, affected_budget, user_id)?;
        }
        Ok(deleted > 0)
    })
}

pub fn export_budgets(connection: &Connection, user_id: UserId) -> DbResult<Vec<Value>> {
    let user_id = UserScope::new(user_id).bind_value()?;
    let budgets = query_budgets_raw(connection, user_id, &BudgetFilters::default())?;
    Ok(budgets
        .iter()
        .map(|row| {
            json!({
                "name": field_or_null(row, "name"),
                "category": field_or_null(row, "category"),
                "sub_category": field_or_null(row, "sub_category"),
                "period_type": field_or_null(row, "period_type"),
                "amount": field_or_null(row, "amount"),
                "start_date": field_or_null(row, "start_date"),
                "end_date": field_or_null(row, "end_date"),
                "alert_threshold": field_or_null(row, "alert_threshold"),
                "enabled": field_or_null(row, "enabled"),
            })
        })
        .collect())
}

pub fn import_budgets(
    connection: &mut Connection,
    user_id: UserId,
    budgets: &[BudgetRecord],
) -> DbResult<Value> {
    let user_id = UserScope::new(user_id).bind_value()?;
    run_transaction(connection, |tx| {
        let now = now_text();
        let mut created_count = 0_i64;
        let mut updated_count = 0_i64;
        let mut error_count = 0_i64;
        let mut errors = Vec::new();

        for (index, budget) in budgets.iter().enumerate() {
            let row_number = index + 1;
            if !budget_import_has_required_name_and_amount(budget) {
                errors.push(format!("第{row_number}条: 缺少必填字段(name或amount)"));
                error_count += 1;
                continue;
            }
            match import_budget_row_on_tx(tx, user_id, budget, &now) {
                Ok(ImportBudgetRowAction::Created) => created_count += 1,
                Ok(ImportBudgetRowAction::Updated) => updated_count += 1,
                Err(error) => {
                    errors.push(format!("第{row_number}条: {error}"));
                    error_count += 1;
                }
            }
        }

        Ok(json!({
            "created": created_count,
            "updated": updated_count,
            "errors": error_count,
            "error_details": errors
        }))
    })
}
