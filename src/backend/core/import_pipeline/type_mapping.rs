/// 中文说明：把后端预览类型文本映射为前端 TransactionType 数值，兼容中文类型和历史数字字符串。
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
