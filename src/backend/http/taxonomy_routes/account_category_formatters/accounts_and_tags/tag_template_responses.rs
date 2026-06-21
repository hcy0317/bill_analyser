fn format_tag_list_response(tags: Vec<TagRecord>) -> Value {
    Value::Array(
        tags.into_iter()
            .map(backend_tag_to_frontend)
            .map(Value::Object)
            .collect(),
    )
}

fn format_template_list_response(templates: Vec<TemplateRecord>) -> Value {
    Value::Array(templates.into_iter().map(Value::Object).collect())
}

fn backend_tag_to_frontend(tag: TagRecord) -> Map<String, Value> {
    let hidden = tag.hidden != 0;
    let mut result = Map::new();
    result.insert("id".to_string(), Value::String(tag.id.to_string()));
    result.insert("name".to_string(), Value::String(tag.name));
    result.insert(
        "color".to_string(),
        tag.color.map_or(Value::Null, Value::String),
    );
    result.insert(
        "icon".to_string(),
        tag.icon.map_or(Value::Null, Value::String),
    );
    result.insert(
        "displayOrder".to_string(),
        Value::Number(Number::from(tag.display_order)),
    );
    result.insert("hidden".to_string(), Value::Bool(hidden));
    result.insert("visible".to_string(), Value::Bool(!hidden));
    result
}
