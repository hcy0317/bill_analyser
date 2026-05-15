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

fn load_persisted_bill_prompt_values(
    connection: &Connection,
    user_id: i64,
    bill_ids: Option<&[i64]>,
    limit: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
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
        let placeholders = std::iter::repeat_n("?", bill_ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let mut values = vec![rusqlite::types::Value::Integer(user_id)];
        values.extend(
            bill_ids
                .iter()
                .copied()
                .map(rusqlite::types::Value::Integer),
        );
        let sql = format!(
            "SELECT id, date, amount, counterparty, description, payment_method FROM bills \
             WHERE user_id = ? AND id IN ({placeholders}) ORDER BY date DESC, id DESC"
        );
        return query_bill_prompt_values(connection, &sql, values, limit);
    }
    query_bill_prompt_values(
        connection,
        "SELECT id, date, amount, counterparty, description, payment_method FROM bills \
         WHERE user_id = ?1 AND (main_category IS NULL OR TRIM(main_category) = '' OR main_category = '未分类') \
         ORDER BY date DESC, id DESC LIMIT ?2",
        vec![
            rusqlite::types::Value::Integer(user_id),
            rusqlite::types::Value::Integer(usize_to_i64(limit)),
        ],
        limit,
    )
}

fn query_bill_prompt_values(
    connection: &Connection,
    sql: &str,
    values: Vec<rusqlite::types::Value>,
    limit: usize,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    let mut statement = connection.prepare(sql).map_err(db_error_response)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values), |row| {
            Ok(json!({
                "id": row.get::<_, i64>(0)?,
                "date": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                "amount": row.get::<_, Option<f64>>(2)?.unwrap_or_default(),
                "counterparty": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                "description": row.get::<_, Option<String>>(4)?.unwrap_or_default(),
                "payment_method": row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            }))
        })
        .map_err(db_error_response)?;
    let mut items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)?;
    items.truncate(limit);
    Ok(items)
}

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
    let recent_feedback = get_llm_memory_events(connection, user_id, None, Some("feedback"), 20, 0)
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

fn load_rule_synthesis_active_model(
    connection: &Connection,
    user_id: i64,
) -> Result<Value, ImportV2RouteResponse> {
    if !table_exists(connection, "import_learning_model_registry")? {
        return Ok(Value::Null);
    }
    connection
        .query_row(
            "
            SELECT model_version, dataset_snapshot_id, metrics_json, updated_at
            FROM import_learning_model_registry
            WHERE user_id = ?1 AND model_key = ?2 AND status = 'active'
            ORDER BY updated_at DESC, id DESC
            LIMIT 1
            ",
            params![user_id, IMPORT_LEARNING_MODEL_KEY],
            |row| {
                let metrics_text = row.get::<_, Option<String>>(2)?.unwrap_or_default();
                let metrics = serde_json::from_str::<Value>(&metrics_text)
                    .ok()
                    .and_then(|value| value.as_object().cloned().map(Value::Object))
                    .unwrap_or_else(|| json!({}));
                Ok(json!({
                    "model_version": row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    "dataset_snapshot_id": row.get::<_, Option<i64>>(1)?.unwrap_or_default(),
                    "feature_schema_version": metrics.get("feature_schema_version").and_then(Value::as_str).unwrap_or_default(),
                    "policy_version": metrics.get("policy_version").and_then(Value::as_str).unwrap_or_default(),
                    "sample_count": metrics.get("sample_count").and_then(value_to_i64).unwrap_or_default(),
                    "updated_at": row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                }))
            },
        )
        .optional()
        .map(|value| value.unwrap_or(Value::Null))
        .map_err(db_error_response)
}

fn load_learning_concept_stats(
    connection: &Connection,
    user_id: i64,
) -> Result<Vec<Value>, ImportV2RouteResponse> {
    if !table_exists(connection, "import_learning_concept_stats")? {
        return Ok(Vec::new());
    }
    let mut statement = connection
        .prepare(
            "SELECT concept_key, concept_type, sample_count, accepted_count, rejected_count, \
                    auto_applied_count, rollback_count, updated_at \
             FROM import_learning_concept_stats WHERE user_id = ?1 ORDER BY updated_at DESC LIMIT 50",
        )
        .map_err(db_error_response)?;
    let rows = statement
        .query_map(params![user_id], |row| {
            Ok(json!({
                "concept_key": row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                "concept_type": row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                "sample_count": row.get::<_, Option<i64>>(2)?.unwrap_or_default(),
                "accepted_count": row.get::<_, Option<i64>>(3)?.unwrap_or_default(),
                "rejected_count": row.get::<_, Option<i64>>(4)?.unwrap_or_default(),
                "auto_applied_count": row.get::<_, Option<i64>>(5)?.unwrap_or_default(),
                "rollback_count": row.get::<_, Option<i64>>(6)?.unwrap_or_default(),
                "updated_at": row.get::<_, Option<String>>(7)?.unwrap_or_default(),
            }))
        })
        .map_err(db_error_response)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(db_error_response)
}

fn table_exists(connection: &Connection, table_name: &str) -> Result<bool, ImportV2RouteResponse> {
    connection
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
        .map_err(db_error_response)
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

fn rule_candidate_duplicate(
    connection: &Connection,
    user_id: i64,
    main_category: &str,
    sub_category: &str,
    expression: &str,
) -> Result<bool, ImportV2RouteResponse> {
    let expression = expression.trim();
    let category_rule_duplicate =
        if table_exists(connection, "category_rules")? && table_exists(connection, "categories")? {
            connection
                .query_row(
                    "
                SELECT 1 FROM category_rules cr
                JOIN categories c ON c.id = cr.category_id AND c.user_id = cr.user_id
                WHERE cr.user_id = ?1 AND c.main_category = ?2 AND c.sub_category = ?3
                  AND cr.rule_expression = ?4
                LIMIT 1
                ",
                    params![user_id, main_category, sub_category, expression],
                    |_| Ok(true),
                )
                .optional()
                .map_err(db_error_response)?
                .unwrap_or(false)
        } else {
            false
        };
    if category_rule_duplicate {
        return Ok(true);
    }
    connection
        .query_row(
            "
            SELECT 1 FROM llm_candidates
            WHERE user_id = ?1 AND status = 'pending' AND type IN ('rule_synthesis', 'rule_induction')
              AND suggested_main_category = ?2 AND suggested_sub_category = ?3
              AND suggested_rule_expression = ?4
            LIMIT 1
            ",
            params![user_id, main_category, sub_category, expression],
            |_| Ok(true),
        )
        .optional()
        .map(|value| value.unwrap_or(false))
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

