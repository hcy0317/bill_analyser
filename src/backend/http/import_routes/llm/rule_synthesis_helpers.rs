// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn rule_induction_groups(
    rows: Vec<ImportPreviewRow>,
) -> Result<Vec<RuleInductionGroup>, ImportV2RouteResponse> {
    let mut groups: BTreeMap<(String, String), Vec<ImportPreviewRow>> = BTreeMap::new();
    for row in rows {
        if row.preview_main_category.trim().is_empty() && row.preview_sub_category.trim().is_empty()
        {
            continue;
        }
        if row.preview_counterparty.trim().is_empty()
            && row.preview_description.trim().is_empty()
            && row.preview_payment_method.trim().is_empty()
        {
            continue;
        }
        groups
            .entry((
                row.preview_main_category.trim().to_string(),
                row.preview_sub_category.trim().to_string(),
            ))
            .or_default()
            .push(row);
    }
    if groups.is_empty() {
        return Err(llm_contract_error_response(
            "Selected preview rows do not contain enough categorized evidence",
            "PREVIEW_SELECTION_INSUFFICIENT",
            422,
        ));
    }
    if groups.len() > 10 {
        return Err(llm_contract_error_response(
            "Selected preview rows contain too many category groups",
            "PREVIEW_SELECTION_TOO_LARGE",
            422,
        ));
    }
    Ok(groups
        .into_iter()
        .map(|((main_category, sub_category), rows)| {
            let category_name = category_path(&main_category, &sub_category);
            RuleInductionGroup {
                category_name,
                main_category,
                sub_category,
                source_ids: rows.iter().map(|row| row.id).collect(),
                transactions: rows.iter().map(preview_row_prompt_value).collect(),
            }
        })
        .collect())
}

#[tracing::instrument(level = "debug", skip_all)]
async fn load_postgres_persisted_bill_prompt_values(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    bill_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let mut builder = QueryBuilder::<Postgres>::new(
        "
        SELECT b.id,
               b.occurred_at,
               b.amount_cents,
               b.merchant,
               b.description,
               b.payment_method
        FROM bills b
        LEFT JOIN categories c ON c.user_id = b.user_id AND c.id = b.category_id
        WHERE b.user_id = ",
    );
    builder.push_bind(user_id);
    builder.push(" AND b.is_deleted = false");

    if let Some(bill_ids) = bill_ids {
        let mut seen_ids = BTreeSet::new();
        let bill_ids = bill_ids
            .iter()
            .copied()
            .filter(|value| *value > 0)
            .filter(|value| seen_ids.insert(*value))
            .take(limit)
            .collect::<Vec<_>>();
        if bill_ids.is_empty() {
            return Ok(Vec::new());
        }
        builder.push(" AND b.id IN (");
        let mut separated = builder.separated(", ");
        for bill_id in bill_ids {
            separated.push_bind(bill_id);
        }
        separated.push_unseparated(")");
    } else {
        builder.push(
            " AND (
                b.category_id IS NULL
                OR COALESCE(NULLIF(c.path, ''), c.name, '') = ''
                OR COALESCE(c.path, c.name, '') = '未分类'
            )",
        );
    }

    builder.push(" ORDER BY b.occurred_at DESC, b.id DESC LIMIT ");
    builder.push_bind(i64::try_from(limit).unwrap_or(i64::MAX));
    let rows = builder.build().fetch_all(pool).await.map_err(db_error_response)?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let occurred_at = row
                .try_get::<chrono::DateTime<Utc>, _>("occurred_at")
                .map(|value| value.to_rfc3339())
                .unwrap_or_default();
            let amount_cents = row.try_get::<i64, _>("amount_cents").unwrap_or_default();
            json!({
                "id": row.try_get::<i64, _>("id").unwrap_or_default(),
                "date": occurred_at,
                "amount": amount_cents as f64 / 100.0,
                "counterparty": row.try_get::<Option<String>, _>("merchant").ok().flatten().unwrap_or_default(),
                "description": row.try_get::<Option<String>, _>("description").ok().flatten().unwrap_or_default(),
                "payment_method": row.try_get::<Option<String>, _>("payment_method").ok().flatten().unwrap_or_default(),
            })
        })
        .collect())
}

#[tracing::instrument(level = "debug", skip_all)]
fn build_rule_synthesis_knowledge_pack(
    connection: &Connection,
    user_id: UserId,
    categories: &[Value],
) -> Result<Value, ImportV2RouteResponse> {
    let rules = load_import_learning_rules(connection, user_id, Some(true), 50, 0)?;
    let suggestions = load_learning_suggestions(connection, user_id, None, 50, 0)?
        .into_iter()
        .filter(|suggestion| {
            suggestion
                .get("status")
                .and_then(Value::as_str)
                .is_none_or(|status| status != "rejected")
        })
        .collect::<Vec<_>>();
    let concept_stats = load_learning_concept_stats(connection, user_id_i64_value(user_id)?)?;
    let concept_stats_by_key = concept_stats
        .iter()
        .filter_map(|stats| {
            stats
                .get("concept_key")
                .and_then(Value::as_str)
                .map(|key| (key.to_string(), stats.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    let category_paths_by_id = categories
        .iter()
        .filter_map(|category| {
            Some((
                category.get("id").and_then(value_to_i64)?,
                category.get("path").and_then(Value::as_str)?.to_string(),
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let mut existing_category_paths = category_paths_by_id.values().cloned().collect::<Vec<_>>();
    existing_category_paths.sort();
    let mut category_bundles: BTreeMap<String, RuleSynthesisCategoryBundle> = BTreeMap::new();
    for rule in &rules {
        let Some(category_id) = rule.get("learnedCategoryId").and_then(value_to_i64) else {
            continue;
        };
        let Some(category_path) = category_paths_by_id.get(&category_id) else {
            continue;
        };
        let learned_type = rule
            .get("learnedType")
            .and_then(value_to_text)
            .unwrap_or_default();
        let concept_key = format!(
            "rule:{}",
            rule.get("id").and_then(value_to_i64).unwrap_or_default()
        );
        let feedback = concept_stats_by_key
            .get(&concept_key)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let bundle =
            ensure_rule_synthesis_bundle(&mut category_bundles, category_path, &learned_type);
        bundle.push_evidence(json!({
            "source": "learning_rule",
            "source_id": concept_key,
            "match_type": rule.get("matchType").and_then(value_to_text).unwrap_or_default(),
            "match_value": rule.get("matchValue").and_then(value_to_text).unwrap_or_default(),
            "match_features": rule.get("matchFeatures").cloned().unwrap_or_else(|| json!({})),
            "sample_count": rule.get("appliedCount").and_then(value_to_i64).unwrap_or_default(),
            "enabled": rule.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            "feedback": feedback,
        }));
    }
    for suggestion in &suggestions {
        let Some(category_id) = suggestion
            .get("suggested_category_id")
            .and_then(value_to_i64)
        else {
            continue;
        };
        let Some(category_path) = category_paths_by_id.get(&category_id) else {
            continue;
        };
        let learned_type = suggestion
            .get("suggested_type")
            .and_then(value_to_text)
            .unwrap_or_default();
        let concept_key = format!(
            "suggestion:{}",
            suggestion
                .get("id")
                .and_then(value_to_i64)
                .unwrap_or_default()
        );
        let feedback = concept_stats_by_key
            .get(&concept_key)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let bundle =
            ensure_rule_synthesis_bundle(&mut category_bundles, category_path, &learned_type);
        bundle.push_evidence(json!({
            "source": "learning_suggestion",
            "source_id": concept_key,
            "match_type": suggestion.get("match_type").and_then(value_to_text).unwrap_or_default(),
            "match_value": suggestion.get("match_value").and_then(value_to_text).unwrap_or_default(),
            "match_features": json_object_from_text_field(suggestion, "match_features_json"),
            "sample_count": suggestion.get("sample_count").and_then(value_to_i64).unwrap_or_default(),
            "status": suggestion.get("status").and_then(value_to_text).unwrap_or_default(),
            "feedback": feedback,
        }));
    }
    let mut ranked_categories = category_bundles.into_values().collect::<Vec<_>>();
    ranked_categories.sort_by_key(|category| std::cmp::Reverse(category.rank()));
    let category_bundles = ranked_categories
        .into_iter()
        .take(LLM_RULE_SYNTHESIS_MAX_GROUPS)
        .map(RuleSynthesisCategoryBundle::into_value)
        .collect::<Vec<_>>();
    let recent_feedback = get_llm_memory_events(connection, user_id, None, 20)
        .map_err(db_error_response)?
        .into_iter()
        .map(|event| {
            json!({
                "decision": event.decision.unwrap_or_default(),
                "suggested_main_category": event.suggested_main_category.unwrap_or_default(),
                "suggested_sub_category": event.suggested_sub_category.unwrap_or_default(),
                "confidence": event.confidence,
                "created_at": event.created_at,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "knowledge_summary_version": "a6-rule-synthesis-v1",
        "existing_categories": existing_category_paths,
        "durable_learning_rules": rules,
        "learning_suggestions": suggestions,
        "concept_stats": concept_stats,
        "active_model": load_rule_synthesis_active_model(connection, user_id_i64_value(user_id)?)?,
        "recent_llm_feedback": recent_feedback,
        "categories": category_bundles,
    }))
}

#[derive(Debug, Clone)]
struct RuleSynthesisCategoryBundle {
    category_name: String,
    learned_type: String,
    evidence: Vec<Value>,
    accepted_count: i64,
    rejected_count: i64,
    auto_applied_count: i64,
    rollback_count: i64,
}

impl RuleSynthesisCategoryBundle {
    fn new(category_name: &str, learned_type: &str) -> Self {
        Self {
            category_name: category_name.to_string(),
            learned_type: learned_type.to_string(),
            evidence: Vec::new(),
            accepted_count: 0,
            rejected_count: 0,
            auto_applied_count: 0,
            rollback_count: 0,
        }
    }

    fn push_evidence(&mut self, evidence: Value) {
        if let Some(learned_type) = evidence
            .get("learned_type")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            if self.learned_type.is_empty() {
                self.learned_type = learned_type.to_string();
            }
        }
        let feedback = evidence.get("feedback").unwrap_or(&Value::Null);
        self.accepted_count += rule_synthesis_feedback_count(feedback, "accepted_count");
        self.rejected_count += rule_synthesis_feedback_count(feedback, "rejected_count");
        self.auto_applied_count += rule_synthesis_feedback_count(feedback, "auto_applied_count");
        self.rollback_count += rule_synthesis_feedback_count(feedback, "rollback_count");
        self.evidence.push(evidence);
    }

    fn rank(&self) -> (i64, usize, String) {
        (
            self.accepted_count + self.auto_applied_count.saturating_mul(2),
            self.evidence.len(),
            self.category_name.clone(),
        )
    }

    fn into_value(mut self) -> Value {
        self.evidence.sort_by(|left, right| {
            rule_synthesis_evidence_rank(right).cmp(&rule_synthesis_evidence_rank(left))
        });
        self.evidence
            .truncate(LLM_RULE_SYNTHESIS_MAX_EVIDENCE_PER_GROUP);
        json!({
            "category_name": self.category_name,
            "learned_type": self.learned_type,
            "evidence": self.evidence,
            "feedback_summary": {
                "accepted_count": self.accepted_count,
                "rejected_count": self.rejected_count,
                "auto_applied_count": self.auto_applied_count,
                "rollback_count": self.rollback_count,
            },
        })
    }
}

#[tracing::instrument(level = "debug", skip_all)]
fn ensure_rule_synthesis_bundle<'a>(
    bundles: &'a mut BTreeMap<String, RuleSynthesisCategoryBundle>,
    category_name: &str,
    learned_type: &str,
) -> &'a mut RuleSynthesisCategoryBundle {
    let bundle = bundles
        .entry(category_name.to_string())
        .or_insert_with(|| RuleSynthesisCategoryBundle::new(category_name, learned_type));
    if bundle.learned_type.is_empty() && !learned_type.trim().is_empty() {
        bundle.learned_type = learned_type.to_string();
    }
    bundle
}

fn rule_synthesis_evidence_rank(evidence: &Value) -> (i64, i64, String) {
    let feedback = evidence.get("feedback").unwrap_or(&Value::Null);
    (
        rule_synthesis_feedback_count(feedback, "accepted_count")
            + rule_synthesis_feedback_count(feedback, "auto_applied_count").saturating_mul(2),
        evidence
            .get("sample_count")
            .and_then(value_to_i64)
            .unwrap_or_default(),
        evidence
            .get("match_value")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    )
}

fn rule_synthesis_feedback_count(feedback: &Value, key: &str) -> i64 {
    feedback.get(key).and_then(value_to_i64).unwrap_or_default()
}

fn json_object_from_text_field(value: &Value, key: &str) -> Value {
    value
        .get(key)
        .and_then(Value::as_str)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| value.as_object().cloned().map(Value::Object))
        .unwrap_or_else(|| json!({}))
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_rule_synthesis_active_model(
    _connection: &Connection,
    _user_id: i64,
) -> Result<Value, ImportV2RouteResponse> {
    Ok(Value::Null)
}

#[tracing::instrument(level = "debug", skip_all)]
fn load_learning_concept_stats(
    _connection: &Connection,
    _user_id: i64,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    Ok(Vec::new())
}

fn llm_advanced_prompt_template(config: &Value, key: &str) -> String {
    copy_runtime_llm_config(config)
        .get("advanced_settings")
        .and_then(Value::as_object)
        .and_then(|object| object.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn preview_id_from_llm_value(value: &Value) -> i64 {
    value
        .as_object()
        .and_then(|object| first_value(object, &["preview_id", "previewId", "id"]))
        .and_then(value_to_i64)
        .unwrap_or_default()
}

fn main_category_from_llm_value(value: &Value) -> String {
    value
        .as_object()
        .and_then(|object| {
            first_value(
                object,
                &[
                    "suggested_main_category",
                    "suggestedMainCategory",
                    "main_category",
                    "mainCategory",
                ],
            )
        })
        .and_then(value_to_text)
        .unwrap_or_default()
}

fn sub_category_from_llm_value(value: &Value) -> String {
    value
        .as_object()
        .and_then(|object| {
            first_value(
                object,
                &[
                    "suggested_sub_category",
                    "suggestedSubCategory",
                    "sub_category",
                    "subCategory",
                ],
            )
        })
        .and_then(value_to_text)
        .unwrap_or_default()
}

fn rule_expression_from_llm_value(value: &Value) -> String {
    value
        .as_object()
        .and_then(|object| {
            first_value(
                object,
                &[
                    "rule_expression",
                    "ruleExpression",
                    "suggested_rule_expression",
                    "suggestedRuleExpression",
                ],
            )
        })
        .and_then(value_to_text)
        .map(|expression| expression.trim().to_string())
        .unwrap_or_default()
}

fn confidence_from_value(value: &Value) -> f64 {
    value
        .as_object()
        .and_then(|object| first_value(object, &["confidence"]))
        .and_then(value_to_f64)
        .unwrap_or_default()
        .clamp(0.0, 1.0)
}

fn llm_candidate_raw_response(value: &Value) -> String {
    let text = value.to_string();
    if text.len() <= LLM_CANDIDATE_RAW_RESPONSE_MAX_BYTES {
        return text;
    }
    let truncated = text
        .char_indices()
        .take_while(|(index, _)| *index < LLM_CANDIDATE_RAW_RESPONSE_MAX_BYTES)
        .map(|(_, ch)| ch)
        .collect::<String>();
    json!({
        "truncated": true,
        "payload": truncated,
    })
    .to_string()
}

fn valid_rule_expression(expression: &str) -> bool {
    !bill_analyser_core::category_rules::compile_rule_expression(expression.trim(), false).is_empty
}

#[tracing::instrument(level = "debug", skip_all)]
async fn postgres_rule_candidate_duplicate(
    pool: &bill_analyser_db::PostgresPool,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
    expression: &str,
) -> Result<bool, ImportV2RouteResponse> {
    let expression = expression.trim();
    let path = category_path(main_category, sub_category);
    let category_rule_duplicate = sqlx::query(
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
    .bind(main_category.trim())
    .bind(sub_category.trim())
    .bind(expression)
    .fetch_optional(pool)
    .await
    .map_err(db_error_response)?
    .is_some();
    if category_rule_duplicate {
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
    .bind(main_category.trim())
    .bind(sub_category.trim())
    .bind(expression)
    .fetch_optional(pool)
    .await
    .map(|row| row.is_some())
    .map_err(db_error_response)
}

fn category_path(main_category: &str, sub_category: &str) -> String {
    let main_category = main_category.trim();
    let sub_category = sub_category.trim();
    if sub_category.is_empty() {
        main_category.to_string()
    } else if main_category.is_empty() {
        sub_category.to_string()
    } else {
        format!("{main_category}/{sub_category}")
    }
}
