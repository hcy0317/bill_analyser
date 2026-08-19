use super::*;

/// 判断分类规则 LLM 候选是否会与当前用户的已落库规则或 pending 候选重复。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn has_postgres_llm_rule_candidate_duplicate(
    pool: &PostgresPool,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
    expression: &str,
) -> DbResult<bool> {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    let expression = expression.trim();
    let path = llm_candidate_category_path(main_category, sub_category);
    if sqlx::query(
        "
        SELECT 1
        FROM category_rules cr
        JOIN categories c ON c.id = cr.category_id AND c.user_id = cr.user_id
        WHERE cr.user_id = $1
          AND (
              c.path = $2
              OR (split_part(COALESCE(c.path, ''), '/', 1) = $3
                  AND COALESCE(NULLIF(substring(COALESCE(c.path, '') from position('/' in COALESCE(c.path, '')) + 1), ''), '') = $4)
              OR (c.path IS NULL AND c.name = $3 AND $4 = '')
          )
          AND COALESCE(cr.rule_expression->>'expression', cr.rule_expression->>'rule_expression', '') = $5
        LIMIT 1
        ",
    )
    .bind(user_id)
    .bind(path)
    .bind(main_category)
    .bind(sub_category)
    .bind(expression)
    .fetch_optional(pool)
    .await?
    .is_some()
    {
        return Ok(true);
    }

    sqlx::query(
        "
        SELECT 1
        FROM llm_candidates
        WHERE user_id = $1
          AND status = 'pending'
          AND type IN ('rule_synthesis', 'rule_induction')
          AND suggested_main_category = $2
          AND suggested_sub_category = $3
          AND suggested_rule_expression = $4
        LIMIT 1
        ",
    )
    .bind(user_id)
    .bind(main_category)
    .bind(sub_category)
    .bind(expression)
    .fetch_optional(pool)
    .await
    .map(|row| row.is_some())
    .map_err(Into::into)
}

/// 判断账户规则 LLM 候选是否会与当前用户的已落库规则或 pending 候选重复。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn has_postgres_llm_account_rule_candidate_duplicate(
    pool: &PostgresPool,
    user_id: i64,
    account_id: i64,
    expression: &str,
) -> DbResult<bool> {
    let expression = expression.trim();
    let expression_json = json!({"expression": expression, "regex_enabled": false});
    if sqlx::query(
        "SELECT 1 FROM account_rules WHERE user_id = $1 AND account_id = $2 AND rule_expression = $3 LIMIT 1",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(expression_json)
    .fetch_optional(pool)
    .await?
    .is_some()
    {
        return Ok(true);
    }
    sqlx::query(
        "SELECT 1 FROM llm_candidates WHERE user_id = $1 AND status = 'pending' AND type = 'account_rule_induction' AND suggested_account_id = $2 AND suggested_rule_expression = $3 LIMIT 1",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(expression)
    .fetch_optional(pool)
    .await
    .map(|row| row.is_some())
    .map_err(Into::into)
}

fn llm_candidate_category_path(main_category: &str, sub_category: &str) -> String {
    if sub_category.is_empty() {
        main_category.to_string()
    } else if main_category.is_empty() {
        sub_category.to_string()
    } else {
        format!("{main_category}/{sub_category}")
    }
}

/// 查询用户的 LLM 候选建议列表，支持按状态和候选类型分页过滤。
pub async fn list_postgres_llm_candidates(
    pool: &PostgresPool,
    user_id: i64,
    status: Option<&str>,
    candidate_type: Option<&str>,
    limit: i64,
    offset: i64,
) -> DbResult<Vec<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "list_postgres_llm_candidates",
        "business operation entered"
    );
    let mut builder = QueryBuilder::<Postgres>::new(
        "
        SELECT id, user_id, type, source_bill_ids, suggested_main_category,
               suggested_sub_category, suggested_account_id, suggested_account_name,
               suggested_rule_expression, confidence,
               llm_provider, llm_model, llm_response_raw, status, created_at, reviewed_at
        FROM llm_candidates
        WHERE user_id = ",
    );
    builder.push_bind(user_id);
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status.trim());
    }
    if let Some(candidate_type) = candidate_type {
        builder.push(" AND type = ");
        builder.push_bind(candidate_type.trim());
    }
    builder.push(" ORDER BY created_at DESC LIMIT ");
    builder.push_bind(limit.max(0));
    builder.push(" OFFSET ");
    builder.push_bind(offset.max(0));
    let rows = builder.build().fetch_all(pool).await?;
    rows.into_iter().map(llm_candidate_from_row).collect()
}

/// 统计用户当前筛选条件下的 LLM 候选建议数量，供分页元数据使用。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn count_postgres_llm_candidates(
    pool: &PostgresPool,
    user_id: i64,
    status: Option<&str>,
    candidate_type: Option<&str>,
) -> DbResult<i64> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "SELECT COUNT(*)::BIGINT AS total FROM llm_candidates WHERE user_id = ",
    );
    builder.push_bind(user_id);
    if let Some(status) = status {
        builder.push(" AND status = ");
        builder.push_bind(status.trim());
    }
    if let Some(candidate_type) = candidate_type {
        builder.push(" AND type = ");
        builder.push_bind(candidate_type.trim());
    }
    builder
        .build()
        .fetch_one(pool)
        .await?
        .try_get("total")
        .map_err(Into::into)
}

/// 按候选编号和用户边界读取单条 LLM 候选建议，防止跨用户访问。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn get_postgres_llm_candidate_by_id(
    pool: &PostgresPool,
    candidate_id: i64,
    user_id: i64,
) -> DbResult<Option<Value>> {
    let row = sqlx::query(
        "
        SELECT id, user_id, type, source_bill_ids, suggested_main_category,
               suggested_sub_category, suggested_account_id, suggested_account_name,
               suggested_rule_expression, confidence,
               llm_provider, llm_model, llm_response_raw, status, created_at, reviewed_at
        FROM llm_candidates
        WHERE id = $1 AND user_id = $2
        ",
    )
    .bind(candidate_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    row.map(llm_candidate_from_row).transpose()
}

/// 写入 LLM 生成的候选建议并立即回读，确保返回值与数据库投影格式一致。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn create_postgres_llm_candidate(
    pool: &PostgresPool,
    draft: &LlmCandidateDraft,
) -> DbResult<Value> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "create_postgres_llm_candidate",
        "business operation entered"
    );
    let source_bill_ids = Value::Array(
        draft
            .source_bill_ids
            .iter()
            .copied()
            .map(Value::from)
            .collect(),
    );
    let row = sqlx::query(
        "
        INSERT INTO llm_candidates (
            user_id, type, source_bill_ids, suggested_main_category,
            suggested_sub_category, suggested_account_id, suggested_account_name,
            suggested_rule_expression, confidence,
            llm_provider, llm_model, llm_response_raw, status, created_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, 'pending', now())
        RETURNING id
        ",
    )
    .bind(draft.user_id)
    .bind(draft.candidate_type.trim())
    .bind(source_bill_ids)
    .bind(draft.suggested_main_category.trim())
    .bind(draft.suggested_sub_category.trim())
    .bind(draft.suggested_account_id)
    .bind(draft.suggested_account_name.trim())
    .bind(draft.suggested_rule_expression.trim())
    .bind(draft.confidence)
    .bind(draft.llm_provider.trim())
    .bind(draft.llm_model.trim())
    .bind(&draft.llm_response_raw)
    .fetch_one(pool)
    .await?;
    let candidate_id: i64 = row.try_get("id")?;
    get_postgres_llm_candidate_by_id(pool, candidate_id, draft.user_id)
        .await?
        .ok_or_else(|| {
            DbError::InvalidOperation("created llm candidate could not be reloaded".to_string())
        })
}

/// 更新候选建议审核状态，并写入 reviewed_at 与版本号供审计和同步使用。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn update_postgres_llm_candidate_status(
    pool: &PostgresPool,
    candidate_id: i64,
    status: &str,
    user_id: i64,
) -> DbResult<bool> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "update_postgres_llm_candidate_status",
        "business operation entered"
    );
    let updated = sqlx::query(
        "
        UPDATE llm_candidates
        SET status = $1, reviewed_at = now(), version = version + 1
        WHERE id = $2 AND user_id = $3
        ",
    )
    .bind(status.trim())
    .bind(candidate_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(updated.rows_affected() > 0)
}

/// 接受 LLM 候选建议；规则类候选会同步物化为导入规则并返回新规则编号。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn accept_postgres_llm_candidate(
    pool: &PostgresPool,
    candidate_id: i64,
    user_id: i64,
) -> DbResult<Option<Value>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "accept_postgres_llm_candidate",
        "business operation entered"
    );
    let Some(candidate) = get_postgres_llm_candidate_by_id(pool, candidate_id, user_id).await?
    else {
        return Ok(None);
    };
    update_postgres_llm_candidate_status(pool, candidate_id, "accepted", user_id).await?;

    let mut result = Map::new();
    result.insert("candidate_id".to_string(), json!(candidate_id));
    result.insert("status".to_string(), json!("accepted"));
    if should_materialize_rule_candidate(&candidate) {
        if let Some(rule_id) =
            create_postgres_rule_for_llm_candidate(pool, &candidate, user_id).await?
        {
            result.insert("created_rule_id".to_string(), json!(rule_id));
        }
    } else if should_materialize_account_rule_candidate(&candidate) {
        if let Some(rule_id) =
            create_postgres_account_rule_for_llm_candidate(pool, &candidate, user_id).await?
        {
            result.insert("created_account_rule_id".to_string(), json!(rule_id));
        }
    }
    Ok(Some(Value::Object(result)))
}

/// 拒绝 LLM 候选建议；候选不存在时返回 None 保持 API 的未找到语义。
#[tracing::instrument(level = "debug", skip_all)]
pub async fn reject_postgres_llm_candidate(
    pool: &PostgresPool,
    candidate_id: i64,
    user_id: i64,
) -> DbResult<Option<bool>> {
    #[cfg(not(coverage))]
    tracing::info!(
        domain = "ai",
        operation = "reject_postgres_llm_candidate",
        "business operation entered"
    );
    if get_postgres_llm_candidate_by_id(pool, candidate_id, user_id)
        .await?
        .is_none()
    {
        return Ok(None);
    }
    update_postgres_llm_candidate_status(pool, candidate_id, "rejected", user_id)
        .await
        .map(Some)
}
