// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn format_category_tree_response(categories: Vec<CategoryRecord>) -> Value {
    let mut grouped: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
    let mut main_indices: BTreeMap<(i64, String), (i64, usize)> = BTreeMap::new();

    for category in categories {
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(0);
        let main_name = category_text(&category, "main_category");
        let sub_name = category_text(&category, "sub_category");
        let key = (category_type, main_name.clone());

        if !main_indices.contains_key(&key) {
            let parent_id = if sub_name.is_empty() {
                value_string(category.get("id"), &format!("virtual_{main_name}"))
            } else {
                format!("virtual_{main_name}")
            };
            let mut node = backend_category_to_frontend(&category, "0");
            node.insert("id".to_string(), Value::String(parent_id));
            node.insert("name".to_string(), Value::String(main_name.clone()));
            node.insert("parentId".to_string(), Value::String("0".to_string()));
            node.insert("comment".to_string(), Value::String(String::new()));
            node.insert("hidden".to_string(), Value::Bool(false));
            node.insert("visible".to_string(), Value::Bool(true));
            node.insert("keywords".to_string(), Value::String(String::new()));
            node.insert("subCategories".to_string(), Value::Array(Vec::new()));
            let bucket = grouped.entry(category_type).or_default();
            let index = bucket.len();
            bucket.push(Value::Object(node));
            main_indices.insert(key.clone(), (category_type, index));
        }

        let Some((bucket_key, index)) = main_indices.get(&key).copied() else {
            continue;
        };
        let Some(bucket) = grouped.get_mut(&bucket_key) else {
            continue;
        };
        let Some(node) = bucket.get_mut(index).and_then(Value::as_object_mut) else {
            continue;
        };

        if sub_name.is_empty() {
            let sub_categories = node
                .remove("subCategories")
                .unwrap_or_else(|| Value::Array(Vec::new()));
            *node = backend_category_to_frontend(&category, "0");
            node.insert("subCategories".to_string(), sub_categories);
        } else {
            let parent_id = node
                .get("id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .unwrap_or_else(|| format!("virtual_{main_name}"));
            let sub_node = backend_category_to_frontend(&category, &parent_id);
            node.entry("subCategories".to_string())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("subCategories must stay an array")
                .push(Value::Object(sub_node));
        }
    }

    let mut result = Map::new();
    for (category_type, values) in grouped {
        result.insert(category_type.to_string(), Value::Array(values));
    }
    Value::Object(result)
}

fn format_category_flat_response(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(
        categories
            .iter()
            .map(|category| {
                let parent_id = if category_text(category, "sub_category").is_empty() {
                    "0".to_string()
                } else {
                    format!("virtual_{}", category_text(category, "main_category"))
                };
                Value::Object(backend_category_to_frontend(category, &parent_id))
            })
            .collect(),
    )
}

fn categories_to_value(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(categories.into_iter().map(Value::Object).collect())
}

fn format_category_statistics_response(statistics: Vec<CategoryStatistic>) -> Value {
    let mut result = Map::new();

    for statistic in statistics {
        let entry = result
            .entry(statistic.main_category.clone())
            .or_insert_with(|| {
                json!({
                    "total_amount": 0.0,
                    "count": 0,
                    "sub_categories": {}
                })
            });
        let Some(entry_object) = entry.as_object_mut() else {
            continue;
        };

        let total_amount = entry_object
            .get("total_amount")
            .and_then(value_as_f64)
            .unwrap_or_default()
            + statistic.total_amount.abs();
        entry_object.insert(
            "total_amount".to_string(),
            json_number(round2(total_amount)),
        );

        let count = entry_object
            .get("count")
            .and_then(value_as_i64)
            .unwrap_or_default()
            + statistic.count;
        entry_object.insert("count".to_string(), Value::Number(Number::from(count)));

        if statistic.sub_category.is_empty() {
            continue;
        }
        let sub_categories = entry_object
            .entry("sub_categories".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(sub_categories) = sub_categories.as_object_mut() {
            sub_categories.insert(
                statistic.sub_category.clone(),
                json!({
                    "total_amount": round2(statistic.total_amount.abs()),
                    "count": statistic.count
                }),
            );
        }
    }

    Value::Object(result)
}
