#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImportPreviewSortDirection {
    Asc,
    Desc,
}

impl ImportPreviewSortDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportPreviewPageQuery {
    pub page: usize,
    pub page_size: usize,
    pub sort_by: String,
    pub sort_direction: ImportPreviewSortDirection,
    pub preview_ids: Vec<i64>,
}

/// 中文说明：规范化导入预览分页、排序和显式 preview id 过滤参数，保证路由层只接收受控查询对象。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_import_preview_page_query(
    page: Option<i64>,
    page_size: Option<i64>,
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
    preview_ids: &[i64],
) -> ImportPreviewPageQuery {
    ImportPreviewPageQuery {
        page: normalize_page(page),
        page_size: normalize_page_size(page_size),
        sort_by: normalize_import_preview_page_sort_key(sort_by).to_string(),
        sort_direction: normalize_import_preview_page_sort_direction(sort_direction),
        preview_ids: normalize_preview_ids(preview_ids),
    }
}

/// 中文说明：把前端页码压到有效正整数，避免非法 page 影响预览分页 SQL 与内存切片。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_page(page: Option<i64>) -> usize {
    usize::try_from(page.unwrap_or(1).max(1)).unwrap_or(1)
}

/// 中文说明：限制导入预览 page_size 范围，防止单次预览请求绕过后端分页上限。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_page_size(page_size: Option<i64>) -> usize {
    let page_size = page_size.unwrap_or(50).clamp(1, 200);
    usize::try_from(page_size).unwrap_or(50)
}

/// 中文说明：规范化排序方向，除明确 desc 外全部回落为 asc，保持旧前端默认排序行为。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_import_preview_page_sort_direction(
    sort_direction: Option<&str>,
) -> ImportPreviewSortDirection {
    if sort_direction
        .unwrap_or("")
        .trim()
        .eq_ignore_ascii_case("desc")
    {
        ImportPreviewSortDirection::Desc
    } else {
        ImportPreviewSortDirection::Asc
    }
}

/// 中文说明：只允许白名单内的预览排序字段，未知字段返回空值并保留原始顺序。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_import_preview_page_sort_key(sort_by: Option<&str>) -> &'static str {
    let normalized = sort_by.unwrap_or("").trim();
    IMPORT_PREVIEW_SORT_KEYS
        .iter()
        .copied()
        .find(|key| *key == normalized)
        .unwrap_or("")
}

/// 中文说明：清理显式预览 id 列表，去掉非正数和重复项，保证后续筛选目标稳定。
#[tracing::instrument(level = "debug", skip_all)]
pub fn normalize_preview_ids(preview_ids: &[i64]) -> Vec<i64> {
    let mut seen = HashSet::new();
    preview_ids
        .iter()
        .copied()
        .filter(|preview_id| *preview_id > 0)
        .filter(|preview_id| seen.insert(*preview_id))
        .collect()
}

/// 中文说明：按前端支持的导入预览列做稳定排序，先按 id 固定基础顺序再应用业务字段排序。
#[tracing::instrument(level = "debug", skip_all)]
pub fn sort_import_preview_page_items(
    items: &[Value],
    sort_by: Option<&str>,
    sort_direction: Option<&str>,
) -> Vec<Value> {
    let normalized_sort_by = normalize_import_preview_page_sort_key(sort_by);
    if normalized_sort_by.is_empty() {
        return items.to_vec();
    }

    let sort_field = match normalized_sort_by {
        "time" => "preview_date",
        "type" => "preview_type",
        "sourceAmountCents" => "preview_amount_cents",
        "counterparty" => "preview_counterparty",
        "paymentMethod" => "preview_payment_method",
        "comment" => "preview_description",
        _ => return items.to_vec(),
    };

    let mut sorted = items.to_vec();
    sorted.sort_by_key(|item| integer_field(item, "id"));
    let descending = normalize_import_preview_page_sort_direction(sort_direction)
        == ImportPreviewSortDirection::Desc;
    if sort_field == "preview_amount_cents" {
        sorted.sort_by(|left, right| {
            let left_key = integer_field(left, sort_field);
            let right_key = integer_field(right, sort_field);
            if descending {
                right_key.cmp(&left_key)
            } else {
                left_key.cmp(&right_key)
            }
        });
    } else {
        sorted.sort_by(|left, right| {
            let left_key = string_field(left, sort_field).to_lowercase();
            let right_key = string_field(right, sort_field).to_lowercase();
            if descending {
                right_key.cmp(&left_key)
            } else {
                left_key.cmp(&right_key)
            }
        });
    }
    sorted
}

/// 中文说明：把多种 JSON 表示转换为预览选中布尔值，兼容旧 patch payload 中的字符串、数字和空值。
#[tracing::instrument(level = "debug", skip_all)]
pub fn coerce_preview_selected_value(value: Option<&Value>, default: bool) -> bool {
    match value {
        None | Some(Value::Null) => default,
        Some(Value::Bool(value)) => *value,
        Some(Value::Number(number)) => !json_number_is_zero(number),
        Some(Value::String(text)) => {
            let normalized = text.trim().to_ascii_lowercase();
            match normalized.as_str() {
                "" | "none" | "null" => default,
                "0" | "false" | "no" | "off" | "n" => false,
                "1" | "true" | "yes" | "on" | "y" => true,
                _ => !text.is_empty(),
            }
        }
        Some(Value::Array(values)) => !values.is_empty(),
        Some(Value::Object(values)) => !values.is_empty(),
    }
}

/// 中文说明：从预览更新 payload 中解析选中状态，兼容多个历史字段名并保留默认选择语义。
#[tracing::instrument(level = "debug", skip_all)]
pub fn preview_update_is_selected(update_item: &Map<String, Value>, default: bool) -> bool {
    IMPORT_PREVIEW_SELECTION_KEYS
        .iter()
        .find_map(|key| {
            update_item
                .get(*key)
                .map(|value| coerce_preview_selected_value(Some(value), default))
        })
        .unwrap_or(default)
}
