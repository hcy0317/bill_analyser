pub fn is_import_preview_visible_signal_family(family: &str) -> bool {
    ImportPreviewSignalFamily::parse(family).is_some()
}

pub fn import_preview_recommendation_feedback_family_is_meaningful(
    feedback: &Value,
    family: &str,
) -> bool {
    let Some(family) = ImportPreviewSignalFamily::parse(family) else {
        return false;
    };
    if !matches!(
        family,
        ImportPreviewSignalFamily::Learning | ImportPreviewSignalFamily::Llm
    ) {
        return false;
    }
    let Some(section) = feedback.get(family.as_str()).and_then(Value::as_object) else {
        return false;
    };

    match family {
        ImportPreviewSignalFamily::Learning => learning_matching_section_is_meaningful(section),
        ImportPreviewSignalFamily::Llm => llm_matching_section_is_meaningful(section),
        _ => false,
    }
}

fn learning_matching_section_is_meaningful(section: &Map<String, Value>) -> bool {
    if matching_section_is_suppressed_or_none(section) {
        return false;
    }
    if matching_section_has_unknown_review_status(
        section,
        IMPORT_PREVIEW_LEARNING_CANONICAL_STATUSES,
    ) {
        return false;
    }
    if matching_section_has_terminal_review_status(section) {
        return true;
    }
    if matching_section_has_non_pending_review_status(section) {
        return true;
    }
    matching_section_has_any_meaningful_key(
        section,
        IMPORT_PREVIEW_LEARNING_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LEARNING_TEXT_EVIDENCE_FIELDS,
    )
}

fn llm_matching_section_is_meaningful(section: &Map<String, Value>) -> bool {
    if matching_section_is_suppressed_or_none(section) {
        return false;
    }
    if matching_section_has_unknown_review_status(section, IMPORT_PREVIEW_SIGNAL_CANONICAL_STATUSES)
    {
        return false;
    }
    if matching_section_has_terminal_review_status(section) {
        return true;
    }
    if matching_section_has_non_pending_review_status(section) {
        return true;
    }
    matching_section_has_any_meaningful_key(
        section,
        IMPORT_PREVIEW_LLM_NUMERIC_EVIDENCE_FIELDS,
        IMPORT_PREVIEW_LLM_TEXT_EVIDENCE_FIELDS,
    )
}

fn is_suppressed_by_truthy_flag(section: &Map<String, Value>) -> bool {
    section
        .get("suppressed")
        .map(import_preview_signal_value_is_truthy)
        .unwrap_or(false)
}

fn matching_section_is_suppressed_or_none(section: &Map<String, Value>) -> bool {
    if is_suppressed_by_truthy_flag(section) {
        return true;
    }
    let resolved = resolve_first_nonempty_status(section);
    matches!(resolved.as_str(), "none" | "suppressed")
}

fn matching_section_has_terminal_review_status(section: &Map<String, Value>) -> bool {
    let resolved = resolve_first_nonempty_status(section);
    if resolved.is_empty() {
        return false;
    }
    IMPORT_PREVIEW_SIGNAL_TERMINAL_STATUSES.contains(&resolved.as_str())
}

fn matching_section_has_non_pending_review_status(section: &Map<String, Value>) -> bool {
    let resolved = resolve_first_nonempty_status(section);
    !resolved.is_empty()
        && !IMPORT_PREVIEW_SIGNAL_NON_PENDING_EXCLUDED_STATUSES.contains(&resolved.as_str())
}

fn matching_section_has_unknown_review_status(
    section: &Map<String, Value>,
    canonical_statuses: &[&str],
) -> bool {
    let resolved = resolve_first_nonempty_status(section);
    !resolved.is_empty() && !canonical_statuses.contains(&resolved.as_str())
}

fn matching_section_has_any_meaningful_key(
    section: &Map<String, Value>,
    numeric_keys: &[&str],
    text_keys: &[&str],
) -> bool {
    numeric_keys
        .iter()
        .any(|key| section.get(*key).is_some_and(json_number_is_positive))
        || text_keys
            .iter()
            .any(|key| section.get(*key).is_some_and(json_text_is_meaningful))
}

fn json_number_is_positive(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_f64().is_some_and(|value| value > 0.0),
        Value::String(text) => strict_decimal_is_positive(text),
        _ => false,
    }
}

fn json_text_is_meaningful(value: &Value) -> bool {
    let Some(text) = value
        .as_str()
        .map(trim_import_preview_signal_text)
        .filter(|text| !text.is_empty())
    else {
        return false;
    };
    let normalized = text.to_ascii_lowercase();
    if matches!(normalized.as_str(), "none" | "suppressed" | "null") {
        return false;
    }
    if strict_decimal_matches_grammar(text) {
        return strict_decimal_is_positive(text);
    }
    true
}

pub fn trim_import_preview_signal_text(value: &str) -> &str {
    value.trim_matches(|ch| IMPORT_PREVIEW_SIGNAL_TRIM_CHARS.contains(ch))
}

pub fn import_preview_signal_value_to_lowercase(value: &Value) -> String {
    trim_import_preview_signal_text(&value_to_trimmed_string(Some(value))).to_ascii_lowercase()
}

/// 中文说明：统一信号真值判断。bool true、文本 true/yes/y、以及有限非零十进制数视为真值；
/// 零、负零、畸形数字文本、falsey 词、null、容器和空白串视为假值。
pub fn import_preview_signal_value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        Value::Number(number) => number
            .as_f64()
            .is_some_and(|value| value.is_finite() && value != 0.0),
        Value::String(text) => {
            let normalized = trim_import_preview_signal_text(text).to_ascii_lowercase();
            if IMPORT_PREVIEW_SIGNAL_FALSEY_TEXT_VALUES.contains(&normalized.as_str()) {
                return false;
            }
            if IMPORT_PREVIEW_SIGNAL_TRUTHY_TEXT_VALUES.contains(&normalized.as_str()) {
                return true;
            }
            strict_decimal_has_non_zero(&normalized)
        }
        _ => false,
    }
}

pub fn parse_import_preview_decimal_number(value: &str) -> Option<f64> {
    let text = trim_import_preview_signal_text(value);
    if text.is_empty() {
        return None;
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    let (before_dot, after_dot) = match unsigned.split_once('.') {
        Some((before, after)) if !after.is_empty() => (before, Some(after)),
        Some(_) => return None,
        None => (unsigned, None),
    };
    let before_digits = before_dot.chars().all(|ch| ch.is_ascii_digit());
    let after_digits = after_dot.is_none_or(|after| after.chars().all(|ch| ch.is_ascii_digit()));
    let has_digit = before_dot.chars().any(|ch| ch.is_ascii_digit())
        || after_dot.is_some_and(|after| after.chars().any(|ch| ch.is_ascii_digit()));
    if before_digits && after_digits && has_digit {
        text.parse::<f64>().ok().filter(|value| value.is_finite())
    } else {
        None
    }
}

/// 中文说明：快速检查字符串是否匹配严格十进制语法——无指数、无 hex、无 NaN/Infinity。
/// 格式：^[+-]?([0-9]+([.][0-9]+)?|[.][0-9]+)$
fn strict_decimal_matches_grammar(text: &str) -> bool {
    let text = trim_import_preview_signal_text(text);
    if text.is_empty() {
        return false;
    }
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    if unsigned.is_empty() {
        return false;
    }
    let bytes = unsigned.as_bytes();
    let dot_pos = bytes.iter().position(|&b| b == b'.');
    let (before, after, has_dot) = if let Some(pos) = dot_pos {
        (&bytes[..pos], &bytes[pos + 1..], true)
    } else {
        (bytes, &[][..], false)
    };
    let before_ok = before.iter().all(u8::is_ascii_digit);
    let after_ok = after.iter().all(u8::is_ascii_digit);
    let has_digit = before.iter().any(u8::is_ascii_digit) || after.iter().any(u8::is_ascii_digit);
    before_ok
        && after_ok
        && has_digit
        && !(before.is_empty() && after.is_empty())
        && (!has_dot || !after.is_empty())
}

/// 中文说明：检查十进制字符串（已语法验证后）是否含有至少一个非零数字。
fn strict_decimal_unsigned_has_non_zero(text: &str) -> bool {
    let unsigned = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    unsigned.contains('1')
        || unsigned.contains('2')
        || unsigned.contains('3')
        || unsigned.contains('4')
        || unsigned.contains('5')
        || unsigned.contains('6')
        || unsigned.contains('7')
        || unsigned.contains('8')
        || unsigned.contains('9')
}

/// 中文说明：词法检查十进制字符串是否含有至少一个非零数字（先验证语法，不依赖 f64）。
pub fn strict_decimal_has_non_zero(text: &str) -> bool {
    if !strict_decimal_matches_grammar(text) {
        return false;
    }
    strict_decimal_unsigned_has_non_zero(text)
}

/// 中文说明：词法检查十进制字符串是否为正数（先验证语法，非负且至少有一个非零数字）。
pub fn strict_decimal_is_positive(text: &str) -> bool {
    if !strict_decimal_matches_grammar(text) {
        return false;
    }
    let trimmed = trim_import_preview_signal_text(text);
    if trimmed.starts_with('-') {
        return false;
    }
    strict_decimal_unsigned_has_non_zero(trimmed)
}
