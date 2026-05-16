use chrono::{Datelike, Local, NaiveDate, NaiveTime};
use unicode_normalization::UnicodeNormalization;

use super::types::PaymentScreenshotParseContract;

pub fn parse_payment_screenshot_text(text: &str) -> PaymentScreenshotParseContract {
    let normalized_text = normalize_text(text);
    if normalized_text.is_empty() {
        return PaymentScreenshotParseContract {
            amount: None,
            trade_time: None,
            description: None,
            payment_platform: None,
            confidence: 0.0,
        };
    }

    let lines = normalize_lines(&normalized_text);
    let amount = parse_amount(&normalized_text);
    let trade_time = parse_trade_time(&normalized_text);
    let description = parse_description(&lines);
    let payment_platform = detect_payment_platform(&normalized_text).map(str::to_string);
    let confidence = score_ocr_confidence(
        amount,
        trade_time.as_deref(),
        description.as_deref(),
        payment_platform.as_deref(),
    );

    PaymentScreenshotParseContract {
        amount,
        trade_time,
        description,
        payment_platform,
        confidence,
    }
}

fn normalize_text(text: &str) -> String {
    text.nfkc()
        .collect::<String>()
        .replace('\u{00a0}', " ")
        .trim()
        .to_string()
}

fn normalize_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(collapse_whitespace)
        .map(|line| {
            line.trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '：'))
                .to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn detect_payment_platform(text: &str) -> Option<&'static str> {
    let lowered = text.to_lowercase();
    if text.contains("微信支付") || text.contains("财付通") || lowered.contains("wechat pay")
    {
        Some("wechat_pay")
    } else if text.contains("支付宝")
        || lowered.contains("alipay")
        || text.contains("花呗")
        || text.contains("余额宝")
    {
        Some("alipay")
    } else {
        None
    }
}

fn parse_amount(text: &str) -> Option<f64> {
    for marker in [
        "付款金额",
        "支付金额",
        "实付金额",
        "实付款",
        "订单金额",
        "转账金额",
        "收款金额",
        "金额",
        "合计",
        "总计",
    ] {
        if let Some(index) = text.find(marker) {
            if let Some(amount) = parse_number_after_prefix(text, index + marker.len()) {
                return Some(amount.abs());
            }
        }
    }

    for currency_marker in ['¥', '￥'] {
        if let Some(index) = text.find(currency_marker) {
            if let Some(amount) =
                parse_number_after_prefix(text, index + currency_marker.len_utf8())
            {
                return Some(amount.abs());
            }
        }
    }

    if contains_any_case_insensitive(text, &["元", "CNY", "RMB"]) {
        parse_currency_adjacent_amount(text).map(f64::abs)
    } else {
        None
    }
}

fn parse_number_after_prefix(text: &str, start: usize) -> Option<f64> {
    let mut number_start = None;
    for (offset, ch) in text[start..].char_indices() {
        if ch.is_ascii_digit() || matches!(ch, '+' | '-') {
            number_start = Some(start + offset);
            break;
        }
        if !(ch.is_whitespace() || matches!(ch, ':' | '：' | '¥' | '￥')) {
            return None;
        }
    }
    number_start.and_then(|index| parse_number_at(text, index))
}

fn parse_currency_adjacent_amount(text: &str) -> Option<f64> {
    for marker in ["CNY", "cny", "RMB", "rmb"] {
        for (index, _) in text.match_indices(marker) {
            if let Some(amount) = parse_number_after_prefix(text, index + marker.len())
                .or_else(|| parse_number_before_index(text, index))
            {
                return Some(amount);
            }
        }
    }

    for (index, _) in text.match_indices("元") {
        if let Some(amount) = parse_number_before_index(text, index)
            .or_else(|| parse_number_after_prefix(text, index + "元".len()))
        {
            return Some(amount);
        }
    }

    None
}

fn parse_number_before_index(text: &str, end: usize) -> Option<f64> {
    let mut trimmed_end = end;
    while let Some((index, ch)) = text[..trimmed_end].char_indices().next_back() {
        if ch.is_whitespace() {
            trimmed_end = index;
        } else {
            break;
        }
    }

    let mut start = trimmed_end;
    let mut has_digit = false;
    let mut has_decimal = false;
    for (index, ch) in text[..trimmed_end].char_indices().rev() {
        if ch.is_ascii_digit() {
            start = index;
            has_digit = true;
        } else if ch == '.' && !has_decimal {
            start = index;
            has_decimal = true;
        } else if ch == ',' {
            start = index;
        } else if matches!(ch, '+' | '-') {
            start = index;
            break;
        } else {
            break;
        }
    }

    has_digit.then(|| parse_number_at(text, start)).flatten()
}

fn parse_number_at(text: &str, start: usize) -> Option<f64> {
    let mut raw = String::new();
    let mut has_digit = false;
    let mut has_decimal = false;
    for ch in text[start..].chars() {
        if ch.is_ascii_digit() {
            has_digit = true;
            raw.push(ch);
        } else if matches!(ch, '+' | '-') && raw.is_empty() {
            raw.push(ch);
        } else if ch == '.' && !has_decimal {
            has_decimal = true;
            raw.push(ch);
        } else if ch == ',' {
            continue;
        } else {
            break;
        }
    }
    if has_digit {
        raw.parse::<f64>().ok()
    } else {
        None
    }
}

fn parse_trade_time(text: &str) -> Option<String> {
    let search_text = normalize_datetime_text(text);
    let tokens = search_text.split_whitespace().collect::<Vec<_>>();
    for (index, token) in tokens.iter().enumerate() {
        let Some(date_parts) = parse_date_token(token) else {
            continue;
        };
        let time_parts = tokens
            .get(index + 1)
            .and_then(|next| parse_time_token(next));
        return Some(format_date_time(date_parts, time_parts));
    }
    None
}

fn normalize_datetime_text(text: &str) -> String {
    text.nfkc()
        .collect::<String>()
        .chars()
        .map(|ch| match ch {
            '年' | '月' | '/' | '.' => '-',
            '日' => ' ',
            _ => ch,
        })
        .collect()
}

fn parse_date_token(token: &str) -> Option<(i32, u32, u32)> {
    let cleaned = trim_to_ascii_date_token(token);
    let parts = cleaned.split('-').collect::<Vec<_>>();
    match parts.as_slice() {
        [year, month, day] if year.len() == 4 => {
            let parsed = (year.parse().ok()?, month.parse().ok()?, day.parse().ok()?);
            validate_date(parsed)
        }
        [month, day] => {
            let parsed = (
                Local::now().date_naive().year(),
                month.parse().ok()?,
                day.parse().ok()?,
            );
            validate_date(parsed)
        }
        _ => None,
    }
}

fn parse_time_token(token: &str) -> Option<(u32, u32, u32, bool)> {
    let cleaned = trim_to_ascii_time_token(token);
    let parts = cleaned.split(':').collect::<Vec<_>>();
    match parts.as_slice() {
        [hour, minute] => {
            let parsed = (hour.parse().ok()?, minute.parse().ok()?, 0, false);
            validate_time(parsed)
        }
        [hour, minute, second] => {
            let parsed = (
                hour.parse().ok()?,
                minute.parse().ok()?,
                second.parse().ok()?,
                true,
            );
            validate_time(parsed)
        }
        _ => None,
    }
}

fn trim_to_ascii_date_token(token: &str) -> String {
    token
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
        .collect()
}

fn trim_to_ascii_time_token(token: &str) -> String {
    token
        .chars()
        .skip_while(|ch| !ch.is_ascii_digit())
        .take_while(|ch| ch.is_ascii_digit() || *ch == ':')
        .collect()
}

fn validate_date(parts: (i32, u32, u32)) -> Option<(i32, u32, u32)> {
    NaiveDate::from_ymd_opt(parts.0, parts.1, parts.2).map(|_| parts)
}

fn validate_time(parts: (u32, u32, u32, bool)) -> Option<(u32, u32, u32, bool)> {
    NaiveTime::from_hms_opt(parts.0, parts.1, parts.2).map(|_| parts)
}

fn format_date_time(
    date_parts: (i32, u32, u32),
    time_parts: Option<(u32, u32, u32, bool)>,
) -> String {
    let (year, month, day) = date_parts;
    if let Some((hour, minute, second, has_second)) = time_parts {
        if has_second {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
        } else {
            format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
        }
    } else {
        format!("{year:04}-{month:02}-{day:02}")
    }
}

fn parse_description(lines: &[String]) -> Option<String> {
    parse_labeled_description(lines).or_else(|| {
        lines
            .iter()
            .find(|line| is_description_candidate(line))
            .map(|line| clean_description(line))
    })
}

fn parse_labeled_description(lines: &[String]) -> Option<String> {
    for (index, line) in lines.iter().enumerate() {
        for label in description_labels() {
            if let Some(value) = value_after_label(line, label) {
                if is_description_candidate(value) {
                    return Some(clean_description(value));
                }
            }
            if line == label {
                if let Some(next_line) = lines
                    .get(index + 1)
                    .filter(|item| is_description_candidate(item))
                {
                    return Some(clean_description(next_line));
                }
            }
        }
    }
    None
}

fn value_after_label<'a>(line: &'a str, label: &str) -> Option<&'a str> {
    let remainder = line.strip_prefix(label)?;
    if remainder.is_empty() {
        return None;
    }
    let mut chars = remainder.chars();
    if let Some(first) = chars.next() {
        if !(first.is_whitespace() || matches!(first, ':' | '：')) {
            return None;
        }
    }
    let value = remainder.trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '：'));
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn is_description_candidate(line: &str) -> bool {
    let text = line.trim();
    if text.is_empty() || text.chars().count() > 80 {
        return false;
    }
    if noise_tokens().iter().any(|token| text.contains(token)) {
        return false;
    }
    if description_labels().contains(&text) {
        return false;
    }
    if parse_amount(text).is_some() || parse_trade_time(text).is_some() {
        return false;
    }
    if text.chars().count() >= 8
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':'))
    {
        return false;
    }
    text.chars().any(|ch| ch.is_alphabetic() || is_cjk(ch))
}

fn clean_description(value: &str) -> String {
    collapse_whitespace(value)
        .trim_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '：'))
        .chars()
        .take(60)
        .collect()
}

fn score_ocr_confidence(
    amount: Option<f64>,
    trade_time: Option<&str>,
    description: Option<&str>,
    payment_platform: Option<&str>,
) -> f64 {
    let mut score = 0.0;
    if amount.is_some() {
        score += 0.3;
    }
    if trade_time.is_some_and(|value| !value.is_empty()) {
        score += 0.25;
    }
    if description.is_some_and(|value| !value.is_empty()) {
        score += 0.25;
    }
    if payment_platform.is_some_and(|value| !value.is_empty()) {
        score += 0.2;
    }
    f64::min(1.0, score)
}

fn description_labels() -> &'static [&'static str] {
    &[
        "交易对方",
        "收款方",
        "付款方",
        "商户",
        "商家",
        "对方账户",
        "商品",
        "商品说明",
        "订单名称",
        "备注",
        "说明",
    ]
}

fn noise_tokens() -> &'static [&'static str] {
    &[
        "微信支付",
        "支付宝",
        "支付成功",
        "交易成功",
        "商家服务",
        "当前状态",
        "付款方式",
        "支付方式",
        "交易单号",
        "商户单号",
        "订单号",
        "创建时间",
        "支付时间",
        "交易时间",
        "付款时间",
        "收款时间",
        "账单详情",
        "交易详情",
    ]
}

fn is_cjk(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch)
}

fn contains_any_case_insensitive(text: &str, needles: &[&str]) -> bool {
    let lowered = text.to_lowercase();
    needles
        .iter()
        .any(|needle| lowered.contains(&needle.to_lowercase()))
}
