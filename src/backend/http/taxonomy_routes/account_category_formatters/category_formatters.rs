// 中文导读：HTTP 运行态层，负责 Axum 路由、认证上下文、请求 DTO 解析和前端当前响应投影。
// 维护重点：handler 只编排请求到 core/db 的调用，复杂 SQL、事务和跨表规则应下沉到 repository 或业务合同层。
// 不变式：所有 /api/... 路由保持 Rust-only 主链、user-scope 校验和既有 success/data 或 success/result envelope。

fn canonical_category_parent_ids(
    categories: &[CategoryRecord],
) -> BTreeMap<(i64, String), String> {
    let mut parent_ids = BTreeMap::new();
    for category in categories {
        if !category_text(category, "sub_category").is_empty() {
            continue;
        }
        let category_id = value_string(category.get("id"), "");
        if category_id.is_empty() {
            continue;
        }
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(0);
        let main_name = category_text(category, "main_category");
        parent_ids
            .entry((category_type, main_name))
            .or_insert(category_id);
    }
    parent_ids
}

fn canonical_category_parent_id(
    parent_ids: &BTreeMap<(i64, String), String>,
    category_type: i64,
    main_name: &str,
) -> String {
    parent_ids
        .get(&(category_type, main_name.to_string()))
        .cloned()
        .unwrap_or_else(|| format!("virtual_{main_name}"))
}

fn format_category_tree_response(categories: Vec<CategoryRecord>) -> Value {
    let parent_ids = canonical_category_parent_ids(&categories);
    let mut grouped: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
    let mut main_indices: BTreeMap<(i64, String), (i64, usize)> = BTreeMap::new();

    for category in categories {
        let category_type = category.get("type").and_then(value_as_i64).unwrap_or(0);
        let main_name = category_text(&category, "main_category");
        let sub_name = category_text(&category, "sub_category");
        let key = (category_type, main_name.clone());

        if !main_indices.contains_key(&key) {
            let parent_id =
                canonical_category_parent_id(&parent_ids, category_type, &main_name);
            let mut node = backend_category_to_frontend(&category, "0");
            node.insert("id".to_string(), Value::String(parent_id));
            node.insert("name".to_string(), Value::String(main_name.clone()));
            node.insert("parentId".to_string(), Value::String("0".to_string()));
            node.insert("comment".to_string(), Value::String(String::new()));
            node.insert("hidden".to_string(), Value::Bool(false));
            node.insert("visible".to_string(), Value::Bool(true));
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
            let parent_id =
                canonical_category_parent_id(&parent_ids, category_type, &main_name);
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
    let parent_ids = canonical_category_parent_ids(&categories);
    Value::Array(
        categories
            .iter()
            .map(|category| {
                let parent_id = if category_text(category, "sub_category").is_empty() {
                    "0".to_string()
                } else {
                    let category_type =
                        category.get("type").and_then(value_as_i64).unwrap_or(0);
                    canonical_category_parent_id(
                        &parent_ids,
                        category_type,
                        &category_text(category, "main_category"),
                    )
                };
                Value::Object(backend_category_to_frontend(category, &parent_id))
            })
            .collect(),
    )
}

fn categories_to_value(categories: Vec<CategoryRecord>) -> Value {
    Value::Array(categories.into_iter().map(Value::Object).collect())
}

#[cfg(test)]
mod category_formatter_tests {
    use super::*;

    fn category(id: i64, main_category: &str, sub_category: &str) -> CategoryRecord {
        json!({
            "id": id,
            "type": 2,
            "main_category": main_category,
            "sub_category": sub_category,
            "hidden": false
        })
        .as_object()
        .expect("category record")
        .clone()
    }

    #[test]
    fn category_parent_identity_is_order_independent_in_tree_and_flat_responses() {
        let categories = vec![
            category(43, "其他收入", "原路退款"),
            category(42, "其他收入", ""),
            category(44, "其他收入", "意外收入"),
        ];

        let tree = format_category_tree_response(categories.clone());
        let parent = tree["2"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(Value::as_object)
            .expect("income category parent");
        assert_eq!(parent.get("id"), Some(&Value::String("42".to_string())));
        let children = parent["subCategories"]
            .as_array()
            .expect("income category children");
        assert_eq!(children.len(), 2);
        assert!(children.iter().all(|child| child["parentId"] == "42"));

        let flat = format_category_flat_response(categories);
        let flat_children = flat.as_array().expect("flat categories");
        assert_eq!(flat_children[0]["parentId"], "42");
        assert_eq!(flat_children[2]["parentId"], "42");
    }
}

fn format_category_statistics_response(statistics: Vec<CategoryStatistic>) -> Value {
    let mut result = Map::new();

    for statistic in statistics {
        let entry = result
            .entry(statistic.main_category.clone())
            .or_insert_with(|| {
                json!({
                    "totalAmountCents": 0,
                    "count": 0,
                    "sub_categories": {}
                })
            });
        let Some(entry_object) = entry.as_object_mut() else {
            continue;
        };

        let total_amount_cents = entry_object
            .get("totalAmountCents")
            .and_then(value_as_i64)
            .unwrap_or_default()
            + statistic.total_amount_cents.abs();
        entry_object.insert(
            "totalAmountCents".to_string(),
            Value::Number(Number::from(total_amount_cents)),
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
                    "totalAmountCents": statistic.total_amount_cents.abs(),
                    "count": statistic.count
                }),
            );
        }
    }

    Value::Object(result)
}
