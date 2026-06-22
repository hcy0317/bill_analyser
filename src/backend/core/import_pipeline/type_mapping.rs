#[tracing::instrument(level = "debug", skip_all)]
pub fn map_import_preview_type_to_frontend_value(preview_type: Option<&Value>) -> i64 {
    match value_to_trimmed_string(preview_type).as_str() {
        "收入" | "income" => 2,
        "支出" | "expense" => 3,
        "转账" | "transfer" => 4,
        "投资" | "investment" => 5,
        _ => 1,
    }
}
