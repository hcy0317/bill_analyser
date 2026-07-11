/// 中文说明：为导入预览行补齐标准 matching payload，供前端统一读取 transfer、learning、dedup 等信号。
#[tracing::instrument(level = "debug", skip_all)]
pub fn attach_import_preview_matching_payload(preview_item: &mut Map<String, Value>) {
    let matching = build_import_preview_matching_payload(preview_item);
    preview_item.insert("matching".to_string(), matching);
}

/// 中文说明：从新旧预览字段合成标准 matching payload，保留已记录反馈并补齐 parser、dedup、recurring 和 annotation 默认值。
#[tracing::instrument(level = "debug", skip_all)]
pub fn build_import_preview_matching_payload(preview_item: &Map<String, Value>) -> Value {
    #[cfg(not(coverage))]
    tracing::debug!(
        domain = "import_parser",
        operation = "build_import_preview_matching_payload",
        "business operation entered"
    );
    let mut payload =
        serde_json::to_value(ImportPreviewMatchingPayload::default()).unwrap_or_else(|_| json!({}));
    let Some(payload_object) = payload.as_object_mut() else {
        return json!({});
    };

    if let Some(feedback) = preview_matching_payload(preview_item) {
        for section in [
            "transfer",
            "investment",
            "learning",
            "llm",
            "recurring",
            "dedup",
            "parser",
            "annotation",
            "reconciliation",
            "identity_validation",
            "stage2_baseline",
        ] {
            if let Some(feedback_section) = object_field_from_map(feedback, section) {
                merge_object_fields(
                    section_object_mut(payload_object, section),
                    feedback_section,
                );
            }
        }
    }

    populate_parser_matching_section(payload_object, preview_item);
    populate_dedup_matching_section(payload_object, preview_item);
    populate_recurring_matching_section(payload_object, preview_item);
    populate_annotation_matching_section(payload_object, preview_item);
    populate_history_rewrite_confirmation_section(payload_object, preview_item);

    payload
}

fn preview_matching_payload(preview_item: &Map<String, Value>) -> Option<&Map<String, Value>> {
    object_field_from_map(preview_item, "matching")
        .or_else(|| object_field_from_map(preview_item, "preview_matching_feedback"))
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：把旧 matching 子对象字段覆盖到标准 payload section，保留用户反馈优先级。
fn merge_object_fields(target: &mut Map<String, Value>, source: &Map<String, Value>) {
    for (key, value) in source {
        target.insert(key.clone(), value.clone());
    }
}

fn section_object_mut<'a>(
    payload_object: &'a mut Map<String, Value>,
    section: &str,
) -> &'a mut Map<String, Value> {
    if !payload_object
        .get(section)
        .is_some_and(|value| value.is_object())
    {
        payload_object.insert(section.to_string(), json!({}));
    }

    payload_object
        .get_mut(section)
        .and_then(Value::as_object_mut)
        .expect("matching section object")
}

fn populate_parser_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    // 中文说明：同步 parser id/tags 的新旧字段名，保证前端和行为锁测试看到同一来源链。
    let parser = section_object_mut(payload_object, "parser");
    let parser_id = first_non_empty_string_field(
        parser,
        ["id", "parser_id"].into_iter().chain(["preview_parser_id"]),
        preview_item,
    );
    let parser_tags = first_non_empty_value_list(
        [
            parser.get("tags"),
            parser.get("parser_tags"),
            preview_item.get("preview_parser_tags"),
        ]
        .into_iter()
        .flatten(),
    );

    parser.insert("id".to_string(), Value::String(parser_id.clone()));
    parser.insert("parser_id".to_string(), Value::String(parser_id));
    parser.insert("tags".to_string(), Value::Array(parser_tags.clone()));
    parser.insert("parser_tags".to_string(), Value::Array(parser_tags));
}

fn populate_dedup_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    // 中文说明：把去重类型和来源 id 归一进 dedup section，兼容字符串和数组两种历史来源 id。
    let dedup = section_object_mut(payload_object, "dedup");
    let dedup_type = get_first_non_empty_string([
        string_field_from_map(dedup, "type"),
        string_field_from_map(preview_item, "dedup_type"),
    ]);
    let source_ids = first_non_empty_value_list(
        [
            dedup.get("source_ids"),
            preview_item.get("dedup_source_ids"),
        ]
        .into_iter()
        .flatten(),
    );

    dedup.insert("type".to_string(), Value::String(dedup_type));
    dedup.insert("source_ids".to_string(), Value::Array(source_ids.clone()));
    if integer_field_from_map(dedup, "source_count") <= 0 {
        dedup.insert(
            "source_count".to_string(),
            Value::Number(Number::from(source_ids.len() as i64)),
        );
    }
}

fn populate_recurring_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    // 中文说明：补齐周期候选 section 的 id、名称、数量、得分和原因，避免旧预览字段丢失候选提示。
    let recurring = section_object_mut(payload_object, "recurring");
    if recurring
        .get("id")
        .map(|value| normalize_id_text(Some(value)))
        .unwrap_or_default()
        .is_empty()
    {
        recurring.insert(
            "id".to_string(),
            preview_item
                .get("preview_recurring_id")
                .cloned()
                .unwrap_or(Value::Null),
        );
    }

    let recurring_name = get_first_non_empty_string([
        string_field_from_map(recurring, "name"),
        string_field_from_map(preview_item, "preview_recurring_name"),
    ]);
    recurring.insert("name".to_string(), Value::String(recurring_name));

    if integer_field_from_map(recurring, "candidate_count") <= 0 {
        recurring.insert(
            "candidate_count".to_string(),
            Value::Number(Number::from(integer_field_from_map(
                preview_item,
                "preview_recurring_candidate_count",
            ))),
        );
    }
    if float_field_from_map(recurring, "match_score") <= 0.0 {
        recurring.insert(
            "match_score".to_string(),
            Number::from_f64(float_field_from_map(
                preview_item,
                "preview_recurring_match_score",
            ))
            .map(Value::Number)
            .unwrap_or(Value::Number(Number::from(0))),
        );
    }

    let match_reasons = get_first_non_empty_string([
        string_field_from_map(recurring, "match_reasons"),
        string_field_from_map(preview_item, "preview_recurring_match_reasons"),
    ]);
    let matched_date = get_first_non_empty_string([
        string_field_from_map(recurring, "matched_date"),
        string_field_from_map(preview_item, "preview_recurring_matched_date"),
    ]);
    recurring.insert("match_reasons".to_string(), Value::String(match_reasons));
    recurring.insert("matched_date".to_string(), Value::String(matched_date));
}

fn populate_annotation_matching_section(
    payload_object: &mut Map<String, Value>,
    preview_item: &Map<String, Value>,
) {
    // 中文说明：统一手动标注标志，matching payload 和 preview 旧字段任一为真都视为已人工处理。
    let annotation = section_object_mut(payload_object, "annotation");
    let is_manually_annotated = annotation
        .get("is_manually_annotated")
        .map(json_value_is_truthy)
        .unwrap_or(false)
        || preview_item
            .get("preview_is_manually_annotated")
            .map(json_value_is_truthy)
            .unwrap_or(false);
    annotation.insert(
        "is_manually_annotated".to_string(),
        Value::Bool(is_manually_annotated),
    );
}

fn first_non_empty_string_field<'a>(
    section: &Map<String, Value>,
    section_keys: impl Iterator<Item = &'a str>,
    preview_item: &Map<String, Value>,
) -> String {
    get_first_non_empty_string(section_keys.map(|key| {
        let value = section.get(key).or_else(|| preview_item.get(key));
        value_to_trimmed_string(value)
    }))
}

#[tracing::instrument(level = "debug", skip_all)]
// 中文说明：按优先级选择第一个非空字符串，服务于新旧 matching 字段兼容合并。
fn get_first_non_empty_string(values: impl IntoIterator<Item = String>) -> String {
    values
        .into_iter()
        .find(|value| !value.trim().is_empty())
        .unwrap_or_default()
}

fn first_non_empty_value_list<'a>(values: impl IntoIterator<Item = &'a Value>) -> Vec<Value> {
    // 中文说明：按优先级选择第一个非空数组或逗号分隔列表，专门兼容 dedup source ids。
    values
        .into_iter()
        .map(|value| parse_dedup_source_ids(Some(value)))
        .find(|values| !values.is_empty())
        .unwrap_or_default()
}

fn matching_section_string_field(
    preview_item: &Map<String, Value>,
    section: &str,
    key: &str,
) -> Option<String> {
    preview_matching_payload(preview_item)
        .and_then(|matching| object_field(matching.get(section)))
        .map(|section| string_field_from_map(section, key))
}

fn matching_section_float_field(
    preview_item: &Map<String, Value>,
    section: &str,
    key: &str,
) -> f64 {
    preview_matching_payload(preview_item)
        .and_then(|matching| object_field(matching.get(section)))
        .map(|section| float_field_from_map(section, key))
        .unwrap_or_default()
}
